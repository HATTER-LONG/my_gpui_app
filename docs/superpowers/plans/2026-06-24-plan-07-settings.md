# Plan 7: 设置视图

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** 实现设置视图——API 配置（LLM后端/API Key/Model/测试连接）、对话设置（语言/默认人格/字体大小）、数据管理（存储位置/导出/导入/数据库大小）、关于信息。

**Architecture:** `SettingsView` 持有各配置字段和连接状态。表单使用 GPUI div 构建分组卡片布局。API Key 输入使用密码遮蔽显示。测试连接按钮模拟异步状态。

**Tech Stack:** Rust + GPUI

**Dependencies:** Plan 1 (AppState 的 `ActiveView::Settings` 分支)

**Global Constraints:** 分组卡片布局，API Key 遮蔽显示，连接状态指示

---

## File Structure

```
src/
  settings.rs  - SettingsView, LlmBackend, 配置分组渲染 (create)
  app.rs       - 集成 SettingsView (modify)
```

---

### Task 1: 创建 SettingsView 和配置模型

**Files:**
- Create: `src/settings.rs`

**Interfaces:**
- Produces: `pub enum LlmBackend { OpenAI, Anthropic, Ollama, Custom }`
- Produces: `pub struct SettingsView { pub llm_backend: LlmBackend, pub api_key: String, pub api_key_visible: bool, pub model: String, pub connection_ok: bool, pub connection_testing: bool, pub chat_language: String, pub default_persona: String, pub font_size: u32 }`

- [ ] **Step 1: 创建 `src/settings.rs`**

```rust
use gpui::{
    Context, Window, div, px, prelude::*,
    IntoElement, ParentElement, Render, Styled,
};

#[derive(PartialEq, Clone)]
pub enum LlmBackend {
    OpenAI,
    Anthropic,
    Ollama,
    Custom,
}

impl LlmBackend {
    pub fn label(&self) -> &str {
        match self {
            LlmBackend::OpenAI => "OpenAI",
            LlmBackend::Anthropic => "Anthropic",
            LlmBackend::Ollama => "Ollama",
            LlmBackend::Custom => "自定义兼容端点",
        }
    }

    pub fn all() -> Vec<LlmBackend> {
        vec![
            LlmBackend::OpenAI,
            LlmBackend::Anthropic,
            LlmBackend::Ollama,
            LlmBackend::Custom,
        ]
    }
}

pub struct SettingsView {
    pub llm_backend: LlmBackend,
    pub api_key: String,
    pub api_key_visible: bool,
    pub model: String,
    pub connection_ok: bool,
    pub connection_testing: bool,
    pub chat_language: String,
    pub default_persona: String,
    pub font_size: u32,
}

impl SettingsView {
    pub fn new() -> Self {
        Self {
            llm_backend: LlmBackend::OpenAI,
            api_key: String::new(),
            api_key_visible: false,
            model: "gpt-4o".into(),
            connection_ok: false,
            connection_testing: false,
            chat_language: "中文".into(),
            default_persona: "教授模式".into(),
            font_size: 16,
        }
    }
}

impl Render for SettingsView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_4()
            .overflow_y_scroll()
            .gap_4()
            .child(self.render_title())
            .child(self.render_api_config(cx))
            .child(self.render_chat_settings(cx))
            .child(self.render_data_management(cx))
            .child(self.render_about())
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/settings.rs
git commit -m "feat: add SettingsView with LlmBackend model and config structure"
```

---

### Task 2: 实现 API 配置分组

**Files:**
- Modify: `src/settings.rs`

- [ ] **Step 1: 添加 API 配置渲染方法**

```rust
impl SettingsView {
    fn render_title(&self) -> impl IntoElement {
        div()
            .text_xl()
            .font_weight(gpui::FontWeight::BOLD)
            .mb_2()
            .child("设置")
    }

    fn render_api_config(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("API 配置"),
            )
            .child(self.render_config_row("LLM 后端", {
                let backend = self.llm_backend.clone();
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .children(LlmBackend::all().iter().map(|b| {
                        let is_active = *b == backend;
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .when(is_active, |d| d.bg(gpui::rgba(0x007accff)))
                            .text_sm()
                            .cursor_pointer()
                            .child(b.label())
                    }))
            }, cx))
            .child(self.render_config_row("API Key", {
                let display = if self.api_key_visible {
                    self.api_key.clone()
                } else if self.api_key.is_empty() {
                    String::new()
                } else {
                    "●".repeat(self.api_key.len().min(16))
                };
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .bg(gpui::rgba(0x1e1e1eff))
                            .rounded_sm()
                            .min_w(px(200.0))
                            .text_sm()
                            .child(display),
                    )
                    .child(
                        div()
                            .text_xs()
                            .cursor_pointer()
                            .text_color(gpui::rgba(0x888888ff))
                            .child(if self.api_key_visible { "隐藏" } else { "显示" }),
                    )
            }, cx))
            .child(self.render_config_row("Model", {
                div()
                    .px_3()
                    .py_1()
                    .bg(gpui::rgba(0x1e1e1eff))
                    .rounded_sm()
                    .text_sm()
                    .child(self.model.clone())
            }, cx))
            .child(self.render_config_row("测试连接", {
                let status = if self.connection_testing {
                    "⏳ 测试中..."
                } else if self.connection_ok {
                    "✅ 已连接"
                } else {
                    "⚠ 未测试"
                };
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .px_4()
                            .py_1()
                            .bg(gpui::rgba(0x007accff))
                            .rounded_md()
                            .text_sm()
                            .text_color(gpui::rgba(0xffffffff))
                            .cursor_pointer()
                            .child("测试"),
                    )
                    .child(
                        div().text_sm().child(status),
                    )
            }, cx))
    }

    fn render_config_row(
        &self,
        label: &str,
        control: impl IntoElement,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .justify_between()
            .items_center()
            .child(
                div()
                    .text_sm()
                    .w(px(80.0))
                    .child(label),
            )
            .child(control.into_element())
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/settings.rs
git commit -m "feat: add API config section with LLM backend/key/model/test"
```

