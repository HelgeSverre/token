//! Directory-scoped reads shared by preview resources and preview link opens.
//! The authority stays attached to the read, including across worker queues.

use std::fmt;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use cap_std::fs::{Dir, OpenOptions};

use crate::util::{file_validation::MAX_FILE_SIZE, FileIdentity};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResourceError {
    Malformed,
    Forbidden,
    Missing,
    TooLarge,
    Unavailable,
    Stale,
}

impl ResourceError {
    pub fn status(self) -> u16 {
        match self {
            Self::Malformed => 400,
            Self::Forbidden => 403,
            Self::Missing => 404,
            Self::TooLarge => 413,
            Self::Unavailable => 503,
            Self::Stale => 410,
        }
    }
}

impl fmt::Display for ResourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "Invalid preview resource URL",
            Self::Forbidden => {
                "Preview resource is outside the allowed directory or is not a regular file"
            }
            Self::Missing => "Preview resource not found",
            Self::TooLarge => "Preview resource exceeds the file size limit",
            Self::Unavailable => "Preview resource is unavailable",
            Self::Stale => "Preview has changed or closed",
        })
    }
}
impl std::error::Error for ResourceError {}
impl From<std::io::Error> for ResourceError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => Self::Missing,
            std::io::ErrorKind::PermissionDenied => Self::Forbidden,
            std::io::ErrorKind::InvalidInput => Self::Malformed,
            _ => Self::Unavailable,
        }
    }
}

/// An explicit filesystem grant. Opening the directory is deferred to the worker.
#[derive(Debug)]
pub struct ResourceScope {
    root: PathBuf,
    directory: OnceLock<Result<GrantedDirectory, ResourceError>>,
    active: AtomicBool,
}

#[derive(Debug)]
struct GrantedDirectory {
    dir: Dir,
    identity_root: PathBuf,
}

impl ResourceScope {
    pub fn new(root: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            root,
            directory: OnceLock::new(),
            active: AtomicBool::new(true),
        })
    }

    pub fn revoke(&self) {
        self.active.store(false, Ordering::Release);
    }
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    fn directory(&self) -> Result<&GrantedDirectory, ResourceError> {
        self.check_active()?;
        self.directory
            .get_or_init(|| {
                // Resolve only the explicitly selected grant root, on the worker.
                // Every resource beneath it is opened through the retained handle.
                let identity_root = std::fs::canonicalize(&self.root)?;
                let dir = Dir::open_ambient_dir(&identity_root, cap_std::ambient_authority())?;
                Ok(GrantedDirectory { dir, identity_root })
            })
            .as_ref()
            .map_err(|error| *error)
    }

    fn check_active(&self) -> Result<(), ResourceError> {
        if self.is_active() {
            Ok(())
        } else {
            Err(ResourceError::Stale)
        }
    }

    pub fn file(self: &Arc<Self>, relative: PathBuf) -> Result<ScopedFile, ResourceError> {
        self.check_active()?;
        if relative.as_os_str().is_empty()
            || relative
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(ResourceError::Forbidden);
        }
        Ok(ScopedFile {
            display_path: self.root.join(&relative),
            relative,
            scope: Arc::clone(self),
        })
    }
}

/// A file name together with its directory authority and revocation lifetime.
#[derive(Debug, Clone)]
pub struct ScopedFile {
    scope: Arc<ResourceScope>,
    relative: PathBuf,
    display_path: PathBuf,
}
impl PartialEq for ScopedFile {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.scope, &other.scope) && self.relative == other.relative
    }
}
impl Eq for ScopedFile {}

#[derive(Debug)]
pub struct ScopedBytes {
    pub bytes: Vec<u8>,
    pub identity: FileIdentity,
}

impl ScopedFile {
    pub fn path(&self) -> &Path {
        &self.display_path
    }
    pub fn is_active(&self) -> bool {
        self.scope.is_active()
    }

    pub fn read(&self) -> Result<ScopedBytes, ResourceError> {
        let grant = self.scope.directory()?;
        let dir = &grant.dir;
        // Resolve aliases using metadata, never a potentially blocking open.
        // The final open still uses the capability and re-enforces containment
        // if any path component changes between resolution and opening.
        let relative = resolve_relative(dir, &self.relative)?;
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = dir.open_with(&relative, &options)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Err(ResourceError::Forbidden);
        }
        if metadata.len() > MAX_FILE_SIZE.as_u64() {
            return Err(ResourceError::TooLarge);
        }
        let bytes = read_bounded(file, MAX_FILE_SIZE.as_u64())?;
        self.scope.check_active()?;
        Ok(ScopedBytes {
            bytes,
            identity: FileIdentity::from_resolved(
                self.display_path.clone(),
                &grant.identity_root.join(relative),
            ),
        })
    }
}

