# GPUI 与 Wayland 交互详解

> 从 Unix Socket 连接到 GPU 渲染提交，逐层拆解 GPUI 使用的每一个 Wayland 协议操作。

---

## Wayland 协议总览

Wayland 是一个**异步、面向对象的显示协议**。客户端通过 Unix Socket 与 compositor 通信，交换二进制消息。每个 Wayland 对象（`wl_surface`, `wl_pointer` 等）有唯一的 ID，客户端通过 ID 发送请求（requests），compositor 通过 ID 发送事件（events）。

GPUI 使用的 Rust Wayland 栈：

```
gpui_linux (Wayland 操作封装)
  ├─ wayland-client     ← 协议绑定 + Dispatch trait 事件分派
  ├─ wayland-protocols  ← xdg-shell, viewporter, decoration 等扩展协议
  ├─ wayland-cursor     ← 光标主题加载
  ├─ calloop            ← 事件循环 (epoll over Unix socket fd)
  ├─ calloop-wayland-source ← WaylandSocket → calloop EventSource 适配
  └─ wgpu               ← GPU 渲染 (Vulkan 后端)
```

---

## 1. 连接 Composable：`Connection::connect_to_env()`

```rust
// wayland/client.rs:540
let conn = Connection::connect_to_env().unwrap();
```

**底层操作**：
- 读取 `$WAYLAND_DISPLAY` 环境变量（通常 `wayland-0` 或 `wayland-1`）
- 连接到 `$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY` Unix Socket
- 发送 `wl_display::get_registry` 请求，获取全局对象列表
- **每个 Wayland 连接都有一个 `wl_display` 单例**，composition 的根对象

**对应 Wayland 协议消息**：
```
Client → Compositor: wl_display@1.get_registry(new_id wl_registry@2)
Compositor → Client: wl_registry@2.global(name=1, interface="wl_compositor", version=6)
Compositor → Client: wl_registry@2.global(name=2, interface="wl_seat", version=9)
Compositor → Client: wl_registry@2.global(name=3, interface="xdg_wm_base", version=5)
... (compositor 广告自己支持的所有全局对象)
Compositor → Client: wl_registry@2.global(name=N, ...)
```

---

## 2. 协议绑定：GPUI 向 Wayland 请求了什么

### 2a. `registry_queue_init()` → 全局对象列表

```rust
// wayland/client.rs:542
let (globals, event_queue) = registry_queue_init::<WaylandClientStatePtr>(&conn).unwrap();
```

这做了两件事：
1. 向 compositor 请求全局对象列表（`wl_display.get_registry`）
2. 创建 `EventQueue`，它是 Wayland 消息的接收队列，绑定到 Rust 类型 `WaylandClientStatePtr` 作为 dispatch target

### 2b. 核心协议绑定（必需）

**`wl_compositor`** — 创建 surface 的工厂：

```rust
// Globals::new() 中
compositor: globals.bind(&qh, 5..=6, ()).unwrap()
// → wl_registry@2.bind(name=1, "wl_compositor", version=6, new_id wl_compositor@N)
```

**`wl_seat`** — 输入设备抽象（键盘+鼠标+触摸）：

```rust
// client.rs:554
globals.registry().bind::<wl_seat::WlSeat, _, _>(
    global.name, wl_seat_version(global.version), &qh, (),
)
// → wl_registry@2.bind(name=2, "wl_seat", version=9, new_id wl_seat@M)
```

**`wl_shm`** — 共享内存（光标图标、剪贴板用）：

```rust
shm: globals.bind(&qh, 1..=1, ()).unwrap()
```

**`xdg_wm_base`** — XDG Shell 窗口管理协议：

```rust
wm_base: globals.bind(&qh, 1..=5, ()).unwrap()
// 这是创建可拖拽、可最大化、可关闭窗口的基础设施
```

### 2c. 扩展协议绑定（可选，按需降级）

