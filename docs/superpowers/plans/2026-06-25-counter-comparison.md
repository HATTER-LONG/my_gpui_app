# 计数器对比测试组件 — 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在同一个窗口中并排展示 GPUI 原生 div 实现和 gpui-component 实现的计数器，对比两种开发方式。

**Architecture:** 三个文件：`native_counter.rs`（纯 div）、`component_counter.rs`（gpui-component Button/Label）、`main.rs`（MainView 左右分栏组合两者 + init + Root 包装）。

**Tech Stack:** gpui, gpui_platform, gpui-component

## Global Constraints

- `gpui_component::init(cx)` 必须在 `app.run()` 中最先调用
- 每个窗口的第一级必须使用 `Root::new(view, window, cx)` 包装
- 两边计数器独立状态，互不干扰
- FPS 计算逻辑复用现有实现

---

### Task 1: 创建 native_counter.rs（纯 GPUI div 版计数器）

**Files:**
- Create: `src/native_counter.rs`

**Interfaces:**
- Produces: `pub struct NativeCounterView` with fields `count: i32`, `fps: f64`, `last_frame: Instant`, `frame_count: u64`, `accumulated_time: f64`
- Produces: `impl NativeCounterView { pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self }`
- Produces: `impl Render for NativeCounterView`

- [ ] **Step 1: 创建文件并编写完整代码**

```rust
use gpui::{prelude::*, rgb, px, size, Context, Window, div};
use std::time::Instant;

pub struct NativeCounterView {
    count: i32,
    fps: f64,
    last_frame: Instant,
    frame_count: u64,
    accumulated_time: f64,
}

impl NativeCounterView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            count: 0,
            fps: 0.0,
            last_frame: Instant::now(),
            frame_count: 0,
            accumulated_time: 0.0,
        }
    }
}

impl Render for NativeCounterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        self.frame_count += 1;

        self.accumulated_time += delta;
        if self.accumulated_time >= 0.5 {
            self.fps = self.frame_count as f64 / self.accumulated_time;
            self.frame_count = 0;
            self.accumulated_time = 0.0;
        }

        window.request_animation_frame();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .border_1()
            .border_color(rgb(0x555555))
            .rounded_md()
            .bg(rgb(0x1e1e1e))
            .child(
                div()
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child("Native GPUI"),
            )
            .child(
                div()
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(rgb(0xffffff))
                    .child(format!("Count: {}", self.count)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(0x888888))
                    .child(format!("FPS: {:.0}", self.fps)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .id("native-incr")
                            .px_4()
                            .py_2()
                            .bg(rgb(0x007acc))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child("Increment")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .id("native-decr")
                            .px_4()
                            .py_2()
                            .bg(rgb(0xcc3333))
                            .rounded_md()
                            .cursor_pointer()
                            .text_color(rgb(0xffffff))
                            .child("Decrement")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: 编译通过（main.rs 尚未引用新文件，可能有无用警告可忽略）

- [ ] **Step 3: 提交**

```bash
git add src/native_counter.rs
git commit -m "feat: add NativeCounterView (pure GPUI div-based counter)"
```

---

### Task 2: 创建 component_counter.rs（gpui-component 版计数器）

**Files:**
- Create: `src/component_counter.rs`

**Interfaces:**
- Produces: `pub struct ComponentCounterView` with fields `count: i32`, `fps: f64`, `last_frame: Instant`, `frame_count: u64`, `accumulated_time: f64`
- Produces: `impl ComponentCounterView { pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self }`
- Produces: `impl Render for ComponentCounterView`

- [ ] **Step 1: 创建文件并编写完整代码**

```rust
use gpui::{prelude::*, px, size, Context, Window, div};
use gpui_component::button::Button;
use gpui_component::label::Label;
use gpui_component::ActiveTheme as _;
use std::time::Instant;

pub struct ComponentCounterView {
    count: i32,
    fps: f64,
    last_frame: Instant,
    frame_count: u64,
    accumulated_time: f64,
}

impl ComponentCounterView {
    pub fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            count: 0,
            fps: 0.0,
            last_frame: Instant::now(),
            frame_count: 0,
            accumulated_time: 0.0,
        }
    }
}

impl Render for ComponentCounterView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame).as_secs_f64();
        self.last_frame = now;
        self.frame_count += 1;

        self.accumulated_time += delta;
        if self.accumulated_time >= 0.5 {
            self.fps = self.frame_count as f64 / self.accumulated_time;
            self.frame_count = 0;
            self.accumulated_time = 0.0;
        }

        window.request_animation_frame();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .gap_2()
            .p_4()
            .border_1()
            .border_color(cx.theme().border)
            .rounded_md()
            .bg(cx.theme().background)
            .child(
                Label::new("comp-title")
                    .text_xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("Component"),
            )
            .child(
                Label::new("comp-count")
                    .text_3xl()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child(format!("Count: {}", self.count)),
            )
            .child(
                Label::new("comp-fps")
                    .text_sm()
                    .text_color(cx.theme().muted)
                    .child(format!("FPS: {:.0}", self.fps)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        Button::new("comp-incr")
                            .primary()
                            .label("Increment")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count += 1;
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new("comp-decr")
                            .danger()
                            .label("Decrement")
                            .on_click(cx.listener(|this, _event, _, cx| {
                                this.count -= 1;
                                cx.notify();
                            })),
                    ),
            )
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: 编译通过

- [ ] **Step 3: 提交**

```bash
git add src/component_counter.rs
git commit -m "feat: add ComponentCounterView (gpui-component based counter)"
```

---

### Task 3: 重构 main.rs 组合两个计数器并设置 gpui-component

**Files:**
- Modify: `src/main.rs`（完整重写）

**Interfaces:**
- Consumes: `NativeCounterView` from Task 1, `ComponentCounterView` from Task 2
- Consumes: `gpui_component::init`, `Root`, `ActiveTheme`
- Produces: `MainView` struct, `fn main()`

- [ ] **Step 1: 重写 main.rs 完整代码**

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
            native: cx.new(|_cx| NativeCounterView::new(window, cx)),
            component: cx.new(|_cx| ComponentCounterView::new(window, cx)),
        }
    }
}

impl gpui::Render for MainView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .gap_4()
            .p_4()
            .bg(cx.theme().background)
            .child(self.native.clone())
            .child(self.component.clone())
            .children(Root::render_dialog_layer(cx))
            .children(Root::render_sheet_layer(cx))
            .children(Root::render_notification_layer(cx))
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
                let view = cx.new(|_| MainView::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx));
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check 2>&1
```
Expected: 编译通过，无错误

- [ ] **Step 3: 运行验证**

```bash
cargo run
```
Expected: 窗口打开，左右两个计数器并排显示，按钮可点击，计数和 FPS 正常更新。

- [ ] **Step 4: 提交**

```bash
git add src/main.rs
git commit -m "feat: combine native and component counters side-by-side in MainView"
```

---

## 验证清单

| 检查项 | 预期结果 |
|--------|---------|
| `cargo check` | 无编译错误 |
| `cargo run` | 窗口打开，640x360 |
| 左侧 "Native GPUI" 面板 | 显示 Count、FPS、两个按钮 |
| 右侧 "Component" 面板 | 显示 Count、FPS、两个 Button |
| 点击任意 Increment | 对应面板 Count +1 |
| 点击任意 Decrement | 对应面板 Count -1 |
| FPS 显示 | 两边各自显示帧率 |
| 左右两侧独立 | 点击左边不影响右边，反之亦然 |
