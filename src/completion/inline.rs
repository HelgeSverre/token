//! Inline (ghost-text) suggestions — autocomplete.md Phase 2.
//!
//! Pure data and pure functions only: the suggestion state that lives on
//! `UiState`, the request the runtime's completion worker executes, the
//! post-processing chain applied to a backend's raw completion, and the
//! prefix-consumption bookkeeping that lets the user type "through" a
//! suggestion without a new request.

use std::path::PathBuf;

use crate::model::editor_area::DocumentId;
use crate::model::Document;

/// Chars of document text sent before / after the cursor.
pub const PREFIX_BUDGET_CHARS: usize = 4000;
pub const SUFFIX_BUDGET_CHARS: usize = 1000;
/// Consecutive backend failures after which auto-trigger pauses until the
/// user asks explicitly (capped retry, like the LSP crash policy).
pub const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// Bounds provider requests and retained alternatives, including non-HTTP replies.
pub const MAX_ALTERNATIVES: usize = 8;

/// How much of the visible remainder one acceptance inserts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcceptGranularity {
    Full,
    Word,
    Line,
}

/// Captured at request time; every response carries it back. The
/// universal staleness guard: a reply is applied only if the document,
/// its revision, and the cursor are all unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSnapshot {
    pub document_id: DocumentId,
    pub revision: u64,
    pub line: usize,
    pub column: usize,
    /// Monotonic; the worker keeps only the newest per document.
    pub request_id: u64,
}

/// One suggestion request, complete: the worker needs nothing else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineRequest {
    pub snapshot: RequestSnapshot,
    pub prefix: String,
    /// `"\n"` when the document ends at the cursor — FIM models expect a
    /// non-empty suffix.
    pub suffix: String,
    pub language: Option<String>,
    pub file_path: Option<PathBuf>,
    /// Stable, opt-in extra snippets, separate from the active buffer prefix.
    pub extra_context: Vec<super::recency::ContextChunk>,
    pub explicit: bool,
}

/// A suggestion the user can see, plus how much of it they have typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSuggestionState {
    pub snapshot: RequestSnapshot,
    /// Complete candidate texts anchored at the snapshot, in provider order.
    choices: Vec<String>,
    selected: usize,
    /// Chars of `text` the user has typed since the suggestion arrived.
    pub consumed: usize,
    /// The document revision the suggestion was last reconciled with:
    /// the snapshot's, then every consume/un-consume edit's.
    pub valid_revision: u64,
}

impl InlineSuggestionState {
    pub fn new(snapshot: RequestSnapshot, candidates: Vec<String>) -> Option<Self> {
        let mut choices = Vec::new();
        for candidate in candidates.into_iter().take(MAX_ALTERNATIVES) {
            if !candidate.trim().is_empty() && !choices.contains(&candidate) {
                choices.push(candidate);
            }
        }
        (!choices.is_empty()).then_some(Self {
            valid_revision: snapshot.revision,
            snapshot,
            choices,
            selected: 0,
            consumed: 0,
        })
    }

    fn text(&self) -> &str {
        &self.choices[self.selected]
    }

    fn compatible(&self, candidate: &str) -> bool {
        let prefix_len = self.text().len() - self.remaining().len();
        candidate.len() > prefix_len && candidate.starts_with(&self.text()[..prefix_len])
    }

    /// One-based position among choices that preserve the already inserted prefix.
    pub fn choice_position(&self) -> (usize, usize) {
        let mut position = 0;
        let mut count = 0;
        for (index, candidate) in self.choices.iter().enumerate() {
            if self.compatible(candidate) {
                count += 1;
                if index == self.selected {
                    position = count;
                }
            }
        }
        (position, count)
    }

    /// Never rewrite consumed text. Keep incompatible choices so backspacing can
    /// make them eligible again, rather than permanently pruning the response.
    pub(crate) fn cycle(&mut self, forward: bool) -> bool {
        for offset in 1..self.choices.len() {
            let next = if forward {
                (self.selected + offset) % self.choices.len()
            } else {
                (self.selected + self.choices.len() - offset) % self.choices.len()
            };
            if self.compatible(&self.choices[next]) {
                self.selected = next;
                return true;
            }
        }
        false
    }

    /// Word accepts the leading alphabetic run, or the leading non-alphabetic
    /// run when punctuation/spacing comes first. Line includes its newline.
    /// Offsets stay on UTF-8 boundaries; the stored consumption count is chars.
    pub fn acceptance_prefix(&self, granularity: AcceptGranularity) -> &str {
        let remaining = self.remaining();
        match granularity {
            AcceptGranularity::Full => remaining,
            AcceptGranularity::Line => remaining.split_inclusive('\n').next().unwrap_or(remaining),
            AcceptGranularity::Word => {
                let Some(first) = remaining.chars().next() else {
                    return remaining;
                };
                let end = remaining
                    .char_indices()
                    .find_map(|(offset, ch)| {
                        (ch.is_alphabetic() != first.is_alphabetic()).then_some(offset)
                    })
                    .unwrap_or(remaining.len());
                &remaining[..end]
            }
        }
    }