GPUI 的 `Globals` 结构体记录了 compositor **可选支持**的扩展。大部分是 `Option<T>`，缺失时优雅降级：

```
# 输入相关
wp_cursor_shape_manager_v1     → 光标形状（不用 SHM surface，直接用枚举）
zwp_pointer_gestures_v1        → 触控板捏合手势 (pinch-to-zoom)
zwp_text_input_manager_v3      → 输入法 (IME) 支持

# 渲染相关
wp_viewporter                   → 裁剪/缩放 surface（HiDPI 非整数缩放）
wp_fractional_scale_manager_v1  → 分数缩放 (125%, 150%, 175%...)
zwp_linux_dmabuf_v1             → GPU dmabuf 直接共享（零拷贝帧缓冲）

# 窗口装饰
zxdg_decoration_manager_v1      → 服务端装饰协商
xdg_wm_dialog_v1                → 模态对话框
xdg_system_bell_v1              → 系统铃声

# 桌面集成
zwlr_layer_shell_v1             → 面板/覆盖层 (panel, wallpaper, overlay)
xdg_activation_v1               → 窗口激活令牌（跨进程聚焦）

# 特效
org_kde_kwin_blur_manager       → KDE 窗口模糊特效

# 剪贴板
wl_data_device_manager          → Ctrl+C/V 剪贴板 (v3)
zwp_primary_selection_device_manager_v1 → Linux 中键粘贴
```

---

## 3. Surface 创建：`wl_compositor` + `xdg_shell`

### 3a. 创建 wl_surface

```rust
// window.rs:523
let surface = globals.compositor.create_surface(&globals.qh, ());
// → wl_compositor@N.create_surface(new_id wl_surface@S)
```

`wl_surface` 是一个**矩形像素缓冲区**，它：
- 本身不占位置（由 xdg_surface 赋予角色和位置）
- 通过 `attach`, `damage`, `commit` 提交像素内容
- 通过 `frame` 回调实现 VSync
- 通过 `set_buffer_scale` 实现 HiDPI

### 3b. 创建 XDG Surface

```rust
// window.rs:190-194 (WaylandSurfaceState::new)
let xdg_surface = globals.wm_base.get_xdg_surface(&surface, &globals.qh, surface.id());
let toplevel = xdg_surface.get_toplevel(&globals.qh, surface.id());
// → xdg_wm_base@W.get_xdg_surface(new_id xdg_surface@X, wl_surface@S)
// → xdg_surface@X.get_toplevel(new_id xdg_toplevel@T)
```

**`xdg_surface`** 赋予 `wl_surface` 一个"角色"——桌面窗口。compositor 从此开始管理它的位置、大小、层级。

**`xdg_toplevel`** 设置窗口属性：

```rust
// 在 WaylandSurfaceState::new() 中：
toplevel.set_app_id("my_gpui_app");          // 应用 ID
toplevel.set_title("GPUI Component");         // 窗口标题
toplevel.set_min_size(480, 320);              // 最小尺寸
// 设置父窗口（对话框场景）
toplevel.set_parent(parent_toplevel);
```

### 3c. `surface.commit()` — 提交配置

```rust
// window.rs:554
surface.commit();  // ← 初始提交，触发 compositor 的 configure 事件
```

`wl_surface.commit()` 是 Wayland 的**原子提交机制**。所有 `attach`, `damage`, `set_buffer_scale` 等操作不会立即生效，只有 `commit()` 时才批量应用。

### 3d. 分数缩放 (HiDPI 125%/150%/175%)

```rust
// window.rs:527-529
if let Some(fractional_scale_manager) = globals.fractional_scale_manager.as_ref() {
    fractional_scale_manager.get_fractional_scale(&surface, &globals.qh, surface.id());
}
// → wp_fractional_scale_manager_v1.get_fractional_scale(new_id wp_fractional_scale@F, wl_surface@S)
```

