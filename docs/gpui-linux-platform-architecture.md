# GPUI Linux 平台架构详解 — `LinuxPlatform` 泛型包装模式

> 以 Wayland 为例，深入剖析 `LinuxPlatform<P>` 的架构设计、`Platform` trait 实现、`LinuxClient` 抽象层、以及 `WaylandClient` 的具体实现细节。

> **补充阅读**: 每个 Wayland 协议操作的底层细节（Unix Socket、wl_surface、xdg_shell、wl_pointer、GPU dmabuf、clipboard Pipe fd、calloop 事件循环集成）见 [GPUI 与 Wayland 交互详解](./gpui-wayland-protocol.md)

---

## 架构总览

```
┌─────────────────────────────────────────────────┐
│                  gpui crate                      │
│        trait Platform (纯接口)                   │
│            fn run(), fn open_window(), ...       │
└──────────────────────┬──────────────────────────┘
                       │ 实现
┌──────────────────────▼──────────────────────────┐
│              gpui_linux crate                    │
│                                                  │
│  ┌──────────────────────────────────────────┐   │
│  │       LinuxPlatform<P: LinuxClient>       │   │
│  │         ┌──────────────────┐              │   │
│  │         │    inner: P      │─────────────►│   │
│  │         │ (WaylandClient   │              │   │
│  │         │  X11Client       │              │   │
│  │         │  HeadlessClient) │              │   │
│  │         └──────────────────┘              │   │
│  │                 │                         │   │
│  │    Platform trait 方法通过                │   │
│  │    LinuxClient trait 委托给 inner         │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  trait LinuxClient {                             │
│      fn with_common() -> &mut LinuxCommon;       │
│      fn run();           fn open_window();       │
│      fn displays();      fn set_cursor();        │
│      ...                                         │
│  }                                               │
│       │                    │          │           │
│  ┌────▼────┐  ┌──────▼───┐  ┌───────▼──────┐   │
│  │Wayland  │  │  X11     │  │ Headless     │   │
│  │Client   │  │  Client  │  │ Client       │   │
│  └─────────┘  └──────────┘  └──────────────┘   │
└──────────────────────────────────────────────────┘
```

**三层架构**：

| 层级 | 角色 | 位置 |
|------|------|------|
| `Platform` trait | 对外暴露的抽象接口 | `gpui/src/platform.rs` |
| `LinuxPlatform<P>` | 泛型包装器，实现 `Platform` | `gpui_linux/src/linux/platform.rs` |
| `LinuxClient` trait | Linux 内部抽象，解耦 Wayland/X11/Headless | `gpui_linux/src/linux/platform.rs` |
| `WaylandClient` | Wayland 具体实现 | `gpui_linux/src/linux/wayland/client.rs` |

---

## 1. `LinuxPlatform<P>` — 泛型包装结构体

**定义** (`platform.rs:159-161`)：

```rust
pub(crate) struct LinuxPlatform<P> {
    pub(crate) inner: P,
}
```

极其简洁——只包含一个泛型字段 `inner`。不定义 trait bound 在结构体上，而是在 impl 块上约束。

### 实例化方式

```rust
// Wayland
Rc::new(LinuxPlatform { inner: WaylandClient::new() })

// X11
Rc::new(LinuxPlatform { inner: X11Client::new().unwrap() })

// Headless
Rc::new(LinuxPlatform { inner: HeadlessClient::new() })
```

**为什么用泛型而不用 trait object？**

- 零开销静态分发：`Platform` 已在外部用 `Rc<dyn Platform>` 做了动态分发，内部无需再引入第二层虚函数调用
- 不同类型可以有不同的内部字段（WaylandClient 有 `Rc<RefCell<WaylandClientState>>`，X11Client 有自己的结构）
- 编译时绑定，没有 vtable 开销

---

## 2. `Platform` trait 实现 — 委托模式

**impl 签名** (`platform.rs:163`)：

```rust
impl<P: LinuxClient + 'static> Platform for LinuxPlatform<P> {
```

所有给 `LinuxPlatform<P>` 实现的 `Platform` 方法，**全部委托给 `self.inner`**。委托模式分三类：

### 2a. 直接透传（大部分方法）

```rust
fn background_executor(&self) -> BackgroundExecutor {
    self.inner.with_common(|common| common.background_executor.clone())
}

fn foreground_executor(&self) -> ForegroundExecutor {
    self.inner.with_common(|common| common.foreground_executor.clone())
}

fn text_system(&self) -> Arc<dyn PlatformTextSystem> {
    self.inner.with_common(|common| common.text_system.clone())
}

fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
    self.inner.displays()
}

fn open_window(&self, handle, options) -> Result<Box<dyn PlatformWindow>> {
    self.inner.open_window(handle, options)
}

fn active_window(&self) -> Option<AnyWindowHandle> {
    self.inner.active_window()
}

fn window_stack(&self) -> Option<Vec<AnyWindowHandle>> {
    self.inner.window_stack()
}
```

### 2b. 有额外逻辑包装

```rust
fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
    // 1. 先同步执行用户启动回调
    on_finish_launching();

    // 2. 委托进入事件循环
    LinuxClient::run(&self.inner);

    // 3. 事件循环退出后，调用 quit 回调
    let quit = self.inner
        .with_common(|common| common.callbacks.quit.take());
    if let Some(mut fun) = quit {
        fun();
    }
}

fn quit(&self) {
    self.inner.with_common(|common| common.signal.stop());
}

fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>) {
    self.inner.with_common(|common| common.callbacks.open_urls = Some(callback));
}

fn on_keyboard_layout_change(&self, callback: Box<dyn FnMut()>) {
    self.inner.with_common(|common|
        common.callbacks.keyboard_layout_change = Some(callback));
}
```

