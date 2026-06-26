# Plan 2: 书库视图

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现书库视图——空状态引导导入 + 封面网格展示已导入书籍，支持搜索过滤和拖拽/点击导入。

**Architecture:** `BookshelfView` 持有 `Vec<Book>` 列表。`render()` 根据列表是否为空分别渲染空状态引导或封面网格。导入通过文件选择对话框触发。搜索通过 `search_query` 字段实时过滤。

**Tech Stack:** Rust + GPUI, `std::fs` / `walkdir` 扫描文件

**Dependencies:** Plan 1 (AppState 的 `ActiveView::Bookshelf` 分支)

**Global Constraints:** 封面卡片 160×220px, 圆角 8px, 网格间距 24px, 搜索框 180px

---

## File Structure

```
src/
  bookshelf.rs  - BookshelfView, Book model, 空状态/封面网格渲染 (create)
  app.rs        - 集成 BookshelfView 到 ActiveView::Bookshelf 分支 (modify)
```

---

### Task 1: 创建 Book 数据模型

**Files:**
- Create: `src/bookshelf.rs`

**Interfaces:**
- Produces: `pub struct Book { pub title: String, pub author: String, pub file_path: String, pub progress: f32, pub cover_path: Option<String> }`
- Produces: `pub struct BookshelfView { pub books: Vec<Book>, pub search_query: String }`

- [ ] **Step 1: 创建 `src/bookshelf.rs`**

```rust
use gpui::{
    Context, Window, div, img, px, prelude::*,
    IntoElement, ParentElement, Render, Styled,
};

pub struct Book {
    pub title: String,
    pub author: String,
    pub file_path: String,
    pub progress: f32,          // 0.0 ~ 1.0
    pub finished: bool,
    pub cover_path: Option<String>,
}

pub struct BookshelfView {
    pub books: Vec<Book>,
    pub search_query: String,
    pub drag_over: bool,
}

impl BookshelfView {
    pub fn new() -> Self {
        Self {
            books: Vec::new(),
            search_query: String::new(),
            drag_over: false,
        }
    }

    fn filtered_books(&self) -> Vec<&Book> {
        if self.search_query.is_empty() {
            return self.books.iter().collect();
        }
        let q = self.search_query.to_lowercase();
        self.books
            .iter()
            .filter(|b| {
                b.title.to_lowercase().contains(&q)
                    || b.author.to_lowercase().contains(&q)
            })
            .collect()
    }
}

impl Render for BookshelfView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(self.render_toolbar(cx))
            .child(
                if self.books.is_empty() {
                    self.render_empty_state(cx)
                } else {
                    self.render_book_grid(cx)
                },
            )
    }
}
```

- [ ] **Step 2: 编译验证**

```bash
cargo check
```

- [ ] **Step 3: Commit**

```bash
git add src/bookshelf.rs
git commit -m "feat: add BookshelfView with Book model and empty/grid states"
```

---

### Task 2: 实现空状态渲染

**Files:**
- Modify: `src/bookshelf.rs`

- [ ] **Step 1: 添加 `render_empty_state` 方法**

```rust
impl BookshelfView {
    fn render_empty_state(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_4()
            .child(
                div().text_6xl().child("📖"),
            )
            .child(
                div().text_xl().child("导入你的第一本书"),
            )
            .child(
                div().text_sm().text_color(gpui::rgba(0x888888ff)).child(
                    "拖拽 EPUB/PDF 文件到此处，或点击选择文件",
                ),
            )
            .child(
                div()
                    .px_4()
                    .py_2()
                    .bg(gpui::rgba(0x007accff))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(gpui::rgba(0xffffffff))
                    .child("选择文件"),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/bookshelf.rs
git commit -m "feat: add empty state with import CTA to BookshelfView"
```

---

### Task 3: 实现封面网格渲染

**Files:**
- Modify: `src/bookshelf.rs`

- [ ] **Step 1: 添加封面卡片和网格渲染方法**