/// Resolve relative symlink spellings for editor identity. `Dir::canonicalize`
/// opens the target for reading on macOS and can stall forever on a FIFO.
/// This metadata-only walk is not the authorization check: all operations,
/// including the final nonblocking open, remain relative to the capability.
fn resolve_relative(dir: &Dir, path: &Path) -> Result<PathBuf, ResourceError> {
    let mut pending = std::collections::VecDeque::from([path.to_path_buf()]);
    let mut resolved = PathBuf::new();
    let mut links = 0;
    while let Some(part) = pending.pop_front() {
        let mut components = part.components();
        while let Some(component) = components.next() {
            match component {
                Component::CurDir => continue,
                Component::ParentDir => {
                    if !resolved.pop() {
                        return Err(ResourceError::Forbidden);
                    }
                }
                Component::Normal(name) => {
                    resolved.push(name);
                    if dir.symlink_metadata(&resolved)?.file_type().is_symlink() {
                        links += 1;
                        if links > 40 {
                            return Err(ResourceError::Forbidden);
                        }
                        let target = dir.read_link(&resolved)?;
                        resolved.pop();
                        pending.push_front(components.as_path().to_path_buf());
                        pending.push_front(target);
                        break;
                    }
                }
                Component::RootDir | Component::Prefix(_) => return Err(ResourceError::Forbidden),
            }
        }
    }
    Ok(resolved)
}

fn read_bounded(reader: impl Read, limit: u64) -> Result<Vec<u8>, ResourceError> {
    let mut bytes = Vec::new();
    reader.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(ResourceError::TooLarge);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_reads_are_regular_bounded_and_revocable() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("image..png"), b"inside").unwrap();
        let scope = ResourceScope::new(root.path().into());
        let file = scope.file("image..png".into()).unwrap();
        assert_eq!(file.read().unwrap().bytes, b"inside");
        assert_eq!(
            scope.file("missing".into()).unwrap().read().unwrap_err(),
            ResourceError::Missing
        );
        std::fs::create_dir(root.path().join("directory")).unwrap();
        assert_eq!(
            scope.file("directory".into()).unwrap().read().unwrap_err(),
            ResourceError::Forbidden
        );
        for name in ["../outside", "/absolute", "a/../b", ""] {
            assert!(scope.file(name.into()).is_err(), "{name}");
        }
        let large = std::fs::File::create(root.path().join("large")).unwrap();
        large.set_len(MAX_FILE_SIZE.as_u64() + 1).unwrap();
        assert_eq!(
            scope.file("large".into()).unwrap().read().unwrap_err(),
            ResourceError::TooLarge
        );
        scope.revoke();
        assert_eq!(file.read().unwrap_err(), ResourceError::Stale);
        assert!(scope.file("image..png".into()).is_err());
    }

    #[test]
    fn bounded_reader_rejects_growth_after_metadata_check() {
        let limit = crate::util::ByteSize::kibibytes(1).as_u64();
        assert_eq!(
            read_bounded(std::io::repeat(1), limit),
            Err(ResourceError::TooLarge)
        );
        assert_eq!(read_bounded(&b"ok"[..], limit).unwrap(), b"ok");
    }

    #[cfg(unix)]
    #[test]
    fn capability_follows_only_relative_symlinks_inside_root() {
        use std::os::unix::fs::symlink;
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("root");
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::write(root.join("asset"), b"inside").unwrap();
        std::fs::write(outer.path().join("outside"), b"outside").unwrap();
        symlink("../asset", root.join("nested/allowed")).unwrap();
        symlink("../outside", root.join("escape")).unwrap();
        symlink(root.join("asset"), root.join("absolute")).unwrap();
        symlink("loop", root.join("loop")).unwrap();
        let scope = ResourceScope::new(root.clone());
        assert_eq!(
            scope
                .file("nested/allowed".into())
                .unwrap()
                .read()
                .unwrap()
                .bytes,
            b"inside"
        );
        for name in ["escape", "absolute", "loop"] {
            assert!(scope.file(name.into()).unwrap().read().is_err(), "{name}");
        }
        let status = std::process::Command::new("mkfifo")
            .arg(root.join("fifo"))
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(
            scope.file("fifo".into()).unwrap().read().unwrap_err(),
            ResourceError::Forbidden
        );
    }

    #[cfg(unix)]
    #[test]
    fn replacement_during_scoped_reads_never_returns_outside_bytes() {
        use std::os::unix::fs::symlink;
        let outer = tempfile::tempdir().unwrap();
        let root = outer.path().join("root");
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("inside"), b"inside").unwrap();
        std::fs::write(outer.path().join("outside"), b"outside").unwrap();
        symlink("inside", root.join("changing")).unwrap();
        let scope = ResourceScope::new(root.clone());
        let file = scope.file("changing".into()).unwrap();
        // Initialize the grant, then replace a path while canonicalize/open run.
        assert_eq!(file.read().unwrap().bytes, b"inside");
        std::thread::scope(|threads| {
            threads.spawn(|| {
                for index in 0..500 {
                    if index % 2 == 0 {
                        symlink("../outside", root.join("next")).unwrap();
                    } else {
                        std::fs::write(root.join("next"), b"inside").unwrap();
                    }
                    // Replace the resolved target itself, so the final open
                    // must re-enforce containment after alias resolution.
                    std::fs::rename(root.join("next"), root.join("inside")).unwrap();
                }
            });
            for _ in 0..500 {
                if let Ok(read) = file.read() {
                    assert_eq!(read.bytes, b"inside");
                }
            }
        });
    }
}
