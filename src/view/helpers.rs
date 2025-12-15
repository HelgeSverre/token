//! Shared helper functions for the view layer.
//!
//! This module contains utility functions used across multiple view components
//! to avoid code duplication.

use token::model::editor_area::Tab;
use token::model::AppModel;

/// Get the display title for a tab.
///
/// Centralizes the logic for determining what text to show in the tab bar.
/// Returns "Untitled" if the editor or document cannot be found.
pub fn get_tab_display_name(model: &AppModel, tab: &Tab) -> String {
    model
        .editor_area
        .editors
        .get(&tab.editor_id)
        .and_then(|e| e.document_id)
        .and_then(|doc_id| model.editor_area.documents.get(&doc_id))
        .map(|d| d.display_name())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// Trim trailing newline from a line of text.
///
/// Used for display purposes to avoid rendering the newline character.
#[inline]
pub fn trim_line_ending(text: &str) -> &str {
    text.strip_suffix('\n').unwrap_or(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trim_line_ending_with_newline() {
        assert_eq!(trim_line_ending("hello\n"), "hello");
    }

    #[test]
    fn test_trim_line_ending_without_newline() {
        assert_eq!(trim_line_ending("hello"), "hello");
    }

    #[test]
    fn test_trim_line_ending_empty() {
        assert_eq!(trim_line_ending(""), "");
    }

    #[test]
    fn test_trim_line_ending_only_newline() {
        assert_eq!(trim_line_ending("\n"), "");
    }
}
