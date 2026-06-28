# GPUI 平台层详解 — 从 `application().run()` 到进入事件循环

> 详细解释第一层：平台选择、基础设施初始化、事件循环启动的全过程。

---

## 1. 入口：`gpui_platform::application()` (`gpui_platform/src/gpui_platform.rs:13-15`)

```rust
pub fn application() -> gpui::Application {
    gpui::Application::with_platform(current_platform(false))
}
```

极其简洁——一行代码做了两件事：**选平台** + **构建 Application**。

---

## 2. 平台选择：`current_platform()` (`gpui_platform.rs:36-60`)

通过**条件编译**在编译时决定用哪个平台实现：

```rust
pub fn current_platform(headless: bool) -> Rc<dyn Platform> {
    #[cfg(target_os = "macos")]
    { Rc::new(gpui_macos::MacPlatform::new(headless)) }

    #[cfg(target_os = "windows")]
    { Rc::new(gpui_windows::WindowsPlatform::new(headless).expect("...")) }

    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    { gpui_linux::current_platform(headless) }  // ← 您的环境走这里

    #[cfg(target_family = "wasm")]
    { Rc::new(gpui_web::WebPlatform::new(true)) }
}
```

在 Linux 上进一步判断 (`gpui_linux/src/linux.rs:29-57`)：

```rust
pub fn current_platform(headless: bool) -> Rc<dyn Platform> {
    if headless {
        return Rc::new(LinuxPlatform { inner: HeadlessClient::new() });
    }
    match gpui::guess_compositor() {
        "Wayland" => Rc::new(LinuxPlatform { inner: WaylandClient::new() }),
        "X11"     => Rc::new(LinuxPlatform { inner: X11Client::new().unwrap() }),
        _         => Rc::new(LinuxPlatform { inner: HeadlessClient::new() }),
    }
}
```

`guess_compositor()` (`gpui/src/platform.rs:94-100`) 通过检查 `WAYLAND_DISPLAY` 环境变量来判断。**这一步就决定了您后面走的是 Wayland 还是 X11 的渲染和事件处理路径。**

> **深入阅读**: `LinuxPlatform` 泛型包装模式的详细架构分析见 [GPUI Linux 平台架构详解](./gpui-linux-platform-architecture.md)（三层 trait 抽象、`LinuxClient` 接口、`WaylandClient` 实现细节、鼠标事件完整链路）

---

## 3. `Platform` trait — 平台抽象接口 (`gpui/src/platform.rs:122-201`)

所有平台实现（macOS/Linux/Windows/WASM）都必须实现这个 trait：

```rust
pub trait Platform: 'static {
    // 执行器
    fn background_executor(&self) -> BackgroundExecutor;
    fn foreground_executor(&self) -> ForegroundExecutor;

    // 文本系统
    fn text_system(&self) -> Arc<dyn PlatformTextSystem>;

    // 生命周期
    fn run(&self, on_finish_launching: Box<dyn 'static + FnOnce()>);
    fn quit(&self);
    fn restart(&self, binary_path: Option<PathBuf>);

    // 窗口管理
    fn open_window(&self, handle: AnyWindowHandle, options: WindowParams)
        -> anyhow::Result<Box<dyn PlatformWindow>>;

    // 显示管理
    fn displays(&self) -> Vec<Rc<dyn PlatformDisplay>>;
    fn primary_display(&self) -> Option<Rc<dyn PlatformDisplay>>;

    // 菜单系统
    fn set_menus(&self, menus: Vec<Menu>, keymap: &Keymap);
    fn set_dock_menu(&self, menu: Vec<MenuItem>, keymap: &Keymap);

    // 文件对话框
    fn prompt_for_paths(&self, options: PathPromptOptions) -> oneshot::Receiver<Result<...>>;
    fn prompt_for_new_path(&self, directory: &Path, ...) -> oneshot::Receiver<Result<...>>;

    // URL / 应用激活
    fn open_url(&self, url: &str);
    fn activate(&self, ignoring_other_apps: bool);
    fn hide(&self);
    fn hide_other_apps(&self);

    // 剪贴板
    fn write_to_clipboard(&self, item: ClipboardItem);
    fn read_from_clipboard(&self) -> Option<ClipboardItem>;

    // 回调注册
    fn on_quit(&self, callback: Box<dyn FnMut()>);
    fn on_reopen(&self, callback: Box<dyn FnMut()>);
    fn on_open_urls(&self, callback: Box<dyn FnMut(Vec<String>)>);

    // 窗口外观
    fn window_appearance(&self) -> WindowAppearance;
    // ...
}
```

