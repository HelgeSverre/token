//! Theme system for the editor
//!
//! Provides YAML-based theming support with compile-time embedded themes.

use serde::Deserialize;

// Embed theme YAML files at compile time
pub const DEFAULT_DARK_YAML: &str = include_str!("../themes/dark.yaml");
pub const FLEET_DARK_YAML: &str = include_str!("../themes/fleet-dark.yaml");
pub const GITHUB_DARK_YAML: &str = include_str!("../themes/github-dark.yaml");
pub const GITHUB_LIGHT_YAML: &str = include_str!("../themes/github-light.yaml");

/// A built-in theme entry
pub struct BuiltinTheme {
    /// Stable identifier for config (e.g. "default-dark", "fleet-dark")
    pub id: &'static str,
    /// Embedded YAML content
    pub yaml: &'static str,
}

/// Registry of all built-in themes
pub const BUILTIN_THEMES: &[BuiltinTheme] = &[
    BuiltinTheme {
        id: "default-dark",
        yaml: DEFAULT_DARK_YAML,
    },
    BuiltinTheme {
        id: "fleet-dark",
        yaml: FLEET_DARK_YAML,
    },
    BuiltinTheme {
        id: "github-dark",
        yaml: GITHUB_DARK_YAML,
    },
    BuiltinTheme {
        id: "github-light",
        yaml: GITHUB_LIGHT_YAML,
    },
];

/// RGBA color (0-255 per channel)
#[derive(Debug, Clone, Copy, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    /// Create a new color from RGB values (alpha defaults to 255)
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    /// Create a new color from RGBA values
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Convert to ARGB u32 for softbuffer
    pub fn to_argb_u32(&self) -> u32 {
        ((self.a as u32) << 24) | ((self.r as u32) << 16) | ((self.g as u32) << 8) | (self.b as u32)
    }

    /// Return a new color with the specified alpha value
    pub const fn with_alpha(&self, a: u8) -> Self {
        Self {
            r: self.r,
            g: self.g,
            b: self.b,
            a,
        }
    }

    /// Parse from "#RRGGBB" or "#RRGGBBAA" hex string
    pub fn from_hex(s: &str) -> Result<Self, String> {
        let s = s.trim_start_matches('#');
        match s.len() {
            6 => Ok(Color {
                r: u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?,
                g: u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?,
                b: u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?,
                a: 255,
            }),
            8 => Ok(Color {
                r: u8::from_str_radix(&s[0..2], 16).map_err(|e| e.to_string())?,
                g: u8::from_str_radix(&s[2..4], 16).map_err(|e| e.to_string())?,
                b: u8::from_str_radix(&s[4..6], 16).map_err(|e| e.to_string())?,
                a: u8::from_str_radix(&s[6..8], 16).map_err(|e| e.to_string())?,
            }),
            _ => Err(format!("Invalid color format: {}", s)),
        }
    }
}

/// Raw theme data as parsed from YAML
#[derive(Debug, Clone, Deserialize)]
pub struct ThemeData {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub ui: UiThemeData,
}

/// UI theme colors (raw strings from YAML)
#[derive(Debug, Clone, Deserialize)]
pub struct UiThemeData {
    pub editor: EditorThemeData,
    pub gutter: GutterThemeData,
    pub status_bar: StatusBarThemeData,
    #[serde(default)]
    pub overlay: OverlayThemeData,
}

/// Editor area colors
#[derive(Debug, Clone, Deserialize)]
pub struct EditorThemeData {
    pub background: String,
    pub foreground: String,
    pub current_line_background: String,
    pub cursor_color: String,
    #[serde(default)]
    pub selection_background: Option<String>,
    #[serde(default)]
    pub secondary_cursor_color: Option<String>,
}

/// Gutter (line numbers) colors
#[derive(Debug, Clone, Deserialize)]
pub struct GutterThemeData {
    pub background: String,
    pub foreground: String,
    pub foreground_active: String,
    pub border_color: Option<String>,
}

/// Status bar colors
#[derive(Debug, Clone, Deserialize)]
pub struct StatusBarThemeData {
    pub background: String,
    pub foreground: String,
}

/// Overlay colors (all optional for backward compatibility)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct OverlayThemeData {
    #[serde(default)]
    pub border: Option<String>,
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub foreground: Option<String>,
    #[serde(default)]
    pub highlight: Option<String>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Resolved theme with parsed colors
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub editor: EditorTheme,
    pub gutter: GutterTheme,
    pub status_bar: StatusBarTheme,
    pub overlay: OverlayTheme,
}

