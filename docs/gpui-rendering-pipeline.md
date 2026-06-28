# GPUI 渲染管线全流程追溯（Wayland）

> 以 `my_gpui_app/src/main.rs` 为实例，从 `main()` 到 GPU 像素输出的完整链路，重点讲解元素组织与渲染。

**相关文档**：
- [GPUI 平台层详解](./gpui-platform-layer.md) — 平台选择、`App::new_app()` 大初始化、事件循环启动
- [GPUI Linux 平台架构详解](./gpui-linux-platform-architecture.md) — `LinuxPlatform<P>` 泛型模式、`WaylandClient` 实现、wayland/ 目录全文件说明
- [GPUI 与 Wayland 交互详解](./gpui-wayland-protocol.md) — 每个 Wayland 协议操作的底层细节

---

## 示例代码总览

`my_gpui_app/src/main.rs`（61 行）：

```rust
mod native_counter;
mod component_counter;

use gpui::{
    App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::{ActiveTheme as _, Root};
use gpui_platform::application;
use native_counter::NativeCounterView;
use component_counter::ComponentCounterView;

struct MainView {
    native: gpui::Entity<NativeCounterView>,
    component: gpui::Entity<ComponentCounterView>,
}

impl MainView {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            native: cx.new(|cx| NativeCounterView::new(window, cx)),
            component: cx.new(|cx| ComponentCounterView::new(window, cx)),
        }
    }
}

impl gpui::Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex().flex_row()          // 水平 flex 布局
            .size_full()                // 占满窗口
            .gap_4().p_4()
            .bg(cx.theme().background)
            .child(self.native.clone())          // NativeCounterView
            .child(self.component.clone())        // ComponentCounterView
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

fn main() {
    application().run(|cx: &mut App| {
        gpui_component::init(cx);

        let bounds = Bounds::centered(None, size(px(640.0), px(360.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| MainView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            },
        ).unwrap();
        cx.activate(true);
    });
}
```

`NativeCounterView` (`native_counter.rs`) — 纯原生 GPUI 组件，展示 FPS 计数器 + Increment/Decrement 按钮：

```rust
impl Render for NativeCounterView {
    fn render(&mut self, window, cx) -> impl IntoElement {
        window.request_animation_frame();   // ← 触发持续渲染

        div()
            .flex().flex_col().flex_1()     // 弹性填充、垂直列
            .gap_2().p_4()
            .border_1().border_color(...).rounded_md()
            .bg(rgb(0x1e1e1e))
            .child(div().text_xl().child("Native GPUI"))
            .child(div().text_3xl().child(format!("Count: {}", self.count)))
            .child(div().text_sm().child(format!("FPS: {:.0}", self.fps)))
            .child(
                div().flex().gap_2()
                    .child(div().px_4().py_2().bg(blue).child("Increment")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count += 1; cx.notify();
                        })))
                    .child(div().px_4().py_2().bg(red).child("Decrement")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count -= 1; cx.notify();
                        })))
            )
    }
}
```

`ComponentCounterView` (`component_counter.rs`) — 使用 gpui-component 的 Button/Label 组件：

```rust
impl Render for ComponentCounterView {
    fn render(&mut self, window, cx) -> impl IntoElement {
        window.request_animation_frame();

        div().flex().flex_col().flex_1().gap_2().p_4()
            .border_1().border_color(cx.theme().border).rounded_md()
            .child(Label::new("Component").text_xl().font_weight(BOLD))
            .child(Label::new(format!("Count: {}", self.count)).text_3xl())
            .child(Label::new(format!("FPS: {:.0}", self.fps)).text_sm())
            .child(
                div().flex().gap_2()
                    .child(Button::new("incr").primary().label("Increment")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count += 1; cx.notify();
                        })))
                    .child(Button::new("decr").danger().label("Decrement")
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.count -= 1; cx.notify();
                        })))
            )
    }
}
```

**运行时行为**：两个 Counter 并排显示，各自有 Increment/Decrement 按钮。点击按钮 → `count` 变化 → `cx.notify()` → 重新 `render()` → 更新 UI。`window.request_animation_frame()` 驱动每帧 `draw()`，实现 FPS 更新。

---

## 第 1 步：平台选择与初始化（Wayland）

### 1a. `application()` — 选平台

```rust
// gpui_platform/src/gpui_platform.rs:13
pub fn application() -> gpui::Application {
    Application::with_platform(current_platform(false))
}
```

**`current_platform()`** 编译期 `#[cfg]` + 运行时 Wayland 检测：

```rust
// Linux → gpui_linux::current_platform(false)
match gpui::guess_compositor() {
    "Wayland" => Rc::new(LinuxPlatform {
        inner: WaylandClient::new()  // ← Wayland 路径
    }),
    "X11" => Rc::new(LinuxPlatform { inner: X11Client::new().unwrap() }),
    _     => Rc::new(LinuxPlatform { inner: HeadlessClient::new() }),
}
```

`WaylandClient::new()` (`wayland/client.rs:539-742`) 14 步初始化：

| 步骤 | 操作 |
|------|------|
| 1 | `Connection::connect_to_env()` 连接 Wayland compositor |
| 2 | `registry_queue_init()` 获取全局对象列表 |
| 3 | 绑定 `wl_seat`、`wl_output` |
| 4 | 创建 `calloop::EventLoop` |
| 5 | `LinuxCommon::new()` → `LinuxDispatcher::new()` |
| 6 | 注册 `main_receiver` 到 event_loop（主线程异步任务通道） |
| 7 | `detect_compositor_gpu()` |
| 8 | `Globals::new()` 绑定所有 Wayland 协议扩展 |
| 9 | 创建 `wl_data_device` + `Clipboard` |
| 10 | `Cursor::new()` 加载系统光标主题 |
| 11 | 注册 XDG Desktop Portal 事件源 |
| 12 | 组装 `WaylandClientState`（50 字段） |
| 13 | `WaylandSource::new().insert()` 注册 Wayland 事件源 |
| 14 | 返回 `Self(state)` |

