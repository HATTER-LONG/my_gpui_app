# Plan 6: 词汇本视图

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现词汇本视图——按书籍分组展示已查询词汇，支持按时间排序切换，每个词条展示词性/释义/原文/复习次数，支持展开详情和删除。

**Architecture:** `VocabularyView` 持有 `Vec<WordEntry>`，每个词条关联到一本书。渲染时默认按书籍分组，通过 `sort_mode` 切换排序方式。词条卡片可展开显示完整 AI 解释。

**Tech Stack:** Rust + GPUI

**Dependencies:** Plan 1 (AppState 的 `ActiveView::Vocabulary` 分支)

**Global Constraints:** 每个词条展示：词汇名(粗体) + 词性+释义 + 原文引用(斜体) + 查询日期 + 复习次数

---

## File Structure

```
src/
  vocabulary.rs  - VocabularyView, WordEntry, SortMode, 词条渲染 (create)
  app.rs         - 集成 VocabularyView (modify)
```

---

### Task 1: 创建 WordEntry 模型和 VocabularyView

**Files:**
- Create: `src/vocabulary.rs`

**Interfaces:**
- Produces: `pub struct WordEntry { pub word: String, pub part_of_speech: String, pub definition: String, pub original_text: String, pub book_title: String, pub date: String, pub review_count: u32, pub expanded: bool }`
- Produces: `pub enum SortMode { ByBook, ByTime }`
- Produces: `pub struct VocabularyView { pub entries: Vec<WordEntry>, pub sort_mode: SortMode }`

- [ ] **Step 1: 创建 `src/vocabulary.rs`**

```rust
use gpui::{Context, Window, div, px, prelude::*, IntoElement, ParentElement, Render, Styled};

pub struct WordEntry {
    pub word: String,
    pub part_of_speech: String,
    pub definition: String,
    pub full_explanation: String,
    pub original_text: String,
    pub book_title: String,
    pub date: String,
    pub review_count: u32,
    pub expanded: bool,
}

#[derive(PartialEq)]
pub enum SortMode {
    ByBook,
    ByTime,
}

pub struct VocabularyView {
    pub entries: Vec<WordEntry>,
    pub sort_mode: SortMode,
}

impl VocabularyView {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            sort_mode: SortMode::ByBook,
        }
    }

    /// 按书籍分组，返回 (book_title, Vec<&WordEntry>)
    fn grouped_entries(&self) -> Vec<(&str, Vec<&WordEntry>)> {
        let mut groups: std::collections::BTreeMap<&str, Vec<&WordEntry>> =
            std::collections::BTreeMap::new();
        for entry in &self.entries {
            groups.entry(&entry.book_title).or_default().push(entry);
        }
        groups.into_iter().collect()
    }

    /// 按时间倒序
    fn time_sorted_entries(&self) -> Vec<&WordEntry> {
        let mut sorted: Vec<&WordEntry> = self.entries.iter().collect();
        sorted.sort_by(|a, b| b.date.cmp(&a.date));
        sorted
    }
}

impl Render for VocabularyView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .child(self.render_header(cx))
            .child(self.render_entries(cx))
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/vocabulary.rs
git commit -m "feat: add VocabularyView with WordEntry model and sort modes"
```

---

### Task 2: 实现顶部排序切换头部

**Files:**
- Modify: `src/vocabulary.rs`

- [ ] **Step 1: 添加 `render_header` 方法**

```rust
impl VocabularyView {
    fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .mb_4()
            .child(
                div().text_xl().child("词汇本"),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(self.render_sort_button("按书籍", SortMode::ByBook, cx))
                    .child(self.render_sort_button("按时间", SortMode::ByTime, cx)),
            )
    }

    fn render_sort_button(
        &self,
        label: &str,
        mode: SortMode,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_active = self.sort_mode == mode;
        div()
            .px_3()
            .py_1()
            .rounded_md()
            .when(is_active, |d| d.bg(gpui::rgba(0x007accff)))
            .text_color(if is_active {
                gpui::rgba(0xffffffff)
            } else {
                gpui::rgba(0x888888ff)
            })
            .cursor_pointer()
            .child(label)
            .on_click(cx.listener(|this, _event, _window, cx| {
                this.sort_mode = mode;
                cx.notify();
            }))
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/vocabulary.rs
git commit -m "feat: add sort toggle header to VocabularyView"
```

---

### Task 3: 实现词条渲染（分组/列表视图）

**Files:**
- Modify: `src/vocabulary.rs`

- [ ] **Step 1: 添加 `render_entries` 方法**