compositor 会通过 `wp_fractional_scale.preferred_scale(scale120)` 事件返回 120 表示 120% 缩放。

### 3e. Viewporter（裁剪/子像素缩放）

```rust
// window.rs:531-534
let viewport = globals.viewporter.as_ref()
    .map(|viewporter| viewporter.get_viewport(&surface, &globals.qh, ()));
```

允许设置 surface 的源矩形和目标矩形，实现精确的像素对齐。

---

## 4. 窗口生命周期事件

compositor 通过事件告知窗口配置变更：

### `xdg_surface::configure`

```
compositor → Client: xdg_surface@X.configure(serial=N)
```

客户端必须回复 `ack_configure`：

```rust
// window.rs:628-701
pub fn handle_xdg_surface_event(&self, event: xdg_surface::Event) {
    match event {
        xdg_surface::Event::Configure { serial } => {
            // 如果之前收到 toplevel configure 决定的大小
            if let Some(configure) = state.in_progress_configure.take() {
                state.bounds.size = configure.size;
                state.fullscreen = configure.fullscreen;
                state.maximized = configure.maximized;
                // ...
            }
            xdg_surface.ack_configure(serial);  // ← 确认配置
            drop(state);
            self.resize(new_size);
        }
    }
}
```

### `xdg_toplevel::configure`

```
compositor → Client: xdg_toplevel@T.configure(width=800, height=600, states=[activated])
```

### `xdg_toplevel::close`

```
compositor → Client: xdg_toplevel@T.close()
→ 调用 on_close 回调 → 用户确认关闭
```

---

## 5. GPU 渲染：`wl_surface.attach()` + `wgpu`

### 5a. WgpuRenderer 创建 Wayland Surface

```rust
// wgpu_renderer.rs:199
let target = wgpu::SurfaceTargetUnsafe::RawHandle {
    raw_display_handle: None,
    raw_window_handle: window_handle.as_raw(),
};

// wgpu 内部: wl_egl_window_create(surface, width, height) + eglCreateWindowSurface
// 或通过 Vulkan WSI: vkCreateWaylandSurfaceKHR
let surface = unsafe {
    instance.create_surface_unsafe(target)?
};
```

**GPUI 的 `RawWindow`** 就是为了满足 wgpu 的 `HasWindowHandle` 需求：

```rust
// window.rs:56-81
struct RawWindow {
    window: *mut c_void,   // → wl_surface 的原始指针
    display: *mut c_void,  // → wl_display 的原始指针
}

impl HasWindowHandle for RawWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let window = NonNull::new(self.window).unwrap();
        let handle = WaylandWindowHandle::new(window);
        Ok(unsafe { WindowHandle::borrow_raw(handle.into()) })
    }
}
```

### 5b. `WgpuRenderer::draw()` — 逐帧渲染

```rust
// wgpu_renderer.rs:1082-1335
pub fn draw(&mut self, scene: &Scene) -> bool {
    // ① 获取下一帧的 swapchain texture
    let frame = self.resources().surface.get_current_texture();
    //     → wl_surface 背后: Vulkan swapchain 或 EGL surface
    //     → Wayland 通过 dmabuf 或 shared memory 获取像素

    let frame_view = frame.texture.create_view(&Default::default());

    // ② 写入全局 uniform buffer（viewport size, alpha mode, gamma）
    // ...

    // ③ 创建 CommandEncoder → 开始 RenderPass
    let mut encoder = device.create_command_encoder(&Default::default());
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &frame_view,
            ops: Operations {
                load: LoadOp::Clear(Color::TRANSPARENT),
                store: StoreOp::Store,
            },
        })],
        ..Default::default()
    });

    // ④ 遍历 Scene 的绘制批次
    for batch in scene.batches() {
        match batch {
            PrimitiveBatch::Quads(range)         → draw_quads()          // 矩形背景/边框
            PrimitiveBatch::Shadows(range)       → draw_shadows()        // 阴影
            PrimitiveBatch::MonochromeSprites{}  → draw_monochrome_sprites()  // 文字
            PrimitiveBatch::PolychromeSprites{}  → draw_polychrome_sprites()  // emoji
            PrimitiveBatch::Paths(range)         → draw_paths_to_intermediate()
            PrimitiveBatch::SubpixelSprites{}    → draw_subpixel_sprites()
        }
    }

    // ⑤ 提交 GPU 命令 + Present
    self.resources().queue.submit(Some(encoder.finish()));
    frame.present();
    //     → wl_surface 收到新的 buffer
    //     → compositor 在下次合成时用它更新屏幕

    true
}
```