### 1b. `App::new_app()` — 全局状态大初始化

`Application::with_platform()` 最终调用 `App::new_app()` (`app.rs:704-831`)，初始化整个应用的全局状态。

**初始化过程** (`app.rs:704-831`)：

```rust
pub(crate) fn new_app(platform, asset_source, http_client) -> Rc<AppCell> {
    // ── ① 从平台获取关键依赖 ──
    let background_executor = platform.background_executor();
    let foreground_executor = platform.foreground_executor();
    assert!(background_executor.is_main_thread());  // 必须在主线程

    let text_system = Arc::new(TextSystem::new(platform.text_system()));
    let entities = EntityMap::new();     // ★ 所有 View 的全局 SlotMap
    let keyboard_layout = platform.keyboard_layout();
    let keyboard_mapper = platform.keyboard_mapper();

    // ── ② Rc::new_cyclic 自引用创建 App ──
    let app = Rc::new_cyclic(|this| AppCell {
        app: RefCell::new(App {
            // === 自引用 ===
            this: this.clone(),                    // Weak<AppCell>

            // === 平台 (从 Platform trait 获取) ===
            platform: platform.clone(),
            text_system,                           // 文本塑形/渲染 (Linux: CosmicText)
            background_executor,                    // 后台线程池 (cx.background_spawn)
            foreground_executor,                    // 主线程调度 (cx.spawn)
            keyboard_layout,                        // xkbcommon 键盘布局
            keyboard_mapper,

            // === Entity 系统 (全空) ===
            entities,                               // EntityMap: SlotMap<EntityId, EntityState>
            new_entity_observers: SubscriberSet::new(),
            observers: SubscriberSet::new(),        // cx.observe() 的回调集合
            event_listeners: SubscriberSet::new(),   // cx.subscribe() 的回调集合
            release_listeners: SubscriberSet::new(),
            tracked_entities: FxHashMap::default(),
            current_window_by_entity: FxHashMap::default(),
            pending_notifications: FxHashSet::default(),  // 待处理的 cx.notify()

            // === 窗口系统 (全空) ===
            windows: SlotMap::with_key(),           // WindowId → Box<Window>
            window_handles: FxHashMap::default(),
            window_update_stack: Vec::new(),
            window_invalidators_by_entity: FxHashMap::default(),
            window_closed_observers: SubscriberSet::new(),
            focus_handles: Arc::new(RwLock::new(SlotMap::with_key())),

            // === 事件/动作系统 ===
            actions: Rc::new(ActionRegistry::default()),
            keymap: Rc::new(RefCell::new(Keymap::default())),
            global_action_listeners: TypeIdHashMap::default(),
            keystroke_observers: SubscriberSet::new(),
            keystroke_interceptors: SubscriberSet::new(),
            keyboard_layout_observers: SubscriberSet::new(),
            propagate_event: true,

            // === 全局状态 (空) ===
            globals_by_type: TypeIdHashMap::default(),  // cx.set_global() 写入
            global_observers: SubscriberSet::new(),
            pending_global_notifications: TypeIdHashSet::default(),

            // === 资源 ===
            asset_source,
            svg_renderer: SvgRenderer::new(asset_source.clone()),
            loading_assets: FxHashMap::default(),
            http_client,

            // === 渲染 ===
            text_rendering_mode: Rc::new(Cell::new(TextRenderingMode::default())),
            mode: GpuiMode::Production,
            element_arena: RefCell::new(Arena::new(1024 * 1024)),  // 1MB 预分配!
            event_arena: Arena::new(1024 * 1024),                   // 1MB 预分配!
            layout_id_buffer: Vec::default(),           // Taffy 布局缓冲区 (跨帧复用)

            // === 生命周期 ===
            quit_mode: QuitMode::default(),
            quitting: false,
            quit_observers: SubscriberSet::new(),
            restart_observers: SubscriberSet::new(),
            restart_path: None,
            cursor_hide_mode: CursorHideMode::default(),
            prompt_builder: Some(PromptBuilder::Default),
            thermal_state_observers: SubscriberSet::new(),

            // === 拖拽/效果 ===
            active_drag: None,
            pending_effects: VecDeque::new(),
            flushing_effects: false,
            pending_updates: 0,
        }),
    });

    // ── ③ 初始化平台菜单 ──
    init_app_menus(platform.as_ref(), &app.borrow());
    SystemWindowTabController::init(&mut app.borrow_mut());

    // ── ④ 注册平台生命周期回调 ──
    // 键盘布局变更 → 重新加载并通知观察者
    platform.on_keyboard_layout_change(/* 更新 keyboard_layout + 通知 observers */);
    // 热状态变更 → 通知观察者
    platform.on_thermal_state_change(/* 通知 thermal_state_observers */);
    // 平台退出 → App::shutdown()
    platform.on_quit(/* shutdown() → 标记 quitting=true + 调用 quit_observers */);

    app
}
```

### 关键设计模式

