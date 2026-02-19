//! Centralized configuration paths for token-editor
//!
//! All config files live under:
//! - Unix/macOS: `~/.config/token-editor/`
//! - Windows: `%APPDATA%\token-editor\`
//!
//! This module is the single source of truth for config paths.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

const APP_DIR: &str = "token-editor";

/// Base config directory for token-editor
///
/// Unix/macOS:
///   - If XDG_CONFIG_HOME is set: `$XDG_CONFIG_HOME/token-editor`
///   - Else: `~/.config/token-editor`
///
/// Windows:
///   - `%APPDATA%\token-editor`
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join(APP_DIR))
    }

    #[cfg(not(target_os = "windows"))]
    {
        env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".config")))
            .map(|config| config.join(APP_DIR))
    }
}

/// `~/.config/token-editor/themes/`
pub fn themes_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("themes"))
}

/// `~/.config/token-editor/config.yaml`
pub fn config_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.yaml"))
}

/// `~/.config/token-editor/keymap.yaml`
pub fn keymap_file() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("keymap.yaml"))
}

/// `~/.config/token-editor/recent.json`
pub fn recent_files_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("recent.json"))
}

/// `~/.config/token-editor/logs/`
pub fn logs_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("logs"))
}

/// Returns the most recent log file in `~/.config/token-editor/logs/`
/// (e.g., `token.log.2026-01-07`)
///
/// The logging system uses daily rotation, creating files like `token.log.YYYY-MM-DD`.
/// This function scans the logs directory and returns the newest file.
pub fn log_file() -> Option<PathBuf> {
    let logs_dir = logs_dir()?;

    // Try to find the most recent token.log.YYYY-MM-DD file
    let mut log_files: Vec<PathBuf> = fs::read_dir(&logs_dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("token.log"))
                .unwrap_or(false)
        })
        .collect();

    // Sort by filename in descending order (newest first)
    // YYYY-MM-DD format sorts naturally
    log_files.sort_by(|a, b| b.cmp(a));

    // Return the most recent log file, or fallback to token.log if none exist
    log_files
        .into_iter()
        .next()
        .or_else(|| Some(logs_dir.join("token.log")))
}

fn ensure_dir(path: &Path) -> Result<(), String> {
    fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create directory {}: {}", path.display(), e))
}

/// Ensure the base config dir exists, returning it
pub fn ensure_config_dir() -> Result<PathBuf, String> {
    let dir = config_dir().ok_or_else(|| "No config directory available".to_string())?;
    ensure_dir(&dir)?;
    Ok(dir)
}

/// Ensure themes dir exists, returning it
pub fn ensure_themes_dir() -> Result<PathBuf, String> {
    let config = ensure_config_dir()?;
    let themes = config.join("themes");
    ensure_dir(&themes)?;
    Ok(themes)
}

/// Ensure logs dir exists, returning it
pub fn ensure_logs_dir() -> Result<PathBuf, String> {
    let config = ensure_config_dir()?;
    let logs = config.join("logs");
    ensure_dir(&logs)?;
    Ok(logs)
}

/// Ensure full config structure (config dir + themes)
pub fn ensure_all_config_dirs() {
    match ensure_themes_dir() {
        Ok(themes) => {
            tracing::info!(
                "Config directories ready (themes dir: {})",
                themes.display()
            );
        }
        Err(e) => {
            tracing::warn!("Failed to ensure config directories: {}", e);
        }
    }
}
