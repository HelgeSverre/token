//! Theme system for the editor
//!
//! Provides YAML-based theming support with compile-time embedded themes
//! and user-defined themes from config directories.
//!
//! Theme loading priority:
//! 1. User config: `~/.config/token-editor/themes/{id}.yaml`
//! 2. Embedded: Built-in themes compiled into binary

use std::path::Path;

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
    if let Some(user_dir) = crate::config_paths::themes_dir() {
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
    if let Some(user_dir) = crate::config_paths::themes_dir() {
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
    #[serde(default)]
    pub sidebar: SidebarThemeData,
    #[serde(default)]
    pub tab_bar: TabBarThemeData,
    #[serde(default)]
    pub syntax: SyntaxThemeData,
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

/// Sidebar theme colors (all optional for backward compatibility)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SidebarThemeData {
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub foreground: Option<String>,
    #[serde(default)]
    pub selection_background: Option<String>,
    #[serde(default)]
    pub selection_foreground: Option<String>,
    #[serde(default)]
    pub hover_background: Option<String>,
    #[serde(default)]
    pub folder_icon: Option<String>,
    #[serde(default)]
    pub file_icon: Option<String>,
    #[serde(default)]
    pub border: Option<String>,
}

/// Tab bar colors (all optional for backward compatibility)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct TabBarThemeData {
    #[serde(default)]
    pub background: Option<String>,
    #[serde(default)]
    pub active_background: Option<String>,
    #[serde(default)]
    pub active_foreground: Option<String>,
    #[serde(default)]
    pub inactive_background: Option<String>,
    #[serde(default)]
    pub inactive_foreground: Option<String>,
    #[serde(default)]
    pub border: Option<String>,
    #[serde(default)]
    pub modified_indicator: Option<String>,
}

/// CSV mode colors (all optional for backward compatibility)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct CsvThemeData {
    #[serde(default)]
    pub header_background: Option<String>,
    #[serde(default)]
    pub header_foreground: Option<String>,
    #[serde(default)]
    pub grid_line: Option<String>,
    #[serde(default)]
    pub selected_cell_background: Option<String>,
    #[serde(default)]
    pub selected_cell_border: Option<String>,
    #[serde(default)]
    pub number_foreground: Option<String>,
}

/// Syntax highlighting colors (all optional for backward compatibility)
#[derive(Debug, Clone, Deserialize, Default)]
pub struct SyntaxThemeData {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub function: Option<String>,
    #[serde(default)]
    pub function_builtin: Option<String>,
    #[serde(default)]
    pub string: Option<String>,
    #[serde(default)]
    pub number: Option<String>,
    #[serde(default)]
    pub comment: Option<String>,
    #[serde(default, rename = "type")]
    pub type_name: Option<String>,
    #[serde(default)]
    pub variable: Option<String>,
    #[serde(default)]
    pub variable_builtin: Option<String>,
    #[serde(default)]
    pub property: Option<String>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub punctuation: Option<String>,
    #[serde(default)]
    pub constant: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub attribute: Option<String>,
    #[serde(default)]
    pub escape: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub text_emphasis: Option<String>,
    #[serde(default)]
    pub text_strong: Option<String>,
    #[serde(default)]
    pub text_title: Option<String>,
    #[serde(default)]
    pub text_uri: Option<String>,
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
    pub sidebar: SidebarTheme,
    pub csv: CsvTheme,
    pub syntax: SyntaxTheme,
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

/// Sidebar / file tree colors (resolved)
#[derive(Debug, Clone)]
pub struct SidebarTheme {
    /// Sidebar background color
    pub background: Color,
    /// Default text color
    pub foreground: Color,
    /// Selected item background
    pub selection_background: Color,
    /// Selected item foreground
    pub selection_foreground: Color,
    /// Hover background
    pub hover_background: Color,
    /// Folder icon color
    pub folder_icon: Color,
    /// File icon color (default for unknown types)
    pub file_icon: Color,
    /// Resize border color
    pub border: Color,
}

impl SidebarTheme {
    /// Default dark sidebar theme
    pub fn default_dark() -> Self {
        Self {
            background: Color::rgb(0x21, 0x21, 0x21),
            foreground: Color::rgb(0xCC, 0xCC, 0xCC),
            selection_background: Color::rgba(0x26, 0x4F, 0x78, 0xFF),
            selection_foreground: Color::rgb(0xFF, 0xFF, 0xFF),
            hover_background: Color::rgba(0x5A, 0x5A, 0x5A, 0x40),
            folder_icon: Color::rgb(0xDC, 0xDC, 0xAA), // Yellow/gold
            file_icon: Color::rgb(0x9C, 0xDC, 0xFE),   // Light blue
            border: Color::rgb(0x3C, 0x3C, 0x3C),
        }
    }
}

/// CSV mode colors (resolved)
#[derive(Debug, Clone)]
pub struct CsvTheme {
    /// Background color for header row
    pub header_background: Color,
    /// Foreground color for header text
    pub header_foreground: Color,
    /// Color for grid lines
    pub grid_line: Color,
    /// Background color for selected cell
    pub selected_cell_background: Color,
    /// Border color for selected cell
    pub selected_cell_border: Color,
    /// Color for numeric values (right-aligned)
    pub number_foreground: Color,
}

impl CsvTheme {
    /// Default dark CSV theme (derives from gutter/editor colors)
    pub fn default_dark() -> Self {
        Self {
            header_background: Color::rgb(0x2D, 0x2D, 0x2D),
            header_foreground: Color::rgb(0xE0, 0xE0, 0xE0),
            grid_line: Color::rgb(0x40, 0x40, 0x40),
            selected_cell_background: Color::rgba(0x26, 0x4F, 0x78, 0x80),
            selected_cell_border: Color::rgb(0x00, 0x7A, 0xCC),
            number_foreground: Color::rgb(0xB5, 0xCE, 0xA8), // Same as syntax numbers
        }
    }