| 模式 | 说明 |
|------|------|
| `Rc::new_cyclic` 自引用 | `App.this: Weak<AppCell>` 能在构造期间获得自身弱引用 |
| `Rc<RefCell<App>>` 双包装 | 共享所有权 + 运行时借用检查（主线程，无锁） |
| `SlotMap` 窗口/Entity | 分代索引容器，key 永不重复（分配→使用→删除→复用） |
| Arena 分配器 | 1MB 预分配，帧结束后整个 Arena 重置，无逐个 free |
| SubscriberSet | 所有观察者用此类型，支持迭代中安全的增删观察者 |

### 初始化完成时的状态

```
App (Rc<RefCell<App>>)
  ├── platform: LinuxPlatform<WaylandClient>   ✓
  ├── background_executor: N 个 Worker 线程    ✓
  ├── foreground_executor: calloop 主线程通道  ✓
  ├── text_system: CosmicTextSystem            ✓
  ├── entities: EntityMap (空)       ← 等待 cx.new()
  ├── windows: SlotMap (空)          ← 等待 cx.open_window()
  ├── globals_by_type: {}            ← 等待 cx.set_global()
  ├── keymap: Keymap (默认)           ← 等待 set_keymap()
  ├── element_arena: 1MB Arena       ← 等待第一帧
  └── [50+ 个观察者集合] 全部为空    ← 等待 observe/subscribe
```

**到这一步，`Application` 构建完成，但还没有启动事件循环。**

---

## 第 2 步：`app.run()` — 平台事件循环启动

```rust
// app.rs:200
pub fn run<F>(self, on_finish_launching: F) where F: FnOnce(&mut App) {
    let platform = self.0.borrow().platform.clone();
    platform.run(Box::new(move || {
        let cx = &mut *this.borrow_mut();
        on_finish_launching(cx);   // ← 您的 main() 闭包在这里执行
    }));
}
```

**Linux `run()`** (`linux/platform.rs:197-208`)：

```rust
fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
    // ① 同步执行用户启动回调
    on_finish_launching();   // init(cx) + open_window() + cx.activate() 全在这里！

    // ② 进入 calloop 事件循环（阻塞直到退出）
    LinuxClient::run(&self.inner);
    //     └→ WaylandClient::run()
    //         └→ event_loop.run() → 循环等待 Wayland 协议事件

    // ③ 退出后 quit 回调
}
```

**① 是同步的**。您的 `init(cx)`, `cx.open_window()`, `cx.activate()` 和首次 `window.draw()` 都在 `on_finish_launching()` 中同步执行完成。

### Wayland 事件循环接收什么

```
calloop EventLoop 阻塞等待：
  ├─ wl_pointer::Event   → 鼠标移动/点击/滚动 → PlatformInput::Mouse*
  ├─ wl_keyboard::Event  → 键盘按键/释放       → PlatformInput::KeyDown/KeyUp
  ├─ wl_seat::Event      → 输入设备变更        → 重新绑定 pointer/keyboard
  ├─ wl_output::Event    → 显示器变更          → 更新 displays()
  └─ frame callback      → VSync               → on_request_frame → draw()
```

---

## 第 3 步：`gpui_component::init(cx)` — 注册全局状态

```rust
// main.rs:45
application().run(|cx: &mut App| {
    gpui_component::init(cx);   // ← 必须第一个调用
    // ...
});
```

`gpui_component::init(cx)` 内部调用 `cx.set_global()` 注册：
- **`Theme`** — 色彩系统（亮/暗模式），`cx.theme()` 的来源
- **`HighlightTheme`** — 语法高亮配色
- **`FontConfig`** — 系统字体和等宽字体配置
- **`AssetCache`** — 静态资源缓存

---

## 第 4 步：`cx.open_window()` — 创建窗口 + 首次渲染

```rust
// main.rs:48-58
cx.open_window(
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    },
    |window, cx| {
        let view = cx.new(|cx| MainView::new(window, cx));
        cx.new(|cx| Root::new(view, window, cx))
    },
).unwrap();
```

### `App::open_window()` 内部 (`app.rs:1136-1169`)

```
App::open_window(window_options, build_root_view)
  │
  ├─ cx.windows.insert(None)              // 分配窗口 SlotMap ID
  │
  ├─ Window::new(handle, options, cx)      // 创建 GPUI Window
  │   ├─ platform.open_window(...)        // → 创建原生 Wayland 窗口
  │   │   └→ WaylandClient::open_window()
  │   │       ├─ WaylandWindow::new()
  │   │       │   ├─ WaylandSurfaceState::new() → XDG Surface + XdgToplevel
  │   │       │   ├─ WgpuRenderer 初始化（Vulkan GPU 上下文）
  │   │       │   └─ 注册 Callbacks（input, request_frame, resize, close...）
  │   │       └─ state.windows.insert(surface_id, window)
  │   │
  │   ├─ platform_window.on_input(|event| { window.dispatch_event(event, cx) })
  │   │   // ↑ 最关键的回调：平台事件 → GPUI 事件系统
  │   ├─ platform_window.on_request_frame(|options| { window.draw() })
  │   └─ platform_window.on_resize(|size, scale| { /* 更新视口 */ })
  │
  ├─ build_root_view(&mut window, cx)
  │   ├─ cx.new(|cx| MainView::new(window, cx))
  │   │   └─ MainView { native: Entity<NativeCounterView>, component: Entity<ComponentCounterView> }
  │   └─ cx.new(|cx| Root::new(view, window, cx))
  │       └─ Root 是最外层视图，管理 dialog/sheet/notification 层
  │
  ├─ window.root.replace(root_view)       // 设置根视图
  ├─ window.defer(appearance_changed)      // 延迟外观更新
  │
  └─ window.draw(cx)                       // ★ 首次渲染！
      └→ draw_roots()                      // → 第 6 步
```

