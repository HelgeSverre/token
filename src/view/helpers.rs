//! Shared helper functions for the view layer.
//!
//! This module contains utility functions used across multiple view components
//! to avoid code duplication.

use std::borrow::Cow;

#[derive(Clone, Copy)]
pub(crate) enum EllipsisSide {
    Start,
    End,
}

/// Keep the original string when it fits, otherwise retain whole characters
/// beside the ellipsis. Measurement follows the caller's painting metrics.
pub(super) fn ellipsize(
    text: &str,
    width: f32,
    side: EllipsisSide,
    mut measure: impl FnMut(&str) -> f32,
) -> Cow<'_, str> {
    let ellipsis = measure("…");
    let mut used = 0.0;
    let mut boundary = match side {
        EllipsisSide::Start => text.len(),
        EllipsisSide::End => 0,
    };
    let mut chars = text.char_indices();
    let mut bytes = [0; 4];
    loop {
        let next = match side {
            EllipsisSide::Start => chars.next_back(),
            EllipsisSide::End => chars.next(),
        };
        let Some((index, ch)) = next else {
            return Cow::Borrowed(text);
        };
        used += measure(ch.encode_utf8(&mut bytes));
        if used > width {
            return if width < ellipsis {
                Cow::Borrowed("")
            } else {
                Cow::Owned(match side {
                    EllipsisSide::Start => format!("…{}", &text[boundary..]),
                    EllipsisSide::End => format!("{}…", &text[..boundary]),
                })
            };
        }
        if used <= width - ellipsis {
            boundary = match side {
                EllipsisSide::Start => index,
                EllipsisSide::End => index + ch.len_utf8(),
            };
        }
    }
}

/// Trim trailing line ending (`\r\n` or `\n`) from a line of text.
///
/// Used for display purposes to avoid rendering the line-ending characters.
/// Mirrors `Document::get_line_cow`'s CRLF-aware trim so cursor-column math
/// derived from this text matches what's actually rendered.
#[inline]
pub fn trim_line_ending(text: &str) -> &str {
    crate::util::text::trim_line_ending(text)
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
