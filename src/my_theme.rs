// ============================================================================
// my_theme.rs — 主题配色
// ============================================================================
// 对应设计文档 §9 视觉风格
// 深色: bg=#1e1e1e, secondary=#2d2d2d, text=#e0e0e0, accent=#007acc
// 浅色: bg=#ffffff, secondary=#f5f5f5, text=#1a1a1a, accent=#007acc

use gpui::rgb;

/// 配色结构体：字段对应 bg/面板/文字/强调色
pub struct Colors {
    pub bg: gpui::Hsla,
    pub secondary: gpui::Hsla,
    pub text: gpui::Hsla,
    pub text_secondary: gpui::Hsla,
    pub accent: gpui::Hsla,
}

/// 深色主题
pub fn dark() -> Colors {
    Colors {
        bg: rgb(0x1e1e1e).into(),
        secondary: rgb(0x2d2d2d).into(),
        text: rgb(0xe0e0e0).into(),
        text_secondary: rgb(0x888888).into(),
        accent: rgb(0x007acc).into(),
    }
}

/// 浅色主题
pub fn light() -> Colors {
    Colors {
        bg: rgb(0xffffff).into(),
        secondary: rgb(0xf5f5f5).into(),
        text: rgb(0x1a1a1a).into(),
        text_secondary: rgb(0x888888).into(),
        accent: rgb(0x007acc).into(),
    }
}