/// Editor colors (resolved)
#[derive(Debug, Clone)]
pub struct EditorTheme {
    pub background: Color,
    pub foreground: Color,
    pub current_line_background: Color,
    pub cursor_color: Color,
    /// Background color for selected text
    pub selection_background: Color,
    /// Color for non-primary cursors in multi-cursor mode
    pub secondary_cursor_color: Color,
}

/// Gutter colors (resolved)
#[derive(Debug, Clone)]
pub struct GutterTheme {
    pub background: Color,
    pub foreground: Color,
    pub foreground_active: Color,
    pub border_color: Color,
}

/// Status bar colors (resolved)
#[derive(Debug, Clone)]
pub struct StatusBarTheme {
    pub background: Color,
    pub foreground: Color,
}

/// Overlay colors (resolved)
#[derive(Debug, Clone)]
pub struct OverlayTheme {
    /// Optional border color (None = no border)
    pub border: Option<Color>,
    /// Semi-transparent background color
    pub background: Color,
    /// Default text color
    pub foreground: Color,
    /// Success/good indicator color (green)
    pub highlight: Color,
    /// Caution/warning indicator color (yellow)
    pub warning: Color,
    /// Error/bad indicator color (red)
    pub error: Color,
}

impl OverlayTheme {
    /// Default overlay theme (dark)
    pub fn default_dark() -> Self {
        Self {
            border: None,
            background: Color::rgba(0x20, 0x20, 0x20, 0xE0),
            foreground: Color::rgb(0xE0, 0xE0, 0xE0),
            highlight: Color::rgb(0x80, 0xFF, 0x80),
            warning: Color::rgb(0xFF, 0xFF, 0x80),
            error: Color::rgb(0xFF, 0x80, 0x80),
        }
    }
}