### 5c. VSync 帧同步：`wl_surface.frame()`

```rust
// window.rs:587-602
pub fn frame(&self) {
    let mut state = self.state.borrow_mut();
    state.surface.frame(&state.globals.qh, state.surface.id());
    // → wl_surface@S.frame(new_id wl_callback@C)
    // compositor 在下一帧渲染完成后发送:
    // → wl_callback@C.done(timestamp_ms)

    // 触发 on_request_frame 回调 → window.draw()
    let mut cb = self.callbacks.borrow_mut();
    if let Some(fun) = cb.request_frame.as_mut() {
        fun(RequestFrameOptions { force_render: state.force_render_after_recovery, ..default() });
    }
}
```

**完整的 VSync 渲染循环**：

```
1. window.draw() → draw_roots() → Scene → WgpuRenderer::draw(scene)
2. WgpuRenderer::draw() → queue.submit() + frame.present()
                                ↓
3. wl_surface 的 buffer 被 compositor 接收
4. window.frame() → wl_surface.frame(callback)
                                ↓
5. compositor 完成合成 → wl_callback.done(callback_time)
                                ↓
6. calloop 事件循环收到 wl_callback 事件
7. WaylandWindowStatePtr::frame() → on_request_frame → window.draw()
   → 回到步骤 1
```

### 5d. `surface.commit()` + `completed_frame()`

```rust
// window.rs:1438-1445
fn completed_frame(&self) {
    let mut state = self.borrow_mut();
    if !state.renderer_presented {
        state.surface.commit();  // ← 无 buffer 时也 commit（兼容旧 wlroots 的 bug）
    }
}
```

正常情况：`frame.present()` 内部已经做了 `commit`。

---

## 6. 输入事件：wl_seat → wl_pointer / wl_keyboard

### 6a. 设备能力变更

```
compositor → Client: wl_seat@M.capabilities(capabilities=pointer|keyboard)
```

```rust
// client.rs:1385-1438 (Dispatch<wl_seat>)
match event {
    wl_seat::Event::Capabilities { capabilities } => {
        if capabilities.contains(Capability::Pointer) {
            let pointer = seat.get_pointer(&qh, ());
            // → wl_seat@M.get_pointer(new_id wl_pointer@P)
            state.wl_pointer = Some(pointer);
        }
        if capabilities.contains(Capability::Keyboard) {
            let keyboard = seat.get_keyboard(&qh, ());
            // → wl_seat@M.get_keyboard(new_id wl_keyboard@K)
            state.wl_keyboard = Some(keyboard);
        }
    }
}
```

### 6b. 鼠标事件 (`wl_pointer`)

```
compositor → Client: wl_pointer@P.enter(serial, surface, surface_x, surface_y)
compositor → Client: wl_pointer@P.motion(time, surface_x, surface_y)
compositor → Client: wl_pointer@P.button(serial, time, button=272, state=pressed)
compositor → Client: wl_pointer@P.axis(time, axis=vertical_scroll, value=15.0)
compositor → Client: wl_pointer@P.leave(serial, surface)
```

GPUI 的处理链路 (`client.rs:1773-2100+`)：

