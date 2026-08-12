//! Canonical path <-> `file://` URI conversion.
//!
//! Every LSP message that carries a document identity goes through here
//! in both directions — raw `PathBuf`s are never compared against each
//! other or against a URI's path, because two spellings of the same
//! file (a symlink vs. its target, differing drive-letter case) look
//! unequal as strings and would silently drop diagnostics/responses for
//! the "other" spelling.

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
                percent_encode_segment(&segment.to_string_lossy(), &mut uri);
            }
            // `canonicalize`/`make_absolute` never leave `.`/`..` components.
            std::path::Component::CurDir | std::path::Component::ParentDir => {}
        }
    }

    uri.parse()
        .unwrap_or_else(|e| panic!("percent-encoded path formed an invalid URI {uri:?}: {e}"))
}

/// Converts a `file://` URI back to a filesystem path.
///
/// Returns `None` for non-`file` schemes or a body that isn't valid
/// percent-encoded UTF-8.
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let text = uri.as_str();
    let rest = text.strip_prefix("file://")?;
    let decoded = percent_decode(rest)?;

    // A canonical Windows URI is `file:///C:/path`; strip the leading
    // slash in front of the drive letter so `PathBuf::from` sees `C:/path`
    // rather than treating it as a rooted-but-driveless path.
    let decoded = match decoded.as_bytes() {
        [b'/', drive, b':', ..] if drive.is_ascii_alphabetic() => decoded[1..].to_string(),
        _ => decoded,
    };

    Some(PathBuf::from(decoded))
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
            percent_encode_segment(&prefix.as_os_str().to_string_lossy(), uri);
        }
    }
}

/// RFC 3986 `pchar` percent-encoding: unreserved + sub-delims + `:` `@`
/// pass through untouched, everything else (including `/`, which the
/// caller inserts itself as the segment separator) is escaped.
fn percent_encode_segment(segment: &str, out: &mut String) {
    for byte in segment.bytes() {
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

fn percent_decode(s: &str) -> Option<String> {
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
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

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
