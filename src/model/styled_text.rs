//! `StyledText`: plaintext plus byte-range style spans — the one rich-text
//! shape the overlay surface's text zones, the hover card, the completion
//! docs card, and signature help all share. Markdown from language servers
//! is reduced to this (see `lsp::markdown`) instead of being flattened to
//! bare text, so inline code, emphasis, and the active parameter survive
//! the trip to the painter.

use std::ops::Range;

/// How a span is painted. `Strong` and `Accent` use synthetic bold;
/// code styles select the editor font independently of the UI prose font.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanStyle {
    /// Emphasis (`**x**`, `*x*`, headings): synthetic bold, primary color.
    Strong,
    /// Plain code; inline runs get a recessed chip, whole code lines do not.
    Code,
    /// Code token colored with the current theme's syntax palette.
    Syntax(crate::syntax::HighlightId),
    /// The thing to look at (signature help's active parameter): accent
    /// color, synthetic bold.
    Accent,
    /// Secondary text (signature counts, meta).
    Dim,
}

impl SpanStyle {
    pub(crate) fn is_code(self) -> bool {
        matches!(self, Self::Code | Self::Syntax(_))
    }
}

/// Whether sorted, non-overlapping runs cover the entire range as code.
pub(crate) fn code_spans_cover(
    range: Range<usize>,
    runs: impl IntoIterator<Item = (Range<usize>, SpanStyle)>,
) -> bool {
    let mut covered = range.start;
    for (run, style) in runs {
        if run.end <= covered {
            continue;
        }
        if run.start > covered || !style.is_code() {
            return false;
        }
        covered = run.end;
        if covered >= range.end {
            return true;
        }
    }
    false
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

    /// Splits off the leading block of whole code lines (a hover's
    /// signature fence) from the prose that follows: `(code, rest)`. `code`
    /// is `None` when the text doesn't start with a code line. The
    /// separating blank line, if any, is dropped from `rest`.
    pub fn split_leading_code(&self) -> (Option<StyledText>, StyledText) {
        let mut end = 0usize; // byte end of the leading code block
        for (start, line) in line_ranges(&self.text) {
            if line == start {
                // A blank line inside a fence: part of the block if more
                // code follows (`end` only advances on code lines, so a
                // blank line before prose is left to the prose side).
                continue;
            }
            let covered = code_spans_cover(
                start..line,
                self.spans.iter().map(|s| (s.range.clone(), s.style)),
            );
            if covered {
                end = line;
            } else {
                break;
            }
        }
        if end == 0 {
            return (None, self.clone());
        }
        let code = StyledText {
            text: self.text[..end].to_owned(),
            spans: self
                .runs_in(0..end)
                .into_iter()
                .map(|(range, style)| Span { range, style })
                .collect(),
        };
        let mut rest_start = end;
        while self.text[rest_start..].starts_with('\n') {
            rest_start += 1;
        }
        let rest = StyledText {
            text: self.text[rest_start..].to_owned(),
            spans: self
                .runs_in(rest_start..self.text.len())
                .into_iter()
                .map(|(range, style)| Span { range, style })
                .collect(),
        };
        (Some(code), rest)
    }

    /// The styled runs covering `line` (a byte range into `text`, e.g. one
    /// wrapped line), as ranges relative to the line's own start. Spans
    /// crossing the line's edges are clipped; unstyled gaps are omitted.
    pub fn runs_in(&self, line: Range<usize>) -> Vec<(Range<usize>, SpanStyle)> {
        runs_in_spans(&self.spans, line)
    }
}

/// `(start, end)` byte ranges of each line of `text`, newline excluded.
fn line_ranges(text: &str) -> impl Iterator<Item = (usize, usize)> + '_ {
    let mut offset = 0;
    text.split_inclusive('\n').map(move |raw| {
        let start = offset;
        offset += raw.len();
        (start, start + raw.trim_end_matches('\n').len())
    })
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
    fn split_leading_code_separates_the_signature_fence_from_the_prose() {
        let mut t = StyledText::default();
        t.push_styled("fn foo()", SpanStyle::Code);
        t.push_str("\n");
        t.push_styled("    -> u8", SpanStyle::Code);
        t.push_str("\n\nReturns ");
        t.push_styled("nothing", SpanStyle::Strong);
        let (code, rest) = t.split_leading_code();
        let code = code.unwrap();
        assert_eq!(code.text, "fn foo()\n    -> u8");
        assert_eq!(code.spans.len(), 2);
        assert_eq!(rest.text, "Returns nothing");
        assert_eq!(&rest.text[rest.spans[0].range.clone()], "nothing");
        // A blank line inside the fence does not end the block; the
        // blank line before the prose is not part of it.
        let mut t = StyledText::default();
        t.push_styled("fn a()", SpanStyle::Code);
        t.push_str("\n\n");
        t.push_styled("fn b()", SpanStyle::Code);
        t.push_str("\n\nprose");
        let (code, rest) = t.split_leading_code();
        assert_eq!(code.unwrap().text, "fn a()\n\nfn b()");
        assert_eq!(rest.text, "prose");
        // Prose-first text has no leading code.
        let (none, same) = StyledText::plain("hi").split_leading_code();
        assert!(none.is_none());
        assert_eq!(same.text, "hi");
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