---

## 第 5 步：元素树组织 — `Render` trait 与 `Entity<T>` 子视图

### 5a. 完整的元素树结构

`MainView::render()` (`main.rs:27-40`) 构建的元素树：

```
Root (最外层)
  └─ MainView::render()
      └─ div()                          ← 根 Div（flex row, size_full, gap_4, p_4）
          │    .flex().flex_row()
          │    .size_full()
          │    .bg(cx.theme().background)
          │
          ├─ .child(self.native.clone())
          │   └─ Entity<NativeCounterView> → 调用 NativeCounterView::render()
          │       └─ div()                ← Native 面板（flex col, flex_1, 圆角, 边框）
          │           │    .flex().flex_col().flex_1()
          │           │    .border_1().border_color().rounded_md().bg(0x1e1e1e)
          │           │
          │           ├─ .child(div().text_xl().child("Native GPUI"))        ← 标题
          │           ├─ .child(div().text_3xl().child("Count: 0"))           ← 计数
          │           ├─ .child(div().text_sm().child("FPS: 0"))              ← FPS
          │           └─ .child(
          │               div().flex().gap_2()
          │               ├─ .child(div().px_4().py_2().bg(blue)
          │               │           .child("Increment")
          │               │           .on_click(|this| { this.count += 1; cx.notify() }))
          │               └─ .child(div().px_4().py_2().bg(red)
          │                           .child("Decrement")
          │                           .on_click(|this| { this.count -= 1; cx.notify() }))
          │
          ├─ .child(self.component.clone())
          │   └─ Entity<ComponentCounterView> → 调用 ComponentCounterView::render()
          │       └─ div()                ← Component 面板（flex col, flex_1, 圆角, 边框）
          │           │    .flex().flex_col().flex_1()
          │           │    .border_1().border_color(cx.theme().border)
          │           │
          │           ├─ .child(Label::new("Component"))                      ← 标题
          │           ├─ .child(Label::new("Count: 0"))                       ← 计数
          │           ├─ .child(Label::new("FPS: 0"))                         ← FPS
          │           └─ .child(
          │               div().flex().gap_2()
          │               ├─ .child(Button::new("incr").primary().label("Increment")
          │               │           .on_click(|this| { this.count += 1; cx.notify() }))
          │               └─ .child(Button::new("decr").danger().label("Decrement")
          │                           .on_click(|this| { this.count -= 1; cx.notify() }))
          │
          ├─ .children(Root::render_dialog_layer(...))        ← 对话框层
          ├─ .children(Root::render_sheet_layer(...))          ← 侧面板层
          └─ .children(Root::render_notification_layer(...))   ← 通知层
```

### 5b. `Entity<T>` 子视图是如何工作的

```rust
// main.rs:12
struct MainView {
    native: gpui::Entity<NativeCounterView>,    // Entity 句柄，不是 NativeCounterView 本身
    component: gpui::Entity<ComponentCounterView>,
}
```

`Entity<T>` 是一个**轻量句柄**，指向 `EntityMap` 中的槽位。实际数据存储在 `App` 全局的 `entities: EntityMap` 中。

当元素树中有 `.child(self.native.clone())`：
1. 框架发现 child 是 `Entity<T>` 类型
2. 在 `request_layout` 阶段，调用 `entity.read_with(cx, |state, cx| state.render(window, cx))` 获取子元素树
3. 子元素树作为当前 Div 的 child 注册到 Taffy

**`Render` trait** (`element.rs:147`)：

```rust
pub trait Render: 'static + Sized {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement;
}
```

`impl IntoElement` 意味着可以返回任意元素类型——`Div`、`Label`、`Button`、`SharedString` 等。

### 5c. `div()` 与样式链（Builder 模式）

```rust
// elements/div.rs:1505
pub fn div() -> Div {
    Div {
        interactivity: Interactivity::new(),  // 60+ 交互状态字段
        children: SmallVec::default(),         // 子元素列表
        prepaint_listener: None,
        image_cache: None,
        prepaint_order_fn: None,
    }
}
```

每个样式方法返回 `Self`，内部修改 `Interactivity.base_style: StyleRefinement`：

| 您代码中的方法 | GPUI 内部设置 | 布局效果 |
|--------------|-------------|---------|
| `.flex()` | `display: Flex` | 子元素参与 flex 布局 |
| `.flex_row()` | `flex_direction: Row` | 水平排列 |
| `.flex_col()` | `flex_direction: Column` | 垂直排列 |
| `.flex_1()` | `flex_grow: 1.0, flex_shrink: 1.0` | 弹性填充剩余空间 |
| `.size_full()` | `size: relative(1.0, 1.0)` | 宽高 100% |
| `.gap_2()` | `gap: 0.5rem` | 子元素间距 |
| `.gap_4()` | `gap: 1rem` | 子元素间距 |
| `.p_4()` | `padding: 1rem` | 内边距 |
| `.px_4()` | `padding_left: 1rem, padding_right: 1rem` | 水平内边距 |
| `.py_2()` | `padding_top: 0.5rem, padding_bottom: 0.5rem` | 垂直内边距 |
| `.bg(color)` | `background: Fill::color(...)` | 背景色 |
| `.border_1()` | `border_width: 1px` | 边框宽度 |
| `.border_color(c)` | `border_color: c` | 边框颜色 |
| `.rounded_md()` | `border_radius: 0.375rem` | 圆角 |
| `.text_xl()` | `font_size: 1.25rem` | 字号 |
| `.text_color(c)` | `text_color: c` | 文字颜色 |
| `.font_weight(w)` | `font_weight: w` | 字重 |
| `.cursor_pointer()` | `mouse_cursor: PointingHand` | 鼠标指针样式 |
| `.child(x)` | 追加到 `children` | 添加子元素 |
| `.children(iter)` | 批量追加 | 批量添加子元素 |
| `.on_click(f)` | 追加到 `click_listeners` | 点击回调 |

