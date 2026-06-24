// ============================================================================
// app.rs — 应用状态与导航框架
// ============================================================================
// 职责：
//   1. 作为 GPUI 的根实体 (Entity)，持有所有应用级状态
//   2. 实现 Render trait，构建三栏式窗口布局
//   3. 管理视图切换 (ActiveView) 和主题模式 (ThemeMode)
//
// 对应设计文档 §1 总体布局、§2 左侧导航栏
//
// GPUI 核心概念速览（给 Rust/GPUI 新手）：
// ┌──────────────────────────────────────────────────────────┐
// │ Entity<T>:  GPUI 中所有状态都存储在 Entity 中。          │
// │             AppState 就是一个 Entity。                   │
// │             cx.new(|_| AppState::new()) → Entity<AppState> │
// │                                                          │
// │ Render trait: 类似 React 的 render()，每次状态变更后     │
// │               被 GPUI 框架调用以重建 UI。                 │
// │               pub trait Render {                         │
// │                   fn render(&mut self, ...)               │
// │                       -> impl IntoElement;               │
// │               }                                         │
// │                                                          │
// │ cx.notify(): 标记实体已变更，GPUI 会在下一帧重新调用      │
// │              render() 来刷新界面。                       │
// │                                                          │
// │ cx.listener(): 创建类型安全的事件回调闭包。              │
// │                闭包参数: (this, event, window, cx)       │
// │                this = &mut Self, 可直接修改实体字段       │
// │                                                          │
// │ div(): GPUI 声明式 UI 的构建起点，类似于 HTML 的 <div>。  │
// │        通过链式调用 tailwind 风格方法构建 UI。            │
// │        .flex() .size_full() .bg(color) .child(...)       │
// └──────────────────────────────────────────────────────────┘
// ============================================================================

// ---------- 导入 ----------
//
// gpui 的 prelude 模块通过 glob 导入 (*) 可以获取常用的 trait:
//   Render, Styled, IntoElement, ParentElement 等
// 这些 trait 让我们可以调用 .child(), .bg(), .flex() 等链式方法
use gpui::{
    Context, Window,            // Context<T>: 实体上下文, Window: 窗口句柄
    div,                        // div(): 创建 Div 元素的工厂函数
    px,                         // px(56.0): 创建像素单位的类型安全包装
    prelude::*,                 // 导入 Render/Styled/IntoElement/InteractiveElement 等常用 trait
};
// 导入我们刚创建的 theme 模块
use crate::theme::{ThemeColors, dark_colors, light_colors};

// ===========================================================================
// ActiveView — 当前激活的视图枚举
// ===========================================================================
// PartialEq: 让我们可以用 == 判断当前是否选中某个导航项
// Clone:     允许复制枚举值（在闭包中使用时需要）
// Copy:      允许隐式复制（栈上数据，无堆分配，复制开销极低）
#[derive(PartialEq, Clone, Copy)]
pub enum ActiveView {
    Bookshelf,       // 📚 书库视图 — 默认激活
    CurrentReading,  // 📖 当前阅读 — 仅在有打开的书时显示
    Vocabulary,      // 📝 词汇本视图
    Settings,        // ⚙  设置视图
}

// ===========================================================================
// ThemeMode — 主题模式
// ===========================================================================
pub enum ThemeMode {
    Dark,   // 深色模式 (默认)
    Light,  // 浅色模式
}

// ===========================================================================
// AppState — 应用全局状态 (GPUI 根实体)
// ===========================================================================
// 所有需要在 UI 中显示和修改的状态都作为这个结构体的字段。
// GPUI 框架通过 Render trait 读取这些字段来构建界面。
pub struct AppState {
    /// 当前激活的视图（决定中间主内容区显示什么）
    pub active_view: ActiveView,
    /// 当前主题模式
    pub theme_mode: ThemeMode,
    /// 当前主题对应的配色方案
    pub colors: ThemeColors,
    /// 是否有正在阅读的书（决定"当前阅读"导航项是否显示）
    pub has_active_book: bool,
    /// 右侧对话面板是否展开（true=320px宽, false=0）
    pub chat_panel_visible: bool,
}

impl AppState {
    /// 创建应用初始状态
    ///
    /// GPUI 在窗口打开时调用 `cx.new(|_| AppState::new())` 创建此实体。
    pub fn new() -> Self {
        // 默认使用深色主题
        let theme_mode = ThemeMode::Dark;
        // 根据主题模式选择配色
        let colors = match theme_mode {
            ThemeMode::Dark => dark_colors(),
            ThemeMode::Light => light_colors(),
        };
        Self {
            active_view: ActiveView::Bookshelf,  // 默认进入书库
            theme_mode,
            colors,
            has_active_book: false,               // 初始没有打开的书
            chat_panel_visible: false,            // 对话面板默认隐藏
        }
    }
}