```rust
impl BookshelfView {
    fn render_book_grid(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let books = self.filtered_books();
        div()
            .flex()
            .flex_wrap()
            .gap_6()   // 24px spacing
            .child(books.iter().map(|book| self.render_book_card(book)))
    }

    fn render_book_card(&self, book: &Book) -> impl IntoElement {
        let progress_pct = (book.progress * 100.0) as u32;
        let progress_color = if book.finished {
            gpui::rgba(0x4caf50ff)
        } else {
            gpui::rgba(0x007accff)
        };

        div()
            .w(px(160.0))
            .rounded_md()
            .flex()
            .flex_col()
            .child(
                // 封面图片占位（用纯色背景 + 书名首字）
                div()
                    .w(px(160.0))
                    .h(px(220.0))
                    .bg(gpui::rgba(0x444444ff))
                    .rounded_md()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_2xl()
                            .text_color(gpui::rgba(0xffffffff))
                            .child(book.title.chars().next().unwrap_or('?').to_string()),
                    ),
            )
            .child(
                // 书名（单行截断）
                div()
                    .text_sm()
                    .pt_1()
                    .child(&book.title),
            )
            .child(
                // 作者（次级色）
                div()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888ff))
                    .child(&book.author),
            )
            .child(
                // 进度条
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(book.progress * 156.0))
                            .h(px(4.0))
                            .bg(progress_color)
                            .rounded_full(),
                    )
                    .child(
                        div().text_xs().child(format!("{}%", progress_pct)),
                    ),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/bookshelf.rs
git commit -m "feat: add cover grid rendering with progress bars to BookshelfView"
```

---

### Task 4: 实现搜索/过滤栏

**Files:**
- Modify: `src/bookshelf.rs`

- [ ] **Step 1: 添加 `render_toolbar` 方法**

```rust
impl BookshelfView {
    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .mb_4()
            .child(
                div()
                    .w(px(180.0))
                    .px_3()
                    .py_1()
                    .bg(gpui::rgba(0x3a3a3aff))
                    .rounded_md()
                    .child(format!("搜索书籍... ({})", self.filtered_books().len())),
            )
            .child(
                div()
                    .px_4()
                    .py_1()
                    .bg(gpui::rgba(0x007accff))
                    .rounded_md()
                    .cursor_pointer()
                    .text_color(gpui::rgba(0xffffffff))
                    .child("导入 +"),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/bookshelf.rs
git commit -m "feat: add search toolbar with book count to BookshelfView"
```

---

### Task 5: 将 BookshelfView 集成到 AppState

**Files:**
- Modify: `src/main.rs` — 添加 `mod bookshelf;`
- Modify: `src/app.rs` — 在 `AppState` 中持有 `BookshelfView`，渲染时委托

- [ ] **Step 1: 在 `AppState` 中添加字段**

```rust
// src/app.rs 中
use crate::bookshelf::BookshelfView;

pub struct AppState {
    // ... existing fields ...
    pub bookshelf: BookshelfView,   // <-- 新增
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing ...
            bookshelf: BookshelfView::new(),   // <-- 新增
        }
    }
}
```

- [ ] **Step 2: 修改 `render_main_content` 中的 Bookshelf 分支**

在 `render_main_content` 方法中，将 `ActiveView::Bookshelf` 分支改为委托给 `BookshelfView::render`：

```rust
fn render_main_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .flex_grow()
        .h_full()
        .child(
            match self.active_view {
                ActiveView::Bookshelf => self.bookshelf.render()
                    .into_any_element(),
                ActiveView::CurrentReading => div()
                    .child("阅读视图（后续 Plan 实现）")
                    .into_any_element(),
                ActiveView::Vocabulary => div()
                    .child("词汇本视图（后续 Plan 实现）")
                    .into_any_element(),
                ActiveView::Settings => div()
                    .child("设置视图（后续 Plan 实现）")
                    .into_any_element(),
            },
        )
}
```

注意：GPUI 中在 `match` 分支返回不同具体类型时需要 `.into_any_element()` 统一类型。

- [ ] **Step 3: 添加测试书籍数据验证网格渲染**

在 `AppState::new()` 中，往 `bookshelf.books` 添加 1-2 条测试数据：

```rust
bookshelf.books.push(Book {
    title: "三体".into(),
    author: "刘慈欣".into(),
    file_path: "".into(),
    progress: 0.76,
    finished: false,
    cover_path: None,
});
```

- [ ] **Step 4: 编译并运行验证**

```bash
cargo run
```
Expected: 书库视图显示封面网格（含测试书籍），搜索栏可见。

- [ ] **Step 5: Commit**

```bash
git add src/main.rs src/app.rs src/bookshelf.rs
git commit -m "feat: integrate BookshelfView into AppState with test data"
```

---

## Completeness Check

- ✅ 空状态（首次使用引导）
- ✅ 封面网格（160×220px 卡片，书名/作者/进度条）
- ✅ 搜索过滤栏
- ✅ 导入按钮占位
- ✅ 集成到 AppState 导航

## Post-Plan Verification

```bash
cargo run
```
验证：书库视图显示网格，可在空状态和有书状态之间切换（通过注释/取消注释测试数据）。