---

## 第 6 步：渲染帧循环 — `window.draw()` → `draw_roots()`

在 `my_gpui_app` 中，有两类触发 `draw()` 的场景：
1. **首次渲染**：`cx.open_window()` 内部调用 `window.draw(cx)` 确保窗口有初始内容
2. **每帧动画**：`window.request_animation_frame()` (在 `NativeCounterView::render()` 和 `ComponentCounterView::render()` 中调用) → VSync → `on_request_frame` → `draw()`
3. **状态变更**：点击按钮 → `cx.notify()` → 标记 View 为脏 → `draw()` → 重新 `render()`

### `draw()` (`window.rs:2603-2652`)

```rust
pub fn draw(&mut self, cx: &mut App) -> ArenaClearNeeded {
    // ① 创建 Element Arena（本帧所有元素的分配器，帧结束后清空释放）
    let _arena_scope = ElementArenaScope::enter(&cx.element_arena);

    // ② 处理标记为脏的 Entity → 调用它们的 render()
    self.invalidate_entities();   // cx.notify() 和 request_animation_frame() 的结果在此生效

    // ③ 恢复之前使用的 input handler（IME 复用）
    if let Some(input_handler) = self.platform_window.take_input_handler() { ... }

    // ④ 绘制根元素树（三阶段管线）
    if !cx.mode.skip_drawing() {
        self.draw_roots(cx);
    }

    // ⑤ 注册新的 input handler 到平台窗口
    // ...
}
```

### `draw_roots()` 三阶段管线 (`window.rs:2764-2859`)

```
draw_roots()
  │
  ├─ [PHASE 1: LAYOUT]
  │   root_element.layout_as_root(available_space)  // ← Taffy Flexbox
  │   └→ 递归: 每个 Div 调用 request_layout → 注册到 Taffy → compute_layout
  │       └→ 所有元素的 x, y, width, height 确定
  │
  ├─ [PHASE 2: PREPAINT]
  │   root_element.prepaint_as_root(origin, available_space)
  │   └→ 递归: Div::prepaint → Interactivity::prepaint
  │       ├─ 创建 Hitbox（为每个交互元素建立命中检测矩形）
  │       ├─ 注册 mouse_listeners（on_click, on_mouse_down...）
  │       └─ 注册 key_listeners, scroll_listeners
  │
  ├─ [PHASE 2.5: HIT_TEST]
  │   mouse_hit_test = rendered_frame.hit_test(mouse_position)
  │   └→ 遍历所有 Hitbox Z 序 → 找到鼠标下面的元素
  │
  └─ [PHASE 3: PAINT]
      root_element.paint(window, cx)
      └→ 递归: Div::paint → Interactivity::paint → Style::paint
          ├─ 绘制投影（box-shadow）
          ├─ 绘制背景（.bg(), .border_1(), .rounded_md() 在这里生效）
          ├─ 递归绘制子元素
          └─ 绘制边框
              ↓
      WaylandWindow::draw(scene) → WgpuRenderer::draw() → GPU
```

---

## 第 7 步：Phase 1 — Taffy Flexbox 布局

这是 **元素树组织** → **像素坐标** 的关键转换。

### `Div::request_layout()` (`div.rs:1626-1659`)

```rust
fn request_layout(&mut self, global_id, inspector_id, window, cx) -> (LayoutId, ...) {
    // ① 递归注册子元素
    let child_layout_ids = self.children.iter_mut()
        .map(|child| child.request_layout(window, cx))
        .collect::<SmallVec<_>>();

    // ② 将自己 + 子元素提交给 Taffy
    let layout_id = window.request_layout(style, child_layout_ids, cx);
    (layout_id, DivFrameState { child_layout_ids })
}
```

### `compute_layout()` (`taffy.rs:162-209`)

```rust
pub fn compute_layout(&mut self, id, available_space, window, cx) {
    // 缩放因子转换（HiDPI → 物理像素）
    let available_space = size(
        transform(available_space.width),   // * scale_factor
        transform(available_space.height),
    );

    // Taffy 核心 CSS Flexbox 算法
    self.taffy.compute_layout(id.into(), available_space).unwrap();

    // 读取结果
    for child in self.taffy.children(id.into())? {
        let layout = self.taffy.layout(child)?;
        let bounds = self.convert_layout_to_bounds(layout, scale_factor);
        self.absolute_layout_bounds.insert(LayoutId(child), bounds);
    }
}
```

### 您的代码在这里发生了什么

以 MainView 的根 div 为例（`.flex().flex_row().size_full().gap_4().p_4()`）：

```
窗口大小: 640 x 360 px
  → MainView div (flex row, size_full, p_4=1rem=16px, gap_4=1rem=16px)
    ├─ 可用空间: (640-32) x (360-32) = 608 x 328
    │
    ├─ child 1: NativeCounterView div (flex_1)
    │   └─ Taffy 计算: width = (608 - gap_4) / 2 = 296px, height = 328px
    │
    └─ child 2: ComponentCounterView div (flex_1)
        └─ Taffy 计算: width = 296px, height = 328px
```

Taffy 执行完整 CSS Flexbox 算法：
- `width`, `height`, `min_size`, `max_size`
- `flex_grow`, `flex_shrink`, `flex_basis`（`flex_1()` 设置 grow=1）
- `flex_direction`, `justify_content`, `align_items`, `gap`
- `padding`, `margin`, `border`
- `position: absolute`（相对于最近的 `position: relative` 父元素）

