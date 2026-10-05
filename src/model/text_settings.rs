//! Text behavior belongs to the document; every pane uses the same policy.

use serde::{Deserialize, Serialize};

use crate::util::text::TabStops;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndentStyle {
    #[default]
    Tab,
    Space,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineEnding {
    #[default]
    Lf,
    Crlf,
    Cr,
}

impl LineEnding {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lf => "\n",
            Self::Crlf => "\r\n",
            Self::Cr => "\r",
        }
    }

    /// Most common ending, with first occurrence breaking ties. No normalization.
    pub fn detect(text: &str) -> Self {
        Self::detect_bytes(text.bytes())
    }

    /// Detect without allocating a contiguous copy of the document.
    pub fn detect_rope(text: &ropey::Rope) -> Self {
        Self::detect_bytes(text.chunks().flat_map(str::bytes))
    }

    fn detect_bytes(bytes: impl Iterator<Item = u8>) -> Self {
        let mut counts = [
            (Self::Lf, 0, usize::MAX),
            (Self::Crlf, 0, usize::MAX),
            (Self::Cr, 0, usize::MAX),
        ];
        let mut chars = bytes.enumerate().peekable();
        while let Some((offset, ch)) = chars.next() {
            let index = match ch {
                b'\r' if chars.peek().is_some_and(|&(_, next)| next == b'\n') => {
                    chars.next();
                    1
                }
                b'\r' => 2,
                b'\n' => 0,
                _ => continue,
            };
            counts[index].1 += 1;
            counts[index].2 = counts[index].2.min(offset);
        }
        counts
            .into_iter()
            .max_by_key(|&(_, count, first)| (count, std::cmp::Reverse(first)))
            .filter(|&(_, count, _)| count > 0)
            .map_or(Self::Lf, |(ending, _, _)| ending)
    }
}

/// Missing values preserve detection/inference. Explicit false disables a rule.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextPreferences {
    pub indent_style: Option<IndentStyle>,
    pub indent_size: Option<usize>,
    pub tab_width: Option<usize>,
    pub end_of_line: Option<LineEnding>,
    pub trim_trailing_whitespace: Option<bool>,
    pub insert_final_newline: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentTextSettings {
    pub indent_style: IndentStyle,
    pub indent_size: usize,
    pub tabs: TabStops,
    pub explicit_indent: bool,
    pub end_of_line: Option<LineEnding>,
    pub trim_trailing_whitespace: Option<bool>,
    pub insert_final_newline: Option<bool>,
}

impl Default for DocumentTextSettings {
    fn default() -> Self {
        Self::resolve(TextPreferences::default(), TextPreferences::default())
    }
}

impl DocumentTextSettings {
    pub fn resolve(user: TextPreferences, file: TextPreferences) -> Self {
        Self {
            indent_style: file.indent_style.or(user.indent_style).unwrap_or_default(),
            indent_size: file
                .indent_size
                .or(user.indent_size)
                .unwrap_or(4)
                .clamp(1, 256),
            tabs: TabStops::new(file.tab_width.or(user.tab_width).unwrap_or(4)),
            explicit_indent: file.indent_size.or(user.indent_size).is_some()
                || file.indent_style.or(user.indent_style).is_some(),
            end_of_line: file.end_of_line.or(user.end_of_line),
            trim_trailing_whitespace: file
                .trim_trailing_whitespace
                .or(user.trim_trailing_whitespace),
            insert_final_newline: file.insert_final_newline.or(user.insert_final_newline),
        }
    }

    /// Create exactly the requested visual indentation, using tabs where allowed.
    pub fn indentation(self, start: usize, end: usize) -> String {
        let mut result = String::new();
        let mut column = start;
        while column < end {
            let advance = self.tabs.advance(column);
            if self.indent_style == IndentStyle::Tab && advance <= end - column {
                result.push('\t');
                column += advance;
            } else {
                result.push(' ');
                column += 1;
            }
        }
        result
    }

    pub fn tab_insertion(self, column: usize) -> String {
        // Preserve a literal Tab outside indentation for hard-tab documents.
        if self.indent_style == IndentStyle::Tab {
            "\t".into()
        } else {
            " ".repeat(self.indent_size - column % self.indent_size)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::LineEnding;

    #[test]
    fn line_ending_detection_preserves_majority_and_first_occurrence_across_chunks() {
        for (text, expected) in [
            ("æ\r\nø\n", LineEnding::Crlf),
            ("æ\nø\r\n", LineEnding::Lf),
            ("æ\rø\r\nå\r", LineEnding::Cr),
            ("æ\rø\r\nå\r\n", LineEnding::Crlf),
            ("æøå", LineEnding::Lf),
        ] {
            // One-byte chunks split CRLF and multi-byte Unicode sequences.
            assert_eq!(
                LineEnding::detect_bytes(text.as_bytes().chunks(1).flatten().copied()),
                expected
            );
            assert_eq!(
                LineEnding::detect_rope(&ropey::Rope::from_str(text)),
                expected
            );
            assert_eq!(LineEnding::detect(text), expected);
        }
    }
}
