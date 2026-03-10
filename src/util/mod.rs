//! Utility modules

pub mod file_validation;
pub mod text;
pub mod tree;

// Re-export text utilities at the util level for backward compatibility
pub use text::{char_type, is_punctuation, is_word_boundary, CharType};

// Re-export file validation utilities
pub use file_validation::{
    filename_for_display, is_likely_binary, is_supported_image, validate_file_for_opening,
    FileOpenError, MAX_FILE_SIZE,
};

// Re-export tree traversal utilities
pub use tree::{
    visible_tree_count, visible_tree_index_of, visible_tree_row_at_index,
    visible_tree_row_matching, TreeNodeLike, VisibleTreeRow,
};