---

## 第 8 步：Phase 2 — Prepaint（Hitbox + 事件注册）

### `Interactivity::prepaint()` (`div.rs:1975-2148`)

```rust
pub fn prepaint(&mut self, global_id, bounds, window, cx, f) {
    // ① 创建 Hitbox
    let hitbox = window.insert_hitbox(bounds, self.hitbox_behavior);
    //    ↑ 为 onclick / on_mouse_down 等注册命中检测矩形

    // ② 计算合并后的 Style（base + hover + focus + active + group_* 等）
    let style = self.compute_style_internal(hitbox, element_state, window, cx);

    // ③ 注册鼠标事件监听器
    self.paint_mouse_listeners(&hitbox, element_state, window, cx);
    //   └→ 将 click_listeners, mouse_down_listeners 等推入 rendered_frame.mouse_listeners

    // ④ 注册键盘/action 监听器
    self.paint_keyboard_listeners(window, cx);

    // ⑤ 注册滚动监听器
    self.paint_scroll_listener(&hitbox, &style, window, cx);
}
```

### Hitbox 结构

```rust
pub struct Hitbox {
    pub id: HitboxId,
    pub bounds: Bounds<Pixels>,             // 命中检测矩形
    pub content_mask: ContentMask<Pixels>,  // overflow 裁剪区域
    pub behavior: HitboxBehavior,           // occlude_mouse | block_mouse_except_scroll | default
}
```

### 事件传播模式

所有 `mouse_listeners` 在事件到来时按两个阶段调用：

```
mouse_listeners
  ├─ Capture 阶段：正序遍历（从底层到上层，从后往前命中检测）
  └─ Bubble 阶段：  逆序遍历（从上层到底层，类似 DOM 冒泡）
       └─ cx.propagate_event = false 可停止传播
```

---

## 第 9 步：Phase 3 — Paint（像素级渲染）

### `Style::paint()` (`style.rs:684-758`)

```rust
pub fn paint(&self, bounds, window, cx, continuation) {
    // ① 绘制投影（box-shadow）
    window.paint_drop_shadows(bounds, corner_radii, &self.box_shadow);

    // ② 绘制背景 —— .bg(), .border_1(), .rounded_md() 在这里生效
    if background_color.is_some_and(|c| !c.is_transparent()) {
        window.paint_quad(fill_quad);
    }

    // ③ 绘制内阴影
    window.paint_inset_shadows(bounds, corner_radii, &self.box_shadow);

    // ④ 递归绘制子元素
    continuation(window, cx);

    // ⑤ 绘制边框 —— .border_1(), .border_color() 在这里生效
    if self.is_border_visible() {
        window.paint_quad(border_quad);
    }
}
```

`window.paint_quad()` → `Scene::push_quad()` 推入绘制指令：

```
Scene (每帧的绘制指令集合)
  ├─ quads: Vec<Quad>          // 矩形（背景、边框、圆角）
  ├─ shadows: Vec<Shadow>      // 投影
  ├─ underlines: Vec<Underline>
  ├─ mono_sprites: Vec<Sprite>  // 单色文字
  ├─ poly_sprites: Vec<Sprite>  // 多色文字（emoji）
  └─ paths: Vec<Path>           // 矢量路径
```

### GPU 渲染 — WaylandWindow::draw()

```
Style::paint() 等收集完成 → Scene
  ↓
WaylandWindow::draw(scene)
  └→ WgpuRenderer::draw(scene)
      ├─ 更新 vertex/index/uniform buffers
      ├─ 发出 draw commands:
      │   ├─ draw_quads()               → 渲染所有矩形背景/边框
      │   ├─ draw_shadows()             → 渲染阴影
      │   ├─ draw_monochrome_sprites()  → 渲染文字（"Native GPUI", "Count: 5", "FPS: 60"）
      │   ├─ draw_polychrome_sprites()  → 渲染 emoji/彩色文字
      │   ├─ draw_underlines()          → 渲染下划线
      │   └─ draw_paths()               → 渲染矢量路径
      └─ wgpu::Queue::submit()          → GPU 执行
          ↓
      wl_surface::frame() callback      → Wayland compositor 合成到屏幕
```

---

## 第 10 步：Click 交互全链路（Wayland）

以点击 "Increment" 按钮为例，追踪从物理点击到 `count += 1` 的完整链路：

### 10a. Wayland compositor → PlatformInput

```
用户点击 "Increment" 按钮
  ↓
Wayland compositor 发送 wl_pointer::button 事件
  ↓
calloop EventLoop 唤醒 → WaylandSource 分发
  ↓
WaylandClientStatePtr::event() 中的 Dispatch<wl_pointer> 实现
  (wayland/client.rs:1889-1949)
  │
  ├─ linux_button_to_gpui(button)     // WL 按钮码 → GPUI MouseButton::Left
  ├─ ClickState 双击/三击检测
  │   ├─ click_elapsed < 400ms?
  │   ├─ 同按钮 && 距离 < 5px?
  │   └─ → click_count = 1 / 2 / 3
  ├─ serial_tracker.update(SerialKind::MousePress, serial)
  └─ 构造 PlatformInput::MouseDown(MouseDownEvent {
         button: Left,
         position: Point { x: ..., y: ... },
         modifiers: Modifiers::default(),
         click_count: 1,
     })
  └─ focused_window.handle_input(input)
      ↓
WaylandWindow::handle_input(input) (wayland/window.rs:1042)
  ├─ 检查 self.is_blocked()
  ├─ Callbacks::input.take()
  └─ fun(input)  // ← Window::dispatch_event
```

