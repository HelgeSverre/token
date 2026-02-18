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

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
            cursor_blink_ms: default_cursor_blink_ms(),
            auto_surround: true,
            bracket_matching: true,
        }
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