    /// What is still ghost text.
    pub fn remaining(&self) -> &str {
        let byte = self
            .text()
            .char_indices()
            .nth(self.consumed)
            .map_or(self.text().len(), |(i, _)| i);
        &self.text()[byte..]
    }

    /// Where the cursor must be for the remainder to apply: the snapshot
    /// cursor advanced over the consumed chars.
    pub fn expected_cursor(&self) -> (usize, usize) {
        let (mut line, mut column) = (self.snapshot.line, self.snapshot.column);
        for ch in self.text().chars().take(self.consumed) {
            if ch == '\n' {
                line += 1;
                column = 0;
            } else {
                column += 1;
            }
        }
        (line, column)
    }

    /// Whether the suggestion still applies at `document`/cursor: same
    /// document, no edit since the last reconciled one, and the cursor
    /// sitting right after the consumed prefix.
    pub fn applies_to(&self, document: &Document, cursor: (usize, usize)) -> bool {
        document.id == Some(self.snapshot.document_id)
            && document.revision == self.valid_revision
            && cursor == self.expected_cursor()
    }
}

/// Apply the post-processing chain (autocomplete.md filters 1–4) to a
/// backend's raw output. `None` means "nothing worth showing".
pub fn postprocess(raw: &str, suffix: &str) -> Option<String> {
    // 1. Leaked sentinel tokens end the suggestion.
    let mut text = raw;
    for sentinel in super::prompt::sentinels() {
        if let Some(index) = text.find(sentinel) {
            text = &text[..index];
        }
    }
    // 2. Never suggest past the current block: stop at a blank line that
    //    is followed by a line indented less than the first line.
    let text = trim_to_block(text);
    // 3. Drop what merely restates the text already after the cursor.
    let text = strip_suffix_overlap(&text, suffix);
    // 4. Drop degenerate results: nothing but whitespace or punctuation
    //    (a lone `)` or `;`), or the same line three times in a row.
    //    Short but real completions like ` + b` must survive.
    if text.trim().is_empty() || !text.chars().any(|c| c.is_alphanumeric()) || is_repetitive(&text)
    {
        return None;
    }
    Some(text.trim_end_matches('\n').to_owned() + if text.ends_with('\n') { "\n" } else { "" })
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).count()
}

fn trim_to_block(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let Some(first_content) = lines.iter().find(|l| !l.trim().is_empty()) else {
        return text.to_owned();
    };
    let base = indent_of(first_content);
    let mut keep = lines.len();
    let mut i = 1;
    while i < lines.len() {
        if lines[i].trim().is_empty() {
            if let Some(next) = lines[i + 1..].iter().find(|l| !l.trim().is_empty()) {
                if indent_of(next) < base {
                    keep = i;
                    break;
                }
            }
        }
        i += 1;
    }
    lines[..keep].join("\n")
}

fn strip_suffix_overlap(text: &str, suffix: &str) -> String {
    let suffix_head: String = suffix.chars().take(30).collect();
    let suffix_head = suffix_head.trim_start();
    if suffix_head.is_empty() {
        return text.to_owned();
    }
    let trimmed = text.trim_end();
    // Whole-suffix duplicate: the model retyped what follows the cursor.
    if !trimmed.is_empty() && suffix_head.starts_with(trimmed) {
        return String::new();
    }
    // Trailing overlap: cut where the suggestion starts repeating the suffix.
    let chars: Vec<char> = trimmed.chars().collect();
    for start in 1..chars.len() {
        let tail: String = chars[start..].iter().collect();
        if tail.len() >= 3 && suffix_head.starts_with(&tail) {
            return chars[..start].iter().collect();
        }
    }
    text.to_owned()
}

fn is_repetitive(text: &str) -> bool {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    lines.len() >= 3 && lines.windows(3).any(|w| w[0] == w[1] && w[1] == w[2])
}

/// Build the worker request for the document's cursor, or `None` when
/// the document has no id.
pub fn build_request(
    document: &Document,
    cursor: (usize, usize),
    request_id: u64,
    language: Option<String>,
    explicit: bool,
) -> Option<InlineRequest> {
    let document_id = document.id?;
    let offset = document.cursor_to_offset(cursor.0, cursor.1);
    let total = document.buffer.len_chars();
    let prefix: String = document
        .buffer
        .slice(offset.saturating_sub(PREFIX_BUDGET_CHARS)..offset)
        .chars()
        .collect();
    let mut suffix: String = document
        .buffer
        .slice(offset..(offset + SUFFIX_BUDGET_CHARS).min(total))
        .chars()
        .collect();
    if suffix.is_empty() {
        suffix.push('\n');
    }
    Some(InlineRequest {
        snapshot: RequestSnapshot {
            document_id,
            revision: document.revision,
            line: cursor.0,
            column: cursor.1,
            request_id,
        },
        prefix,
        suffix,
        language,
        file_path: document.file_path.clone(),
        extra_context: Vec::new(),
        explicit,
    })
}