    /// Create CSV theme from theme data and fallback colors
    pub fn from_data(
        data: Option<&CsvThemeData>,
        gutter: &GutterTheme,
        editor: &EditorTheme,
    ) -> Self {
        let default = Self::default_dark();

        Self {
            header_background: data
                .and_then(|d| d.header_background.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(gutter.background),
            header_foreground: data
                .and_then(|d| d.header_foreground.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(gutter.foreground_active),
            grid_line: data
                .and_then(|d| d.grid_line.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(default.grid_line),
            selected_cell_background: data
                .and_then(|d| d.selected_cell_background.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(editor.selection_background),
            selected_cell_border: data
                .and_then(|d| d.selected_cell_border.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(default.selected_cell_border),
            number_foreground: data
                .and_then(|d| d.number_foreground.as_ref())
                .and_then(|s| Color::from_hex(s).ok())
                .unwrap_or(default.number_foreground),
        }
    }
}

/// Syntax highlighting colors (resolved)
#[derive(Debug, Clone)]
pub struct SyntaxTheme {
    pub keyword: Color,
    pub function: Color,
    pub function_builtin: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub type_name: Color,
    pub variable: Color,
    pub variable_builtin: Color,
    pub property: Color,
    pub operator: Color,
    pub punctuation: Color,
    pub constant: Color,
    pub tag: Color,
    pub attribute: Color,
    pub escape: Color,
    pub label: Color,
    pub text: Color,
    pub text_emphasis: Color,
    pub text_strong: Color,
    pub text_title: Color,
    pub text_uri: Color,
}

impl SyntaxTheme {
    /// Default dark syntax theme (VS Code-like)
    pub fn default_dark() -> Self {
        Self {
            keyword: Color::rgb(0xC5, 0x86, 0xC0),  // Purple/pink
            function: Color::rgb(0xDC, 0xDC, 0xAA), // Yellow
            function_builtin: Color::rgb(0xDC, 0xDC, 0xAA),
            string: Color::rgb(0xCE, 0x91, 0x78), // Orange/brown
            number: Color::rgb(0xB5, 0xCE, 0xA8), // Light green
            comment: Color::rgb(0x6A, 0x99, 0x55), // Green
            type_name: Color::rgb(0x4E, 0xC9, 0xB0), // Teal
            variable: Color::rgb(0x9C, 0xDC, 0xFE), // Light blue
            variable_builtin: Color::rgb(0x56, 0x9C, 0xD6), // Blue
            property: Color::rgb(0x9C, 0xDC, 0xFE), // Light blue
            operator: Color::rgb(0xD4, 0xD4, 0xD4), // Light gray
            punctuation: Color::rgb(0xD4, 0xD4, 0xD4), // Light gray
            constant: Color::rgb(0x56, 0x9C, 0xD6), // Blue
            tag: Color::rgb(0x56, 0x9C, 0xD6),    // Blue (HTML tags)
            attribute: Color::rgb(0x9C, 0xDC, 0xFE), // Light blue
            escape: Color::rgb(0xD7, 0xBA, 0x7D), // Gold
            label: Color::rgb(0xD7, 0xBA, 0x7D),  // Gold (anchors, labels)
            text: Color::rgb(0xD4, 0xD4, 0xD4),   // Default text
            text_emphasis: Color::rgb(0xD4, 0xD4, 0xD4),
            text_strong: Color::rgb(0xD4, 0xD4, 0xD4),
            text_title: Color::rgb(0x56, 0x9C, 0xD6), // Blue for headings
            text_uri: Color::rgb(0x3E, 0x9C, 0xD6),   // Slightly different blue
        }
    }

    /// Get color for a highlight ID
    pub fn color_for_highlight(&self, highlight_id: crate::syntax::HighlightId) -> Color {
        use crate::syntax::HIGHLIGHT_NAMES;

        let name = HIGHLIGHT_NAMES
            .get(highlight_id as usize)
            .copied()
            .unwrap_or("text");

        match name {
            "keyword" | "keyword.return" | "keyword.function" | "keyword.operator" => self.keyword,
            "function" | "function.method" => self.function,
            "function.builtin" => self.function_builtin,
            "string" | "string.special" => self.string,
            "number" => self.number,
            "comment" => self.comment,
            "type" | "type.builtin" => self.type_name,
            "variable" | "variable.parameter" => self.variable,
            "variable.builtin" => self.variable_builtin,
            "property" | "tag.attribute" => self.property,
            "operator" => self.operator,
            "punctuation"
            | "punctuation.bracket"
            | "punctuation.delimiter"
            | "punctuation.special" => self.punctuation,
            "constant" | "constant.builtin" | "boolean" => self.constant,
            "tag" => self.tag,
            "attribute" => self.attribute,
            "escape" => self.escape,
            "label" => self.label,
            "text" => self.text,
            "text.emphasis" => self.text_emphasis,
            "text.strong" => self.text_strong,
            "text.title" => self.text_title,
            "text.uri" => self.text_uri,
            "constructor" => self.type_name,
            _ => self.text, // Default fallback
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
            tab_bar: {
                let defaults = TabBarTheme::default_dark();
                TabBarTheme {
                    background: data
                        .ui
                        .tab_bar
                        .background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.background),
                    active_background: data
                        .ui
                        .tab_bar
                        .active_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.active_background),
                    active_foreground: data
                        .ui
                        .tab_bar
                        .active_foreground
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.active_foreground),
                    inactive_background: data
                        .ui
                        .tab_bar
                        .inactive_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.inactive_background),
                    inactive_foreground: data
                        .ui
                        .tab_bar
                        .inactive_foreground
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.inactive_foreground),
                    border: data
                        .ui
                        .tab_bar
                        .border
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.border),
                    modified_indicator: data
                        .ui
                        .tab_bar
                        .modified_indicator
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.modified_indicator),
                }
            },
            splitter: SplitterTheme::default_dark(),
            sidebar: {
                let defaults = SidebarTheme::default_dark();
                SidebarTheme {
                    background: data
                        .ui
                        .sidebar
                        .background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.background),
                    foreground: data
                        .ui
                        .sidebar
                        .foreground
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.foreground),
                    selection_background: data
                        .ui
                        .sidebar
                        .selection_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.selection_background),
                    selection_foreground: data
                        .ui
                        .sidebar
                        .selection_foreground
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.selection_foreground),
                    hover_background: data
                        .ui
                        .sidebar
                        .hover_background
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.hover_background),
                    folder_icon: data
                        .ui
                        .sidebar
                        .folder_icon
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.folder_icon),
                    file_icon: data
                        .ui
                        .sidebar
                        .file_icon
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.file_icon),
                    border: data
                        .ui
                        .sidebar
                        .border
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.border),
                }
            },
            csv: CsvTheme::default_dark(),
            syntax: {
                let defaults = SyntaxTheme::default_dark();
                SyntaxTheme {
                    keyword: data
                        .ui
                        .syntax
                        .keyword
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.keyword),
                    function: data
                        .ui
                        .syntax
                        .function
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.function),
                    function_builtin: data
                        .ui
                        .syntax
                        .function_builtin
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.function_builtin),
                    string: data
                        .ui
                        .syntax
                        .string
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.string),
                    number: data
                        .ui
                        .syntax
                        .number
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.number),
                    comment: data
                        .ui
                        .syntax
                        .comment
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.comment),
                    type_name: data
                        .ui
                        .syntax
                        .type_name
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.type_name),
                    variable: data
                        .ui
                        .syntax
                        .variable
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.variable),
                    variable_builtin: data
                        .ui
                        .syntax
                        .variable_builtin
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.variable_builtin),
                    property: data
                        .ui
                        .syntax
                        .property
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.property),
                    operator: data
                        .ui
                        .syntax
                        .operator
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.operator),
                    punctuation: data
                        .ui
                        .syntax
                        .punctuation
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.punctuation),
                    constant: data
                        .ui
                        .syntax
                        .constant
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.constant),
                    tag: data
                        .ui
                        .syntax
                        .tag
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.tag),
                    attribute: data
                        .ui
                        .syntax
                        .attribute
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.attribute),
                    escape: data
                        .ui
                        .syntax
                        .escape
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.escape),
                    label: data
                        .ui
                        .syntax
                        .label
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.label),
                    text: data
                        .ui
                        .syntax
                        .text
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.text),
                    text_emphasis: data
                        .ui
                        .syntax
                        .text_emphasis
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.text_emphasis),
                    text_strong: data
                        .ui
                        .syntax
                        .text_strong
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.text_strong),
                    text_title: data
                        .ui
                        .syntax
                        .text_title
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.text_title),
                    text_uri: data
                        .ui
                        .syntax
                        .text_uri
                        .as_ref()
                        .map(|s| Color::from_hex(s))
                        .transpose()?
                        .unwrap_or(defaults.text_uri),
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
                    tab_bar: TabBarTheme::default_dark(),
                    splitter: SplitterTheme::default_dark(),
                    sidebar: SidebarTheme::default_dark(),
                    csv: CsvTheme::default_dark(),
                    syntax: SyntaxTheme::default_dark(),
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
