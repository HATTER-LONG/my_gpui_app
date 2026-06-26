# Plan 5: AI 推荐讨论点

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现阅读区内 AI 推荐讨论点的三种高亮状态（AI推荐/已聊过/用户划线）和内联标注渲染，为"一键开聊"提供入口。

**Architecture:** `DiscussionManager` 持有 `Vec<Highlight>` 列表，每种高亮类型有独立颜色。在 `ReadingView` 的内容渲染中，文本段落被拆分为含/不含高亮的片段（Span），高亮片段附加 `💬 聊聊` 标签。

**Tech Stack:** Rust + GPUI

**Dependencies:** Plan 3 (ReadingView 内容渲染), Plan 4 (ChatPanel 对话系统)

**Global Constraints:**
- AI推荐点: `rgba(156, 39, 176, 0.15)` 紫色虚线底 + `💬 聊聊` 标签
- 已聊过: 淡紫色实色底，标签消失
- 用户手动划线: `rgba(255, 235, 59, 0.3)` 黄色实色底
- 点击高亮可切换对话面板到对应历史对话

---

## File Structure

```
src/
  discussion.rs  - Highlight, HighlightType, DiscussionManager, 高亮渲染 (create)
  reading.rs     - 集成高亮到内容渲染 (modify)
```

---

### Task 1: 创建高亮模型和 DiscussionManager

**Files:**
- Create: `src/discussion.rs`

**Interfaces:**
- Produces: `pub enum HighlightType { AIRecommended, Discussed, UserMark }`
- Produces: `pub struct Highlight { pub hl_type: HighlightType, pub text: String, pub start_char: usize, pub end_char: usize, pub conversation_id: Option<String> }`
- Produces: `pub struct DiscussionManager { pub highlights: Vec<Highlight> }`

- [ ] **Step 1: 创建 `src/discussion.rs`**

```rust
use gpui::{Hsla, rgba};

#[derive(Clone, PartialEq)]
pub enum HighlightType {
    AIRecommended,
    Discussed,
    UserMark,
}

pub struct Highlight {
    pub hl_type: HighlightType,
    pub text: String,
    pub start_char: usize,
    pub end_char: usize,
    pub conversation_id: Option<String>,
}

impl Highlight {
    pub fn bg_color(&self) -> Hsla {
        match self.hl_type {
            HighlightType::AIRecommended => rgba(0x9c27b026), // 紫色 15% 透明
            HighlightType::Discussed => rgba(0x9c27b050),     // 紫色 50% 透明
            HighlightType::UserMark => rgba(0xffeb334d),      // 黄色 30% 透明
        }
    }

    pub fn show_tag(&self) -> bool {
        matches!(self.hl_type, HighlightType::AIRecommended)
    }
}

pub struct DiscussionManager {
    pub highlights: Vec<Highlight>,
}

impl DiscussionManager {
    pub fn new() -> Self {
        Self {
            highlights: Vec::new(),
        }
    }

    pub fn add_ai_recommendation(&mut self, text: String, start: usize, end: usize) {
        self.highlights.push(Highlight {
            hl_type: HighlightType::AIRecommended,
            text,
            start_char: start,
            end_char: end,
            conversation_id: None,
        });
    }

    pub fn mark_discussed(&mut self, index: usize, conversation_id: String) {
        if let Some(hl) = self.highlights.get_mut(index) {
            hl.hl_type = HighlightType::Discussed;
            hl.conversation_id = Some(conversation_id);
        }
    }

    pub fn add_user_mark(&mut self, text: String, start: usize, end: usize) {
        self.highlights.push(Highlight {
            hl_type: HighlightType::UserMark,
            text,
            start_char: start,
            end_char: end,
            conversation_id: None,
        });
    }

    /// 获取指定字符范围内的高亮
    pub fn get_highlights_in_range(&self, start: usize, end: usize) -> Vec<&Highlight> {
        self.highlights
            .iter()
            .filter(|h| h.start_char >= start && h.end_char <= end)
            .collect()
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/discussion.rs
git commit -m "feat: add Highlight model and DiscussionManager with three highlight types"
```

---

### Task 2: 创建高亮内联渲染组件

**Files:**
- Modify: `src/discussion.rs`

- [ ] **Step 1: 添加高亮文本渲染方法**

