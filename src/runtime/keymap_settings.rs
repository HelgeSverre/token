//! Bounded keymap reads and optimistic, locked atomic override saves.
use std::fs::{self, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use token::keymap::preferences::{KeymapChange, KeymapSave, KeymapSnapshot, MAX_KEYMAP_BYTES};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}

fn read(path: &Path) -> io::Result<Option<String>> {
    match fs::symlink_metadata(path) {
        Ok(meta) if !meta.is_file() || meta.file_type().is_symlink() => {
            return Err(invalid(
                "Keymap settings require a regular, non-symlink file",
            ))
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(invalid("Keymap is not a regular file"));
    }
    let mut text = String::new();
    file.take(MAX_KEYMAP_BYTES.as_u64() + 1)
        .read_to_string(&mut text)?;
    if text.len() as u64 > MAX_KEYMAP_BYTES.as_u64() {
        return Err(invalid("Keymap exceeds 1 MiB"));
    }
    Ok(Some(text))
}

pub(super) fn prepare(
    config_dir: Option<&Path>,
    save: Option<&KeymapSave>,
) -> io::Result<KeymapSnapshot> {
    let config_dir = config_dir.ok_or_else(|| invalid("No configuration directory available"))?;
    let path = config_dir.join("keymap.yaml");
    let Some(save) = save else {
        return KeymapSnapshot::parse(read(&path)?).map_err(|e| invalid(e.to_string()));
    };
    fs::create_dir_all(config_dir)?;
    let lock_path = config_dir.join("keymap-settings.lock");
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .mode(0o600);
    }
    let lock = options.open(lock_path)?;
    if !lock.metadata()?.is_file() {
        return Err(invalid("Keymap lock is not a regular file"));
    }
    lock.try_lock()
        .map_err(|e| invalid(format!("Keymap is busy: {e}")))?;
    let observed = read(&path)?;
    if observed != save.expected {
        return Err(invalid(
            "Keymap changed on disk; reopen Settings before saving",
        ));
    }
    let snapshot = KeymapSnapshot::parse(observed).map_err(|e| invalid(e.to_string()))?;
    let text = match &save.change {
        KeymapChange::Base(base) => snapshot.with_base(*base),
        KeymapChange::Rebind {
            original,
            command,
            strokes,
        } => snapshot.rebind(original.as_ref(), *command, strokes),
    }
    .map_err(|e| invalid(e.to_string()))?;
    let result = KeymapSnapshot::parse(Some(text.clone())).map_err(|e| invalid(e.to_string()))?;
    let permissions = if save.expected.is_some() {
        let metadata = fs::metadata(&path)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if metadata.nlink() != 1 {
                return Err(invalid(
                    "Keymap has hard links; edit it directly to preserve file identity",
                ));
            }
        }
        if metadata.permissions().readonly() {
            return Err(invalid("Keymap is read-only"));
        }
        Some(metadata.permissions())
    } else {
        None
    };
    let mut temporary = None;
    for _ in 0..16 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let candidate = config_dir.join(format!(".keymap-{}-{sequence}.tmp", std::process::id()));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&candidate) {
            Ok(file) => {
                temporary = Some((candidate, file));
                break;
            }
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    let (temporary, mut file) =
        temporary.ok_or_else(|| invalid("Could not create keymap temporary file"))?;
    let written = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        if read(&path)? != save.expected {
            return Err(invalid("Keymap changed while saving; reopen Settings"));
        }
        if save.expected.is_none() {
            // Atomic no-clobber installation: a file created by another writer
            // since the comparison must not be replaced.
            fs::hard_link(&temporary, &path)?;
        } else {
            fs::rename(&temporary, &path)?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(&temporary);
    written?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::keymap::preferences::{parse_sequence, BaseKeymap};
    use token::keymap::{Command, Keybinding};

    fn base_save(expected: Option<String>) -> KeymapSave {
        KeymapSave {
            expected,
            change: KeymapChange::Base(BaseKeymap::Conventional),
        }
    }

    #[test]
    fn settings_keymap_worker_creates_only_overrides_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("config");
        let initial = prepare(Some(&config), None).unwrap();
        assert!(initial.source.is_none());
        let changed = prepare(Some(&config), Some(&base_save(None))).unwrap();
        assert_eq!(changed.base, BaseKeymap::Conventional);
        let value: serde_yaml::Value =
            serde_yaml::from_str(changed.source.as_ref().unwrap()).unwrap();
        assert!(value["bindings"].as_sequence().unwrap().is_empty());
        assert!(!config.join("config.yaml").exists());
        let strokes = parse_sequence("ctrl+f24 ctrl+plus").unwrap();
        let save = KeymapSave {
            expected: changed.source,
            change: KeymapChange::Rebind {
                original: None,
                command: Command::SaveFile,
                strokes: strokes.clone(),
            },
        };
        let saved = prepare(Some(&config), Some(&save)).unwrap();
        let reloaded = prepare(Some(&config), None).unwrap();
        assert_eq!(saved.source, reloaded.source);
        assert!(reloaded
            .bindings
            .contains(&Keybinding::chord(strokes, Command::SaveFile)));
        assert!(!fs::read_dir(&config).unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
    }

    #[test]
    fn settings_keymap_worker_refuses_stale_invalid_and_read_only_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keymap.yaml");
        fs::write(&path, "bindings: []\n").unwrap();
        assert!(prepare(Some(dir.path()), Some(&base_save(None))).is_err());
        let old = prepare(Some(dir.path()), None).unwrap();
        fs::write(&path, "bindings: [").unwrap();
        assert!(prepare(Some(dir.path()), None).is_err());
        assert!(prepare(Some(dir.path()), Some(&base_save(old.source))).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "bindings: [");
        fs::write(&path, "bindings: []\n").unwrap();
        let original_permissions = fs::metadata(&path).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&path, permissions).unwrap();
        assert!(prepare(
            Some(dir.path()),
            Some(&base_save(Some("bindings: []\n".into())))
        )
        .is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "bindings: []\n");
        fs::set_permissions(&path, original_permissions).unwrap();
    }

    #[test]
    fn settings_keymap_worker_bounds_reads_and_reports_missing_config_or_busy_lock() {
        assert!(prepare(None, None).is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keymap.yaml");
        let file = fs::File::create(&path).unwrap();
        file.set_len(MAX_KEYMAP_BYTES.as_u64() + 1).unwrap();
        assert!(prepare(Some(dir.path()), None).is_err());
        fs::write(&path, "bindings: []\n").unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(dir.path().join("keymap-settings.lock"))
            .unwrap();
        lock.lock().unwrap();
        let error = prepare(
            Some(dir.path()),
            Some(&base_save(Some("bindings: []\n".into()))),
        )
        .unwrap_err();
        assert!(error.to_string().contains("busy"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "bindings: []\n");
    }

    #[test]
    fn settings_keymap_worker_rejects_non_files_and_unwritable_config_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keymap.yaml");
        fs::create_dir(&path).unwrap();
        assert!(prepare(Some(dir.path()), None).is_err());
        assert!(prepare(Some(dir.path()), Some(&base_save(None))).is_err());
        let file = dir.path().join("not-a-directory");
        fs::write(&file, "unchanged").unwrap();
        assert!(prepare(Some(&file), Some(&base_save(None))).is_err());
        assert_eq!(fs::read_to_string(file).unwrap(), "unchanged");
    }

    #[cfg(unix)]
    #[test]
    fn settings_keymap_worker_preserves_permissions_and_rejects_linked_files() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("keymap.yaml");
        fs::write(&path, "bindings: []\n").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let saved = prepare(
            Some(dir.path()),
            Some(&base_save(Some("bindings: []\n".into()))),
        )
        .unwrap();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        fs::hard_link(&path, dir.path().join("hardlink")).unwrap();
        assert!(prepare(Some(dir.path()), Some(&base_save(saved.source.clone()))).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), saved.source.unwrap());
        let linked_dir = tempfile::tempdir().unwrap();
        symlink(&path, linked_dir.path().join("keymap.yaml")).unwrap();
        assert!(prepare(Some(linked_dir.path()), None).is_err());
        assert!(prepare(Some(linked_dir.path()), Some(&base_save(None))).is_err());
    }
}
