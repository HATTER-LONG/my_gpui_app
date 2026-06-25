# Plan 1: App Shell & 导航框架

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 搭建三栏式窗口框架（左侧 56px 导航栏 + 中间主内容区 + 右侧对话面板占位），实现 4 个导航项的视图切换和深色/浅色主题支持。

**Architecture:** `AppState` 作为根实体管理 `ActiveView` 枚举和 `ThemeMode`。`render()` 根据当前活跃视图渲染对应内容"占位"，后续 Plan 直接替换占位为真实视图。导航栏通过 `cx.listener` 处理点击切换。

**Tech Stack:** Rust + GPUI (声明式 UI), gpui_platform (Linux 平台后端)

**Global Constraints:**
- 窗口最小宽度 900px
- 左侧导航栏固定 56px
- 使用 GPUI `div()` / `Styled` trait 构建 UI
- 深色主题: bg `rgb(0x1e1e1e)`, secondary `rgb(0x2d2d2d)`, text `rgb(0xe0e0e0)`, accent `rgb(0x007acc)`
- 浅色主题: bg `rgb(0xffffff)`, secondary `rgb(0xf5f5f5)`, text `rgb(0x1a1a1a)`, accent `rgb(0x007acc)`

---

## File Structure

```
src/
  main.rs     - 入口，创建 AppState 实体和窗口 (modify)
  app.rs      - AppState, ActiveView, ThemeMode, 导航逻辑 (create)
  theme.rs    - ThemeColors, 深色/浅色配色常量 (create)
```

---

### Task 1: 定义主题颜色模块

**Files:**
- Create: `src/theme.rs`

**Interfaces:**
- Produces: `pub struct ThemeColors` — 包含 `bg`, `secondary_bg`, `text`, `accent` 字段，类型均为 `gpui::Hsla`
- Produces: `pub fn dark_colors() -> ThemeColors`
- Produces: `pub fn light_colors() -> ThemeColors`

- [ ] **Step 1: 创建 `src/theme.rs`**

```rust
use gpui::rgba;

pub struct ThemeColors {
    pub bg: gpui::Hsla,
    pub secondary_bg: gpui::Hsla,
    pub text: gpui::Hsla,
    pub text_secondary: gpui::Hsla,
    pub accent: gpui::Hsla,
}

pub fn dark_colors() -> ThemeColors {
    ThemeColors {
        bg: rgba(0x1e1e1eff),
        secondary_bg: rgba(0x2d2d2dff),
        text: rgba(0xe0e0e0ff),
        text_secondary: rgba(0x888888ff),
        accent: rgba(0x007accff),
    }
}

pub fn light_colors() -> ThemeColors {
    ThemeColors {
        bg: rgba(0xffffffff),
        secondary_bg: rgba(0xf5f5f5ff),
        text: rgba(0x1a1a1aff),
        text_secondary: rgba(0x888888ff),
        accent: rgba(0x007accff),
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: 新增模块编译通过。

- [ ] **Step 3: Commit**

```bash
git add src/theme.rs
git commit -m "feat: add theme colors module with dark/light palettes"
```

---

### Task 2: 创建 AppState 和导航枚举

**Files:**
- Create: `src/app.rs`

**Interfaces:**
- Produces: `pub enum ActiveView { Bookshelf, CurrentReading, Vocabulary, Settings }`
- Produces: `pub enum ThemeMode { Dark, Light }`
- Produces: `pub struct AppState { pub active_view: ActiveView, pub theme_mode: ThemeMode, pub colors: ThemeColors, pub has_active_book: bool }`
- Produces: `impl Render for AppState` — 渲染三栏布局

- [ ] **Step 1: 创建 `src/app.rs` 基本结构**

```rust
use gpui::{
    Context, div, impl_internable, px, prelude::*,
    IntoElement, ParentElement, Render, Styled,
    Window,
};

use crate::theme::{self, ThemeColors};

pub enum ActiveView {
    Bookshelf,
    CurrentReading,
    Vocabulary,
    Settings,
}

pub enum ThemeMode {
    Dark,
    Light,
}

pub struct AppState {
    pub active_view: ActiveView,
    pub theme_mode: ThemeMode,
    pub colors: ThemeColors,
    pub has_active_book: bool,
}