/// Auto-trigger's end-of-line rule: at most `max_line_suffix` chars of
/// text right of the cursor, not counting whitespace and closers.
pub fn line_tail_is_short(rest_of_line: &str, max_line_suffix: usize) -> bool {
    rest_of_line
        .chars()
        .filter(|c| !c.is_whitespace() && !matches!(c, ')' | ']' | '}' | ';' | ',' | '"' | '\''))
        .count()
        <= max_line_suffix
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(text: &str, consumed: usize) -> InlineSuggestionState {
        InlineSuggestionState {
            snapshot: RequestSnapshot {
                document_id: DocumentId(1),
                revision: 10,
                line: 2,
                column: 4,
                request_id: 1,
            },
            choices: vec![text.to_owned()],
            selected: 0,
            consumed,
            valid_revision: 10,
        }
    }

    #[test]
    fn remaining_and_expected_cursor_follow_consumed_chars() {
        let s = state("abc\ndef", 0);
        assert_eq!(s.remaining(), "abc\ndef");
        assert_eq!(s.expected_cursor(), (2, 4));
        let s = state("abc\ndef", 2);
        assert_eq!(s.remaining(), "c\ndef");
        assert_eq!(s.expected_cursor(), (2, 6));
        let s = state("abc\ndef", 5);
        assert_eq!(s.remaining(), "ef");
        assert_eq!(s.expected_cursor(), (3, 1));
    }

    #[test]
    fn alternatives_are_bounded_deduplicated_and_nonempty() {
        let snapshot = state("abc", 0).snapshot;
        assert!(
            InlineSuggestionState::new(snapshot.clone(), vec![" \n".into(), String::new()])
                .is_none()
        );
        let candidates = vec!["abc".into(), "abc".into(), "\n".into(), "def".into()];
        let mut choices = InlineSuggestionState::new(snapshot.clone(), candidates).unwrap();
        assert_eq!(choices.choice_position(), (1, 2));
        assert!(choices.cycle(false));
        assert_eq!(choices.remaining(), "def");
        assert_eq!(choices.choice_position(), (2, 2));
        assert!(choices.cycle(true));
        assert_eq!(choices.remaining(), "abc");
        let choices =
            InlineSuggestionState::new(snapshot, (0..100).map(|i| format!("choice{i}")).collect())
                .unwrap();
        assert_eq!(choices.choice_position(), (1, MAX_ALTERNATIVES));
    }

    #[test]
    fn cycling_preserves_unicode_consumption_and_backspace_restores_choices() {
        let mut choices = InlineSuggestionState::new(
            state("abc", 0).snapshot,
            ["héllo_one", "different", "héllo", "héllo_two"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        choices.consumed = 5;
        let cursor = choices.expected_cursor();
        assert_eq!(choices.choice_position(), (1, 2));
        assert!(choices.cycle(true));
        assert_eq!(choices.remaining(), "_two");
        assert_eq!(choices.expected_cursor(), cursor);
        assert_eq!(choices.choice_position(), (2, 2));
        assert!(choices.cycle(true));
        assert_eq!(choices.remaining(), "_one");
        choices.consumed = 6;
        assert!(choices.cycle(false));
        assert_eq!(choices.remaining(), "two");
        choices.consumed = 0;
        assert_eq!(choices.choice_position(), (4, 4));
        assert!(choices.cycle(false));
        assert_eq!(choices.remaining(), "héllo");
        assert!(choices.cycle(false));
        assert_eq!(choices.remaining(), "different");
    }

    #[test]
    fn multiline_partial_accept_cycling_never_changes_the_inserted_prefix() {
        let mut choices = InlineSuggestionState::new(
            state("abc", 0).snapshot,
            ["first\r\n  one", "first\n  two", "first\r\n  three"]
                .map(String::from)
                .to_vec(),
        )
        .unwrap();
        choices.consumed = 7;
        assert_eq!(choices.expected_cursor(), (3, 0));
        assert!(choices.cycle(true));
        assert_eq!(choices.remaining(), "  three");
        assert_eq!(choices.expected_cursor(), (3, 0));
        assert_eq!(choices.choice_position(), (2, 2));
        choices.consumed += 3;
        assert!(!choices.cycle(true));
        assert_eq!(choices.remaining(), "hree");
        assert_eq!(choices.choice_position(), (1, 1));
    }

    #[test]
    fn partial_acceptance_uses_utf8_safe_leading_runs() {
        for (text, word) in [
            ("hello_world42", "hello"),
            ("  (123)_world", "  (123)_"),
            ("你好🙂 world", "你好"),
            ("🙂 _42hello", "🙂 _42"),
            ("\r\n  next", "\r\n  "),
            ("", ""),
        ] {
            assert_eq!(
                state(text, 0).acceptance_prefix(AcceptGranularity::Word),
                word
            );
        }
        let suggestion = state("foo\r\n  bar\nend", 0);
        assert_eq!(
            suggestion.acceptance_prefix(AcceptGranularity::Line),
            "foo\r\n"
        );
        assert_eq!(
            state("foo\n  bar", 4).acceptance_prefix(AcceptGranularity::Line),
            "  bar"
        );
        assert_eq!(
            state("\nnext", 0).acceptance_prefix(AcceptGranularity::Line),
            "\n"
        );
    }

    #[test]
    fn applies_to_checks_document_revision_and_cursor() {
        let mut doc = Document::with_text("line0\nline1\nline2\n");
        doc.id = Some(DocumentId(1));
        doc.revision = 10;
        let s = state("xy", 0);
        assert!(s.applies_to(&doc, (2, 4)));
        assert!(!s.applies_to(&doc, (2, 5)));
        let mut consumed = state("xy", 1);
        consumed.valid_revision = 11;
        assert!(
            !consumed.applies_to(&doc, (2, 5)),
            "the consuming edit must have landed"
        );
        doc.revision = 11;
        assert!(consumed.applies_to(&doc, (2, 5)));
        doc.id = Some(DocumentId(2));
        assert!(!consumed.applies_to(&doc, (2, 5)));
    }

    #[test]
    fn postprocess_strips_sentinels_and_everything_after() {
        assert_eq!(
            postprocess("foo(bar)<|endoftext|>garbage", "\n").as_deref(),
            Some("foo(bar)")
        );
        assert_eq!(postprocess("x = 1<EOT>", "").as_deref(), Some("x = 1"));
    }

    #[test]
    fn postprocess_stops_at_a_lower_indented_block_after_a_blank_line() {
        let raw = "    let a = 1;\n    let b = 2;\n\nfn other() {}\n";
        assert_eq!(
            postprocess(raw, "\n").as_deref(),
            Some("    let a = 1;\n    let b = 2;")
        );
        // A blank line followed by same-or-deeper indentation is kept.
        let raw = "    let a = 1;\n\n    let b = 2;";
        assert_eq!(postprocess(raw, "\n").as_deref(), Some(raw));
    }

    #[test]
    fn postprocess_drops_text_that_duplicates_the_suffix() {
        assert_eq!(postprocess("world)", "world) + 1",), None);
        assert_eq!(
            postprocess("hello world)", "world) + 1").as_deref(),
            Some("hello ")
        );
        assert_eq!(postprocess("abc", "xyz").as_deref(), Some("abc"));
    }

    #[test]
    fn postprocess_drops_degenerate_results() {
        assert_eq!(postprocess("   \n", "\n"), None);
        assert_eq!(postprocess(");", "\n"), None);
        assert_eq!(postprocess(" + b", "\n").as_deref(), Some(" + b"));
        assert_eq!(postprocess("x = 1\nx = 1\nx = 1\n", "\n"), None);
        assert_eq!(
            postprocess("x = 1\nx = 1\ny = 2\n", "\n").as_deref(),
            Some("x = 1\nx = 1\ny = 2\n")
        );
    }

    #[test]
    fn build_request_windows_prefix_and_suffix_and_never_sends_an_empty_suffix() {
        let mut doc = Document::with_text("fn main() {\n    let x = ");
        doc.id = Some(DocumentId(3));
        doc.revision = 5;
        let req = build_request(&doc, (1, 12), 9, Some("rust".into()), false).unwrap();
        assert_eq!(req.prefix, "fn main() {\n    let x = ");
        assert_eq!(req.suffix, "\n");
        assert_eq!(req.snapshot.revision, 5);
        assert_eq!((req.snapshot.line, req.snapshot.column), (1, 12));
        let mid = build_request(&doc, (0, 3), 9, None, true).unwrap();
        assert_eq!(mid.prefix, "fn ");
        assert!(mid.suffix.starts_with("main() {"));
        assert!(mid.explicit);
    }

    #[test]
    fn line_tail_rule_ignores_whitespace_and_closers() {
        assert!(line_tail_is_short("", 8));
        assert!(line_tail_is_short("  );  ", 8));
        assert!(line_tail_is_short("abcdefgh", 8));
        assert!(!line_tail_is_short("abcdefghi", 8));
    }
}