---

## 4. 构建 Application：`App::with_platform()` (`app.rs:150-156`)

```rust
pub fn with_platform(platform: Rc<dyn Platform>) -> Self {
    Self(App::new_app(platform, Arc::new(()), Arc::new(NullHttpClient)))
}
```

### `App::new_app()` (`app.rs:704-779`) — GPUI 大初始化

```rust
pub(crate) fn new_app(
    platform: Rc<dyn Platform>,
    asset_source: Arc<dyn AssetSource>,
    http_client: Arc<dyn HttpClient>,
) -> Rc<AppCell> {
    // 1. 从平台获取执行器
    let background_executor = platform.background_executor();
    let foreground_executor = platform.foreground_executor();

    // 2. 断言必须在主线程上运行
    assert!(
        background_executor.is_main_thread(),
        "must construct App on main thread"
    );

    // 3. 创建文本系统
    let text_system = Arc::new(TextSystem::new(platform.text_system()));

    // 4. 创建 EntityMap — 所有 View/Entity 的注册表
    let entities = EntityMap::new();

    let keyboard_layout = platform.keyboard_layout();
    let keyboard_mapper = platform.keyboard_mapper();

    // 5. 构建 App 状态（Rc<RefCell<App>> 模式）
    let app = Rc::new_cyclic(|this| AppCell {
        app: RefCell::new(App {
            this: this.clone(),
            // === 平台相关 ===
            platform: platform.clone(),     // 平台 trait 对象
            text_system,                    // 文本塑形/渲染引擎
            background_executor,            // 后台线程池
            foreground_executor,            // 主线程调度器
            keyboard_layout,                // 键盘布局
            keyboard_mapper,                // 键盘映射

            // === 状态管理 ===
            entities,                       // Entity 注册表（SlotMap）
            globals_by_type: Default::default(),  // 全局状态
            windows: SlotMap::with_key(),   // 窗口槽（此时为空）
            window_handles: FxHashMap::default(),
            focus_handles: Arc::new(RwLock::new(SlotMap::with_key())),

            // === 事件系统 ===
            actions: Rc::new(ActionRegistry::default()),
            keymap: Rc::new(RefCell::new(Keymap::default())),
            global_action_listeners: Default::default(),
            keystroke_observers: SubscriberSet::new(),
            keystroke_interceptors: SubscriberSet::new(),

            // === 观察者系统 ===
            observers: SubscriberSet::new(),
            new_entity_observers: SubscriberSet::new(),
            event_listeners: SubscriberSet::new(),
            release_listeners: SubscriberSet::new(),
            global_observers: SubscriberSet::new(),
            quit_observers: SubscriberSet::new(),
            restart_observers: SubscriberSet::new(),
            window_closed_observers: SubscriberSet::new(),

            // === 异步/效果 ===
            pending_effects: VecDeque::new(),
            pending_updates: 0,
            flushing_effects: false,

            // === 拖拽 ===
            active_drag: None,

            // === 渲染 ===
            mode: GpuiMode::Production,
            text_rendering_mode: Rc::new(Cell::new(TextRenderingMode::default())),
            svg_renderer: SvgRenderer::new(asset_source.clone()),

            // === 资源 ===
            asset_source,
            http_client,

            // === 其他 ===
            layout_id_buffer: Default::default(),
            propagate_event: true,
            prompt_builder: Some(PromptBuilder::Default),
            quit_mode: QuitMode::default(),
            quitting: false,
            cursor_hide_mode: CursorHideMode::default(),
            element_arena: RefCell::new(ElementArena::new()),

            // --- 内部追踪 ---
            window_update_stack: Vec::new(),
            pending_notifications: FxHashSet::default(),
            pending_global_notifications: Default::default(),
            tracked_entities: FxHashMap::default(),
            window_invalidators_by_entity: FxHashMap::default(),
            current_window_by_entity: FxHashMap::default(),
            // ...
        }),
    });
    app
}
```