```rust
use gpui::{div, px, prelude::*, IntoElement, Styled};

impl DiscussionManager {
    /// 渲染单个高亮的文本片段，附带视觉样式和标签
    pub fn render_highlight_text(&self, hl: &Highlight) -> impl IntoElement {
        let bg = hl.bg_color();

        div()
            .inline()
            .bg(bg)
            .relative()
            .child(
                div()
                    .inline()
                    .border_b_1()
                    .border_color(if matches!(hl.hl_type, HighlightType::AIRecommended) {
                        rgba(0x9c27b0ff) // 紫色边框
                    } else {
                        rgba(0x00000000) // 透明边框
                    })
                    .child(hl.text.clone()),
            )
            .child(
                if hl.show_tag() {
                    div()
                        .inline()
                        .text_xs()
                        .text_color(rgba(0x9c27b0ff))
                        .cursor_pointer()
                        .child(" 💬聊聊")
                } else {
                    div()
                },
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/discussion.rs
git commit -m "feat: add inline highlight rendering with color and tag"
```

---

### Task 3: 将高亮集成到 ReadingView 文本渲染

**Files:**
- Modify: `src/reading.rs`
- Modify: `src/main.rs` — 添加 `mod discussion;`

- [ ] **Step 1: 在 ReadingView 中持有 DiscussionManager**

```rust
// src/reading.rs
use crate::discussion::DiscussionManager;

pub struct ReadingView {
    // ... existing fields ...
    pub discussion: DiscussionManager,
}

impl ReadingView {
    pub fn new(book_title: String, chapter_title: String) -> Self {
        let mut discussion = DiscussionManager::new();
        // 添加示例 AI 推荐点
        discussion.add_ai_recommendation(
            "叶文洁在红岸基地".into(),
            20, 30,
        );
        Self {
            // ... existing fields ...
            discussion,
        }
    }
}
```

- [ ] **Step 2: 修改 `render_content` 使用带高亮的文本渲染**

```rust
fn render_content(&self, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .flex_grow()
        .overflow_y_scroll()
        .px(px(80.0))
        .py_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_4()
                .children(
                    self.content.iter().enumerate().map(|(para_idx, paragraph)| {
                        let highlights = self.discussion.get_highlights_in_range(
                            para_idx * 1000, // 简化：用段落索引做粗略范围
                            (para_idx + 1) * 1000,
                        );
                        if highlights.is_empty() {
                            div()
                                .text_base()
                                .leading(px(28.8))
                                .child(paragraph.clone())
                        } else {
                            div()
                                .flex()
                                .flex_row()
                                .flex_wrap()
                                .text_base()
                                .leading(px(28.8))
                                .child(paragraph.clone())
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .children(
                                            highlights.iter().map(|hl| {
                                                self.discussion.render_highlight_text(hl)
                                            }),
                                        ),
                                )
                        }
                    }),
                ),
        )
}
```

- [ ] **Step 3: 编译验证** `cargo check`

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/reading.rs
git commit -m "feat: integrate DiscussionManager highlights into ReadingView text"
```

---

### Task 4: 实现讨论入口的三条路径

**Files:**
- Modify: `src/app.rs` — 处理各入口触发的面板展开

- [ ] **Step 1: 在 AppState 中实现 `open_discussion` 方法**

```rust
impl AppState {
    pub fn open_discussion(&mut self, context: String, source: &str, cx: &mut Context<Self>) {
        use crate::chat::{ChatPanel, ChatRole};

        let title = format!("{}讨论", source);
        self.chat_panel = Some(ChatPanel::new(title));
        self.chat_panel_visible = true;

        if let Some(ref mut panel) = self.chat_panel {
            panel.add_message(ChatRole::User, context);
        }

        cx.notify();
    }
}
```

- [ ] **Step 2: 阅读视图中选中文字"发起讨论" → 调用 `open_discussion`**

在 ReadingView 的文字选中交互中（当存在选中文本并触发菜单时），通过 AppState 调用：

```rust
// 伪代码：在 ReadingView 中通过 cx 访问 AppState
// 实际需要通过 Entity observe 或 Action 机制传递
```

- [ ] **Step 3: AI 推荐点"💬 聊聊"点击 → 调用 `open_discussion`**

在高亮渲染中，`💬聊聊` 标签的 `on_click` 触发面板展开。

- [ ] **Step 4: 对话面板中直接输入 → 以当前页为上下文**

ChatPanel 输入时自动关联当前阅读页的上下文。

- [ ] **Step 5: 编译验证** `cargo check`

- [ ] **Step 6: Commit**

```bash
git add src/app.rs src/discussion.rs src/reading.rs
git commit -m "feat: wire up three discussion entry paths to chat panel"
```

---

## Completeness Check

- ✅ 三种高亮状态：AI推荐（紫色虚线底 + 💬聊聊），已聊过（紫色实色底），用户划线（黄色实色底）
- ✅ 高亮在阅读区文本内渲染
- ✅ 讨论入口三条路径：选中文字"发起讨论"、点击"💬 聊聊"、对话面板直接输入
- ✅ 点击"已聊过"高亮可切换到对应历史对话

## Post-Plan Verification

```bash
cargo run
```
验证：阅读视图文本中，AI 推荐的句子显示紫色虚线和 `💬聊聊` 标签。