// ===========================================================================
// Render trait 实现 — GPUI 声明式 UI 的入口
// ===========================================================================
// 每次 cx.notify() 或窗口事件后，GPUI 自动调用此方法重建 UI。
// &mut self:  可以读取/修改实体状态（如 active_view, colors）
// _window:    窗口句柄，本计划暂不直接使用（下划线前缀表示忽略警告）
// cx:         实体上下文，用于创建事件监听器和触发重绘
impl Render for AppState {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // 三栏式布局:
        // ┌──────┬──────────────────────┬──────────┐
        // │ 侧栏  │      主内容区         │ 对话面板  │
        // │ 56px  │     弹性宽度          │ 0/320px  │
        // └──────┴──────────────────────┴──────────┘
        div()
            .flex()             // display: flex;        — 启用 Flexbox 布局
            .flex_row()         // flex-direction: row;   — 水平排列子元素
            .size_full()        // width: 100%; height: 100%; — 撑满窗口
            .bg(self.colors.bg) // 主背景色（深色模式下为 #1e1e1e）
            .text_color(self.colors.text) // 默认文字色
            // ---- 左侧导航栏 ----
            .child(self.render_sidebar(cx))
            // ---- 中间主内容区 ----
            .child(self.render_main_content())
            // ---- 右侧对话面板（按需显示/隐藏） ----
            .child(self.render_chat_panel(cx))
    }
}

// ===========================================================================
// 渲染辅助方法 — 将复杂 UI 拆分为独立方法
// ===========================================================================
// 每个方法返回 impl IntoElement，可以嵌入到 div().child(...) 中