### 关键初始化项一览

| 字段 | 类型 | 作用 |
|------|------|------|
| `platform` | `Rc<dyn Platform>` | 平台抽象，后续所有原生操作都通过它 |
| `background_executor` | `BackgroundExecutor` | 后台线程池，`cx.background_spawn()` 的任务在此执行 |
| `foreground_executor` | `ForegroundExecutor` | 主线程调度器，`cx.spawn()` 的任务在此执行 |
| `text_system` | `Arc<TextSystem>` | 文本塑形/渲染引擎（Linux 用 CosmicText + xkbcommon） |
| `entities` | `EntityMap` | 全局 Entity 注册表，每个 View/Model 在此有唯一 SlotMap 槽位 |
| `windows` | `SlotMap<...>` | 窗口槽，空初始，`open_window()` 时填充 |
| `globals_by_type` | `FxHashMap<TypeId, Box<dyn Any>>` | 全局状态容器，`cx.set_global()` 写入 |
| `observers` | `SubscriberSet` | `cx.observe()` 注册的观察者集合 |
| `actions` | `ActionRegistry` | 全局 Action 注册表 |

**到这一步，`Application` 构建完成，但还没有启动事件循环。**

---

## 5. `Application::run()` (`app.rs:200-210`)

```rust
pub fn run<F>(self, on_finish_launching: F)
where
    F: 'static + FnOnce(&mut App),
{
    let this = self.0.clone();
    let platform = self.0.borrow().platform.clone();

    // 把用户闭包装箱，传给平台层
    platform.run(Box::new(move || {
        let cx = &mut *this.borrow_mut();   // 获取 &mut App
        on_finish_launching(cx);            // 调用用户回调
    }));
}
```

关键点：
- `self.0` = `Rc<AppCell>` = `Rc<RefCell<App>>`
- `this.borrow_mut()` 获取 `&mut App`，作为 `cx` 传递
- `on_finish_launching` 被**装箱**成 `Box<dyn FnOnce()>`，失去类型信息（平台层不关心回调内容）

---

## 6. Linux 平台 `run()` (`gpui_linux/src/linux/platform.rs:197-208`)

```rust
fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
    // 第一步：立即调用用户的启动回调
    on_finish_launching();

    // 第二步：进入客户端事件循环（阻塞在这里直到退出）
    LinuxClient::run(&self.inner);

    // 第三步：事件循环退出后，调用 quit 回调
    let quit = self.inner
        .with_common(|common| common.callbacks.quit.take());
    if let Some(mut fun) = quit {
        fun();
    }
}
```

**这是关键！** `on_finish_launching()` 是**同步**调用：
- 您的代码中 `gpui_component::init(cx)` 在这里执行 — 注册全局状态
- `cx.open_window(...)` 在这里执行 — 创建原生窗口 + 首次渲染
- `cx.activate(true)` 在这里执行 — 激活应用

全部在主线程同步完成，然后才调用 `LinuxClient::run()`。

---

## 7. Linux 基础设施初始化：`LinuxCommon::new()` (`platform.rs:128-156`)

在 `WaylandClient::new()` / `X11Client::new()` 被调用时（`current_platform` 阶段），内部创建 `LinuxCommon`：