```
wl_pointer::Event 到达 Dispatch<wl_pointer> 实现
  │
  ├─ Enter  → 更新 mouse_focused_window、serial_tracker(MouseEnter)
  │           → 通知窗口 set_hovered(true)
  │
  ├─ Motion → 更新 mouse_location
  │           → 构造 PlatformInput::MouseMove { position, pressed_button, modifiers }
  │           → window.handle_input(input)
  │
  ├─ Button → linux_button_to_gpui(button)     // 272(BTN_LEFT) → MouseButton::Left
  │           → ClickState 双击检测:
  │               click_elapsed < 400ms && 同按钮 && 距离 < 5px → click_count = 2/3
  │           → serial_tracker.update(MousePress, serial)
  │           → 构造 PlatformInput::MouseDown { button, position, modifiers, click_count }
  │           → window.handle_input(input)
  │
  ├─ Axis   → 滚动事件处理
  │           → 区分 continuous (触控板) / discrete (鼠标滚轮)
  │           → 构造 PlatformInput::ScrollWheel { delta, position, modifiers }
  │           → window.handle_input(input)
  │
  └─ Leave  → notification window set_hovered(false)
```

### 6c. 键盘事件 (`wl_keyboard`)

```
compositor → Client: wl_keyboard@K.keymap(format=xkb_v1, fd, size)
compositor → Client: wl_keyboard@K.enter(serial, surface, keys[])
compositor → Client: wl_keyboard@K.key(serial, time, key, state=pressed)
compositor → Client: wl_keyboard@K.modifiers(serial, mods_depressed, ...)
compositor → Client: wl_keyboard@K.leave(serial, surface)
```

**`Keymap` 事件** — 加载键盘布局：

```rust
// client.rs:1457-1481
wl_keyboard::Event::Keymap { format: XkbV1, fd, size } => {
    let xkb_context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
    let keymap = unsafe {
        xkb::Keymap::new_from_fd(&xkb_context, fd, size, XKB_KEYMAP_FORMAT_TEXT_V1, ...)
    };
    state.keymap_state = Some(xkb::State::new(&keymap));   // ← 保存键盘状态
    state.compose_state = get_xkb_compose_state(&xkb_context); // ← 组合键 (dead keys)
}
```

**`Key` 事件** → 按键处理：

```rust
// 1. keymap_state.update_key(keycode, direction)  更新 xkb 状态
// 2. keymap_state.key_get_utf8(keycode) → keystroke.key_char
// 3. modifiers_from_xkb(keymap_state) → keystroke.modifiers
// 4. 构造 PlatformInput::KeyDown { keystroke, keycode }
// 5. keyboard_focused_window.handle_input(input)
```

---

## 7. 光标：`wl_pointer.set_cursor()` + SHM Surface

Wayland 光标不是通过 GPU 渲染的，而是通过 **SHM (Shared Memory)** 传递像素数据给 compositor：

```rust
// cursor.rs:34-45
pub fn new(connection, globals, size) -> Self {
    let surface = globals.compositor.create_surface(&globals.qh, ());
    // ↑ 创建一个专用的 wl_surface 用于光标
    let theme = CursorTheme::load(&connection, globals.shm.clone(), size);
    // ↑ 加载系统光标主题 (如 Adwaita)
}
```

### 设置光标图标 (`cursor.rs:94-151`)

```rust
pub fn set_icon(&mut self, wl_pointer, serial, cursor_icon_names, scale) {
    // ① 从光标主题获取图标像素
    let buffer: &CursorImageBuffer = theme.get_cursor("left_ptr")[0];
    //    ↑ RGBA 像素数组 + hotspot 偏移 (箭头尖角位置)

    // ② 将像素数据写入 wl_shm_pool (共享内存)
    //    wayland-cursor 库内部:
    //      wl_shm@S.create_pool(fd, size)
    //      wl_shm_pool@P.create_buffer(offset, width, height, stride, ARGB8888)

    // ③ 将 buffer 绑定到光标 surface
    self.surface.attach(Some(buffer), 0, 0);
    self.surface.damage(0, 0, width, height);
    self.surface.commit();

    // ④ 告诉 compositor 把这个 surface 作为光标
    wl_pointer.set_cursor(serial, Some(&self.surface), hot_x, hot_y);
    // → wl_pointer@P.set_cursor(serial, wl_surface@CS, 4, 4)
    // compositor 从此刻起在对应位置绘制这个 surface 代替默认光标
}
```