### 10b. `Window::dispatch_event()` (`window.rs:4498`)

```rust
pub fn dispatch_event(&mut self, event: PlatformInput, cx: &mut App) {
    // 更新内部状态
    self.mouse_position = mouse_event.position;
    self.modifiers = mouse_event.modifiers;
    self.last_input_modality = InputModality::Mouse;

    if let Some(any_mouse_event) = event.mouse_event() {
        self.dispatch_mouse_event(any_mouse_event, cx);
    }
}
```

### 10c. `dispatch_mouse_event()` — 命中检测 + 事件传播

```rust
fn dispatch_mouse_event(&mut self, event, cx) {
    // ① 命中检测（Z 序遍历 Hitbox）
    let hit_test = self.rendered_frame.hit_test(self.mouse_position());
    //   → 找到最顶层的 Hitbox："Increment" 按钮的 div

    // ② Capture 阶段：从底层到上层
    for listener in &mut mouse_listeners {
        listener(event, DispatchPhase::Capture, self, cx);
        if !cx.propagate_event { break; }
    }

    // ③ Bubble 阶段：从上层到底层（类似 DOM 冒泡）
    if cx.propagate_event {
        for listener in mouse_listeners.iter_mut().rev() {
            listener(event, DispatchPhase::Bubble, self, cx);
            if !cx.propagate_event { break; }
        }
    }
}
```

### 10d. Interactivity 状态机 → Click 触发

```
MouseDown → Div::on_mouse_down 回调
  → 记录 pressed_hitbox + button
MouseUp   → Div::on_mouse_up 回调  
  → Interactivity 状态机:
      ├─ 同一 hitbox? ✓ ("Increment" 按钮)
      ├─ 同一 button? ✓
      └─ 触发 click_listeners
          └─ 您注册的闭包:
              cx.listener(|this, _event, _, cx| {
                  this.count += 1;    // ← NativeCounterView.count 变为 1
                  cx.notify();        // ← 标记 View 为脏
              })
```

### 10e. `cx.notify()` → 下一帧重新渲染

```
cx.notify()
  → invalidator.set_dirty(true)
  → 如果不在 draw 阶段 → 触发 window.draw()
  → window.draw() 中 invalidate_entities()
      → 调用 NativeCounterView::render()
          → 新的 count 值 → 新的元素树 → "Count: 1"
          → 重新 LAYOUT / PREPAINT / PAINT
          → WgpuRenderer::draw() → GPU 提交新帧
          → 屏幕显示 "Count: 1"
```

---

## 动画/持续渲染链路

`NativeCounterView::render()` 中的 `window.request_animation_frame()` 创建了每帧的渲染循环：

```
render() 被调用
  └→ window.request_animation_frame()
      └→ 注册 frame callback 到 platform_window
          └→ VSync 到来
              └→ Wayland frame callback 事件
                  └→ calloop 事件循环处理
                      └→ on_request_frame 回调
                          └→ window.draw()
                              └→ draw_roots() → render() → request_animation_frame() → 循环...
```

这就是为什么 FPS 计数器能实时更新——渲染过程中调用 `request_animation_frame()`，注册下一帧的回调，形成**无限循环**。

---

## 完整时序图（Wayland 路径，my_gpui_app 为例）

```
main()
 │
 ├─ application()
 │   ├─ current_platform(false) → "Wayland"
 │   │   └→ WaylandClient::new()
 │   │       ├─ Connection::connect_to_env()
 │   │       ├─ registry_queue_init() → 全局对象
 │   │       ├─ calloop::EventLoop::new()
 │   │       ├─ LinuxCommon::new() → LinuxDispatcher::new()
 │   │       │   ├─ N 个 Worker 线程
 │   │       │   ├─ Timer 线程
 │   │       │   └─ 主线程 calloop 通道
 │   │       ├─ Globals::new() → 绑定所有协议扩展
 │   │       ├─ Clipboard::new(), Cursor::new()
 │   │       └─ WaylandSource::new().insert()  // 注册 Wayland 事件源
 │   └→ App::new_app() → Rc<RefCell<App>>
 │
 ├─ app.run(|cx| {
 │     ←─ on_finish_launching() 同步执行
 │     │
 │     ├─ gpui_component::init(cx)     // 注册 Theme, HighlightTheme
 │     │
 │     ├─ cx.open_window(options, |window, cx| {
 │     │   ├─ Window::new(handle, options, cx)
 │     │   │   ├─ platform.open_window() → WaylandWindow::new()
 │     │   │   │   ├─ WaylandSurfaceState::new() → XDG Surface + Toplevel
 │     │   │   │   └─ WgpuRenderer 初始化
 │     │   │   └─ platform_window.on_input(|e| window.dispatch_event(e, cx))
 │     │   │
 │     │   ├─ cx.new(|cx| MainView::new(window, cx))
 │     │   │   ├─ cx.new(|cx| NativeCounterView::new(window, cx))
 │     │   │   └─ cx.new(|cx| ComponentCounterView::new(window, cx))
 │     │   ├─ cx.new(|cx| Root::new(view, window, cx))
 │     │   ├─ window.root.replace(root_view)
 │     │   │
 │     │   └─ 【★ 首次渲染】
 │     │       window.draw(cx)
 │     │       └→ draw_roots()
 │     │           ├─ [LAYOUT]   Taffy Flexbox 布局: 640x360 → 两列 flex_1 并排
 │     │           ├─ [PREPAINT] 创建 Hitbox + 注册 click_listeners
 │     │           ├─ [HIT_TEST] 建立鼠标命中树
 │     │           └─ [PAINT]    Style::paint() 背景→子元素→边框
 │     │                          └→ Scene → WgpuRenderer::draw() → GPU
 │     │   })
 │     │
 │     └─ cx.activate(true)
 │   })
 │
 ├─ ② LinuxClient::run(&self.inner)   ← 阻塞在 calloop 事件循环
 │   └→ WaylandClient::run() → event_loop.run()
 │       │
 │       └─ [循环等待事件]
 │           ├─ wl_pointer::button (用户点击 "Increment")
 │           │   ├─ ClickState 双击检测
 │           │   ├─ PlatformInput::MouseDown
 │           │   ├─ handle_input → dispatch_event
 │           │   ├─ hit_test → 找到按钮的 Hitbox
 │           │   ├─ Bubble → click_listeners → count += 1 → cx.notify()
 │           │   └─ draw() → draw_roots() → 新 count 值渲染
 │           │
 │           ├─ wl_surface::frame (VSync)
 │           │   └─ on_request_frame → draw()
 │           │       └→ render() → request_animation_frame() → 循环...
 │           │
 │           ├─ wl_keyboard::key (键盘按键)
 │           ├─ wl_output::done (显示器变更)
 │           └─ calloop timer (到期的异步任务)
 │
 └─ ③ quit 回调（event_loop 被 signal.stop() 终止）
```