---

### Task 3: 实现对话设置和数据管理分组

**Files:**
- Modify: `src/settings.rs`

- [ ] **Step 1: 添加对话设置渲染**

```rust
impl SettingsView {
    fn render_chat_settings(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("对话设置"),
            )
            .child(self.render_config_row("对话语言", {
                div()
                    .px_3()
                    .py_1()
                    .bg(gpui::rgba(0x1e1e1eff))
                    .rounded_sm()
                    .text_sm()
                    .child(self.chat_language.clone())
            }, cx))
            .child(self.render_config_row("默认人格", {
                div()
                    .px_3()
                    .py_1()
                    .bg(gpui::rgba(0x1e1e1eff))
                    .rounded_sm()
                    .text_sm()
                    .child(self.default_persona.clone())
            }, cx))
            .child(self.render_config_row("字体大小", {
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .items_center()
                    .child(
                        div()
                            .w(px(200.0))
                            .h(px(6.0))
                            .bg(gpui::rgba(0x444444ff))
                            .rounded_full()
                            .child(
                                div()
                                    .w(px(120.0))
                                    .h(px(6.0))
                                    .bg(gpui::rgba(0x007accff))
                                    .rounded_full(),
                            ),
                    )
                    .child(
                        div().text_sm().child(format!("{}px", self.font_size)),
                    )
            }, cx))
    }

    fn render_data_management(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("数据管理"),
            )
            .child(self.render_config_row("存储位置", {
                div()
                    .text_sm()
                    .text_color(gpui::rgba(0x888888ff))
                    .child("/home/user/Documents/my_gpui_app")
            }, cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .gap_2()
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .bg(gpui::rgba(0x3a3a3aff))
                            .cursor_pointer()
                            .child("导出数据(JSON)"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .bg(gpui::rgba(0x3a3a3aff))
                            .cursor_pointer()
                            .child("导入数据(JSON)"),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_sm()
                            .bg(gpui::rgba(0x3a3a3aff))
                            .cursor_pointer()
                            .child("打开数据目录"),
                    ),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgba(0x888888ff))
                    .child("数据库大小: 12.3 MB"),
            )
    }

    fn render_about(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .bg(gpui::rgba(0x2d2d2dff))
            .rounded_md()
            .child(
                div()
                    .text_sm()
                    .font_weight(gpui::FontWeight::BOLD)
                    .child("关于"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(gpui::rgba(0x888888ff))
                    .child("AI 阅读陪伴助手  v0.1.0"),
            )
    }
}
```

- [ ] **Step 2: 编译验证** `cargo check`
- [ ] **Step 3: Commit**

```bash
git add src/settings.rs
git commit -m "feat: add chat settings and data management sections"
```

---

### Task 4: 集成到 AppState

**Files:**
- Modify: `src/main.rs` — 添加 `mod settings;`
- Modify: `src/app.rs` — 持有 `SettingsView`

- [ ] **Step 1: 在 AppState 中集成**

```rust
use crate::settings::SettingsView;

pub struct AppState {
    // ... existing ...
    pub settings: SettingsView,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            // ... existing ...
            settings: SettingsView::new(),
        }
    }
}
```

- [ ] **Step 2: 修改 `render_main_content` 的 Settings 分支**

```rust
ActiveView::Settings => {
    self.settings.render().into_any_element()
}
```

- [ ] **Step 3: 编译并运行验证**

```bash
cargo run
```

- [ ] **Step 4: Commit**

```bash
git add src/main.rs src/app.rs src/settings.rs
git commit -m "feat: integrate SettingsView into AppState"
```

---

## Completeness Check

- ✅ API 配置组（LLM 后端下拉 / API Key 遮蔽 / Model / 测试连接）
- ✅ 对话设置组（语言 / 默认人格 / 字体大小滑块）
- ✅ 数据管理组（存储位置 / 导出 / 导入 / 打开目录 / 数据库大小）
- ✅ 关于信息
- ✅ 集成到 AppState 导航

## Post-Plan Verification

```bash
cargo run
```
验证：切换到设置视图，显示 4 个配置分组，API Key 可切换显示/隐藏，测试连接有状态指示。