**为什么是 SHM 而不是 GPU？** Wayland 光标协议设计为 compositor 负责合成光标，客户端只需提供像素数据。SHM 是最简单的跨进程共享方式。

现代 compositor 也支持 `wp_cursor_shape_manager_v1`（直接用枚举值 `"pointer"`, `"text"` 等），不需要 SHM surface。

---

## 8. 剪贴板：`wl_data_device` + Pipe fd

### 8a. 写入剪贴板（Ctrl+C）

```
1. 客户端创建 wl_data_source
   → wl_data_device_manager.create_data_source(new_id wl_data_source@DS)
2. 声明支持的 MIME 类型
   → wl_data_source@DS.offer("text/plain;charset=utf-8")
3. 设置为当前选区
   → wl_data_device@DD.set_selection(wl_data_source@DS, serial)
4. 其他程序请求内容时，compositor 发送:
   → wl_data_source@DS.send(mime_type, fd)
5. 客户端将数据写入 fd (Pipe):
   → Clipboard::send_internal(fd, text.as_bytes())
   → 通过 calloop Generic source 异步写入 Pipe
```

**`Clipboard::send_internal()`** (`clipboard.rs:235-262`)：

```rust
fn send_internal(&self, fd: OwnedFd, bytes: Vec<u8>) {
    let mut written = 0;
    self.loop_handle.insert_source(
        calloop::generic::Generic::new(
            File::from(fd),               // 将 fd 包装为 File
            calloop::Interest::WRITE,     // 关注可写事件
            calloop::Mode::Level,
        ),
        move |_, file, _| {
            let file = unsafe { file.get_mut() };
            loop {
                match file.write(&bytes[written..]) {
                    Ok(n) if written + n == bytes.len() => {
                        break Ok(PostAction::Remove); // 写完，移除事件源
                    }
                    Ok(n) => written += n,             // 继续写
                    Err(err) if err.kind() == WouldBlock => break Ok(PostAction::Continue),
                    Err(_) => break Ok(PostAction::Remove),
                }
            }
        },
    ).unwrap();
}
```

### 8b. 读取剪贴板（Ctrl+V）

```
1. compositor 发送: wl_data_device@DD.data_offer(new_id wl_data_offer@DO)
2. compositor 发送: wl_data_offer@DO.offer("text/plain;charset=utf-8")
3. compositor 发送: wl_data_device@DD.selection(wl_data_offer@DO)
4. 客户端调用 Clipboard::read()
   → DataOffer::read_text()
   → 创建 Pipe (read + write 两端)
   → wl_data_offer@DO.receive("text/plain", pipe.write_fd)
   → Connection::flush() → 等待 compositor 写入数据到 pipe.write_fd
   → read_fd_with_timeout(pipe.read_fd) → 从 pipe 读取字节
   → String::from_utf8() → ClipboardItem
```

### 8c. Linux 中键粘贴 (Primary Selection)

Linux 有两套独立的剪贴板：

| 操作 | Wayland 协议 | GPUI 字段 |
|------|------------|----------|
| Ctrl+C / Ctrl+V | `wl_data_device` | `contents`, `current_offer` |
| 中键粘贴（选中即复制） | `zwp_primary_selection_device_v1` | `primary_contents`, `current_primary_offer` |

---

## 9. 事件循环集成：calloop + WaylandSource

