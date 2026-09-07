//! Canonical path <-> `file://` URI conversion.
//!
//! FileIdentity uses this conversion at I/O boundaries. UI consumers compare
//! its known original/resolved spellings without calling the filesystem again;
//! previously unknown aliases still require boundary resolution.

use std::path::{Path, PathBuf};

use lsp_types::Uri;

/// Converts a filesystem path to a canonical `file://` URI.
///
/// Canonicalizes via `std::fs::canonicalize` when the file exists,
/// resolving symlinks (e.g. macOS `/tmp` -> `/private/tmp`) so the
/// server and the editor always agree on one identity for a file. Falls
/// back to an absolute (but non-canonicalized) path for files that
/// don't exist yet.
pub fn path_to_uri(path: &Path) -> Uri {
    let absolute = std::fs::canonicalize(path).unwrap_or_else(|_| make_absolute(path));
    resolved_path_to_uri(&absolute)
}

/// Pure encoding for a path already resolved at an I/O boundary.
pub(crate) fn resolved_path_to_uri(absolute: &Path) -> Uri {
    let mut uri = String::from("file://");
    for component in absolute.components() {
        match component {
            std::path::Component::Prefix(prefix) => push_windows_prefix(&mut uri, prefix),
            std::path::Component::RootDir => {
                if !uri.ends_with('/') {
                    uri.push('/');
                }
            }
            std::path::Component::Normal(segment) => {
                uri.push('/');
                percent_encode_segment(segment.as_encoded_bytes(), &mut uri);
            }
            // A missing path can retain parents after make_absolute. Dropping
            // them would identify a different child (a/../b must not become a/b).
            std::path::Component::ParentDir => uri.push_str("/.."),
            std::path::Component::CurDir => {}
        }
    }

    if uri == "file://" {
        uri.push('/');
    }

    uri.parse()
        .unwrap_or_else(|e| panic!("percent-encoded path formed an invalid URI {uri:?}: {e}"))
}

/// Converts a `file://` URI back to a filesystem path.
///
/// Returns `None` for non-`file` schemes or a body that isn't valid
/// percent encoding or contains NUL. Unix filenames preserve arbitrary bytes;
/// other platforms require a UTF-8 representation.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let text = uri.as_str();
    let rest = text.strip_prefix("file://")?;
    let decoded = percent_decode(rest)?;
    if decoded.contains(&0) {
        return None;
    }

    // A canonical Windows URI is `file:///C:/path`; strip the leading
    // slash in front of the drive letter so `PathBuf::from` sees `C:/path`
    // rather than treating it as a rooted-but-driveless path.
    #[cfg(windows)]
    let decoded = match decoded.as_slice() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => decoded[1..].to_vec(),
        _ => decoded,
    };

    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStringExt;
        Some(PathBuf::from(std::ffi::OsString::from_vec(decoded)))
    }
    #[cfg(not(unix))]
    {
        String::from_utf8(decoded).ok().map(PathBuf::from)
    }
}

fn make_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn push_windows_prefix(uri: &mut String, prefix: std::path::PrefixComponent<'_>) {
    use std::path::Prefix;
    match prefix.kind() {
        Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
            uri.push('/');
            uri.push(letter.to_ascii_uppercase() as char);
            uri.push(':');
        }
        _ => {
            // UNC/verbatim-server prefixes: not a target scenario, but
            // encode losslessly rather than dropping the component.
            uri.push('/');
            percent_encode_segment(prefix.as_os_str().as_encoded_bytes(), uri);
        }
    }
}

/// RFC 3986 `pchar` percent-encoding: unreserved + sub-delims + `:` `@`
/// pass through untouched, everything else (including `/`, which the
/// caller inserts itself as the segment separator) is escaped.
fn percent_encode_segment(segment: &[u8], out: &mut String) {
    for &byte in segment {
        let unreserved = byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~');
        let extra = matches!(
            byte,
            b'!' | b'$'
                | b'&'
                | b'\''
                | b'('
                | b')'
                | b'*'
                | b'+'
                | b','
                | b';'
                | b'='
                | b':'
                | b'@'
        );
        if unreserved || extra {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
}

fn percent_decode(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn file_identity_uri_preserves_root_and_drive_like_unix_paths() {
        for path in ["/", "/C:/native/file.rs"] {
            let path = Path::new(path);
            assert_eq!(uri_to_path(&path_to_uri(path)).as_deref(), Some(path));
        }
        assert_eq!(path_to_uri(Path::new("/")).as_str(), "file:///");
    }

    #[test]
    fn file_identity_uri_rejects_nul_and_malformed_percent_encoding() {
        for text in ["file:///tmp/%00.rs", "file:///tmp/%FF%", "file:///tmp/%GG"] {
            if let Ok(uri) = text.parse() {
                assert!(uri_to_path(&uri).is_none());
            }
        }
    }

    #[test]
    fn round_trips_a_simple_absolute_path() {
        let path = Path::new("/usr/local/bin/rustc");
        let uri = path_to_uri(path);
        assert_eq!(uri.as_str(), "file:///usr/local/bin/rustc");
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn percent_encodes_spaces_and_reserved_characters() {
        let path = Path::new("/tmp/does not exist/a file (v2)#2.rs");
        let uri = path_to_uri(path);
        assert!(uri.as_str().contains("%20"));
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn round_trips_unicode_segments() {
        let path = Path::new("/tmp/does-not-exist/日本語/emoji-🦀.rs");
        let uri = path_to_uri(path);
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn resolves_symlinks_via_canonicalize_when_the_file_exists() {
        // /tmp exists on every CI/dev platform we target; on macOS it is
        // itself a symlink to /private/tmp, which is exactly the case
        // raw PathBuf comparison would get wrong.
        let path = Path::new("/tmp");
        if !path.exists() {
            return;
        }
        let canonical = std::fs::canonicalize(path).unwrap();

        let uri = path_to_uri(path);
        let round_tripped = uri_to_path(&uri).unwrap();
        assert_eq!(round_tripped, canonical);

        #[cfg(target_os = "macos")]
        assert!(
            canonical.starts_with("/private"),
            "expected macOS to resolve /tmp through /private, got {canonical:?}"
        );
    }

    #[test]
    fn round_trips_through_a_real_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("sample.rs");
        std::fs::write(&file_path, b"fn main() {}").unwrap();

        let canonical = std::fs::canonicalize(&file_path).unwrap();
        let uri = path_to_uri(&file_path);
        assert_eq!(uri_to_path(&uri).unwrap(), canonical);
    }

    #[test]
    fn falls_back_to_absolute_path_for_a_file_that_does_not_exist_yet() {
        let path = Path::new("/tmp/token-lsp-uri-test-does-not-exist/new_file.rs");
        let uri = path_to_uri(path);
        assert_eq!(uri_to_path(&uri).unwrap(), path);
    }

    #[test]
    fn uri_to_path_rejects_non_file_scheme() {
        let uri: Uri = "https://example.com/foo".parse().unwrap();
        assert_eq!(uri_to_path(&uri), None);
    }
}
