//! Bounded, cross-window aggregate persistence on the ordered file worker.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use token::completion::statistics::{Statistics, UsageEvent};
use token::util::ByteSize;

const MAX_FILE: ByteSize = ByteSize::kibibytes(256);
const MAX_PROVIDER_NAME: ByteSize = ByteSize::bytes(256);
const MAX_PROVIDERS: usize = 256;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Unsupported or invalid inline statistics file",
    )
}

fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() as u64 <= MAX_PROVIDER_NAME.as_u64()
}

fn temporary_file(config_dir: &Path) -> io::Result<(PathBuf, File)> {
    // Stale files after a crash must not cause an unbounded retry loop.
    for _ in 0..16 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path = config_dir.join(format!(
            ".inline-statistics-{}-{sequence}.tmp",
            std::process::id()
        ));
        match File::create_new(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "Could not create an inline statistics temporary file",
    ))
}

/// `None` prepares the user-visible resource; an event merges one outcome.
/// A stable sidecar lock protects read/merge/atomic-replace across windows.
pub(super) fn update(config_dir: &Path, event: Option<&UsageEvent>) -> io::Result<PathBuf> {
    fs::create_dir_all(config_dir)?;
    let lock_path = config_dir.join("inline-statistics.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)?;
    let started = Instant::now();
    loop {
        match lock.try_lock() {
            Ok(()) => break,
            Err(TryLockError::WouldBlock) if started.elapsed() < Duration::from_millis(200) => {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "Inline statistics are busy",
                ))
            }
            Err(TryLockError::Error(error)) => return Err(error),
        }
    }
    let path = config_dir.join("inline-statistics.json");
    // Do not replace a user-created symlink with an unrelated regular file.
    if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(invalid());
    }
    let mut missing = false;
    let mut statistics = match File::open(&path) {
        Ok(file) => {
            let mut bytes = Vec::new();
            file.take(MAX_FILE.as_u64() + 1).read_to_end(&mut bytes)?;
            if bytes.len() as u64 > MAX_FILE.as_u64() {
                return Err(invalid());
            }
            serde_json::from_slice::<Statistics>(&bytes).map_err(|_| invalid())?
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            missing = true;
            Statistics::default()
        }
        Err(error) => return Err(error),
    };
    if statistics.version != 1
        || statistics.providers.len() > MAX_PROVIDERS
        || !statistics.providers.keys().all(|name| valid_name(name))
    {
        return Err(invalid());
    }
    if let Some(event) = event {
        if !valid_name(&event.provider)
            || (statistics.providers.len() == MAX_PROVIDERS
                && !statistics.providers.contains_key(&event.provider))
        {
            return Err(invalid());
        }
        statistics.record(event);
    }
    if missing || event.is_some() {
        let data = serde_json::to_vec_pretty(&statistics)?;
        if data.len() as u64 > MAX_FILE.as_u64() {
            return Err(invalid());
        }
        // Never truncate the old file. A failed write/rename leaves it readable.
        let (temporary, mut file) = temporary_file(config_dir)?;
        let result = file.write_all(&data).and_then(|_| file.sync_all());
        drop(file);
        let result = result.and_then(|_| fs::rename(&temporary, &path));
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result?;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::completion::statistics::{Outcome, ProviderCounts};

    fn event(provider: &str, outcome: Outcome) -> UsageEvent {
        UsageEvent {
            provider: provider.into(),
            outcome,
        }
    }

    fn read(path: &Path) -> Statistics {
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn statistics_merge_and_resource_open_preserve_existing_counts() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("config");
        let path = update(&nested, None).unwrap();
        assert_eq!(read(&path), Statistics::default());
        for outcome in [
            Outcome::Accepted,
            Outcome::Accepted,
            Outcome::Dismissed,
            Outcome::TypedThrough,
        ] {
            update(&nested, Some(&event("local", outcome))).unwrap();
        }
        update(&nested, Some(&event("other", Outcome::Dismissed))).unwrap();
        let counts = read(&path);
        assert_eq!(
            counts.providers["local"],
            ProviderCounts {
                accepted: 2,
                dismissed: 1,
                typed_through: 1
            }
        );
        assert_eq!(counts.providers["other"].dismissed, 1);
        let bytes = fs::read(&path).unwrap();
        update(&nested, None).unwrap();
        assert_eq!(fs::read(path).unwrap(), bytes);
        assert_eq!(
            fs::read_dir(nested).unwrap().count(),
            2,
            "only JSON and stable lock remain"
        );
    }

    #[test]
    fn statistics_invalid_or_newer_files_are_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("inline-statistics.json");
        let oversized = vec![b' '; MAX_FILE.as_u64() as usize + 1];
        for bytes in [
            b"".as_slice(),
            b"invalid json",
            br#"{"version":99,"providers":{}}"#,
            br#"{"version":1,"providers":{},"future":true}"#,
            br#"{"version":1,"providers":{"x":{"accepted":1}}}"#,
            oversized.as_slice(),
        ] {
            fs::write(&path, bytes).unwrap();
            for input in [None, Some(event("local", Outcome::Accepted))] {
                assert_eq!(
                    update(dir.path(), input.as_ref()).unwrap_err().kind(),
                    io::ErrorKind::InvalidData
                );
                assert_eq!(fs::read(&path).unwrap(), bytes);
            }
        }
    }

    #[test]
    fn statistics_provider_limits_preserve_old_file_but_allow_existing_provider() {
        let dir = tempfile::tempdir().unwrap();
        let path = update(dir.path(), None).unwrap();
        let full = Statistics {
            version: 1,
            providers: (0..MAX_PROVIDERS)
                .map(|i| (format!("provider-{i}"), ProviderCounts::default()))
                .collect(),
        };
        let bytes = serde_json::to_vec(&full).unwrap();
        fs::write(&path, &bytes).unwrap();
        for name in [String::new(), "é".repeat(129), "additional".into()] {
            assert!(update(dir.path(), Some(&event(&name, Outcome::Accepted))).is_err());
            assert_eq!(fs::read(&path).unwrap(), bytes);
        }
        update(dir.path(), Some(&event("provider-0", Outcome::Accepted))).unwrap();
        assert_eq!(read(&path).providers["provider-0"].accepted, 1);
    }

    #[test]
    fn statistics_independent_writers_merge_without_lost_updates() {
        let dir = tempfile::tempdir().unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let path = dir.path();
                scope.spawn(move || {
                    let deadline = Instant::now() + Duration::from_secs(5);
                    for _ in 0..8 {
                        loop {
                            match update(path, Some(&event("local", Outcome::Accepted))) {
                                Ok(_) => break,
                                // The public persistence contract permits a busy
                                // lock to time out before any update is committed.
                                // Retry only that result: this test verifies exact
                                // read/merge/write counts, not scheduler/fsync speed.
                                Err(error)
                                    if error.kind() == io::ErrorKind::WouldBlock
                                        && Instant::now() < deadline => {}
                                Err(error) => panic!("statistics writer failed: {error}"),
                            }
                        }
                    }
                });
            }
        });
        assert_eq!(
            read(&dir.path().join("inline-statistics.json")).providers["local"].accepted,
            32
        );
    }

    #[test]
    fn statistics_busy_lock_returns_an_error_and_recovers_after_release() {
        let dir = tempfile::tempdir().unwrap();
        let path = update(dir.path(), None).unwrap();
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir.path().join("inline-statistics.lock"))
            .unwrap();
        lock.try_lock().unwrap();
        assert_eq!(
            update(dir.path(), Some(&event("local", Outcome::Accepted)))
                .unwrap_err()
                .kind(),
            io::ErrorKind::WouldBlock
        );
        assert_eq!(read(&path), Statistics::default());
        drop(lock);
        update(dir.path(), Some(&event("local", Outcome::Accepted))).unwrap();
        assert_eq!(read(&path).providers["local"].accepted, 1);
    }

    #[cfg(unix)]
    #[test]
    fn statistics_user_symlinks_are_preserved() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("custom.json");
        let bytes = serde_json::to_vec(&Statistics::default()).unwrap();
        fs::write(&target, &bytes).unwrap();
        let path = dir.path().join("inline-statistics.json");
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert!(update(dir.path(), Some(&event("local", Outcome::Accepted))).is_err());
        assert_eq!(fs::read_link(path).unwrap(), target);
        assert_eq!(fs::read(target).unwrap(), bytes);
    }
}