```rust
impl VocabularyView {
    fn render_entries(&self, cx: &mut Context<Self>) -> impl IntoElement {
        match self.sort_mode {
            SortMode::ByBook => self.render_grouped_entries(cx),
            SortMode::ByTime => self.render_time_sorted_entries(cx),
        }
    }

    fn render_grouped_entries(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let groups = self.grouped_entries();
        if groups.is_empty() {
            return self.render_empty_state();
        }
        div()
            .flex()
            .flex_col()
            .gap_4()
            .overflow_y_scroll()
            .children(groups.iter().map(|(book, entries)| {
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::BOLD)
                            .pb_1()
                            .child(format!("┌─ {} ───", book)),
                    )
                    .children(entries.iter().map(|entry| self.render_word_entry(entry)))
            })),
    }

    fn render_time_sorted_entries(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let sorted = self.time_sorted_entries();
        if sorted.is_empty() {
            return self.render_empty_state();
        }
        div()
            .flex()
            .flex_col()
            .gap_2()
            .overflow_y_scroll()
            .children(sorted.iter().map(|entry| self.render_word_entry(entry)))
    }

    fn render_word_entry(&self, entry: &WordEntry) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .px_3()
            .py_2()
            .bg(gpui::rgba(0x2d2d2dff))
            .rounded_md()
            .gap_1()
            .child(
                // 词汇名（粗体） + 词性 + 删除按钮
                div()
                    .flex()
                    .flex_row()
                    .justify_between()
                    .items_center()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_baseline()
                            .child(
                                div()
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .child(entry.word.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(gpui::rgba(0x888888ff))
                                    .child(format!("{}. {}", entry.part_of_speech, entry.definition)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(gpui::rgba(0x888888ff))
                            .cursor_pointer()
                            .child("×"),
                    ),
            )
            .child(
                // 原文引用（斜体）
                div()
                    .text_xs()
                    .text_color(gpui::rgba(0x888888ff))
                    .child(format!("原文：\"{}\"", entry.original_text)),
            )
            .child(
                // 日期 + 复习次数
                div()
                    .flex()
                    .flex_row()
                    .gap_4()
                    .text_xs()
                    .text_color(gpui::rgba(0x666666ff))
                    .child(entry.date.clone())
                    .child(format!("复习 {} 次", entry.review_count)),
            )
            .child(
                // 展开详情（如有完整解释）
                if entry.expanded {
                    div()
                        .mt_1()
                        .px_2()
                        .py_1()
                        .bg(gpui::rgba(0x3a3a3aff))
                        .rounded_sm()
                        .text_sm()
                        .child(entry.full_explanation.clone())
                } else {
                    div()
                },
            )
    }

    fn render_empty_state(&self) -> impl IntoElement {
        div()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .text_color(gpui::rgba(0x888888ff))
            .child("还没有词汇。阅读时选中文字点\"查询词义\"即可收录。")
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/vocabulary.rs
git commit -m "feat: add word entry rendering with grouped and time-sorted views"
```

---

### Task 4: 添加点击展开/折叠详情功能

**Files:**
- Modify: `src/vocabulary.rs`

- [ ] **Step 1: 在 `render_word_entry` 中添加点击切换 `expanded`**

修改词条渲染，将整卡片包裹为可点击：

```rust
fn render_word_entry(&self, entry: &WordEntry, cx: &mut Context<Self>, entry_idx: usize) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .px_3()
        .py_2()
        .bg(gpui::rgba(0x2d2d2dff))
        .rounded_md()
        .cursor_pointer()
        .gap_1()
        .on_click(cx.listener(move |this, _event, _window, cx| {
            if let Some(e) = this.entries.get_mut(entry_idx) {
                e.expanded = !e.expanded;
                cx.notify();
            }
        }))
        // ... rest of rendering
}
```

注意：需要修改调用处传入 `cx` 和索引。

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/vocabulary.rs
git commit -m "feat: add expand/collapse toggle for word entry details"
```

---

### Task 5: 集成到 AppState 并添加测试数据

**Files:**
- Modify: `src/main.rs` — 添加 `mod vocabulary;`
- Modify: `src/app.rs` — 持有 `VocabularyView`

- [ ] **Step 1: 在 AppState 中添加字段并初始化测试数据**

```rust
use crate::vocabulary::{VocabularyView, WordEntry};

pub struct AppState {
    // ... existing ...
    pub vocabulary: VocabularyView,
}

impl AppState {
    pub fn new() -> Self {
        let mut vocabulary = VocabularyView::new();
        vocabulary.entries.push(WordEntry {
            word: "红岸基地".into(),
            part_of_speech: "n.".into(),
            definition: "位于雷达峰的天体物理观测站".into(),
            full_explanation: "红岸基地是三体第一部中的重要场景...".into(),
            original_text: "她被安排到红岸基地工作".into(),
            book_title: "三体".into(),
            date: "2026-06-20".into(),
            review_count: 3,
            expanded: false,
        });
        vocabulary.entries.push(WordEntry {
            word: "恒纪元".into(),
            part_of_speech: "n.".into(),
            definition: "三体世界中文明稳定发展的时期".into(),
            full_explanation: "恒纪元是三体世界的特殊概念...".into(),
            original_text: "恒纪元的到来是不可预测的".into(),
            book_title: "三体".into(),
            date: "2026-06-18".into(),
            review_count: 1,
            expanded: false,
        });

        Self {
            // ... existing ...
            vocabulary,
        }
    }
}
```

- [ ] **Step 2: 修改 `render_main_content` 的 Vocabulary 分支**

```rust
ActiveView::Vocabulary => {
    self.vocabulary.render().into_any_element()
}
```

- [ ] **Step 3: 编译并运行验证**

```bash
cargo run
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/app.rs src/vocabulary.rs
git commit -m "feat: integrate VocabularyView into AppState with sample data"
```

---

## Completeness Check

- ✅ 按书籍分组默认视图
- ✅ 按时间倒序排序切换
- ✅ 每个词条展示：词汇名(粗体)、词性+释义、原文引用(斜体)、日期、复习次数
- ✅ 点击展开完整 AI 解释
- ✅ 删除按钮（×）
- ✅ 空状态占位文字
- ✅ 集成到 AppState 导航

## Post-Plan Verification

```bash
cargo run
```
验证：切换到词汇本视图，显示按书籍分组的词条，可切换按时间排序。
