//! Theme system for the editor
//!
//! Provides YAML-based theming support with compile-time embedded themes
//! and user-defined themes from config directories.
//!
//! Theme loading priority:
//! 1. User config: `~/.config/token-editor/themes/{id}.yaml`
//! 2. Embedded: Built-in themes compiled into binary

use std::path::{Path, PathBuf};

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

/// Where the theme came from
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemeSource {
    /// User-defined theme in ~/.config/token-editor/themes/
    User,
    /// Built-in theme embedded in binary
    Builtin,
}

/// Information about an available theme
#[derive(Debug, Clone)]
pub struct ThemeInfo {
    /// Stable identifier (e.g., "default-dark", "my-custom-theme")
    pub id: String,
    /// Display name from YAML (e.g., "Default Dark")
    pub name: String,
    /// Where this theme is loaded from
    pub source: ThemeSource,
}

/// Get the user's theme configuration directory
///
/// Returns `~/.config/token-editor/themes/` on Unix
/// Returns `%APPDATA%\token-editor\themes\` on Windows
pub fn get_user_themes_dir() -> Option<PathBuf> {
    get_config_dir().map(|config| config.join("themes"))
}

/// Get the user's config directory for token-editor
///
/// Returns `~/.config/token-editor/` on Unix/macOS
/// Returns `%APPDATA%\token-editor\` on Windows
pub fn get_config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join("token-editor"))
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Use XDG-style ~/.config on all Unix systems including macOS
        // (dirs::config_dir() returns ~/Library/Application Support on macOS)
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
            .map(|config| config.join("token-editor"))
    }
}

/// Load a theme from a YAML file
pub fn from_file(path: &Path) -> Result<Theme, String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read theme file {}: {}", path.display(), e))?;
    Theme::from_yaml(&content)
}

/// Load theme by id with priority: user → builtin
///
/// Searches in order:
/// 1. `~/.config/token-editor/themes/{id}.yaml`
/// 2. Embedded builtin themes
pub fn load_theme(id: &str) -> Result<Theme, String> {
    // Try user themes directory
    if let Some(user_dir) = get_user_themes_dir() {
        let user_path = user_dir.join(format!("{}.yaml", id));
        if user_path.exists() {
            tracing::info!("Loading user theme from {}", user_path.display());
            return from_file(&user_path);
        }
    }

    // Fall back to builtin
    tracing::info!("Loading builtin theme: {}", id);
    Theme::from_builtin(id)
}

/// List all available themes from all sources
///
/// Returns themes grouped by source, with duplicates resolved by priority:
/// user themes override builtins with the same id.
pub fn list_available_themes() -> Vec<ThemeInfo> {
    let mut themes = Vec::new();
    let mut seen_ids = std::collections::HashSet::new();

    // Collect user themes (highest priority)
    if let Some(user_dir) = get_user_themes_dir() {
        if let Ok(entries) = std::fs::read_dir(&user_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|ext| ext == "yaml" || ext == "yml")
                {
                    if let Some(id) = path.file_stem().and_then(|s| s.to_str()) {
                        if seen_ids.insert(id.to_string()) {
                            let name = extract_theme_name(&path).unwrap_or_else(|| id.to_string());
                            themes.push(ThemeInfo {
                                id: id.to_string(),
                                name,
                                source: ThemeSource::User,
                            });
                        }
                    }
                }
            }
        }
    }

    // Add builtins (user themes with same id take priority)
    for builtin in BUILTIN_THEMES {
        if seen_ids.insert(builtin.id.to_string()) {
            let name = Theme::from_yaml(builtin.yaml)
                .map(|t| t.name)
                .unwrap_or_else(|_| builtin.id.to_string());
            themes.push(ThemeInfo {
                id: builtin.id.to_string(),
                name,
                source: ThemeSource::Builtin,
            });
        }
    }

    themes
}

/// Extract theme name from YAML file without full parsing
fn extract_theme_name(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // Quick extraction - look for "name:" line
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("name:") {
            let value = trimmed.strip_prefix("name:")?.trim();
            // Remove quotes if present
            let value = value.trim_matches('"').trim_matches('\'');
            return Some(value.to_string());
        }
    }
    None
}

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
    pub input_background: Option<String>,
    #[serde(default)]
    pub selection_background: Option<String>,
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
    pub tab_bar: TabBarTheme,
    pub splitter: SplitterTheme,
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
    /// Input field background color
    pub input_background: Color,
    /// Selection/highlight background color in overlay lists
    pub selection_background: Color,
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
            background: Color::rgba(0x2B, 0x2D, 0x30, 0xFF),
            foreground: Color::rgb(0xE0, 0xE0, 0xE0),
            input_background: Color::rgb(0x1E, 0x1E, 0x1E),
            selection_background: Color::rgba(0x26, 0x4F, 0x78, 0xFF),
            highlight: Color::rgb(0x80, 0xFF, 0x80),
            warning: Color::rgb(0xFF, 0xFF, 0x80),
            error: Color::rgb(0xFF, 0x80, 0x80),
        }
    }
}

/// Tab bar colors (resolved)
#[derive(Debug, Clone)]
pub struct TabBarTheme {
    /// Background of the tab bar strip
    pub background: Color,
    /// Background of the active tab
    pub active_background: Color,
    /// Foreground (text) of the active tab
    pub active_foreground: Color,
    /// Background of inactive tabs
    pub inactive_background: Color,
    /// Foreground (text) of inactive tabs
    pub inactive_foreground: Color,
    /// Border between tabs and editor
    pub border: Color,
    /// Modified indicator dot color
    pub modified_indicator: Color,
}

impl TabBarTheme {
    /// Default dark tab bar theme
    pub fn default_dark() -> Self {
        Self {
            background: Color::rgb(0x25, 0x25, 0x25),
            active_background: Color::rgb(0x1E, 0x1E, 0x1E),
            active_foreground: Color::rgb(0xFF, 0xFF, 0xFF),
            inactive_background: Color::rgb(0x2D, 0x2D, 0x2D),
            inactive_foreground: Color::rgb(0x80, 0x80, 0x80),
            border: Color::rgb(0x3C, 0x3C, 0x3C),
            modified_indicator: Color::rgb(0xFF, 0xFF, 0xFF),
        }
    }
}

/// Splitter bar colors (resolved)
#[derive(Debug, Clone)]
pub struct SplitterTheme {
    /// Default background color
    pub background: Color,
    /// Color when hovered
    pub hover: Color,
    /// Color when actively being dragged
    pub active: Color,
}

impl SplitterTheme {
    /// Default dark splitter theme
    pub fn default_dark() -> Self {
        Self {
            background: Color::rgb(0x25, 0x25, 0x25),
            hover: Color::rgb(0x00, 0x7A, 0xCC),
            active: Color::rgb(0x00, 0x7A, 0xCC),
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
                    input_background: data
                        .ui
                        .overlay
                        .input_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.input_background),
                    selection_background: data
                        .ui
                        .overlay
                        .selection_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.selection_background),
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
            // Use defaults for tab_bar and splitter (not in YAML yet)
            tab_bar: TabBarTheme::default_dark(),
            splitter: SplitterTheme::default_dark(),
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
                    tab_bar: TabBarTheme::default_dark(),
                    splitter: SplitterTheme::default_dark(),
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