```rust
impl LinuxCommon {
    pub fn new(signal: LoopSignal) -> (Self, PriorityQueueCalloopReceiver<RunnableVariant>) {
        // 1. 创建主线程消息通道（calloop 事件源 + 优先级队列）
        let (main_sender, main_receiver) = PriorityQueueCalloopReceiver::new();

        // 2. 创建文本系统（CosmicText + 默认字体）
        let text_system = Arc::new(CosmicTextSystem::new("IBM Plex Sans"));

        // 3. 创建 LinuxDispatcher — GPUI 的任务调度核心
        let dispatcher = Arc::new(LinuxDispatcher::new(main_sender));

        // 4. 创建执行器
        let background_executor = BackgroundExecutor::new(dispatcher.clone());
        let foreground_executor = ForegroundExecutor::new(dispatcher);

        // 5. 组装
        let common = LinuxCommon {
            background_executor,
            foreground_executor,
            text_system,
            appearance: WindowAppearance::Light,
            auto_hide_scrollbars: false,
            button_layout: WindowButtonLayout::linux_default(),
            callbacks: PlatformHandlers::default(),
            signal,
            menus: Vec::new(),
        };

        (common, main_receiver)
    }
}
```

### `LinuxDispatcher::new()` (`linux/dispatcher.rs:31-80`)

```rust
impl LinuxDispatcher {
    pub fn new(main_sender: PriorityQueueCalloopSender<RunnableVariant>) -> Self {
        // 1. 创建后台任务通道
        let (background_sender, background_receiver) = PriorityQueueReceiver::new();

        // 2. 启动 N 个工作线程（N = CPU 核心数，最少 2 个）
        let thread_count = std::thread::available_parallelism()
            .map_or(2, |i| i.get().max(2));

        let background_threads = (0..thread_count)
            .map(|i| {
                let receiver = background_receiver.clone();
                std::thread::Builder::new()
                    .name(format!("Worker-{i}"))
                    .spawn(move || {
                        for runnable in receiver.iter() {
                            // 更新 profiler 上下文
                            profiler::update_running_task(
                                runnable.metadata().spawned,
                                runnable.metadata().location,
                            );
                            runnable.run();          // 执行后台任务
                            profiler::save_task_timing();
                        }
                    }).unwrap()
            }).collect::<Vec<_>>();

        // 3. 启动定时器线程（独立的 calloop 事件循环）
        let (timer_sender, timer_channel) = calloop::channel::channel::<TimerAfter>();
        let timer_thread = std::thread::Builder::new()
            .name("Timer".to_owned())
            .spawn(move || {
                let mut event_loop = EventLoop::try_new().expect("...");
                let handle = event_loop.handle();
                handle.insert_source(timer_channel, move |event, _, _| {
                    if let channel::Event::Msg(timer) = event {
                        // 在到期时执行 runnable
                        handle.insert_source(
                            calloop::timer::Timer::from_duration(timer.duration),
                            move |_, _, _| {
                                timer.runnable.run();
                                TimeoutAction::Drop
                            },
                        ).ok();
                    }
                });
                event_loop.run(None, &mut (), |_| {}).ok();
            }).unwrap();

        LinuxDispatcher {
            main_sender,       // 主线程任务投递
            timer_sender,      // 定时器任务投递
            background_sender, // 后台线程任务投递
            _background_threads: background_threads,
            main_thread_id: thread::current().id(),
        }
    }
}
```

### 三层任务调度总结

```
                    ┌─────────────────────────────────┐
                    │       GPUI 任务调度架构          │
                    ├─────────────────────────────────┤
                    │                                 │
    cx.spawn() ────►│  ForegroundExecutor             │
                    │  ├─ main_sender                 │
                    │  └─ calloop main event_loop     │
                    │      └─ 主线程执行              │
                    │                                 │
    cx.background_  │  BackgroundExecutor             │
    spawn() ───────►│  ├─ background_sender           │
                    │  └─ Worker-0, Worker-1, ...     │
                    │      └─ 后台线程池执行           │
                    │                                 │
    cx.background_  │  Timer 线程                     │
    executor.       │  ├─ timer_sender                │
    timer() ───────►│  └─ 独立 calloop 事件循环       │
                    │      └─ 到期后触发 runnable     │
                    │                                 │
                    └─────────────────────────────────┘
```

---

## 8. 平台事件循环

### Wayland (`wayland/client.rs:927-942`)

