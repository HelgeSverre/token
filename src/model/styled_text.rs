//! `StyledText`: plaintext plus byte-range style spans — the one rich-text
//! shape the overlay surface's text zones, the hover card, the completion
//! docs card, and signature help all share. Markdown from language servers
//! is reduced to this (see `lsp::markdown`) instead of being flattened to
//! bare text, so inline code, emphasis, and the active parameter survive
//! the trip to the painter.

use std::ops::Range;

/// How a span is painted. The overlay has one monospace face, so `Strong`
/// and `Accent` are synthetic-bold (double strike) rather than a bold face.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    /// Emphasis (`**x**`, `*x*`, headings): synthetic bold, primary color.
    Strong,
    /// Inline code / fenced code: a recessed chip behind the run.
    Code,
    /// The thing to look at (signature help's active parameter): accent
    /// color, synthetic bold.
    Accent,
    /// Secondary text (signature counts, meta).
    Dim,
}

/// `range` is a byte range into the owning `StyledText::text`; spans are
/// kept sorted and non-overlapping by construction (`push_span`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: SpanStyle,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyledText {
    pub text: String,
    pub spans: Vec<Span>,
}

impl StyledText {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            spans: Vec::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Appends unstyled text.
    pub fn push_str(&mut self, s: &str) {
        self.text.push_str(s);
    }

    /// Appends `s` as one span of `style` (empty `s` appends nothing).
    pub fn push_styled(&mut self, s: &str, style: SpanStyle) {
        if s.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(s);
        self.spans.push(Span {
            range: start..self.text.len(),
            style,
        });
    }

    /// Appends another styled text, shifting its spans.
    pub fn extend(&mut self, other: &StyledText) {
        let base = self.text.len();
        self.text.push_str(&other.text);
        self.spans.extend(other.spans.iter().map(|s| Span {
            range: s.range.start + base..s.range.end + base,
            style: s.style,
        }));
    }

    /// Styles the char range `[start, end)` (char offsets) — used where an
    /// offset comes from the protocol rather than from parsing (signature
    /// help's `activeParameter`). Out-of-range or inverted offsets are
    /// ignored. Existing spans overlapping the range are trimmed away so
    /// spans stay non-overlapping.
    pub fn style_chars(&mut self, start: usize, end: usize, style: SpanStyle) {
        let byte = |c: usize| {
            self.text
                .char_indices()
                .nth(c)
                .map(|(b, _)| b)
                .unwrap_or(self.text.len())
        };
        let count = self.text.chars().count();
        if start >= end || end > count {
            return;
        }
        let (bs, be) = (byte(start), byte(end));
        self.spans
            .retain(|s| s.range.end <= bs || s.range.start >= be);
        let at = self.spans.partition_point(|s| s.range.start < bs);
        self.spans.insert(
            at,
            Span {
                range: bs..be,
                style,
            },
        );
    }

    /// The styled runs covering `line` (a byte range into `text`, e.g. one
    /// wrapped line), as ranges relative to the line's own start. Spans
    /// crossing the line's edges are clipped; unstyled gaps are omitted.
    pub fn runs_in(&self, line: Range<usize>) -> Vec<(Range<usize>, SpanStyle)> {
        runs_in_spans(&self.spans, line)
    }
}

/// [`StyledText::runs_in`] over a bare span slice (the overlay's `Zones`
/// carries `&str` + `&[Span]` rather than an owned `StyledText`).
pub fn runs_in_spans(spans: &[Span], line: Range<usize>) -> Vec<(Range<usize>, SpanStyle)> {
    {
        spans
            .iter()
            .filter(|s| s.range.start < line.end && s.range.end > line.start)
            .map(|s| {
                let start = s.range.start.max(line.start) - line.start;
                let end = s.range.end.min(line.end) - line.start;
                (start..end, s.style)
            })
            .filter(|(r, _)| !r.is_empty())
            .collect()
    }
}

impl From<String> for StyledText {
    fn from(text: String) -> Self {
        Self::plain(text)
    }
}

impl From<&str> for StyledText {
    fn from(text: &str) -> Self {
        Self::plain(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_in_clips_spans_to_the_line_and_rebases_them() {
        let mut t = StyledText::plain("abc def ghi");
        t.spans.push(Span {
            range: 2..9, // "c def g"
            style: SpanStyle::Code,
        });
        // First wrapped line "abc" (0..3): the span starts inside it.
        assert_eq!(t.runs_in(0..3), vec![(2..3, SpanStyle::Code)]);
        // Second line "def" (4..7): fully inside the span.
        assert_eq!(t.runs_in(4..7), vec![(0..3, SpanStyle::Code)]);
        // Third line "ghi" (8..11): the span ends inside it.
        assert_eq!(t.runs_in(8..11), vec![(0..1, SpanStyle::Code)]);
        // A line entirely outside: nothing.
        assert!(t.runs_in(9..11).is_empty());
    }

    #[test]
    fn style_chars_uses_char_offsets_and_replaces_overlaps() {
        let mut t = StyledText::plain("fn f(é: T, b: U)");
        t.push_styled("", SpanStyle::Dim); // no-op
        t.spans.push(Span {
            range: 0..2,
            style: SpanStyle::Strong,
        });
        // "é: T" is chars 5..9; é is two bytes.
        t.style_chars(5, 9, SpanStyle::Accent);
        assert_eq!(&t.text[t.spans[1].range.clone()], "é: T");
        assert_eq!(t.spans[1].style, SpanStyle::Accent);
        // An overlapping restyle trims the old span out.
        t.style_chars(3, 9, SpanStyle::Dim);
        assert_eq!(t.spans.len(), 2);
        assert_eq!(&t.text[t.spans[1].range.clone()], "f(é: T");
        // Inverted / past-the-end offsets are ignored.
        t.style_chars(9, 3, SpanStyle::Code);
        t.style_chars(0, 99, SpanStyle::Code);
        assert_eq!(t.spans.len(), 2);
    }

    #[test]
    fn extend_shifts_the_appended_spans() {
        let mut a = StyledText::plain("ab");
        let mut b = StyledText::default();
        b.push_styled("cd", SpanStyle::Code);
        a.extend(&b);
        assert_eq!(a.text, "abcd");
        assert_eq!(a.spans[0].range, 2..4);
    }
}