### 2c. 平台差异化实现

```rust
// Linux 不实现的
fn activate(&self, _ignoring_other_apps: bool) {
    log::info!("activate is not implemented on Linux, ignoring the call")
}

fn hide(&self) { /* no-op */ }
fn hide_other_apps(&self) { /* no-op */ }
fn unhide_other_apps(&self) { /* no-op */ }

// Linux 特定实现
fn restart(&self, binary_path: Option<PathBuf>) {
    // wait-kill-restart 脚本 + spawn + quit
}

// 文件选择器通过 xdg-desktop-portal (ashpd)
fn prompt_for_paths(&self, options) -> oneshot::Receiver<...> {
    self.foreground_executor().spawn(async move {
        let request = ashpd::desktop::file_chooser::OpenFileRequest::default()
            .identifier(identifier.await)
            .modal(true)
            .send().await?;
        // ...
    }).detach();
}
```

### 关键观察

`LinuxPlatform<P>` **不关心 `P` 是 Wayland 还是 X11 还是 Headless**。它只调用 `LinuxClient` trait 定义的方法。这正是一个干净的**桥接模式**：`Platform`（外部接口）→ `LinuxPlatform`（桥）→ `LinuxClient`（内部接口）。

---

## 3. `LinuxClient` trait — 内部抽象层

**定义** (`platform.rs:50-103`)：

```rust
pub(crate) trait LinuxClient {
    // === 共享状态访问 ===
    fn with_common<R>(&self, f: impl FnOnce(&mut LinuxCommon) -> R) -> R;

    // === 生命周期 ===
    fn run(&self);
    fn compositor_name(&self) -> &'static str;

    // === 显示 ===
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;

    // === 窗口 ===
    fn open_window(&self, handle, options) -> Result<Box<dyn PlatformWindow>>;

    // === 输入 ===
    fn keyboard_layout(&self) -> Box<dyn PlatformKeyboardLayout>;
    fn set_cursor_style(&self, style: CursorStyle);
    fn hide_cursor_until_mouse_moves(&self) {}
    fn is_cursor_visible(&self) -> bool { true }

    // === 剪贴板 ===
    fn write_to_primary(&self, item: ClipboardItem);
    fn write_to_clipboard(&self, item: ClipboardItem);
    fn read_from_primary(&self) -> Option<ClipboardItem>;
    fn read_from_clipboard(&self) -> Option<ClipboardItem>;

    // === 应用 ===
    fn active_window(&self) -> Option<AnyWindowHandle>;
    fn window_stack(&self) -> Option<Vec<AnyWindowHandle>>;
    fn open_uri(&self, uri: &str);
    fn reveal_path(&self, path: PathBuf);
}
```

**设计意图**：
- `pub(crate)` 可见性 — 只在 `gpui_linux` crate 内部使用
- `with_common()` 方法 — 所有实现者共享同一个 `LinuxCommon` 状态，访问模式由各自决定
- 默认实现 — 部分方法提供默认空实现，子类按需覆盖

---

## 4. `LinuxCommon` — 共享状态

**定义** (`platform.rs:116-126`)：

```rust
pub(crate) struct LinuxCommon {
    pub(crate) background_executor: BackgroundExecutor,
    pub(crate) foreground_executor: ForegroundExecutor,
    pub(crate) text_system: Arc<dyn PlatformTextSystem>,
    pub(crate) appearance: WindowAppearance,
    pub(crate) auto_hide_scrollbars: bool,
    pub(crate) button_layout: WindowButtonLayout,
    pub(crate) callbacks: PlatformHandlers,
    pub(crate) signal: LoopSignal,
    pub(crate) menus: Vec<OwnedMenu>,
}
```

这是 Wayland、X11、Headless 三者**共享的基础设施状态**。无论底层协议如何，执行器、文本系统、外观、信号都是一样的。

**`PlatformHandlers::callbacks`** (`platform.rs:106-113`)：

```rust
pub(crate) struct PlatformHandlers {
    pub(crate) open_urls: Option<Box<dyn FnMut(Vec<String>)>>,
    pub(crate) quit: Option<Box<dyn FnMut()>>,
    pub(crate) reopen: Option<Box<dyn FnMut()>>,
    pub(crate) app_menu_action: Option<Box<dyn FnMut(&dyn Action)>>,
    pub(crate) will_open_app_menu: Option<Box<dyn FnMut()>>,
    pub(crate) validate_app_menu_command: Option<Box<dyn FnMut(&dyn Action) -> bool>>,
    pub(crate) keyboard_layout_change: Option<Box<dyn FnMut()>>,
}
```

这些回调在 `LinuxPlatform` 层设置，在具体 Client 的事件循环退出或特定事件触发时调用。

---

## 5. `WaylandClient` — 具体实现

### 5a. 结构体

**定义** (`wayland/client.rs:485`)：

```rust
pub struct WaylandClient(Rc<RefCell<WaylandClientState>>);
```

单字段 `Rc<RefCell<...>>`，这是一个**单线程内部可变性**模式（所有操作都在主线程，无需 `Mutex`）。

### 5b. `Drop` 实现

```rust
impl Drop for WaylandClient {
    fn drop(&mut self) {
        let mut state = self.0.borrow_mut();
        state.windows.clear();                // 清理所有窗口
        if let Some(wl_pointer) = &state.wl_pointer { wl_pointer.release(); }
        if let Some(cursor_shape_device) = &state.cursor_shape_device { cursor_shape_device.destroy(); }
        if let Some(data_device) = &state.data_device { data_device.release(); }
        if let Some(text_input) = &state.text_input { text_input.destroy(); }
    }
}
```