```rust
fn run(&self) {
    let mut event_loop = self.0.borrow_mut().event_loop
        .take().expect("App is already running");

    event_loop.run(
        None,
        &mut WaylandClientStatePtr(Rc::downgrade(&self.0)),
        |_| {},
    ).log_err();
}
```

`event_loop.run()` 是 calloop 事件循环的核心。它**阻塞在主线程**，等待以下 Wayland 协议事件：

- **wl_keyboard** → 键盘按下/释放 → `PlatformInput::KeyDown/KeyUp`
- **wl_pointer** → 鼠标移动/点击/滚动 → `PlatformInput::MouseMove/MouseDown/MouseUp/ScrollWheel`
- **wl_seat** → 输入设备能力变更 → 重新绑定键盘/指针
- **frame callback** → VSync → `on_request_frame` → `window.draw()`

### X11 (`x11/client.rs:1775-1788`)

```rust
fn run(&self) {
    let Some(mut event_loop) = self.0.borrow_mut().event_loop.take() else {
        return; // 已经运行
    };
    event_loop.run(None, &mut self.clone(), |_| {}).log_err();
}
```

同样的 calloop 模式，但内核是 X11 协议事件。

### macOS (`gpui_macos/src/platform.rs:474-501`)

```rust
fn run(&self, on_finish_launching: Box<dyn FnOnce()>) {
    let mut state = self.0.lock();
    if state.headless {
        drop(state);
        on_finish_launching();                      // Headless: 直接调用
        unsafe { CFRunLoopRun() };                   // 然后进入 CFRunLoop
    } else {
        state.finish_launching = Some(on_finish_launching);  // 存储推迟调用
        drop(state);
    }

    unsafe {
        let app: id = msg_send![APP_CLASS, sharedApplication];  // [NSApplication sharedApplication]
        let app_delegate: id = msg_send![APP_DELEGATE_CLASS, new]; // AppDelegate.new
        app.setDelegate_(app_delegate);

        let self_ptr = self as *const Self as *const c_void;
        (*app).set_ivar(MAC_PLATFORM_IVAR, self_ptr);      // 注入平台指针到 ObjC 对象
        (*app_delegate).set_ivar(MAC_PLATFORM_IVAR, self_ptr);

        let pool = NSAutoreleasePool::new(nil);
        app.run();      // ← [NSApp run] — Cocoa RunLoop，阻塞在这里！
        pool.drain();   // 退出后清理
    }
}
```

**macOS 与 Linux 的关键差异**：
- Linux：`on_finish_launching()` 在事件循环**之前**同步调用
- macOS：`on_finish_launching` 被存储，在 `applicationDidFinishLaunching:` 回调中调用
  — 窗口创建被推迟到 Cocoa RunLoop 启动之后
- macOS 通过 `set_ivar` 将平台指针注入 ObjC 对象，使得 ObjC 方法能回调 Rust 代码

---

## 9. 事件流转：从平台到 GPUI

### 9a. `PlatformWindow` trait — 原生窗口接口 (`gpui/src/platform.rs:620-665`)

```rust
pub trait PlatformWindow: HasWindowHandle + HasDisplayHandle {
    fn bounds(&self) -> Bounds<Pixels>;
    fn content_size(&self) -> Size<Pixels>;
    fn resize(&mut self, size: Size<Pixels>);
    fn scale_factor(&self) -> f32;
    fn mouse_position(&self) -> Point<Pixels>;
    fn modifiers(&self) -> Modifiers;

    // 输入处理器（IME 等）
    fn set_input_handler(&mut self, input_handler: PlatformInputHandler);
    fn take_input_handler(&mut self) -> Option<PlatformInputHandler>;

    // 事件回调注册
    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>);
    fn on_input(&self, callback: Box<dyn FnMut(PlatformInput) -> DispatchEventResult>);
    fn on_active_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_hover_status_change(&self, callback: Box<dyn FnMut(bool)>);
    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>);
    fn on_moved(&self, callback: Box<dyn FnMut()>);
    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>);
    fn on_close(&self, callback: Box<dyn FnOnce()>);

    // GPU 渲染
    fn draw(&self, scene: &Scene);
    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas>;
    // ...
}
```

