//! Centralized configuration paths for token-editor
//!
//! All config files live under one directory, resolved in this order:
//! 1. `$TOKEN_CONFIG_DIR`, when set and non-empty.
//! 2. Portable mode: a `data` directory beside the executable.
//! 3. Unix/macOS: `~/.config/token-editor/`; Windows: `%APPDATA%\token-editor\`.
//!
//! This module is the single source of truth for config paths.

use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::OnceLock,
};

const APP_DIR: &str = "token-editor";
/// Environment variable naming an explicit configuration directory.
pub const CONFIG_DIR_ENV: &str = "TOKEN_CONFIG_DIR";
/// Directory beside the executable that switches on portable mode.
pub const PORTABLE_DIR: &str = "data";

/// Base config directory for token-editor.
///
/// `$TOKEN_CONFIG_DIR` wins when set. Otherwise a `data` directory next to the
/// executable makes the install portable: configuration, recent files, logs and
/// session state all stay beside the binary, as asked for Windows installs
/// that run from a USB stick or a shared folder. Without either, the platform
/// location applies:
///
/// Unix/macOS:
///   - If XDG_CONFIG_HOME is set: `$XDG_CONFIG_HOME/token-editor`
///   - Else: `~/.config/token-editor`
///
/// Windows:
///   - `%APPDATA%\token-editor`
pub fn config_dir() -> Option<PathBuf> {
    resolve_config_dir(
        env::var_os(CONFIG_DIR_ENV),
        portable_dir(),
        platform_config_dir(),
    )
}

/// Whether configuration currently resolves to the portable `data` directory.
pub fn is_portable() -> bool {
    env::var_os(CONFIG_DIR_ENV).is_none_or(|value| value.is_empty()) && portable_dir().is_some()
}

fn resolve_config_dir(
    explicit: Option<OsString>,
    portable: Option<&Path>,
    platform: Option<PathBuf>,
) -> Option<PathBuf> {
    if let Some(explicit) = explicit.filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(explicit));
    }
    portable.map(Path::to_path_buf).or(platform)
}

/// The `data` directory beside the running executable, if it exists. The
/// executable does not move while the process runs, so probe once.
fn portable_dir() -> Option<&'static Path> {
    static PORTABLE: OnceLock<Option<PathBuf>> = OnceLock::new();
    PORTABLE
        .get_or_init(|| {
            let exe = env::current_exe().ok()?;
            let dir = exe.parent()?.join(PORTABLE_DIR);
            dir.is_dir().then_some(dir)
        })
        .as_deref()
}

fn platform_config_dir() -> Option<PathBuf> {
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

/// `~/.config/token-editor/command-history.json`
pub fn command_history_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("command-history.json"))
}

/// `~/.config/token-editor/logs/`
pub fn logs_dir() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("logs"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_directory_wins_over_portable_and_platform() {
        let resolved = resolve_config_dir(
            Some(OsString::from("/explicit")),
            Some(Path::new("/exe/data")),
            Some(PathBuf::from("/home/.config/token-editor")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/explicit")));
    }

    #[test]
    fn empty_explicit_directory_is_ignored() {
        let resolved = resolve_config_dir(
            Some(OsString::new()),
            Some(Path::new("/exe/data")),
            Some(PathBuf::from("/home/.config/token-editor")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/exe/data")));
    }

    #[test]
    fn portable_directory_wins_over_platform() {
        let resolved = resolve_config_dir(
            None,
            Some(Path::new("/exe/data")),
            Some(PathBuf::from("/home/.config/token-editor")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/exe/data")));
    }

    #[test]
    fn platform_directory_is_the_fallback() {
        let resolved = resolve_config_dir(
            None,
            None,
            Some(PathBuf::from("/home/.config/token-editor")),
        );
        assert_eq!(resolved, Some(PathBuf::from("/home/.config/token-editor")));
        assert_eq!(resolve_config_dir(None, None, None), None);
    }

    #[test]
    fn test_binary_is_not_portable() {
        // `cargo test` binaries live in `target/.../deps`, which has no `data`
        // directory, so the probe must leave ordinary tests on platform paths.
        assert!(portable_dir().is_none());
    }
}