### 9a. 整体架构

```
┌──────────────────────────────────────┐
│         calloop::EventLoop            │
│  (epoll over multiple fd sources)    │
├──────────────────────────────────────┤
│                                      │
│  ┌─────────────────────────────┐     │
│  │ WaylandSource               │     │
│  │ (监听 Wayland socket fd)    │     │
│  │  ├─ wl_pointer events       │     │
│  │  ├─ wl_keyboard events      │     │
│  │  ├─ wl_surface events       │     │
│  │  ├─ xdg_surface events      │     │
│  │  └─ frame callbacks         │     │
│  └─────────────────────────────┘     │
│                                      │
│  ┌─────────────────────────────┐     │
│  │ main_receiver               │     │
│  │ (异步任务通道)               │     │
│  │  cx.spawn() → 这里投递       │     │
│  └─────────────────────────────┘     │
│                                      │
│  ┌─────────────────────────────┐     │
│  │ XDPEventSource               │     │
│  │ (XDG Portal 主题/外观变更)    │     │
│  └─────────────────────────────┘     │
│                                      │
│  ┌─────────────────────────────┐     │
│  │ calloop::timer::Timer        │     │
│  │ (延时任务到期)               │     │
│  └─────────────────────────────┘     │
│                                      │
└──────────────────────────────────────┘
```

### 9b. `WaylandSource` — Wayland Socket → calloop EventSource

```rust
// client.rs:737-739
WaylandSource::new(conn, event_queue)
    .insert(handle)
    .unwrap();
```

**`WaylandSource`** 的工作原理：
1. 获取 Wayland connection 的 fd（Unix Socket）
2. 注册到 calloop 的 epoll：当 fd 可读时，调用 `event_queue.dispatch_pending()`
3. `dispatch_pending()` 读取 socket 消息，解析协议，根据对象 ID 找到对应的 Rust `Dispatch` 实现，调用其 `event()` 方法
4. GPUI 为每个 Wayland 协议对象实现了 `Dispatch` trait（20+ 个 impl）

### 9c. 主线程异步任务通道

```rust
// client.rs:581-596
handle.insert_source(main_receiver, move |event, _, _| {
    if let calloop::channel::Event::Msg(runnable) = event {
        handle.insert_idle(|_| { runnable.run(); });
        // ↑ 使用 insert_idle 而非直接运行:
        //   - 在当前 epoll 周期完成后再执行
        //   - 避免在事件处理期间 borrow 冲突
    }
}).unwrap();
```

---

## 10. 完整交互时序示例：用户点击 "Increment" 按钮

