//! Preview theme - colors for Markdown preview rendering

use crate::theme::{Color, Theme};

/// Theme colors for markdown preview (CSS-formatted)
#[derive(Debug, Clone)]
pub struct PreviewTheme {
    pub background: String,
    pub text: String,
    pub heading: String,
    pub link: String,
    pub code_background: String,
    pub border: String,
    pub accent: String,
    pub muted: String,
}

impl PreviewTheme {
    /// Generate preview theme from editor theme
    pub fn from_editor_theme(theme: &Theme) -> Self {
        Self {
            background: color_to_css(&theme.editor.background),
            text: color_to_css(&theme.editor.foreground),
            heading: color_to_css(&theme.syntax.keyword),
            link: color_to_css(&theme.syntax.string),
            code_background: color_to_css(&theme.gutter.background),
            border: color_to_css(&theme.gutter.border_color),
            accent: color_to_css(&theme.syntax.function),
            muted: color_to_css(&theme.gutter.foreground),
        }
    }
}

impl Default for PreviewTheme {
    fn default() -> Self {
        Self {
            background: "#1e1e1e".to_string(),
            text: "#d4d4d4".to_string(),
            heading: "#569cd6".to_string(),
            link: "#ce9178".to_string(),
            code_background: "#252526".to_string(),
            border: "#3c3c3c".to_string(),
            accent: "#dcdcaa".to_string(),
            muted: "#858585".to_string(),
        }
    }
}

fn color_to_css(color: &Color) -> String {
    if color.a == 255 {
        format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
    } else {
        format!(
            "#{:02x}{:02x}{:02x}{:02x}",
            color.r, color.g, color.b, color.a
        )
    }
}