清理 Wayland 协议对象，防止资源泄漏。

### 5c. `WaylandClientState` — 全部可变状态

**定义** (`wayland/client.rs:215-268`)：

```rust
pub(crate) struct WaylandClientState {
    // === Wayland 协议对象 ===
    serial_tracker: SerialTracker,
    globals: Globals,
    gpu_context: GpuContext,
    compositor_gpu: Option<CompositorGpuHint>,
    wl_seat: wl_seat::WlSeat,
    wl_pointer: Option<wl_pointer::WlPointer>,
    wl_keyboard: Option<wl_keyboard::WlKeyboard>,
    pinch_gesture: Option<zwp_pointer_gesture_pinch_v1::ZwpPointerGesturePinchV1>,
    pinch_scale: f32,
    cursor_shape_device: Option<wp_cursor_shape_device_v1::WpCursorShapeDeviceV1>,
    data_device: Option<wl_data_device::WlDataDevice>,
    primary_selection: Option<zwp_primary_selection_device_v1::ZwpPrimarySelectionDeviceV1>,
    text_input: Option<zwp_text_input_v3::ZwpTextInputV3>,
    pre_edit_text: Option<String>,
    ime_pre_edit: Option<String>,
    composing: bool,

    // === 窗口 & 输出 ===
    windows: HashMap<ObjectId, WaylandWindowStatePtr>,
    outputs: HashMap<ObjectId, Output>,
    in_progress_outputs: HashMap<ObjectId, InProgressOutput>,
    wl_outputs: HashMap<ObjectId, wl_output::WlOutput>,

    // === 输入状态 ===
    keyboard_layout: LinuxKeyboardLayout,
    keymap_state: Option<xkb::State>,
    compose_state: Option<xkb::compose::State>,
    modifiers: Modifiers,
    capslock: Capslock,
    mouse_location: Option<Point<Pixels>>,
    mouse_focused_window: Option<WaylandWindowStatePtr>,
    keyboard_focused_window: Option<WaylandWindowStatePtr>,
    button_pressed: Option<MouseButton>,
    axis_source: AxisSource,
    continuous_scroll_delta: Option<Point<Pixels>>,
    discrete_scroll_delta: Option<Point<f32>>,

    // === 点击状态 ===
    click: ClickState,             // 双击/三击检测
    drag: DragState,

    // === 光标 ===
    cursor_style: Option<CursorStyle>,
    cursor: Cursor,

    // === 剪贴板 ===
    clipboard: Clipboard,
    data_offers: Vec<DataOffer<WlDataOffer>>,

    // === 事件循环 ===
    loop_handle: LoopHandle<'static, WaylandClientStatePtr>,
    event_loop: Option<EventLoop<'static, WaylandClientStatePtr>>,

    // === 共享状态 ===
    pub common: LinuxCommon,       // ← 关键！所有 LinuxClient 共享的

    // === 其他 ===
    ime_enabled: Option<bool>,
    enter_token: Option<()>,
    pending_activation: Option<PendingActivation>,
    // ...
}
```

**`common: LinuxCommon`** 是关键字段——`with_common()` 就是通过这个字段共享状态的。

### 5d. `Globals` — Wayland 全局对象

**定义** (`wayland/client.rs:113-135`)：

```rust
pub struct Globals {
    pub qh: QueueHandle<WaylandClientStatePtr>,
    pub activation: Option<xdg_activation_v1::XdgActivationV1>,
    pub compositor: wl_compositor::WlCompositor,
    pub cursor_shape_manager: Option<wp_cursor_shape_manager_v1::WpCursorShapeManagerV1>,
    pub data_device_manager: Option<wl_data_device_manager::WlDataDeviceManager>,
    pub primary_selection_manager:
        Option<zwp_primary_selection_device_manager_v1::ZwpPrimarySelectionDeviceManagerV1>,
    pub wm_base: xdg_wm_base::XdgWmBase,
    pub shm: wl_shm::WlShm,
    pub seat: wl_seat::WlSeat,
    pub viewporter: Option<wp_viewporter::WpViewporter>,
    pub fractional_scale_manager: Option<wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1>,
    pub decoration_manager: Option<zxdg_decoration_manager_v1::ZxdgDecorationManagerV1>,
    pub layer_shell: Option<zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    pub blur_manager: Option<org_kde_kwin_blur_manager::OrgKdeKwinBlurManager>,
    pub text_input_manager: Option<zwp_text_input_manager_v3::ZwpTextInputManagerV3>,
    pub gesture_manager: Option<zwp_pointer_gestures_v1::ZwpPointerGesturesV1>,
    pub dialog: Option<xdg_wm_dialog_v1::XdgWmDialogV1>,
    pub system_bell: Option<xdg_system_bell_v1::XdgSystemBellV1>,
    pub executor: ForegroundExecutor,
}
```

这代表 Wayland compositor 提供的所有能力。**注意**：大部分是 `Option`——不同 compositor 支持不同协议扩展，缺失时优雅降级。

### 5e. `ClickState` — 双击/三击检测

```rust
pub struct ClickState {
    last_mouse_button: Option<MouseButton>,
    last_click: Instant,
    last_location: Point<Pixels>,
    current_count: usize,
}
```

在鼠标按下事件处理器中 (`wayland/client.rs:1920-1939`)：

