//! File validation utilities for opening files
//!
//! Validates files before attempting to open them, checking for:
//! - File existence and permissions
//! - File size limits
//! - Binary file detection

use std::fs::{self, File};
use std::io::Read;
use std::path::Path;

/// Maximum file size in bytes (50 MB)
pub const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024;

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
    TooLarge { size_mb: f64 },
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
            Self::TooLarge { size_mb } => {
                format!(
                    "{} is too large ({:.1} MB, max {} MB)",
                    filename,
                    size_mb,
                    MAX_FILE_SIZE / (1024 * 1024)
                )
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
            Self::TooLarge { size_mb } => write!(f, "file too large ({:.1} MB)", size_mb),
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

    if metadata.len() > MAX_FILE_SIZE {
        return Err(FileOpenError::TooLarge {
            size_mb: metadata.len() as f64 / (1024.0 * 1024.0),
        });
    }

    Ok(())
}

/// Check if a file is likely binary by scanning for null bytes
///
/// Reads the first 8KB of the file and checks for null bytes,
/// which are common in binary files but rare in text files.
///
/// Returns `true` if the file appears to be binary, `false` if it appears to be text.
/// Returns `false` on any read error (let the actual open fail with a better error).
pub fn is_likely_binary(path: &Path) -> bool {
    let Ok(mut file) = File::open(path) else {
        return false;
    };

    let mut buffer = [0u8; 8192];
    let Ok(bytes_read) = file.read(&mut buffer) else {
        return false;
    };

    // Check for null bytes in the read content
    buffer[..bytes_read].contains(&0)
}

/// Image file extensions supported by the viewer
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp"];

/// Check if a file path has an image extension
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
        .unwrap_or(false)
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
    }

    #[test]
    fn test_is_image_file_png() {
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.PNG")));
    }

    #[test]
    fn test_is_image_file_jpeg() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
    }

    #[test]
    fn test_is_image_file_other_formats() {
        assert!(is_image_file(Path::new("image.gif")));
        assert!(is_image_file(Path::new("image.bmp")));
        assert!(is_image_file(Path::new("image.webp")));
    }

    #[test]
    fn test_is_not_image_file() {
        assert!(!is_image_file(Path::new("code.rs")));
        assert!(!is_image_file(Path::new("data.csv")));
        assert!(!is_image_file(Path::new("readme.md")));
        assert!(!is_image_file(Path::new("noextension")));
    }
}
