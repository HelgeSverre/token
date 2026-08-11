//! Utility modules

pub mod byte_size;
pub mod file_validation;
pub mod text;
pub mod tree;

pub use byte_size::ByteSize;

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