```rust
// 判断是否是连续点击
let click_elapsed = state.click.last_click.elapsed();

if click_elapsed < DOUBLE_CLICK_INTERVAL
    && state.click.last_mouse_button.is_some_and(|prev| prev == button)
    && is_within_click_distance(state.click.last_location, state.mouse_location.unwrap())
{
    state.click.current_count += 1;  // 双击 → 2, 三击 → 3
} else {
    state.click.current_count = 1;   // 重置为单击
}

// 更新状态
state.click.last_click = Instant::now();
state.click.last_mouse_button = Some(button);
state.click.last_location = state.mouse_location.unwrap();

// 构建事件
let input = PlatformInput::MouseDown(MouseDownEvent {
    button,
    position: state.mouse_location.unwrap(),
    modifiers: state.modifiers,
    click_count: state.click.current_count,  // ← 传递点击次数
    first_mouse: state.enter_token.take().is_some(),
});
```

---

## 6. `WaylandClient::new()` — 初始化全流程

```rust
impl WaylandClient {
    pub(crate) fn new() -> Self {
        // 1. 连接 Wayland compositor
        let conn = Connection::connect_to_env().unwrap();

        // 2. 获取 Wayland 全局对象列表
        let (globals, event_queue) = registry_queue_init::<WaylandClientStatePtr>(&conn).unwrap();
        let qh = event_queue.handle();

        // 3. 绑定 seat 和 output
        let mut seat: Option<wl_seat::WlSeat> = None;
        let mut wl_outputs: HashMap<ObjectId, wl_output::WlOutput> = HashMap::default();
        globals.contents().with_list(|list| {
            for global in list {
                match &global.interface[..] {
                    "wl_seat"   => seat = Some(bind_seat(global)),
                    "wl_output" => wl_outputs.insert(id, bind_output(global)),
                    _ => {}
                }
            }
        });

        // 4. 创建 calloop 事件循环 (Linux 原生事件循环)
        let event_loop = EventLoop::<WaylandClientStatePtr>::try_new().unwrap();

        // 5. 创建 LinuxCommon (调度器、线程池、文本系统)
        let (common, main_receiver) = LinuxCommon::new(event_loop.get_signal());

        // 6. 将 main_receiver 注册到 event_loop
        //    (所有 cx.spawn() 的异步任务通过此通道投递到主线程)
        let handle = event_loop.handle();
        handle.insert_source(main_receiver, move |event, _, _| {
            if let calloop::channel::Event::Msg(runnable) = event {
                handle.insert_idle(|_| {
                    runnable.run();    // 在主线程空闲时执行
                });
            }
        }).unwrap();

        // 7. 检测 GPU
        let compositor_gpu = detect_compositor_gpu();

        // 8. 创建 Globals (绑定所有 Wayland 协议扩展)
        let globals = Globals::new(globals, common.foreground_executor.clone(), qh, seat);

        // 9. 创建数据设备和剪贴板
        let data_device = globals.data_device_manager.as_ref()
            .map(|mgr| mgr.get_data_device(&seat, &qh, ()));
        let primary_selection = globals.primary_selection_manager.as_ref()
            .map(|mgr| mgr.get_device(&seat, &qh, ()));

        // 10. 创建光标
        let cursor = Cursor::new(&conn, &globals, 24);

        // 11. 注册 XDG Desktop Portal 事件源 (主题、按钮布局变更)
        handle.insert_source(XDPEventSource::new(&common.background_executor),
            move |event, _, client| {
                match event {
                    XDPEvent::WindowAppearance(appearance) => {
                        client.borrow_mut().common.appearance = appearance;
                        // 通知所有窗口更新外观
                        for window in client.borrow().windows.values() {
                            window.set_appearance(appearance);
                        }
                    }
                    XDPEvent::ButtonLayout(layout_str) => { /* ... */ }
                    XDPEvent::CursorTheme(theme) => { /* ... */ }
                    XDPEvent::CursorSize(size) => { /* ... */ }
                }
            }).unwrap();

        // 12. 组装 WaylandClientState
        let state = Rc::new(RefCell::new(WaylandClientState {
            serial_tracker: SerialTracker::new(),
            globals,
            gpu_context: Rc::new(RefCell::new(None)),
            compositor_gpu,
            wl_seat: seat,
            wl_pointer: None,          // 等 seat 能力变更时绑定
            wl_keyboard: None,         // 等 seat 能力变更时绑定
            data_device,
            primary_selection,
            text_input: None,
            outputs: HashMap::default(),
            in_progress_outputs,
            wl_outputs,
            windows: HashMap::default(),
            common,                     // ← 共享状态注入
            keyboard_layout: LinuxKeyboardLayout::new(UNKNOWN_KEYBOARD_LAYOUT_NAME),
            keymap_state: None,
            compose_state: None,
            click: ClickState::default(),
            modifiers: Modifiers::default(),
            cursor,
            clipboard: Clipboard::new(conn.clone(), handle.clone()),
            event_loop: Some(event_loop),  // ← 事件循环所有权
            // ... 其余字段初始化为默认值
        }));

        // 13. 将 Wayland 事件源注册到 calloop
        WaylandSource::new(conn, event_queue).insert(handle).unwrap();

        // 14. 返回
        Self(state)
    }
}
```

---

## 7. `LinuxClient` trait 在 WaylandClient 上的实现

### `with_common()` — 共享状态访问

```rust
fn with_common<R>(&self, f: impl FnOnce(&mut LinuxCommon) -> R) -> R {
    f(&mut self.0.borrow_mut().common)
}
```

`self.0` → `Rc<RefCell<WaylandClientState>>` → `.borrow_mut()` → `.common` 字段。

**所有 `LinuxPlatform` impl `Platform` 的方法中，只要需要访问共享状态，就通过这个闭包模式**。闭包的优势是可以精确控制 borrow 的范围。

### `compositor_name()`

```rust
fn compositor_name(&self) -> &'static str {
    "Wayland"
}
```

