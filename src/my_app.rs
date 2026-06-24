// ============================================================================
// my_app.rs — 应用状态与导航框架
// ============================================================================
// 全部 UI 代码直写在 render() 中。
// 经测试，GPUI 的 on_click/when/overflow_y_scroll 等方法，仅在
// fn render() 的直接函数体内解析时编译器才能找到对应 trait。
// 辅助方法中无法使用（impl IntoElement 返回类型导致 trait 解析失败）。
// ============================================================================

use gpui::{div, px, prelude::*, Context, Window};
use crate::my_theme::Colors;

#[derive(PartialEq, Clone, Copy)]
pub enum ActiveView { Bookshelf, CurrentReading, Vocabulary, Settings }

pub struct AppState {
    pub active_view: ActiveView,
    pub colors: Colors,
    pub has_active_book: bool,
    pub chat_visible: bool,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            active_view: ActiveView::Bookshelf,
            colors: crate::my_theme::dark(),
            has_active_book: false,
            chat_visible: false,
        }
    }
}

impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // ---- 预先计算选中状态（避免在闭包里再算） ----
        let bookshelf_on = self.active_view == ActiveView::Bookshelf;
        let reading_on = matches!(self.active_view, ActiveView::CurrentReading);
        let vocab_on = self.active_view == ActiveView::Vocabulary;
        let settings_on = self.active_view == ActiveView::Settings;
        let colors = &self.colors; // 借用，避免后续 move 问题

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(colors.bg)
            .text_color(colors.text)

            // ============================================================
            // 左侧导航栏 (56px)
            // ============================================================
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(56.0))
                    .h_full()
                    .bg(colors.secondary)
                    // 📚 书库
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w(px(56.0))
                            .h(px(48.0))
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(bookshelf_on, move |d| d.bg(colors.accent))
                            .child("📚")
                            .on_click(cx.listener(|this, _e, _w, cx| {
                                this.active_view = ActiveView::Bookshelf;
                                cx.notify();
                            })),
                    )
                    // 📖 当前阅读（仅 has_active_book 时显示）
                    .child(
                        if self.has_active_book {
                            div()
                                .flex()
                                .flex_row()
                                .w(px(56.0))
                                .h(px(48.0))
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .when(reading_on, move |d| d.bg(colors.accent))
                                .child("📖")
                                .on_click(cx.listener(|this, _e, _w, cx| {
                                    this.active_view = ActiveView::CurrentReading;
                                    cx.notify();
                                }))
                        } else {
                            div()
                        },
                    )
                    // 📝 词汇本
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w(px(56.0))
                            .h(px(48.0))
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(vocab_on, move |d| d.bg(colors.accent))
                            .child("📝")
                            .on_click(cx.listener(|this, _e, _w, cx| {
                                this.active_view = ActiveView::Vocabulary;
                                cx.notify();
                            })),
                    )
                    // ⚙ 设置
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .w(px(56.0))
                            .h(px(48.0))
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .when(settings_on, move |d| d.bg(colors.accent))
                            .child("⚙")
                            .on_click(cx.listener(|this, _e, _w, cx| {
                                this.active_view = ActiveView::Settings;
                                cx.notify();
                            })),
                    ),
            )

            // ============================================================
            // 主内容区（弹性宽度）
            // ============================================================
            .child(
                div()
                    .flex_grow(1.0)
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(match self.active_view {
                        ActiveView::Bookshelf     => div().child("📚 书库视图（后续 Plan 实现）"),
                        ActiveView::CurrentReading => div().child("📖 阅读视图（后续 Plan 实现）"),
                        ActiveView::Vocabulary    => div().child("📝 词汇本视图（后续 Plan 实现）"),
                        ActiveView::Settings      => div().child("⚙ 设置视图（后续 Plan 实现）"),
                    }),
            )

            // ============================================================
            // 右侧对话面板（0 或 320px）
            // ============================================================
            .child(
                if self.chat_visible {
                    div()
                        .w(px(320.0))
                        .h_full()
                        .bg(colors.secondary)
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
                                .child("💬 对话面板")
                                .child(
                                    div()
                                        .cursor_pointer()
                                        .child("✕")
                                        .on_click(cx.listener(|this, _e, _w, cx| {
                                            this.chat_visible = false;
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(
                            // 消息区
                            div()
                                .flex_grow(1.0)
                                .overflow_y_scroll()
                                .px_3()
                                .py_2()
                                .child("消息区域占位"),
                        )
                        .child(
                            // 输入区
                            div()
                                .px_3()
                                .py_2()
                                .border_t_1()
                                .border_color(colors.text_secondary)
                                .child("输入区域占位"),
                        )
                } else {
                    div()
                },
            )
    }
}
