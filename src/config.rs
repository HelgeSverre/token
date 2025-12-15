//! Editor configuration persistence
//!
//! Stores user preferences in `~/.config/token-editor/config.yaml`

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::theme::get_config_dir;

/// Editor configuration that persists across sessions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorConfig {
    /// Selected theme id (e.g., "default-dark", "fleet-dark")
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "default-dark".to_string()
}

impl Default for EditorConfig {
    fn default() -> Self {
        Self {
            theme: default_theme(),
        }
    }
}

impl EditorConfig {
    /// Get the config file path
    ///
    /// Returns `~/.config/token-editor/config.yaml` on Unix
    /// Returns `%APPDATA%\token-editor\config.yaml` on Windows
    pub fn config_path() -> Option<PathBuf> {
        get_config_dir().map(|dir| dir.join("config.yaml"))
    }

    /// Load config from disk, or return defaults if not found
    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
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

    /// Save config to disk
    ///
    /// Creates the config directory if it doesn't exist.
    pub fn save(&self) -> Result<(), String> {
        let path =
            Self::config_path().ok_or_else(|| "No config directory available".to_string())?;

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

    /// Ensure config directories exist
    ///
    /// Creates `~/.config/token-editor/` and `~/.config/token-editor/themes/` if missing.
    /// Called on startup to prepare the config structure.
    pub fn ensure_config_dirs() {
        if let Some(config_dir) = get_config_dir() {
            if !config_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&config_dir) {
                    tracing::warn!("Failed to create config directory: {}", e);
                } else {
                    tracing::info!("Created config directory: {}", config_dir.display());
                }
            }

            // Also create themes subdirectory
            let themes_dir = config_dir.join("themes");
            if !themes_dir.exists() {
                if let Err(e) = std::fs::create_dir_all(&themes_dir) {
                    tracing::warn!("Failed to create themes directory: {}", e);
                } else {
                    tracing::info!("Created themes directory: {}", themes_dir.display());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = EditorConfig::default();
        assert_eq!(config.theme, "default-dark");
    }

    #[test]
    fn test_config_path_returns_some() {
        // On most systems, this should return a valid path
        let path = EditorConfig::config_path();
        if let Some(p) = path {
            let path_str = p.to_string_lossy();
            assert!(path_str.contains("token-editor"));
            assert!(path_str.contains("config.yaml"));
        }
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = EditorConfig {
            theme: "fleet-dark".to_string(),
        };
        let yaml = serde_yaml::to_string(&config).unwrap();
        let parsed: EditorConfig = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.theme, "fleet-dark");
    }
}