### `displays()`

```rust
fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>> {
    self.0.borrow().outputs.iter()
        .map(|(id, output)| Rc::new(WaylandDisplay {
            id: id.clone(),
            name: output.name.clone(),
            bounds: output.bounds.to_pixels(output.scale as f32),
        }) as Rc<dyn PlatformDisplay>)
        .collect()
}
```

遍历 `WaylandClientState.outputs` HashMap，构造平台无关的 `PlatformDisplay` trait objects。

### `open_window()`

```rust
fn open_window(&self, handle, params) -> Result<Box<dyn PlatformWindow>> {
    let mut state = self.0.borrow_mut();
    let parent = state.keyboard_focused_window.clone();

    // 查找目标输出
    let target_output = params.display_id.and_then(|id| {
        state.wl_outputs.iter()
            .find(|(oid, _)| oid.protocol_id() as u64 == u64::from(id))
            .map(|(_, output)| output.clone())
    });

    // 创建 Wayland 窗口
    let (window, surface_id) = WaylandWindow::new(
        handle,
        state.globals.clone(),
        state.gpu_context.clone(),
        state.compositor_gpu.take(),
        WaylandClientStatePtr(Rc::downgrade(&self.0)),  // 弱引用指针
        params,
        state.common.appearance,
        parent,
        target_output,
    )?;

    // 注册到 windows map
    state.windows.insert(surface_id, window.0.clone());
    Ok(Box::new(window))
}
```

**关键细节**：`WaylandClientStatePtr(Rc::downgrade(&self.0))` 传递给窗口。这样窗口可以：

- 访问 `WaylandClientState`（通过 `upgrade()`）
- 不会导致循环引用（使用 `Weak`）
- `WaylandClient` 持有 `Rc<RefCell<...>>`，窗口持有 `Weak<RefCell<...>>`（实际类型是 `Weak`，但为了孤儿规则包裹在 `WaylandClientStatePtr` 中）

```rust
// 弱引用指针包装（孤儿规则）
pub struct WaylandClientStatePtr(Weak<RefCell<WaylandClientState>>);

impl WaylandClientStatePtr {
    pub fn get_client(&self) -> Rc<RefCell<WaylandClientState>> {
        self.0.upgrade().expect("should always be valid when dispatching")
    }
}
```

### `run()`

```rust
fn run(&self) {
    let mut event_loop = self.0.borrow_mut().event_loop
        .take().expect("App is already running");

    event_loop.run(None, &mut WaylandClientStatePtr(Rc::downgrade(&self.0)), |_| {})
        .log_err();
}
```

从 `WaylandClientState` 中取出 `event_loop` 的所有权，然后调用 `calloop::EventLoop::run()` 进入阻塞事件循环。

---

## 8. 鼠标事件完整链路（Wayland 为例）

```
Wayland compositor 发送 wl_pointer::button 事件
  ↓
calloop EventLoop 唤醒 → WaylandSource 分发
  ↓
wayland_client Dispatch 机制调用 (wayland/client.rs:1889)
  ├─ wl_pointer::Event::Button { serial, button, state, ... }
  ├─ linux_button_to_gpui(button)          // WL 按钮 → GPUI MouseButton
  ├─ ClickState 检测双击/三击
  │   ├─ click_elapsed < DOUBLE_CLICK_INTERVAL (400ms)
  │   ├─ 同按钮 && 在 DOUBLE_CLICK_DISTANCE (5px) 内
  │   └─ → click.current_count = 2 / 3
  ├─ 更新 button_pressed, serial_tracker
  ├─ 构造 PlatformInput::MouseDown(MouseDownEvent { ... })
  └─ window.handle_input(input)
      ↓
WaylandWindow::handle_input(input) (wayland/window.rs:1042)
  ├─ 检查 self.is_blocked()
  ├─ 取出 self.callbacks.borrow_mut().input.take()
  └─ fun(input.clone())  // ← 调用 window.rs 注册的回调
      ↓
Window::dispatch_event(event, cx) (window.rs:4498)
  ├─ 更新 mouse_position, modifiers, input_modality
  └─ dispatch_mouse_event()
      ├─ hit_test(mouse_position)
      ├─ Capture 阶段: mouse_listeners 正序遍历
      └─ Bubble 阶段:  mouse_listeners 逆序遍历
          └─ Interactivity 状态机 → click_listeners
              └─ 用户 .on_click() 回调
```

---

## 9. 架构设计总结

### 设计模式

```
                       ┌──────────────┐
                       │   Platform   │  trait (外部接口)
                       │   (gpui)     │
                       └──────┬───────┘
                              │ impl
                 ┌────────────▼──────────────┐
                 │   LinuxPlatform<P>         │  泛型桥接层 (gpui_linux)
                 │   impl Platform            │
                 └────────────┬──────────────┘
                              │ 委托
                 ┌────────────▼──────────────┐
                 │   LinuxClient             │  trait (内部抽象)
                 │   (gpui_linux)            │
                 └──────┬──────────┬─────────┘
                        │          │
              ┌─────────▼─┐  ┌────▼─────┐  ┌──────────┐
              │ Wayland   │  │   X11    │  │ Headless │  具体实现
              │ Client    │  │  Client  │  │  Client  │
              └───────────┘  └──────────┘  └──────────┘
```

### 状态管理模式