---

## Event Flow 一图总览

```
Wayland Compositor  ──wl_pointer──► calloop EventLoop ──► WaylandSource 分派
                                                              │
                            WaylandClientStatePtr::event() (client.rs)
                            ├─ linux_button_to_gpui()
                            ├─ ClickState 双击检测
                            └─ PlatformInput::MouseDown(...)
                                                              │
                            WaylandWindow::handle_input(input) (window.rs)
                            ├─ Callbacks::input.take()
                            └─ fun(input)  ──►  Window::dispatch_event() (window.rs)
                                                  ├─ 更新 mouse_position/modifiers
                                                  └─ dispatch_mouse_event()
                                                      ├─ hit_test(position)
                                                      │   └─ 遍历 Hitbox → 找到按钮 div
                                                      ├─ Capture: 正序 mouse_listeners
                                                      └─ Bubble:  逆序 mouse_listeners
                                                          │
                                            Div 的鼠标回调 (div.rs)
                                            ├─ on_mouse_down → 记录状态
                                            ├─ on_mouse_up   → 状态机检测
                                            └─ on_click 触发
                                                └─ cx.listener(|this, _, _, cx| {
                                                        this.count += 1;
                                                        cx.notify();
                                                    })
                                                        │
                                            下一帧 draw() → draw_roots()
                                            ├─ invalidate_entities() → render()
                                            │   └─ 新的元素树: Count 值已更新
                                            ├─ LAYOUT: Taffy 重新计算（布局可能不变）
                                            ├─ PREPAINT: 更新 Hitbox（位置可能不变）
                                            └─ PAINT: Style::paint() → Scene
                                                 └─ WgpuRenderer::draw() → GPU
                                                      └─ 屏幕显示: "Count: 1"
```

---

## 关键源文件索引

| 文件 | 行数 | 说明 |
|------|------|------|
| **`my_gpui_app/src/main.rs`** | 61 | **本文档实例** — 双 Counter 并排布局入口 |
| `my_gpui_app/src/native_counter.rs` | 106 | 纯原生 GPUI Counter（FPS + 按钮 + on_click） |
| `my_gpui_app/src/component_counter.rs` | 92 | gpui-component Counter（Button/Label + on_click） |
| `gpui_platform/src/gpui_platform.rs` | 186 | 跨平台入口 `application()` + `current_platform()` |
| `gpui_linux/src/linux.rs` | 57 | Linux 平台选择 `current_platform(headless)` |
| `gpui_linux/src/linux/platform.rs` | 1257 | `LinuxPlatform<P>` + `Platform` impl + `LinuxClient` trait |
| `gpui_linux/src/linux/dispatcher.rs` | 362 | `LinuxDispatcher` — Worker/Timer/Main 三层任务调度 |
| `gpui_linux/src/linux/wayland/client.rs` | 2558 | `WaylandClient` — 连接、事件循环、20+ Dispatch 实现 |
| `gpui_linux/src/linux/wayland/window.rs` | 1730 | `WaylandWindow` — `PlatformWindow` impl + WgpuRenderer |
| `gpui/src/app.rs` | 2822 | `Application` + `App::new_app()` + `open_window()` + `run()` |
| `gpui/src/window.rs` | 6265 | `Window` — `draw()` / `draw_roots()` / `dispatch_event()` |
| `gpui/src/element.rs` | 846 | `Element` / `Render` / `IntoElement` trait 定义 |
| `gpui/src/elements/div.rs` | 4182 | `Div` — 布局 + 交互（`Interactivity`）+ 渲染 |
| `gpui/src/style.rs` | 1525 | `Style` — 背景/边框/阴影渲染（`Style::paint()`） |
| `gpui/src/taffy.rs` | 751 | Taffy Flexbox 引擎封装 |
| `gpui_wgpu/src/wgpu_renderer.rs` | — | WGPU 渲染器（Wayland GPU 后端） |
| `gpui-platform-layer.md` | — | **文档** — 平台选择 + `App::new_app()` 大初始化 |
| `gpui-linux-platform-architecture.md` | — | **文档** — `LinuxPlatform<P>` + `WaylandClient` 实现细节 |
