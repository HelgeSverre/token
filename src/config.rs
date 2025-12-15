//! Editor configuration persistence
//!
//! Stores user preferences in `~/.config/token-editor/config.yaml`

use serde::{Deserialize, Serialize};

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
