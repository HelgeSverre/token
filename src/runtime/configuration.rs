//! Runtime-only preparation for configuration-opening actions.
//!
//! The ordered file worker prepares these resources before loading a tab or
//! returning a directory for the runtime to reveal. No environment or filesystem
//! access belongs in action translation or the request handler.

use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use token::commands::ConfigResource;

/// Startup configuration and histories are effects, not model constructors.
/// Called on the application preparation thread before any user input exists.
pub(super) fn load_startup(model: &mut token::AppModel) {
    token::config_paths::ensure_all_config_dirs();
    model.config = token::config::EditorConfig::load();
    model.theme = token::theme::load_theme(&model.config.theme).unwrap_or_else(|error| {
        tracing::warn!(
            "Failed to load theme '{}': {error}, using default",
            model.config.theme
        );
        token::theme::Theme::default()
    });
    model.recent_files = token::recent_files::RecentFiles::load();
    model.command_history = token::command_history::CommandHistory::load();
}

pub(super) fn prepare_resource(
    resource: ConfigResource,
    config_dir: Option<&Path>,
) -> Result<PathBuf> {
    let config_dir = config_dir.context("No configuration directory available")?;
    match resource {
        ConfigResource::Directory => {
            let themes = config_dir.join("themes");
            fs::create_dir_all(&themes)
                .with_context(|| format!("Creating {}", themes.display()))?;
            Ok(config_dir.to_path_buf())
        }
        ConfigResource::EditorSettings | ConfigResource::Keybindings => {
            fs::create_dir_all(config_dir)
                .with_context(|| format!("Creating {}", config_dir.display()))?;
            let (filename, contents) = if resource == ConfigResource::EditorSettings {
                (
                    "config.yaml",
                    serde_yaml::to_string(&token::config::EditorConfig::default())?,
                )
            } else {
                (
                    "keymap.yaml",
                    token::keymap::get_default_keymap_yaml().to_owned(),
                )
            };
            let path = config_dir.join(filename);
            // Do not check exists() then truncate: another process may create
            // the user's settings between those operations. Existing files (even
            // empty ones) are user-owned and must never be replaced here.
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(mut file) => file
                    .write_all(contents.as_bytes())
                    .with_context(|| format!("Writing {}", path.display()))?,
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    anyhow::ensure!(path.is_file(), "{} is not a file", path.display());
                }
                Err(error) => {
                    return Err(error).with_context(|| format!("Creating {}", path.display()))
                }
            }
            Ok(path)
        }
        ConfigResource::Log => {
            let logs = config_dir.join("logs");
            fs::create_dir_all(&logs).with_context(|| format!("Creating {}", logs.display()))?;
            latest_log_file(&logs)?.context("No log file is available yet")
        }
        ConfigResource::InlineStatistics => super::inline_statistics::update(config_dir, None)
            .context("Could not open inline completion statistics"),
    }
}

/// Daily rotation uses token.log.YYYY-MM-DD; a bare token.log is the fallback.
/// Ignore backups, similarly named files and directories. Selecting a maximum
/// needs neither a sorted list nor per-candidate path clones.
fn latest_log_file(logs: &Path) -> Result<Option<PathBuf>> {
    let mut latest = None;
    for entry in fs::read_dir(logs).with_context(|| format!("Reading {}", logs.display()))? {
        let entry = entry.with_context(|| format!("Reading an entry in {}", logs.display()))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if !is_log_name(name) {
            continue;
        }
        let path = entry.path();
        if !fs::metadata(&path)
            .with_context(|| format!("Inspecting {}", path.display()))?
            .is_file()
        {
            continue;
        }
        if latest.as_ref().is_none_or(|current| path > *current) {
            latest = Some(path);
        }
    }
    Ok(latest)
}

fn is_log_name(name: &str) -> bool {
    name == "token.log"
        || name.strip_prefix("token.log.").is_some_and(|date| {
            date.len() == 10
                && date.bytes().enumerate().all(|(index, byte)| match index {
                    4 | 7 => byte == b'-',
                    _ => byte.is_ascii_digit(),
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_editor_settings_creates_defaults_without_replacing_existing_text() {
        let dir = tempfile::tempdir().unwrap();
        let resource = ConfigResource::EditorSettings;
        let path = prepare_resource(resource, Some(dir.path())).unwrap();
        let config: token::config::EditorConfig =
            serde_yaml::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(config.lsp.enabled);
        assert!(!config.lsp.inlay_hints);
        for content in ["# keep my comments\nlsp:\n  enabled: false\n", ""] {
            fs::write(&path, content).unwrap();
            prepare_resource(resource, Some(dir.path())).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[test]
    fn config_resource_keymap_creates_defaults_and_preserves_existing_files() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("nested/config");
        let path = prepare_resource(ConfigResource::Keybindings, Some(&config)).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            token::keymap::get_default_keymap_yaml()
        );
        for content in ["bindings: [] # user-owned\n", ""] {
            fs::write(&path, content).unwrap();
            assert_eq!(
                prepare_resource(ConfigResource::Keybindings, Some(&config)).unwrap(),
                path
            );
            assert_eq!(fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[cfg(unix)]
    #[test]
    fn config_resource_keymap_preserves_symlink_targets() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("custom-bindings.yaml");
        let path = dir.path().join("keymap.yaml");
        fs::write(&target, "# custom target").unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert_eq!(
            prepare_resource(ConfigResource::Keybindings, Some(dir.path())).unwrap(),
            path
        );
        assert!(fs::symlink_metadata(&path)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(fs::read_to_string(target).unwrap(), "# custom target");
    }

    #[test]
    fn config_resource_directory_prepares_themes_and_reports_failure() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        assert_eq!(
            prepare_resource(ConfigResource::Directory, Some(&config)).unwrap(),
            config
        );
        assert!(config.join("themes").is_dir());
        let file = dir.path().join("not-a-directory");
        fs::write(&file, "keep").unwrap();
        for resource in [
            ConfigResource::Directory,
            ConfigResource::Keybindings,
            ConfigResource::Log,
        ] {
            assert!(prepare_resource(resource, None).is_err());
            assert!(prepare_resource(resource, Some(&file)).is_err());
        }
        assert_eq!(fs::read_to_string(file).unwrap(), "keep");
        fs::create_dir(config.join("keymap.yaml")).unwrap();
        assert!(prepare_resource(ConfigResource::Keybindings, Some(&config)).is_err());
    }

    #[test]
    fn config_resource_log_selects_latest_file_without_creating_an_empty_log() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        assert!(prepare_resource(ConfigResource::Log, Some(&config)).is_err());
        let logs = config.join("logs");
        assert!(logs.is_dir());
        assert_eq!(fs::read_dir(&logs).unwrap().count(), 0);
        let base = logs.join("token.log");
        fs::write(&base, "base").unwrap();
        assert_eq!(
            prepare_resource(ConfigResource::Log, Some(&config)).unwrap(),
            base
        );
        for name in [
            "token.log.2026-09-05",
            "token.log.2026-09-06",
            "token.log.backup",
            "token.logger",
            "token.log.2026-09-07.bak",
        ] {
            fs::write(logs.join(name), name).unwrap();
        }
        fs::create_dir(logs.join("token.log.2026-09-08")).unwrap();
        assert_eq!(
            prepare_resource(ConfigResource::Log, Some(&config)).unwrap(),
            logs.join("token.log.2026-09-06")
        );
    }
}
