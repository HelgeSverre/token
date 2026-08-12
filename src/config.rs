//! Editor configuration persistence
//!
//! Stores user preferences in `~/.config/token-editor/config.yaml`

use serde::{Deserialize, Serialize};

/// Result of reloading configuration
#[derive(Debug, Clone, PartialEq)]
pub enum ReloadResult {
    /// Successfully loaded from file
    Loaded,
    /// File doesn't exist, using defaults
    FileNotFound,
    /// Parse error, using defaults
    ParseError(String),
    /// Read error (locked/no access), using defaults
    ReadError(String),
    /// No config directory available
    NoConfigDir,
}

/// Editor configuration that persists across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Selected theme id (e.g., "default-dark", "fleet-dark")
    #[serde(default = "default_theme")]
    pub theme: String,

    /// Cursor blink interval in milliseconds (default: 600)
    #[serde(default = "default_cursor_blink_ms")]
    pub cursor_blink_ms: u64,

    /// Automatically surround selected text when typing brackets/quotes (default: true)
    #[serde(default = "default_true")]
    pub auto_surround: bool,

    /// Highlight matching bracket when cursor is adjacent to one (default: true)
    #[serde(default = "default_true")]
    pub bracket_matching: bool,

    /// Show scrollbars in editor panes (default: true)
    ///
    /// When false, no scrollbars are rendered and no space is reserved for them.
    #[serde(default = "default_true")]
    pub show_scrollbar: bool,

    /// Status bar font size in logical px (default: 12, editor text is 14)
    #[serde(default = "default_status_bar_font_size")]
    pub status_bar_font_size: f32,

    /// Language server settings (see `LspConfig`).
    #[serde(default)]
    pub lsp: LspConfig,
}

/// Language server settings, stored under `lsp:` in `config.yaml`:
///
/// ```yaml
/// lsp:
///   enabled: true
///   servers:
///     rust-analyzer:
///       command: /custom/path/rust-analyzer
///     pyright:
///       enabled: false
/// ```
///
/// Overrides are keyed by `LspServerDef::id`, not by language — to run
/// `laravel-lsp` instead of the default `phpantom`, override the
/// `phpantom` entry's `command`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspConfig {
    /// Master switch; `false` disables every server regardless of
    /// per-server settings.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub servers: std::collections::HashMap<String, LspServerOverride>,
}

impl Default for LspConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            servers: std::collections::HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LspServerOverride {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

fn default_theme() -> String {
    "default-dark".to_string()
}

fn default_cursor_blink_ms() -> u64 {
    600
}

fn default_true() -> bool {
    true
}

fn default_status_bar_font_size() -> f32 {
    12.0
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            cursor_blink_ms: default_cursor_blink_ms(),
            auto_surround: true,
            bracket_matching: true,
            show_scrollbar: true,
            status_bar_font_size: default_status_bar_font_size(),
            lsp: LspConfig::default(),
        }
    }
}

impl EditorConfig {
    /// Status bar font size clamped to a usable range, in logical px.
    pub fn status_bar_font_size_clamped(&self) -> f32 {
        self.status_bar_font_size.clamp(8.0, 24.0)
    }
}

impl EditorConfig {
    /// Load config from disk, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = crate::config_paths::config_file() else {
            tracing::debug!("No config directory available, using defaults");
            return Self::default();
        };

        if !path.exists() {
            tracing::debug!(
                "Config file not found at {}, using defaults",
                path.display()
            );
            return Self::default();
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(config) => {
                    tracing::info!("Loaded config from {}", path.display());
                    config
                }
                Err(e) => {
                    tracing::warn!("Failed to parse config at {}: {}", path.display(), e);
                    Self::default()
                }
            },
            Err(e) => {
                tracing::warn!("Failed to read config at {}: {}", path.display(), e);
                Self::default()
            }
        }
    }

    /// Reload config from disk with detailed status for user feedback
    pub fn reload() -> (Self, ReloadResult) {
        let Some(path) = crate::config_paths::config_file() else {
            return (Self::default(), ReloadResult::NoConfigDir);
        };

        if !path.exists() {
            return (Self::default(), ReloadResult::FileNotFound);
        }

        match std::fs::read_to_string(&path) {
            Ok(content) => match serde_yaml::from_str(&content) {
                Ok(config) => (config, ReloadResult::Loaded),
                Err(e) => (Self::default(), ReloadResult::ParseError(e.to_string())),
            },
            Err(e) => (Self::default(), ReloadResult::ReadError(e.to_string())),
        }
    }

    /// Save config to disk
    ///
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self) -> Result<(), String> {
        let path = crate::config_paths::config_file()
            .ok_or_else(|| "No config directory available".to_string())?;

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config directory: {}", e))?;
        }

        let content = serde_yaml::to_string(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        std::fs::write(&path, content)
            .map_err(|e| format!("Failed to write config to {}: {}", path.display(), e))?;

        tracing::info!("Saved config to {}", path.display());
        Ok(())
    }

    /// Update theme and save
    pub fn set_theme(&mut self, theme_id: &str) -> Result<(), String> {
        self.theme = theme_id.to_string();
        self.save()
    }
}