```
LinuxPlatform<P>
  └── inner: P (= WaylandClient)
        └── Rc<RefCell<WaylandClientState>>
              ├── common: LinuxCommon     ← 共享基础设施
              │     ├── background_executor
              │     ├── foreground_executor
              │     ├── text_system
              │     ├── callbacks         ← Platform 层设置
              │     └── signal            ← quit() 通过此发送停止信号
              │
              ├── windows: HashMap<ObjectId, WaylandWindowStatePtr>
              │     └── WaylandWindow → PlatformWindow trait
              │
              ├── globals: Globals        ← Wayland 协议全局对象
              ├── event_loop: EventLoop   ← calloop 事件循环
              └── [各种 Wayland 协议状态: seat, pointer, keyboard, ...]
```

### 关键设计决策

1. **两层 trait 抽象** — `Platform` (外部) + `LinuxClient` (内部)，两者解耦，Wayland/X11/Headless 互不可见
2. **泛型桥接** — `LinuxPlatform<P>` 用泛型而非 trait object，零额外开销
3. **`Rc<RefCell<...>>` 单线程模式** — 所有 UI 操作在主线程，无需 Mutex，但需要 RefCell 实现内部可变性
4. **`Weak` 引用避免循环** — `WaylandClientStatePtr` 封装 `Weak<RefCell<...>>`，窗口持弱引用回 client
5. **闭包式状态访问** — `with_common(|common| ...)` 精确控制 borrow 范围，避免 RefCell 借用冲突
6. **Option.take() 所有权模式** — `event_loop: Option<EventLoop>` 用 take() 转移所有权，确保 run() 只能调用一次

---

## 10. wayland/ 目录全部文件说明

Wayland 目录共 7 个源文件（不含 `mod.rs`），按从基础到复杂的层次关系排列：

```
wayland/
├── serial.rs        ← 最底层工具
├── display.rs       ← 显示器封装
├── layer_shell.rs   ← 协议转换桥
├── cursor.rs        ← 光标管理
├── clipboard.rs     ← 剪贴板
├── client.rs        ← 连接管理 + 事件循环 (97KB, 核心)
└── window.rs        ← 窗口管理 + GPU渲染 (60KB, 核心)
```

### 10a. `serial.rs` (67行) — Serial 追踪器

**Wayland 协议核心概念**：Wayland 中许多操作（设置光标、剪贴板选区、输入法）需要提供一个 `serial` 编号来证明请求是用户操作触发的，而不是程序私自发起的。

```rust
pub enum SerialKind {
    DataDevice,    // 剪贴板/拖拽相关
    InputMethod,   // 输入法
    MouseEnter,    // 鼠标进入窗口
    MousePress,    // 鼠标按下
    KeyPress,      // 键盘按下
}

pub struct SerialTracker {
    serials: HashMap<SerialKind, SerialData>,
}
```

**核心方法**：
- `update(kind, value)` — 事件处理器中每次收到 Wayland 事件时更新对应种类的 serial
- `get(kind)` — 获取指定种类的最新 serial，用于剪贴板/光标操作
- `get_latest()` — 获取所有种类中**最大值**的 serial（Wayland serial 单调递增），用于不确定触发源的情况

**使用场景**：`client.rs` 的鼠标事件处理器中调 `state.serial_tracker.update(SerialKind::MousePress, serial)`，之后 `set_cursor_style()` 时用 `serial_tracker.get(SerialKind::MouseEnter)` 获取合法 serial。

### 10b. `display.rs` (42行) — 显示器封装

```rust
pub struct WaylandDisplay {
    pub id: ObjectId,            // wl_output 协议对象 ID
    pub name: Option<String>,    // 显示器名称 (如 "DP-1")
    pub bounds: Bounds<Pixels>,  // 物理像素边界
}
```

实现了 `PlatformDisplay` trait（`gpui/src/platform.rs` 定义）：
- `id()` → 用 `protocol_id` 生成 `DisplayId`
- `uuid()` → 基于显示器名称生成 `Uuid::new_v5`（用于跨会话持久化）
- `bounds()` → 返回像素边界

**数据流**：`client.rs` 接收 `wl_output` 事件 → 填充 `InProgressOutput` → 转换为 `Output` → `displays()` 方法中包装为 `WaylandDisplay`。

### 10c. `layer_shell.rs` (26行) — 协议枚举转换桥

纯函数模块，将 GPUI 的 `layer_shell` 枚举映射到 Wayland `wlr-layer-shell` 协议枚举：

| 函数 | GPUI 类型 → Wayland 类型 |
|------|------------------------|
| `wayland_layer()` | `Layer::Background/Bottom/Top/Overlay` → `zwlr_layer_shell_v1::Layer` |
| `wayland_anchor()` | `Anchor` bits → `zwlr_layer_surface_v1::Anchor` |
| `wayland_keyboard_interactivity()` | `KeyboardInteractivity` → `zwlr_layer_surface_v1::KeyboardInteractivity` |

在 `window.rs` 的 `WaylandSurfaceState::new()` 中，当 `WindowKind::LayerShell` 时调用这些函数配置 layer surface。

### 10d. `cursor.rs` (152行) — 光标管理系统

```rust
struct Cursor {
    loaded_theme: Option<LoadedTheme>,  // 已加载的光标主题
    size: u32,                          // 逻辑尺寸
    scaled_size: u32,                   // 缩放后实际尺寸
    surface: WlSurface,                 // Wayland surface (光标纹理)
    shm: WlShm,                         // 共享内存 (光标图像需要 SHM 传递)
    connection: Connection,             // Wayland 连接
}

struct LoadedTheme {
    theme: CursorTheme,                 // wayland-cursor 库的主题
    name: Option<String>,               // 主题名 (如 "Adwaita")
    scaled_size: u32,
}
```

**核心方法**：

