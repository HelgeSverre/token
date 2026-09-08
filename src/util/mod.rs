//! Utility modules

pub mod byte_size;
mod file_identity;
pub mod file_validation;
pub use file_identity::FileIdentity;
pub mod text;
pub mod tree;

pub use byte_size::ByteSize;

/// Only web URLs may be sent from rendered content to the system browser.
pub fn is_web_url(value: &str) -> bool {
    !value.chars().any(|c| c.is_whitespace() || c.is_control())
        && reqwest::Url::parse(value)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

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
