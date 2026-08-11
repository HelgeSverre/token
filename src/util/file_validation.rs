//! File validation utilities for opening files
//!
//! Validates files before attempting to open them, checking for:
//! - File existence and permissions
//! - File size limits
//! - Binary file detection

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

use super::ByteSize;

/// Maximum file size (50 MiB).
pub const MAX_FILE_SIZE: ByteSize = ByteSize::mebibytes(50);

/// Errors that can occur when validating a file for opening
#[derive(Debug, Clone)]
pub enum FileOpenError {
    /// File does not exist
    NotFound,
    /// Permission denied to read file
    PermissionDenied,
    /// Path is a directory, not a file
    IsDirectory,
    /// File appears to be binary (contains null bytes)
    BinaryFile,
    /// File exceeds size limit
    TooLarge { size: ByteSize },
    /// Other I/O error
    IoError(String),
}

impl FileOpenError {
    /// Get a user-friendly error message
    pub fn user_message(&self, filename: &str) -> String {
        match self {
            Self::NotFound => format!("File not found: {}", filename),
            Self::PermissionDenied => format!("Permission denied: {}", filename),
            Self::IsDirectory => format!("Cannot open directory: {}", filename),
            Self::BinaryFile => format!("Cannot open binary file: {}", filename),
            Self::TooLarge { size } => {
                format!("{} is too large ({size}, max {MAX_FILE_SIZE})", filename)
            }
            Self::IoError(msg) => format!("Error opening {}: {}", filename, msg),
        }
    }
}

impl std::fmt::Display for FileOpenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "file not found"),
            Self::PermissionDenied => write!(f, "permission denied"),
            Self::IsDirectory => write!(f, "is a directory"),
            Self::BinaryFile => write!(f, "binary file"),
            Self::TooLarge { size } => write!(f, "file too large ({size})"),
            Self::IoError(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for FileOpenError {}

/// Validate a file before attempting to open it
///
/// Checks:
/// - File exists
/// - Is not a directory
/// - Has read permissions
/// - Does not exceed size limit
///
/// Does NOT check for binary content (use `is_likely_binary` separately after this passes)
pub fn validate_file_for_opening(path: &Path) -> Result<(), FileOpenError> {
    let metadata = fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => FileOpenError::NotFound,
        std::io::ErrorKind::PermissionDenied => FileOpenError::PermissionDenied,
        _ => FileOpenError::IoError(e.to_string()),
    })?;

    if metadata.is_dir() {
        return Err(FileOpenError::IsDirectory);
    }

    if ByteSize::bytes(metadata.len()) > MAX_FILE_SIZE {
        return Err(FileOpenError::TooLarge {
            size: ByteSize::bytes(metadata.len()),
        });
    }

    Ok(())
}

/// Check if a file path has a supported image extension
pub fn is_supported_image(path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase());
    matches!(
        ext.as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico")
    )
}

/// Check if a file is likely binary by scanning for null bytes
///
/// Reads the first 8 KiB of the file and checks for null bytes,
/// which are common in binary files but rare in text files.
///
/// Returns `true` if the file appears to be binary, `false` if it appears to be text.
/// Returns `false` on any read error (let the actual open fail with a better error).
pub fn is_likely_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };

    let mut buffer = [0u8; ByteSize::kibibytes(8).as_usize()];
    let Ok(bytes_read) = file.read(&mut buffer) else {
        return false;
    };

    // Check for null bytes in the read content
    buffer[..bytes_read].contains(&0)
}

/// Get the filename from a path for display in error messages
pub fn filename_for_display(path: &Path) -> String {
    path.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_validate_nonexistent_file() {
        let result = validate_file_for_opening(Path::new("/nonexistent/path/file.txt"));
        assert!(matches!(result, Err(FileOpenError::NotFound)));
    }

    #[test]
    fn test_validate_directory() {
        let result = validate_file_for_opening(Path::new("/tmp"));
        assert!(matches!(result, Err(FileOpenError::IsDirectory)));
    }

    #[test]
    fn test_validate_valid_file() {
        let temp = NamedTempFile::new().unwrap();
        let result = validate_file_for_opening(temp.path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_file_size_boundary() {
        let at_limit = NamedTempFile::new().unwrap();
        at_limit.as_file().set_len(MAX_FILE_SIZE.as_u64()).unwrap();
        assert!(validate_file_for_opening(at_limit.path()).is_ok());

        let over_limit = NamedTempFile::new().unwrap();
        over_limit
            .as_file()
            .set_len(MAX_FILE_SIZE.as_u64() + 1)
            .unwrap();
        assert!(matches!(
            validate_file_for_opening(over_limit.path()),
            Err(FileOpenError::TooLarge { size }) if size == ByteSize::bytes(MAX_FILE_SIZE.as_u64() + 1)
        ));
    }

    #[test]
    fn test_is_binary_text_file() {
        let mut temp = NamedTempFile::new().unwrap();
        writeln!(temp, "Hello, world!").unwrap();
        writeln!(temp, "This is a text file.").unwrap();
        temp.flush().unwrap();

        assert!(!is_likely_binary(temp.path()));
    }

    #[test]
    fn test_is_binary_with_null_bytes() {
        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"Hello\x00World").unwrap();
        temp.flush().unwrap();

        assert!(is_likely_binary(temp.path()));
    }

    #[test]
    fn test_is_supported_image() {
        assert!(is_supported_image(Path::new("photo.png")));
        assert!(is_supported_image(Path::new("photo.JPG")));
        assert!(is_supported_image(Path::new("photo.jpeg")));
        assert!(is_supported_image(Path::new("animation.gif")));
        assert!(is_supported_image(Path::new("icon.ico")));
        assert!(is_supported_image(Path::new("photo.webp")));
        assert!(is_supported_image(Path::new("photo.bmp")));
        assert!(!is_supported_image(Path::new("code.rs")));
        assert!(!is_supported_image(Path::new("doc.pdf")));
        assert!(!is_supported_image(Path::new("noext")));
    }

    #[test]
    fn test_error_messages() {
        assert_eq!(
            FileOpenError::NotFound.user_message("test.txt"),
            "File not found: test.txt"
        );
        assert_eq!(
            FileOpenError::IsDirectory.user_message("mydir"),
            "Cannot open directory: mydir"
        );
        assert_eq!(
            FileOpenError::BinaryFile.user_message("image.png"),
            "Cannot open binary file: image.png"
        );
        let error = FileOpenError::TooLarge {
            size: ByteSize::bytes(MAX_FILE_SIZE.as_u64() + 1),
        };
        assert_eq!(
            error.user_message("large.txt"),
            "large.txt is too large (50.0 MiB, max 50.0 MiB)"
        );
        assert_eq!(error.to_string(), "file too large (50.0 MiB)");
    }
}
