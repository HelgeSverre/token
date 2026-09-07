//! File identity snapshots resolved at I/O boundaries, never during UI lookup.

use std::path::{Path, PathBuf};

/// Original spelling and resolved file URI belong to the same open/save boundary.
/// Keeping both preserves display paths and permits filesystem-free alias lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity(std::sync::Arc<IdentityPaths>);

#[derive(Debug, PartialEq, Eq)]
struct IdentityPaths {
    source: PathBuf,
    path: PathBuf,
    uri: lsp_types::Uri,
}

impl FileIdentity {
    /// Performs filesystem/environment lookup. Call only from an I/O boundary.
    pub fn resolve(source: PathBuf) -> Self {
        let uri = crate::lsp::path_to_uri(&source);
        Self::with_uri(source, uri)
    }

    /// Builds a snapshot from a path already resolved by an I/O boundary.
    pub fn from_resolved(source: PathBuf, resolved: &Path) -> Self {
        Self(std::sync::Arc::new(IdentityPaths {
            source,
            path: resolved.to_path_buf(),
            uri: crate::lsp::resolved_path_to_uri(resolved),
        }))
    }

    fn with_uri(source: PathBuf, uri: lsp_types::Uri) -> Self {
        let path = crate::lsp::uri_to_path(&uri).unwrap_or_else(|| source.clone());
        Self(std::sync::Arc::new(IdentityPaths { source, path, uri }))
    }

    pub fn source(&self) -> &Path {
        &self.0.source
    }
    pub fn path(&self) -> &Path {
        &self.0.path
    }
    pub fn uri(&self) -> &lsp_types::Uri {
        &self.0.uri
    }

    /// A known original or canonical spelling. Unknown aliases require a worker.
    pub fn matches_path(&self, path: &Path) -> bool {
        path == self.0.source || path == self.0.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_identity_missing_parent_components_do_not_match_a_different_child() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing/../file.rs");
        let identity = FileIdentity::resolve(missing.clone());
        assert_eq!(identity.path(), missing);
        assert!(!identity.matches_path(&dir.path().join("missing/file.rs")));
    }

    #[cfg(unix)]
    #[test]
    fn file_identity_preserves_distinct_native_names() {
        use std::os::unix::ffi::OsStringExt;
        let a = PathBuf::from(std::ffi::OsString::from_vec(
            b"/fixture/file-\xff.rs".to_vec(),
        ));
        let b = PathBuf::from(std::ffi::OsString::from_vec(
            b"/fixture/file-\xfe.rs".to_vec(),
        ));
        let replacement = PathBuf::from("/fixture/file-�.rs");
        let first = FileIdentity::from_resolved(a.clone(), &a);
        let second = FileIdentity::from_resolved(b.clone(), &b);
        assert_ne!(first.uri(), second.uri());
        assert_eq!(first.path(), a);
        assert_eq!(crate::lsp::uri_to_path(first.uri()).unwrap(), a);
        assert!(!first.matches_path(&b));
        assert!(!first.matches_path(&replacement));
        assert_eq!(FileIdentity::resolve(a.clone()).path(), a);
    }
}