| 方法 | 作用 |
|------|------|
| `new()` | 创建 WlSurface + 加载系统默认光标主题 |
| `set_theme(name)` | 切换光标主题（响应 XDG Portal 设置变更） |
| `set_size(size)` | 切换光标大小 |
| `set_icon(pointer, serial, names, scale)` | **设置光标图标**：遍历候选名 → 从主题获取 `CursorImageBuffer` → `wl_pointer.set_cursor()` + `surface.attach()` + `surface.commit()` |

**`set_icon` 的工作流程**：

```
1. 遍历 cursor_icon_names 候选列表
2. theme.get_cursor(name) → CursorImageBuffer (RGBA 像素 + hotspot)
3. 如果都不在主题中 → 回退到 DEFAULT_CURSOR_ICON_NAME ("left_ptr")
4. surface.attach(buffer) → surface.damage() → surface.commit()
5. wl_pointer.set_cursor(serial, surface, hot_x, hot_y)
```

**为什么用 SHM 而非 GPU 纹理**：光标图标是小图像，Wayland 协议要求通过 `wl_shm` 共享内存传递，compositor 自行合成。

### 10e. `clipboard.rs` (263行) — 剪贴板系统

```rust
struct Clipboard {
    connection: Connection,
    loop_handle: LoopHandle<'static, WaylandClientStatePtr>,
    self_mime: String,                 // "pid/12345" — 标识自身剪贴板

    // 内部剪贴板（本程序写入的）
    contents: Option<ClipboardItem>,
    primary_contents: Option<ClipboardItem>,  // Linux 中键粘贴

    // 外部剪贴板（其他程序写入的）
    cached_read: Option<ClipboardItem>,
    current_offer: Option<DataOffer<WlDataOffer>>,       // Ctrl+C/V 剪贴板
    cached_primary_read: Option<ClipboardItem>,
    current_primary_offer: Option<DataOffer<ZwpPrimarySelectionOfferV1>>, // 中键粘贴
}

// 泛型包装，统一 WlDataOffer 和 ZwpPrimarySelectionOfferV1 的操作
struct DataOffer<T: ReceiveData> {
    inner: T,
    mime_types: Vec<String>,
}
```

**设计的 MIME 类型**：

| 方向 | MIME 类型 | 优先级 |
|------|----------|--------|
| 我们提供给其他人 | `text/plain;charset=utf-8`, `UTF8_STRING`, `text/plain` | — |
| 我们从其他人接受 | `text/plain;charset=utf-8`, `UTF8_STRING` | 按此顺序 |
| 自我识别 | `pid/{process_id}` | 检测是否是自己写入的 |

**读写流程**：

```
写入 (Ctrl+C):
  Clipboard::set(item)
    → client.rs 创建 wl_data_source
    → data_source.offer("text/plain;charset=utf-8")
    → data_device.set_selection(data_source, serial)
    → 当其他程序请求时 → send() → send_internal()
        → 通过 calloop Generic source 异步写入 Pipe fd

读取 (Ctrl+V):
  Clipboard::read()
    → 检查 current_offer 是否存在
    → 如果是自己的 mime → 直接返回 contents（避免自循环）
    → DataOffer::read_text() → 创建 Pipe
        → offer.receive_data(mime, pipe_write_fd)
        → connection.flush() → 等待 compositor 写入
        → read_fd_with_timeout(pipe_read_fd) → UTF-8 解码
        → 缓存 + 返回
```

**Linux 特有：Primary Selection（中键粘贴）**：Linux 有两套剪贴板 —— `wl_data_device` (Ctrl+C/V) 和 `zwp_primary_selection` (中键)。`Clipboard` 同时管理两者。

### 10f. `client.rs` (2558行, 97KB) — 连接管理与事件循环

**已在前文详细剖析**，此处概述：

- **`WaylandClient`** — 最外层结构，持有 `Rc<RefCell<WaylandClientState>>`
- **`WaylandClient::new()`** — 14 步初始化（连接 compositor、绑定协议、创建事件循环、注册事件源）
- **`WaylandClientState`** — 50 个字段的全部运行时状态
- **`LinuxClient` trait 实现** — run(), open_window(), displays(), 剪贴板, 光标等
- **Wayland 事件分发** — 实现 20+ 个 `Dispatch` trait，处理 `wl_pointer`、`wl_keyboard`、`wl_seat`、`wl_output`、`xdg_wm_base` 等协议事件

### 10g. `window.rs` (1730行, 60KB) — 窗口管理与 GPU 渲染

**核心结构体**：

| 结构体 | 行 | 用途 |
|--------|-----|------|
| `Callbacks` | 43 | 10 个 `Option<Box<dyn FnMut>>` 回调槽（request_frame, input, resize, close 等） |
| `RawWindow` | 56 | `*mut c_void` 裸指针对，实现 `HasWindowHandle` + `HasDisplayHandle`（给 wgpu 用） |
| `InProgressConfigure` | 83 | Wayland configure 事件的部分状态（等待多个事件拼成完整配置） |
| `WaylandWindowState` | 92 | 窗口核心状态（27 个字段：surface、renderer、bounds、input_handler、decorations 等） |
| `WaylandSurfaceState` | 129 | 枚举：`Xdg(WaylandXdgSurfaceState)` 或 `LayerShell(WaylandLayerSurfaceState)` |
| `WaylandXdgSurfaceState` | 238 | xdg_surface + toplevel + decoration + dialog |
| `WaylandLayerSurfaceState` | 245 | layer_surface |
| `WaylandWindowStatePtr` | 315 | 弱引用指针封装（防循环引用） |
| `WaylandWindow` | 446 | 对外类型，持有 `WaylandWindowStatePtr`，实现 `PlatformWindow` trait |
| `ImeInput` | 447 | IME 输入事件枚举 |