### 9b. `platform_window.on_input()` — 事件回调注册 (`window.rs:1604-1612`)

在 `Window::new()` 初始化中，GPUI 窗口注册了最关键的回调：

```rust
platform_window.on_input({
    let mut cx = cx.to_async();
    Box::new(move |event| {
        handle
            .update(&mut cx, |_, window, cx|
                window.dispatch_event(event, cx)   // ← 核心：平台事件 → GPUI 事件系统
            )
            .log_err()
            .unwrap_or(DispatchEventResult::default())
    })
});
```

### 9c. Wayland 事件 → GPUI 的具体路径

以鼠标点击为例：

```
Wayland compositor (wl_pointer::button)
  ↓
calloop event_loop 接收 wl_pointer 事件
  ↓
WaylandClientStatePtr::event() (wayland/client.rs:1773)

  // 在 event 处理器中：
  let input = PlatformInput::MouseDown(MouseDownEvent {
      button: MouseButton::Left,
      position: Point { x: ..., y: ... },
      modifiers: Modifiers::default(),
      click_count: 1,
  });
  focused_window.handle_input(input);
  ↓
WaylandWindow::handle_input(input) (wayland/window.rs:1042)
  ├─ 检查窗口是否被阻塞
  ├─ 取出预先注册的 input callback
  └─ fun(input)  // 调用上面注册的回调
      ↓
Window::dispatch_event(event, cx) (window.rs:4498)
  ├─ 更新 mouse_position / modifiers
  ├─ 判断 InputModality (Keyboard vs Mouse)
  └─ dispatch_mouse_event()
      ├─ hit_test(mouse_position)
      ├─ Capture 阶段: mouse_listeners 正序遍历
      └─ Bubble 阶段:  mouse_listeners 逆序遍历
          └─ Interactivity 状态机检测 click
              └─ 触发 .on_click() 回调
```

### 9d. macOS 事件路径（Object-C 回调方式）

macOS 通过 Objective-C 方法桥接：

```
NSEvent (鼠标点击)
  ↓
[NSView mouseDown:] → 自定义的 NSView 子类
  ↓
AppDelegate / Platform 的 ObjC 方法
  ├─ 通过 set_ivar 注入的 Rust 指针回调
  └─ 调用 Rust 侧的方法
      └─ MacPlatform::handle_input(event)
          └─ MacWindow::handle_input(input)
              └─ 取出 on_input 回调
                  └─ Window::dispatch_event(event, cx)
```

---

## 10. 完整时序图