impl AppState {
    /// 渲染左侧导航栏
    ///
    /// 固定 56px 宽的垂直导航栏，包含 4 个图标项:
    ///   📚 书库（始终显示，默认选中）
    ///   📖 当前阅读（仅在有打开的书时显示，动态出现/消失）
    ///   📝 词汇本
    ///   ⚙  设置
    ///
    /// 背景色使用 secondary_bg 以与主内容区形成层次区分。
    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()                         // Flexbox 容器
            .flex_col()                     // 垂直排列子元素
            .w(px(56.0))                    // 固定宽度 56px（对应设计规范）
            .h_full()                       // 高度撑满父容器
            .bg(self.colors.secondary_bg)   // 次级背景色
            // ---- 4 个导航项 ----
            .child(self.render_nav_item("📚", ActiveView::Bookshelf, cx))
            .child(self.render_current_reading_item(cx)) // 动态出现
            .child(self.render_nav_item("📝", ActiveView::Vocabulary, cx))
            .child(self.render_nav_item("⚙", ActiveView::Settings, cx))
    }

    /// 渲染一个导航项（通用方法，被书库/词汇本/设置复用）
    ///
    /// 参数:
    ///   label: 显示的文字（这里用 emoji 做图标）
    ///   view:  点击后激活的目标视图枚举值
    ///   cx:    用于注册点击事件监听器
    ///
    /// GPUI 事件处理模式:
    ///   .on_click(cx.listener(|this, _event, _window, cx| { ... }))
    ///     this:    &mut AppState — 可以修改 AppState 的字段
    ///     _event:  &ClickEvent  — 点击事件详情（_ 前缀表示未使用此参数）
    ///     _window: &mut Window  — 窗口句柄
    ///     cx:      &mut Context<Self> — 新上下文，用于 cx.notify() 触发重绘
    fn render_nav_item(
        &self,
        label: &str,
        view: ActiveView,       // Copy trait 使得 view 可以安全地 move 进闭包
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_selected = self.active_view == view; // 判断是否当前选中项

        // GPUI 的链式构建模式：
        //   所有方法（.bg(), .on_click() 等）都消耗 self 并返回新的 Div，
        //   所以条件判断必须在链的末端用 match/if 做分支选择。
        if is_selected {
            div()
                .flex()
                .flex_row()
                .w(px(56.0))
                .h(px(48.0))
                .items_center()
                .justify_center()
                .bg(self.colors.accent) // 选中态高亮
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.active_view = view;  // 切换视图
                    cx.notify();              // 通知 GPUI 重绘
                }))
                .child(label)
        } else {
            div()
                .flex()
                .flex_row()
                .w(px(56.0))
                .h(px(48.0))
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _event, _window, cx| {
                    this.active_view = view;
                    cx.notify();
                }))
                .child(label)
        }
    }

    /// 渲染"当前阅读"动态导航项
    ///
    /// 这个导航项很特别——它根据 has_active_book 的值动态出现/消失：
    ///   true  → 显示 📖 项的导航按钮
    ///   false → 返回一个空的 div()（不占任何空间）
    ///
    /// Rust/GPUI 模式: 从方法中提前返回不同类型的元素
    ///   因为返回值类型是 impl IntoElement，所以任何实现了 IntoElement
    ///   的类型都可以返回。div() 返回 Div 类型。
    fn render_current_reading_item(
        &self,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // 没有活跃的书 → 不显示导航项
        if !self.has_active_book {
            return div(); // 空 div，不渲染任何内容
        }

        // 有活跃的书 → 显示导航项
        let is_selected = matches!(self.active_view, ActiveView::CurrentReading);

        if is_selected {
            div()
                .flex()
                .flex_row()
                .w(px(56.0))
                .h(px(48.0))
                .items_center()
                .justify_center()
                .bg(self.colors.accent)
                .cursor_pointer()
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.active_view = ActiveView::CurrentReading;
                    cx.notify();
                }))
                .child("📖")
        } else {
            div()
                .flex()
                .flex_row()
                .w(px(56.0))
                .h(px(48.0))
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|this, _event, _window, cx| {
                    this.active_view = ActiveView::CurrentReading;
                    cx.notify();
                }))
                .child("📖")
        }
    }

    /// 渲染中间主内容区
    ///
    /// 根据 active_view 的值渲染不同的视图占位：
    ///   Bookshelf      → "书库视图（后续 Plan 实现）"
    ///   CurrentReading → "阅读视图（后续 Plan 实现）"
    ///   Vocabulary     → "词汇本视图（后续 Plan 实现）"
    ///   Settings       → "设置视图（后续 Plan 实现）"
    ///
    /// Rust match 表达式: 类似其他语言的 switch，但更强大——
    ///   必须穷尽所有分支（编译期检查），可以返回值。
    fn render_main_content(&self) -> impl IntoElement {
        // match 表达式返回一个元素，根据当前视图选择显示内容
        let content = match self.active_view {
            ActiveView::Bookshelf => {
                div().child("📚 书库视图（后续 Plan 实现）")
            }
            ActiveView::CurrentReading => {
                div().child("📖 阅读视图（后续 Plan 实现）")
            }
            ActiveView::Vocabulary => {
                div().child("📝 词汇本视图（后续 Plan 实现）")
            }
            ActiveView::Settings => {
                div().child("⚙ 设置视图（后续 Plan 实现）")
            }
        };

        div()
            .flex_grow(1.0)     // flex-grow: 1 — 占据剩余所有空间
            .h_full()           // 高度撑满
            .flex()             // 弹性容器
            .items_center()     // 垂直居中
            .justify_center()   // 水平居中
            .child(content)     // 渲染对应视图的占位内容
    }

    /// 渲染右侧对话面板
    ///
    /// 面板行为:
    ///   chat_panel_visible = false → 返回空 div (宽度 0)
    ///   chat_panel_visible = true  → 返回 320px 宽的对话面板
    ///
    /// 面板结构:
    ///   ┌────────────────┐
    ///   │ 对话面板    ✕   │  ← 标题栏（36px 高）
    ///   ├────────────────┤
    ///   │                │
    ///   │  消息区域占位   │  ← 可滚动消息列表（flex_grow）
    ///   │                │
    ///   ├────────────────┤
    ///   │  输入区域占位   │  ← 底部固定输入区
    ///   └────────────────┘
    ///
    /// 对应的设计文档 §6: 对话面板
    fn render_chat_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        // 面板未展开 → 返回空 div，不占空间
        if !self.chat_panel_visible {
            return div();
        }

        div()
            .w(px(320.0))                   // 固定宽度 320px
            .h_full()                        // 高度撑满窗口
            .bg(self.colors.secondary_bg)    // 次级背景（与侧栏同色，视觉统一）
            .flex()
            .flex_col()                      // 垂直排列：标题栏 → 消息区 → 输入区
            // ---- 标题栏 ----
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_between()       // 左右分布：标题在左，关闭按钮在右
                    .items_center()          // 垂直居中
                    .px_2()                  // 水平内边距 8px
                    .py_1()                  // 垂直内边距 4px
                    .h(px(36.0))             // 固定高度 36px
                    // 标题文字
                    .child("💬 对话面板")
                    // 关闭按钮 (✕)
                    .child(
                        div()
                            .cursor_pointer() // 鼠标悬停变手型
                            .child("✕")
                            // 点击关闭按钮: 隐藏面板 → notify 触发重绘
                            .on_click(cx.listener(|this, _event, _window, cx| {
                                this.chat_panel_visible = false;
                                cx.notify();
                            })),
                    ),
            )
            // ---- 消息列表区域（后续 Plan 4 填充实际消息气泡） ----
            .child(
                div()
                    .flex_grow(1.0)          // 占据标题栏和输入区之间的剩余空间
                    .overflow_y_scroll()     // 内容超出时显示垂直滚动条（GPUI 提供的便捷方法）
                    .px_3()                  // 水平内边距 12px
                    .py_2()                  // 垂直内边距 8px
                    .child("消息区域占位"),
            )
            // ---- 输入区域（后续 Plan 4 填充实际输入框） ----
            .child(
                div()
                    .px_3()
                    .py_2()
                    .border_t_1()            // 顶部分隔线
                    .border_color(self.colors.text_secondary) // 分隔线颜色
                    .child("输入区域占位"),
            )
    }
}