```
[Wayland Compositor]
  │
  ├─ MouseDown: wl_pointer@P.button(serial=42, button=272(BTN_LEFT), state=pressed)
  │   │
  │   ▼ [calloop epoll 唤醒]
  │
  ├─ WaylandSource::process_events()
  │   └→ event_queue.dispatch_pending()
  │       └→ Dispatch<wl_pointer>::event() (client.rs:1889)
  │           ├─ linux_button_to_gpui(272) → MouseButton::Left
  │           ├─ ClickState: click_count = 1
  │           ├─ serial_tracker.update(MousePress, 42)
  │           ├─ PlatformInput::MouseDown { button: Left, position, click_count: 1 }
  │           └─ focused_window.handle_input(input)
  │               └→ WaylandWindow::handle_input() (window.rs:1042)
  │                   └→ Callbacks::input (之前 Window::new 注册的回调)
  │                       └→ Window::dispatch_event() (window.rs:4498)
  │                           └→ dispatch_mouse_event()
  │                               ├─ hit_test(position) → 找到按钮 Hitbox
  │                               ├─ Capture: 空 (按钮没有 capture 处理器)
  │                               └─ Bubble: 逆序遍历 listeners
  │                                   └─ Div::on_mouse_down 回调 → 记录状态
  │
  ├─ MouseUp: wl_pointer@P.button(serial=43, button=272, state=released)
  │   │
  │   ▼ [同上路径到 dispatch_mouse_event]
  │
  ├─ dispatch_mouse_event() → Bubble
  │   └─ Div::on_mouse_up 回调
  │       └─ Interactivity 状态机检测 click
  │           └─ click_listeners 触发
  │               └─ cx.listener(|this, _, _, cx| {
  │                       this.count += 1;
  │                       cx.notify();
  │                   })
  │                   │
  │                   ▼ cx.notify() → invalidator.set_dirty(true)
  │
  ├─ [下一帧]
  │   window.draw() → draw_roots()
  │   ├─ LAYOUT:   Taffy (布局不变)
  │   ├─ PREPAINT: Hitbox (位置不变)
  │   ├─ HIT_TEST: 更新命中树
  │   └─ PAINT:    新 count 值 → "Count: 1" 文字 sprite
  │       └→ WgpuRenderer::draw(scene)
  │           ├─ encoder.begin_render_pass()
  │           ├─ draw_quads()        → 面板背景/边框
  │           ├─ draw_shadows()      → 阴影
  │           ├─ draw_monochrome_sprites() → "Count: 1" 文字
  │           ├─ draw_polychrome_sprites() → (若有 emoji)
  │           └─ queue.submit() + frame.present()
  │               │
  │               ▼ [wl_surface 新 buffer]
  │
  ├─ [compositor 合成]
  │   使用新的 buffer 更新屏幕 → 显示 "Count: 1"
  │
  ├─ wl_surface.frame(callback)
  │   └→ wl_callback@C.done(timestamp)
  │       └→ WaylandWindowStatePtr::frame()
  │           └→ on_request_frame → draw() → 循环...
  │
  └─ [屏幕显示: Count: 1 ✓]
```

---

## 关键 Wayland 协议调用速查

| Wayland 请求 | GPUI 调用位置 | 语义 |
|-------------|-------------|------|
| `wl_display.get_registry` | `client.rs:542` | 获取全局对象列表 |
| `wl_registry.bind(compositor)` | `Globals::new()` | 绑定 wl_compositor |
| `wl_registry.bind(seat)` | `client.rs:554` | 绑定 wl_seat |
| `wl_registry.bind(xdg_wm_base)` | `Globals::new()` | 绑定 XDG Shell |
| `wl_compositor.create_surface` | `window.rs:523` | 创建 wl_surface |
| `xdg_wm_base.get_xdg_surface` | `window.rs:193` | 赋予 surface 窗口角色 |
| `xdg_surface.get_toplevel` | `window.rs:194` | 创建顶层窗口 |
| `xdg_toplevel.set_title` | `window.rs` | 设置窗口标题 |
| `wl_surface.commit` | `window.rs:554` | 原子提交所有变更 |
| `wl_surface.attach(buffer)` | `wgpu frame.present()` 内部 | 绑定 GPU 帧缓冲 |
| `wl_surface.damage` | `wgpu frame.present()` 内部 | 标记脏区域 |
| `wl_surface.frame(callback)` | `window.rs:589` | 请求 VSync 回调 |
| `wl_seat.get_pointer` | `client.rs:1413` | 获取鼠标设备 |
| `wl_seat.get_keyboard` | `client.rs:1392` | 获取键盘设备 |
| `wl_pointer.set_cursor` | `cursor.rs:141` | 设置光标 surface |
| `wl_data_device.set_selection` | `client.rs` | 设置剪贴板内容 |
| `wl_data_offer.receive` | `clipboard.rs:84` | 请求剪贴板数据 |
| `xdg_surface.ack_configure` | `window.rs` | 确认窗口配置 |
| `wp_viewporter.get_viewport` | `window.rs:534` | 创建 viewport |
| `wp_fractional_scale_manager.get_fractional_scale` | `window.rs:528` | 创建分数缩放 |
