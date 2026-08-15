//! Shared helper functions for the view layer.
//!
//! This module contains utility functions used across multiple view components
//! to avoid code duplication.

/// Trim trailing line ending (`\r\n` or `\n`) from a line of text.
///
/// Used for display purposes to avoid rendering the line-ending characters.
/// Mirrors `Document::get_line_cow`'s CRLF-aware trim so cursor-column math
/// derived from this text matches what's actually rendered.
#[inline]
pub fn trim_line_ending(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
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