impl AppState {
    pub fn new() -> Self {
        let theme_mode = ThemeMode::Dark;
        let colors = match theme_mode {
            ThemeMode::Dark => theme::dark_colors(),
            ThemeMode::Light => theme::light_colors(),
        };
        Self {
            active_view: ActiveView::Bookshelf,
            theme_mode,
            colors,
            has_active_book: false,
        }
    }
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(self.colors.bg)
            .text_color(self.colors.text)
            .child(self.render_sidebar(cx))
            .child(self.render_main_content(cx))
            .child(self.render_chat_placeholder())
    }
}

impl AppState {
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .w(px(56.0))
            .h_full()
            .bg(self.colors.secondary_bg)
            .child(self.render_nav_item("📚", ActiveView::Bookshelf, cx))
            .child(self.render_current_reading_item(cx))
            .child(self.render_nav_item("📝", ActiveView::Vocabulary, cx))
            .child(self.render_nav_item("⚙", ActiveView::Settings, cx))
    }

    fn render_nav_item(
        &self,
        label: &str,
        view: ActiveView,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.active_view == view;
        div()
            .flex()
            .flex_row()
            .w(px(56.0))
            .h(px(48.0))
            .items_center()
            .justify_center()
            .when(is_selected, |d| d.bg(self.colors.accent))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.active_view = view;
                cx.notify();
            }))
            .child(label)
    }

    fn render_current_reading_item(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        if !self.has_active_book {
            return div();
        }
        let is_selected = matches!(self.active_view, ActiveView::CurrentReading);
        div()
            .flex()
            .flex_row()
            .w(px(56.0))
            .h(px(48.0))
            .items_center()
            .justify_center()
            .when(is_selected, |d| d.bg(self.colors.accent))
            .cursor_pointer()
            .on_click(cx.listener(move |this, _event, _window, cx| {
                this.active_view = ActiveView::CurrentReading;
                cx.notify();
            }))
            .child("📖")
    }

    fn render_main_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.active_view {
            ActiveView::Bookshelf => div().child("书库视图（后续 Plan 实现）"),
            ActiveView::CurrentReading => div().child("阅读视图（后续 Plan 实现）"),
            ActiveView::Vocabulary => div().child("词汇本视图（后续 Plan 实现）"),
            ActiveView::Settings => div().child("设置视图（后续 Plan 实现）"),
        };

        div()
            .flex_grow()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(content)
    }

    fn render_chat_placeholder(&self) -> impl IntoElement {
        div()
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: 模块编译通过。检查是否有 `when` 方法需要导入，如果没有则调整代码。

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add AppState with navigation and three-column layout"
```

---

### Task 3: 更新 main.rs 入口并注册模块

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `AppState` from `crate::app`
- Consumes: `ThemeColors` from `crate::theme`

- [ ] **Step 1: 修改 `src/main.rs`**

替换整个文件内容：

```rust
mod app;
mod theme;

use app::AppState;
use gpui::{App, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        let bounds = Bounds::centered(
            None,
            size(px(1000.0), px(700.0)),
            cx,
        );
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| {
                cx.new(|_| AppState::new())
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: 主入口编译通过，窗口可创建。

- [ ] **Step 3: 运行验证**

```bash
cargo run
```
Expected: 窗口显示，左侧 56px 导航栏可见（深色背景），点击导航项可切换中间区域文字。

- [ ] **Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat: wire up AppState as root view with navbar routing"
```

---

### Task 4: 添加导航项选中态视觉反馈

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 增强 `render_nav_item` 的选中态样式**

修改 `render_sidebar` 和 `render_nav_item` 方法，添加左侧 3px 竖条指示器：

```rust
fn render_nav_item(
    &self,
    label: &str,
    view: ActiveView,
    cx: &mut Context<Self>,
) -> impl IntoElement {
    let is_selected = self.active_view == view;
    div()
        .flex()
        .flex_row()
        .w(px(56.0))
        .h(px(48.0))
        .items_center()
        .justify_center()
        .relative()
        .cursor_pointer()
        .on_click(cx.listener(move |this, _event, _window, cx| {
            this.active_view = view;
            cx.notify();
        }))
        .child(
            // 选中指示器：左侧 3px 彩色竖条
            if is_selected {
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(3.0))
                    .h_full()
                    .bg(self.colors.accent)
            } else {
                div()
            },
        )
        .child(label)
}
```

- [ ] **Step 2: 编译并运行验证**

```bash
cargo check && cargo run
```
Expected: 选中导航项左侧出现 3px 蓝色竖条。

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add selected indicator bar to nav items"
```

---

### Task 5: 添加对话面板占位（宽度切换）

**Files:**
- Modify: `src/app.rs`

**Interfaces:**
- Produces: `pub chat_panel_visible: bool` on `AppState`

- [ ] **Step 1: 在 `AppState` 中添加聊天面板状态字段**

修改 `AppState` 结构体，添加 `chat_panel_visible: bool` 字段，初始值为 `false`：

```rust
pub struct AppState {
    pub active_view: ActiveView,
    pub theme_mode: ThemeMode,
    pub colors: ThemeColors,
    pub has_active_book: bool,
    pub chat_panel_visible: bool,   // <-- 新增
}

impl AppState {
    pub fn new() -> Self {
        // ... existing ...
        Self {
            active_view: ActiveView::Bookshelf,
            theme_mode,
            colors,
            has_active_book: false,
            chat_panel_visible: false,   // <-- 新增
        }
    }
}
```

- [ ] **Step 2: 修改 `render_chat_placeholder` 为可切换面板**

```rust
fn render_chat_panel(&self) -> impl IntoElement {
    if !self.chat_panel_visible {
        return div();
    }
    div()
        .w(px(320.0))
        .h_full()
        .bg(self.colors.secondary_bg)
        .flex()
        .flex_col()
        .child(
            // 标题栏
            div()
                .flex()
                .flex_row()
                .justify_between()
                .items_center()
                .px_2()
                .py_1()
                .h(px(36.0))
                .child("对话面板")
                .child(
                    div()
                        .cursor_pointer()
                        .child("✕")
                        .on_click(cx.listener(|this, _event, _window, cx| {
                            this.chat_panel_visible = false;
                            cx.notify();
                        })),
                ),
        )
        .child(
            // 消息区域（后续 Plan 填充）
            div()
                .flex_grow()
                .px_3()
                .py_2()
                .child("消息区域占位"),
        )
        .child(
            // 输入区域（后续 Plan 填充）
            div()
                .px_3()
                .py_2()
                .border_t_1()
                .border_color(self.colors.text_secondary)
                .child("输入区域占位"),
        )
}
```

修改 `render` 中调用处：

```rust
fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .size_full()
        .bg(self.colors.bg)
        .text_color(self.colors.text)
        .child(self.render_sidebar(cx))
        .child(self.render_main_content(cx))
        .child(self.render_chat_panel(cx))   // <-- 改这里
}
```

- [ ] **Step 3: 编译并运行验证**

```bash
cargo check && cargo run
```
Expected: 聊天面板默认隐藏，可（在后续 Plan 中通过按钮）切换显示。

- [ ] **Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add toggle-able chat panel placeholder"
```

---

### Task 6: 实现 `PartialEq` for `ActiveView` 以修复导航选中判断

**Files:**
- Modify: `src/app.rs`

- [ ] **Step 1: 为 `ActiveView` 派生 `PartialEq`**

```rust
#[derive(PartialEq)]
pub enum ActiveView {
    Bookshelf,
    CurrentReading,
    Vocabulary,
    Settings,
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```
Expected: `self.active_view == view` 比较正常工作。

- [ ] **Step 3: Commit**

```bash
git add src/app.rs
git commit -m "fix: derive PartialEq for ActiveView to fix nav selection check"
```

---

## Completeness Check

- ✅ 三栏式布局框架（56px 侧栏 + 弹性主区 + 0/320px 面板）
- ✅ 4 个导航项（第 2 项"当前阅读"动态显示）
- ✅ 选中态（左侧 3px 竖条 + 图标高亮）
- ✅ 视图切换通过 `cx.notify()` 触发重渲染
- ✅ 深色主题配色已定义，浅色主题常量已就绪
- ✅ 对话面板可切换显示/隐藏

## Post-Plan Verification

Run the app:
```bash
cargo run
```

Verify:
1. 窗口打开，约 1000x700 尺寸
2. 左侧 56px 深色导航栏可见，4 个导航项显示（📚 📖 📝 ⚙）
3. 点击不同导航项，中间内容区文字切换
4. 选中导航项左侧显示 3px 蓝色指示条
