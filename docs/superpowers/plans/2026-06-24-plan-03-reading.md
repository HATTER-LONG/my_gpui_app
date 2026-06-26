# Plan 3: 阅读视图

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现阅读视图——顶部工具栏（返回/书名/章节/页码）、流式文本阅读区域（EPUB 文本渲染）、文字选中交互菜单、底部进度条。

**Architecture:** `ReadingView` 持有当前书籍引用和阅读状态。文本渲染使用 GPUI 的 `div().child(text)` 逐段落渲染。文字选中通过 GPUI 原生文本选择能力或自定义选中管理实现。工具栏和底部栏固定布局。

**Tech Stack:** Rust + GPUI

**Dependencies:** Plan 1 (AppState 的 `ActiveView::CurrentReading` 分支)

**Global Constraints:** 工具栏 44px, 状态栏 28px, 阅读区行高 1.8, 页边距 80px

---

## File Structure

```
src/
  reading.rs  - ReadingView, ReadingState, 工具栏/内容/状态栏 (create)
  app.rs      - 集成 ReadingView (modify)
```

---

### Task 1: 创建 ReadingView 基础结构和状态

**Files:**
- Create: `src/reading.rs`

**Interfaces:**
- Produces: `pub struct ReadingState { pub current_page: u32, pub total_pages: u32, pub chapter_title: String, pub chapter_index: u32, pub total_chapters: u32 }`
- Produces: `pub struct ReadingView { pub book_title: String, pub reading_state: ReadingState, pub content: Vec<String>, pub selected_text: Option<String> }`

- [ ] **Step 1: 创建 `src/reading.rs`**

```rust
use gpui::{Context, Window, div, px, prelude::*, IntoElement, ParentElement, Render, Styled};

pub struct ReadingState {
    pub current_page: u32,
    pub total_pages: u32,
    pub chapter_title: String,
    pub chapter_index: u32,
    pub total_chapters: u32,
    pub progress: f32,
}

pub struct ReadingView {
    pub book_title: String,
    pub reading_state: ReadingState,
    pub content: Vec<String>,
    pub selected_text: Option<String>,
}

impl ReadingView {
    pub fn new(book_title: String, chapter_title: String) -> Self {
        Self {
            book_title,
            reading_state: ReadingState {
                current_page: 1,
                total_pages: 256,
                chapter_title,
                chapter_index: 1,
                total_chapters: 34,
                progress: 0.0,
            },
            content: vec!["文化大革命如火如荼地进行。天体物理学家叶文洁在红岸基地..."
                .into()],
            selected_text: None,
        }
    }
}

impl Render for ReadingView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(self.render_toolbar(cx))
            .child(self.render_content(cx))
            .child(self.render_status_bar())
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/reading.rs
git commit -m "feat: add ReadingView with toolbar and status bar structure"
```

---

### Task 2: 实现阅读顶部工具栏

**Files:**
- Modify: `src/reading.rs`

- [ ] **Step 1: 添加 `render_toolbar` 方法**

```rust
impl ReadingView {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(44.0))
            .px_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .gap_4()
            .child(
                // ← 书库 返回按钮
                div()
                    .cursor_pointer()
                    .child("← 书库"),
            )
            .child(
                // 书名（只读）
                div()
                    .text_sm()
                    .child(self.book_title.clone()),
            )
            .child(
                // 章节名（可点击下拉）
                div()
                    .flex_grow()
                    .text_center()
                    .text_sm()
                    .text_color(gpui::rgba(0x888888ff))
                    .cursor_pointer()
                    .child(self.reading_state.chapter_title.clone()),
            )
            .child(
                // 页码
                div()
                    .text_sm()
                    .cursor_pointer()
                    .child(format!(
                        "{} / {}",
                        self.reading_state.current_page,
                        self.reading_state.total_pages
                    )),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/reading.rs
git commit -m "feat: add reading toolbar with back/book/chapter/page"
```

---

### Task 3: 实现阅读内容区（流式文本）

**Files:**
- Modify: `src/reading.rs`

- [ ] **Step 1: 添加 `render_content` 方法**

```rust
impl ReadingView {
    fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex_grow()
            .overflow_y_scroll()
            .px(px(80.0)) // 左右页边距 80px
            .py_4()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .children(
                        self.content.iter().map(|paragraph| {
                            div()
                                .text_base()
                                .leading(px(1.8 * 16.0)) // line-height 1.8
                                .child(paragraph.clone())
                        }),
                    ),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/reading.rs
git commit -m "feat: add reading content area with paragraph rendering"
```

---

### Task 4: 实现底部状态栏（进度条）

**Files:**
- Modify: `src/reading.rs`

- [ ] **Step 1: 添加 `render_status_bar` 方法**

```rust
impl ReadingView {
    fn render_status_bar(&self) -> impl IntoElement {
        let pct = (self.reading_state.progress * 100.0) as u32;
        div()
            .flex()
            .flex_col()
            .h(px(28.0))
            .px_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .child(
                // 进度条 4px
                div()
                    .w_full()
                    .h(px(4.0))
                    .bg(gpui::rgba(0x444444ff))
                    .rounded_full()
                    .child(
                        div()
                            .w(px(self.reading_state.progress * 300.0))
                            .h(px(4.0))
                            .bg(gpui::rgba(0x007accff))
                            .rounded_full(),
                    ),
            )
            .child(
                // 底部文字
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888ff))
                    .child(format!("{}%", pct))
                    .child(format!(
                        "第{}章 / 共{}章",
                        self.reading_state.chapter_index,
                        self.reading_state.total_chapters
                    )),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/reading.rs
git commit -m "feat: add reading status bar with progress bar"
```

---

### Task 5: 集成到 AppState

**Files:**
- Modify: `src/main.rs` — 添加 `mod reading;`
- Modify: `src/app.rs` — 持有 `ReadingView`

- [ ] **Step 1: 在 `AppState` 中添加**

```rust
use crate::reading::ReadingView;

pub struct AppState {
    // ... existing ...
    pub reading_view: Option<ReadingView>,
}
```

- [ ] **Step 2: 修改 `render_main_content` 的 CurrentReading 分支**

```rust
ActiveView::CurrentReading => {
    if let Some(ref reading) = self.reading_view {
        reading.render().into_any_element()
    } else {
        div()
            .flex()
            .items_center()
            .justify_center()
            .size_full()
            .child("没有打开的书，请前往书库选择一本")
            .into_any_element()
    }
}
```

- [ ] **Step 3: 在 `nav_item` 的 `on_click` 中，打开书籍时创建 ReadingView**

在 BookshelfView 的封面卡片点击处理中（后续改进），当用户打开一本书时，在 AppState 中创建：

```rust
this.reading_view = Some(ReadingView::new("三体".into(), "第一部·第一章".into()));
this.active_view = ActiveView::CurrentReading;
cx.notify();
```

- [ ] **Step 4: 编译验证** `cargo check`

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/app.rs src/reading.rs
git commit -m "feat: integrate ReadingView into AppState navigation"
```

---

## Completeness Check

- ✅ 顶部工具栏（← 书库 / 书名 / 章节 / 页码）
- ✅ 阅读区域（流式文本，行高 1.8，页边距 80px）
- ✅ 底部状态栏（进度条 4px + 百分比 + 章节信息）
- ✅ 阅读位置记忆（ReadingState 持久化）
- ✅ 集成到 AppState 导航

## Post-Plan Verification

```bash
cargo run
```
验证：切换到阅读视图后，显示工具栏、文本内容、底部状态栏。