impl Theme {
    /// Load theme from YAML string
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        let data: ThemeData =
            serde_yaml::from_str(yaml).map_err(|e| format!("YAML parse error: {}", e))?;
        Self::from_data(data)
    }

    /// Load a built-in theme by id
    pub fn from_builtin(id: &str) -> Result<Self, String> {
        let entry = BUILTIN_THEMES
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| format!("Unknown theme id: {}", id))?;
        Theme::from_yaml(entry.yaml)
    }

    /// Convert raw theme data to resolved theme
    pub fn from_data(data: ThemeData) -> Result<Self, String> {
        let default_selection_bg = Color::rgb(0x26, 0x4F, 0x78);
        let default_secondary_cursor = Color::rgba(0xFF, 0xFF, 0xFF, 0x80);

        Ok(Theme {
            name: data.name,
            editor: EditorTheme {
                background: Color::from_hex(&data.ui.editor.background)?,
                foreground: Color::from_hex(&data.ui.editor.foreground)?,
                current_line_background: Color::from_hex(&data.ui.editor.current_line_background)?,
                cursor_color: Color::from_hex(&data.ui.editor.cursor_color)?,
                selection_background: data
                    .ui
                    .editor
                    .selection_background
                    .as_ref()
                    .map(|s| Color::from_hex(s))
                    .transpose()?
                    .unwrap_or(default_selection_bg),
                secondary_cursor_color: data
                    .ui
                    .editor
                    .secondary_cursor_color
                    .as_ref()
                    .map(|s| Color::from_hex(s))
                    .transpose()?
                    .unwrap_or(default_secondary_cursor),
            },
            gutter: GutterTheme {
                background: Color::from_hex(&data.ui.gutter.background)?,
                foreground: Color::from_hex(&data.ui.gutter.foreground)?,
                foreground_active: Color::from_hex(&data.ui.gutter.foreground_active)?,
                border_color: data
                    .ui
                    .gutter
                    .border_color
                    .as_ref()
                    .map(|s| Color::from_hex(s))
                    .transpose()?
                    .unwrap_or(Color::rgb(0x31, 0x34, 0x38)),
            },
            status_bar: StatusBarTheme {
                background: Color::from_hex(&data.ui.status_bar.background)?,
                foreground: Color::from_hex(&data.ui.status_bar.foreground)?,
            },
            overlay: {
                let defaults = OverlayTheme::default_dark();
                OverlayTheme {
                    border: data
                        .ui
                        .overlay
                        .border
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?,
                    background: data
                        .ui
                        .overlay
                        .background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.background),
                    foreground: data
                        .ui
                        .overlay
                        .foreground
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.foreground),
                    highlight: data
                        .ui
                        .overlay
                        .highlight
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.highlight),
                    warning: data
                        .ui
                        .overlay
                        .warning
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.warning),
                    error: data
                        .ui
                        .overlay
                        .error
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.error),
                }
            },
        })
    }

    /// Default dark theme (YAML-backed with Rust fallback)
    pub fn default_dark() -> Self {
        match Theme::from_yaml(DEFAULT_DARK_YAML) {
            Ok(theme) => theme,
            Err(_) => {
                // Hardcoded fallback if YAML parsing fails
                Theme {
                    name: "Default Dark".to_string(),
                    editor: EditorTheme {
                        background: Color::rgb(0x1E, 0x1E, 0x1E),
                        foreground: Color::rgb(0xD4, 0xD4, 0xD4),
                        current_line_background: Color::rgb(0x2A, 0x2A, 0x2A),
                        cursor_color: Color::rgb(0xFF, 0xFF, 0xFF),
                        selection_background: Color::rgb(0x26, 0x4F, 0x78),
                        secondary_cursor_color: Color::rgba(0xFF, 0xFF, 0xFF, 0x80),
                    },
                    gutter: GutterTheme {
                        background: Color::rgb(0x1E, 0x1E, 0x1E),
                        foreground: Color::rgb(0x85, 0x85, 0x85),
                        foreground_active: Color::rgb(0xC6, 0xC6, 0xC6),
                        border_color: Color::rgb(0x31, 0x34, 0x38),
                    },
                    status_bar: StatusBarTheme {
                        background: Color::rgb(0x00, 0x7A, 0xCC),
                        foreground: Color::rgb(0xFF, 0xFF, 0xFF),
                    },
                    overlay: OverlayTheme::default_dark(),
                }
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::default_dark()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_from_hex_6() {
        let color = Color::from_hex("#1E1E1E").unwrap();
        assert_eq!(color.r, 0x1E);
        assert_eq!(color.g, 0x1E);
        assert_eq!(color.b, 0x1E);
        assert_eq!(color.a, 255);
    }

    #[test]
    fn test_color_from_hex_8() {
        let color = Color::from_hex("#1E1E1E80").unwrap();
        assert_eq!(color.r, 0x1E);
        assert_eq!(color.g, 0x1E);
        assert_eq!(color.b, 0x1E);
        assert_eq!(color.a, 0x80);
    }

    #[test]
    fn test_color_to_argb_u32() {
        let color = Color::rgb(0x1E, 0x1E, 0x1E);
        assert_eq!(color.to_argb_u32(), 0xFF1E1E1E);
    }

    #[test]
    fn test_default_theme() {
        let theme = Theme::default_dark();
        assert_eq!(theme.name, "Default Dark");
        assert_eq!(theme.editor.background.to_argb_u32(), 0xFF1E1E1E);
    }

    #[test]
    fn test_default_dark_yaml_parses() {
        let theme = Theme::from_yaml(DEFAULT_DARK_YAML).unwrap();
        assert_eq!(theme.name, "Default Dark");
    }

    #[test]
    fn test_parse_fleet_dark() {
        let theme = Theme::from_yaml(FLEET_DARK_YAML).unwrap();
        assert_eq!(theme.name, "Fleet Dark");
        assert_eq!(theme.editor.background.r, 0x18);
    }

    #[test]
    fn test_parse_github_dark() {
        let theme = Theme::from_yaml(GITHUB_DARK_YAML).unwrap();
        assert_eq!(theme.name, "GitHub Dark");
        assert_eq!(theme.editor.background.r, 0x0D);
    }

    #[test]
    fn test_parse_github_light() {
        let theme = Theme::from_yaml(GITHUB_LIGHT_YAML).unwrap();
        assert_eq!(theme.name, "GitHub Light");
        assert_eq!(theme.editor.background.r, 0xFF);
    }

    #[test]
    fn test_from_builtin() {
        let theme = Theme::from_builtin("fleet-dark").unwrap();
        assert_eq!(theme.name, "Fleet Dark");

        let result = Theme::from_builtin("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_all_builtin_themes_parse() {
        for builtin in BUILTIN_THEMES {
            let theme = Theme::from_yaml(builtin.yaml)
                .unwrap_or_else(|e| panic!("Failed to parse theme '{}': {}", builtin.id, e));
            assert!(
                !theme.name.is_empty(),
                "Theme '{}' has empty name",
                builtin.id
            );
        }
    }
}
