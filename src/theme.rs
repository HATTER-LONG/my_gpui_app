// ============================================================================
// theme.rs — 主题颜色模块
// ============================================================================
// 职责：定义应用的深色/浅色两种主题配色方案。
// GPUI 中的颜色使用 Hsla 类型（色相/饱和度/明度/透明度），
// 可以用 rgb(u32) 函数从十六进制值创建。
//
// 对应设计文档 §9：视觉风格
//   深色: bg=#1e1e1e, secondary=#2d2d2d, text=#e0e0e0, accent=#007acc
//   浅色: bg=#ffffff, secondary=#f5f5f5, text=#1a1a1a, accent=#007acc
// ============================================================================

// ---------- 导入 ----------
// Rgba: GPUI 的颜色类型（红/绿/蓝/透明度，RGB Hex 格式）。可以直接传给 .bg() .text_color() 等方法
// rgb:  从十六进制整数创建 Rgba 颜色，例如 rgb(0x1e1e1e) → 深灰色
use gpui::{Rgba, rgb};

/// 主题配色集合
///
/// 每个字段对应一个语义色角色：
/// - bg:            页面主背景色
/// - secondary_bg:  卡片/面板/侧栏的次级背景色
/// - text:          正文颜色
/// - text_secondary: 辅助文字颜色（提示、说明等）
/// - accent:        强调色（选中态、按钮、链接等）
pub struct ThemeColors {
    pub bg: Rgba,
    pub secondary_bg: Rgba,
    pub text: Rgba,
    pub text_secondary: Rgba,
    pub accent: Rgba,
}

/// 深色主题配色
///
/// 用于默认模式：暗色背景 + 浅色文字，长时间阅读时减轻眼睛疲劳。
pub fn dark_colors() -> ThemeColors {
    ThemeColors {
        bg: rgb(0x1e1e1e),             // 近乎全黑的深灰底色
        secondary_bg: rgb(0x2d2d2d),   // 稍亮的深灰色，用于区分层次
        text: rgb(0xe0e0e0),           // 白色偏灰，柔和阅读体验
        text_secondary: rgb(0x888888), // 灰色辅助文字
        accent: rgb(0x007acc),         // VS Code 同款蓝色强调色
    }
}

/// 浅色主题配色
///
/// 用于白天/高亮环境：白色背景 + 深色文字。
pub fn light_colors() -> ThemeColors {
    ThemeColors {
        bg: rgb(0xffffff),             // 纯白底色
        secondary_bg: rgb(0xf5f5f5),   // 微灰面板背景
        text: rgb(0x1a1a1a),           // 近黑色正文
        text_secondary: rgb(0x888888), // 灰色辅助文字
        accent: rgb(0x007acc),         // 强调色与深色主题保持一致
    }
}