**`PlatformWindow` trait 实现**（`window.rs` 中 `WaylandWindow` 实现的方法）：

| 方法 | 作用 |
|------|------|
| `bounds()`, `content_size()`, `scale_factor()` | 几何信息查询 |
| `resize()`, `rescale()` | 尺寸/缩放变更 |
| `handle_input()` | 接收 `PlatformInput` → 调用 `Callbacks::input` 回调 → 最终到达 `Window::dispatch_event()` |
| `handle_ime()` | IME 预编辑/提交/删除 |
| `set_focused()` / `set_hovered()` | 焦点/悬停状态变更 |
| `draw(scene)` | **GPU 渲染** → 通过 `WgpuRenderer` 提交场景 |
| `on_request_frame()` | VSync 帧回调注册 |
| `on_input()` / `on_resize()` / `on_close()` | 各种事件回调注册 |
| `a11y_init()` / `a11y_tree_update()` | 可访问性支持（AccessKit） |

**Wayland 协议事件处理**（`window.rs` 中的 `WaylandWindowStatePtr` 方法）：

| 方法 | 处理的协议事件 |
|------|---------------|
| `handle_surface_event()` | `wl_surface::Event` — enter/leave output、preferred buffer scale/transform |
| `handle_xdg_surface_event()` | `xdg_surface::Event` — configure（大小/状态变更） |
| `handle_toplevel_event()` | `xdg_toplevel::Event` — close、minimize/maximize/fullscreen 状态、title |
| `handle_toplevel_decoration_event()` | `zxdg_toplevel_decoration_v1::Event` — 装饰模式协商 |
| `handle_fractional_scale_event()` | `wp_fractional_scale_v1::Event` — 非整数缩放因子 |
| `handle_layersurface_event()` | `zwlr_layer_surface_v1::Event` — layer shell 配置 |

**`Callbacks` 回调槽机制**：

所有回调（`on_input`, `on_resize` 等）通过 `take()`-调用-`放回` 模式，避免 RefCell 借用的冲突：

```rust
// 典型模式（window.rs:1042-1053）
pub fn handle_input(&self, input: PlatformInput) {
    let callback = self.callbacks.borrow_mut().input.take();
    if let Some(mut fun) = callback {
        let result = fun(input.clone());
        self.callbacks.borrow_mut().input = Some(fun);  // 放回
        if !result.propagate { return; }
    }
}
```

---

## 关键源文件索引

| 文件 | 说明 |
|------|------|
| `gpui/src/platform.rs:122-201` | `Platform` trait 定义 |
| `gpui/src/platform.rs:620-665` | `PlatformWindow` trait 定义 |
| `gpui_linux/src/linux/platform.rs:50-103` | `LinuxClient` trait 定义 |
| `gpui_linux/src/linux/platform.rs:116-157` | `LinuxCommon` 结构体 + `new()` |
| `gpui_linux/src/linux/platform.rs:159-161` | `LinuxPlatform<P>` 结构体定义 |
| `gpui_linux/src/linux/platform.rs:163-454+` | `impl Platform for LinuxPlatform<P>` |
| `gpui_linux/src/linux.rs:29-57` | `current_platform()` 平台选择 |
| `gpui_linux/src/linux/wayland/serial.rs` | `SerialTracker` — Wayland serial 追踪器（67行） |
| `gpui_linux/src/linux/wayland/display.rs` | `WaylandDisplay` — 显示器封装（42行） |
| `gpui_linux/src/linux/wayland/layer_shell.rs` | layer shell 协议枚举转换（26行） |
| `gpui_linux/src/linux/wayland/cursor.rs` | `Cursor` — 光标主题加载与图标设置（152行） |
| `gpui_linux/src/linux/wayland/clipboard.rs` | `Clipboard` + `DataOffer` — 剪贴板与中键粘贴（263行） |
| `gpui_linux/src/linux/wayland/client.rs:215-268` | `WaylandClientState` 全部状态 |
| `gpui_linux/src/linux/wayland/client.rs:302-314` | `WaylandClientStatePtr` 弱引用封装 |
| `gpui_linux/src/linux/wayland/client.rs:485` | `WaylandClient` 结构体 |
| `gpui_linux/src/linux/wayland/client.rs:539-742` | `WaylandClient::new()` 完整初始化（14步） |
| `gpui_linux/src/linux/wayland/client.rs:745-924` | `impl LinuxClient for WaylandClient` |
| `gpui_linux/src/linux/wayland/client.rs:927-942` | `WaylandClient::run()` 事件循环入口 |
| `gpui_linux/src/linux/wayland/client.rs:1889-1949` | 鼠标按钮事件处理（含 `ClickState` 双击检测） |
| `gpui_linux/src/linux/wayland/window.rs:43-54` | `Callbacks` — 10 个回调槽 + take/放回模式 |
| `gpui_linux/src/linux/wayland/window.rs:92-127` | `WaylandWindowState` — 窗口核心状态（27字段） |
| `gpui_linux/src/linux/wayland/window.rs:129-236` | `WaylandSurfaceState::new()` — XDG/LayerShell 初始化 |
| `gpui_linux/src/linux/wayland/window.rs:446-600+` | `WaylandWindow` — `PlatformWindow` trait 实现 |
| `gpui_linux/src/linux/wayland/window.rs:628-940` | Wayland 协议事件处理器（6 个 `handle_*_event` 方法） |
| `gpui_linux/src/linux/wayland/window.rs:1042` | `WaylandWindow::handle_input()` — 平台事件入口 |
| `gpui/src/window.rs:1590-1628` | `platform_window.on_input()` 回调注册 |
| `gpui/src/window.rs:4498-4683` | `Window::dispatch_event()` |
