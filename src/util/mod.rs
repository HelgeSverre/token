//! Utility modules

pub mod file_validation;
pub mod text;

// Re-export text utilities at the util level for backward compatibility
pub use text::{char_type, is_punctuation, is_word_boundary, CharType};

// Re-export file validation utilities
pub use file_validation::{
    filename_for_display, is_likely_binary, validate_file_for_opening, FileOpenError, MAX_FILE_SIZE,
};