```
main()
 │
 ├─ gpui_platform::application()
 │   ├─ current_platform(false)
 │   │   └─ Linux: guess_compositor()
 │   │       ├─ WaylandClient::new()
 │   │       │   ├─ Connection::connect_to_env()     // 连接 Wayland 服务器
 │   │       │   ├─ LinuxCommon::new(signal)
 │   │       │   │   ├─ PriorityQueueCalloopReceiver // 主线程消息通道
 │   │       │   │   ├─ CosmicTextSystem("IBM Plex Sans")
 │   │       │   │   ├─ LinuxDispatcher::new()
 │   │       │   │   │   ├─ N 个 Worker 线程
 │   │       │   │   │   └─ Timer 线程
 │   │       │   │   ├─ BackgroundExecutor(dispatcher)
 │   │       │   │   └─ ForegroundExecutor(dispatcher)
 │   │       │   ├─ registry_queue_init()            // Wayland 全局对象
 │   │       │   └─ 创建 calloop EventLoop
 │   │       └─ 或 X11Client::new() / HeadlessClient::new()
 │   │
 │   └─ Application::with_platform(platform)
 │       └─ App::new_app(platform, ...)
 │           ├─ Rc::new_cyclic(|this| AppCell { ... })
 │           │   └─ App {
 │           │       platform,             // ← 平台实例
 │           │       background_executor,  // ← 线程池
 │           │       foreground_executor,  // ← 主线程调度
 │           │       text_system,           // ← 文本引擎
 │           │       entities: EntityMap,  // ← 空的 Entity 注册表
 │           │       windows: SlotMap,     // ← 空的窗口槽
 │           │       observers: SubscriberSet,
 │           │       ... 60+ 字段初始化
 │           │   }
 │           └─ 返回 Rc<AppCell>
 │
 ├─ Application::run(|cx: &mut App| { ... })
 │   └─ LinuxPlatform::run(on_finish_launching)
 │       │
 │       ├─ on_finish_launching()   ← 同步执行！您的闭包在此运行
 │       │   ├─ gpui_component::init(cx)
 │       │   │   └─ 注册全局主题/配置到 cx.globals_by_type
 │       │   │
 │       │   └─ cx.open_window(...) → App::open_window()
 │       │       ├─ cx.windows.insert(None)              // 分配窗口 ID
 │       │       ├─ Window::new(handle, options, cx)      // 创建 GPUI Window
 │       │       │   ├─ platform.open_window(...)        // 创建原生窗口
 │       │       │   │   └─ Wayland/X11 创建 surface + 注册回调
 │       │       │   ├─ platform_window.on_input(|event| {
 │       │       │   │       window.dispatch_event(event, cx)
 │       │       │   │   });                             // 注册事件桥接
 │       │       │   ├─ platform_window.on_request_frame  // 注册 VSync
 │       │       │   └─ platform_window.on_resize         // 注册尺寸变更
 │       │       ├─ build_root_view(&mut window, cx)     // 构建用户视图
 │       │       │   ├─ cx.new(|cx| MainView::new(window, cx))
 │       │       │   └─ cx.new(|cx| Root::new(view, window, cx))
 │       │       ├─ window.root.replace(root_view)       // 设置根视图
 │       │       ├─ window.defer(appearance_changed)      // 延迟外观更新
 │       │       └─ window.draw(cx)                       // 首次绘制！
 │       │           └─ draw_roots()
 │       │               ├─ layout_as_root → Taffy Flexbox
 │       │               ├─ prepaint_as_root → Hitboxes + 事件监听器
 │       │               └─ paint → GPU 绘制指令
 │       │
 │       └─ cx.activate(true) → platform.activate()
 │
 ├─ LinuxClient::run(&self.inner)   ← 阻塞在这里直到退出
 │   └─ WaylandClient::run()
 │       └─ event_loop.run()        ← calloop 事件循环
 │           └─ 循环等待事件:
 │               ├─ wl_keyboard  → handle_input → dispatch_event
 │               ├─ wl_pointer   → handle_input → dispatch_event
 │               ├─ frame callback → on_request_frame → draw
 │               └─ timer        → 到期的异步任务 runnable
 │
 └─ quit callback (事件循环退出后清理)
```

---

## 11. 关键源文件索引

| 文件 | 行数 | 说明 |
|------|------|------|
| `gpui_platform/src/gpui_platform.rs` | 186 | 跨平台入口，`application()` 和 `current_platform()` |
| `gpui_linux/src/linux.rs` | 57 | Linux 平台选择（Wayland/X11/Headless） |
| `gpui/src/platform.rs` | 2518 | `Platform` trait 定义 + 通用平台类型 |
| `gpui/src/app.rs:144-210` | — | `Application` 结构体 + `run()` |
| `gpui/src/app.rs:704-779` | — | `App::new_app()` 大初始化 |
| `gpui_linux/src/linux/platform.rs:116-208` | — | `LinuxCommon` + `LinuxPlatform::run()` |
| `gpui_linux/src/linux/dispatcher.rs` | 362 | `LinuxDispatcher` — 三层任务调度 |
| `gpui_linux/src/linux/wayland/client.rs:927-942` | — | Wayland 事件循环 `run()` |
| `gpui_linux/src/linux/x11/client.rs:1775-1788` | — | X11 事件循环 `run()` |
| `gpui_macos/src/platform.rs:474-501` | — | macOS 平台 `run()` (Cocoa RunLoop) |
