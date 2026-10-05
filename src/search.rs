//! Find/replace search engine: literal, case-sensitive, whole-word, and
//! regex matching, shared by find navigation and the decoration-pipeline
//! match highlighting (see `docs/feature/find-enhancements.md`).
//!
//! Offsets are char offsets, not byte offsets — the offset space used
//! throughout this codebase's document API (`Document::cursor_to_offset`),
//! unlike the `regex` crate which reports byte offsets on `&str`.

use aho_corasick::AhoCorasick;
use regex::Regex;

/// One match's char-offset range in a document, half-open like `Selection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Match {
    pub start: usize,
    pub end: usize,
}

/// A compiled search query with case/whole-word/regex options.
#[derive(Debug, Clone)]
pub struct SearchQuery {
    pattern: String,
    compiled: Option<Regex>,
    /// Optional acceleration for literals whose byte matching has regex-equivalent
    /// semantics. This includes every case-sensitive literal and ASCII-only
    /// case-insensitive literals; the regex remains authoritative otherwise.
    literal: Option<AhoCorasick>,
    case_sensitive: bool,
    /// Error message if regex compilation failed (invalid regex, or an
    /// invalid literal pattern once escaped with word boundaries).
    pub error: Option<String>,
}

impl SearchQuery {
    pub fn new(pattern: &str, case_sensitive: bool, whole_word: bool, is_regex: bool) -> Self {
        let mut query = Self {
            pattern: pattern.to_string(),
            compiled: None,
            literal: None,
            case_sensitive,
            error: None,
        };
        query.compile(case_sensitive, whole_word, is_regex);
        if query.is_valid() && !whole_word && !is_regex && (case_sensitive || pattern.is_ascii()) {
            // One fixed-length pattern has the same non-overlapping match order
            // as the regex. Construction failure simply keeps the regex path.
            query.literal = AhoCorasick::builder()
                .ascii_case_insensitive(!case_sensitive)
                .build([pattern])
                .ok();
        }
        query
    }

    fn compile(&mut self, case_sensitive: bool, whole_word: bool, is_regex: bool) {
        if self.pattern.is_empty() {
            self.compiled = None;
            self.error = None;
            return;
        }

        let body = if is_regex {
            self.pattern.clone()
        } else {
            regex::escape(&self.pattern)
        };
        let body = if whole_word {
            format!(r"\b{}\b", body)
        } else {
            body
        };
        let full_pattern = if case_sensitive {
            body
        } else {
            format!("(?i){}", body)
        };

        match Regex::new(&full_pattern) {
            Ok(re) => {
                self.compiled = Some(re);
                self.error = None;
            }
            Err(e) => {
                self.compiled = None;
                self.error = Some(e.to_string());
            }
        }
    }

    /// Whether the query has a non-empty pattern that compiled successfully.
    pub fn is_valid(&self) -> bool {
        !self.pattern.is_empty() && self.compiled.is_some()
    }

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    /// Stream safe literal matches across rope chunks; regex and Unicode case
    /// folding retain the contiguous engine so chunk boundaries never change semantics.
    pub fn find_all_rope(&self, text: &ropey::Rope) -> Vec<Match> {
        self.find_all_rope_in(text, None)
    }

    /// As `find_all_rope`, but only allocates matches fully contained in `scope`.
    /// Searching remains document-wide so regex anchors and boundaries retain
    /// their authoritative meaning.
    pub(crate) fn find_all_rope_in(
        &self,
        text: &ropey::Rope,
        scope: Option<(usize, usize)>,
    ) -> Vec<Match> {
        if !self.is_valid() {
            return Vec::new();
        }
        if let Some(literal) = &self.literal {
            // Every non-ASCII UTF-8 character occupies more than one byte.
            // Rope stores both counts, so this proof does not scan the document.
            let ascii = text.len_bytes() == text.len_chars();
            if self.case_sensitive || ascii {
                return literal
                    .stream_find_iter(crate::util::text::RopeReader::new(text))
                    .map(|result| {
                        let found = result.expect("in-memory rope reads cannot fail");
                        let char_offset = |byte| {
                            if ascii {
                                byte
                            } else {
                                text.byte_to_char(byte)
                            }
                        };
                        Match {
                            start: char_offset(found.start()),
                            end: char_offset(found.end()),
                        }
                    })
                    .filter(|m| scope.is_none_or(|(start, end)| m.start >= start && m.end <= end))
                    .collect();
            }
        }
        self.find_all_in(&text.to_string(), scope)
    }

    /// Find all matches in `text`, converting the regex crate's byte
    /// offsets to char offsets incrementally (one pass over the matched
    /// spans, not a re-scan of the whole text per match).
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        self.find_all_in(text, None)
    }

    fn find_all_in(&self, text: &str, scope: Option<(usize, usize)>) -> Vec<Match> {
        let Some(re) = &self.compiled else {
            return Vec::new();
        };

        if let Some(literal) = self.literal.as_ref().filter(|_| text.is_ascii()) {
            // Byte and character offsets coincide only under this text gate.
            return literal
                .find_iter(text)
                .map(|m| Match {
                    start: m.start(),
                    end: m.end(),
                })
                .filter(|m| scope.is_none_or(|(start, end)| m.start >= start && m.end <= end))
                .collect();
        }

        let mut matches = Vec::new();
        let mut prev_byte = 0;
        let mut prev_char = 0;
        for m in re.find_iter(text) {
            prev_char += text[prev_byte..m.start()].chars().count();
            let start = prev_char;
            prev_char += text[m.start()..m.end()].chars().count();
            let end = prev_char;
            prev_byte = m.end();
            if scope.is_none_or(|(scope_start, scope_end)| start >= scope_start && end <= scope_end)
            {
                matches.push(Match { start, end });
            }
        }
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rope_search_preserves_boundaries_offsets_and_unicode_case_folding() {
        let pattern = "Ab".repeat(1024);
        let rope = ropey::Rope::from_str(&format!("{}{pattern}z", "p".repeat(700)));
        assert!(rope.chunks().all(|chunk| chunk.len() < pattern.len()));
        let query = SearchQuery::new(&pattern.to_lowercase(), false, false, false);
        assert_eq!(
            query.find_all_rope(&rope),
            vec![Match {
                start: 700,
                end: 2748
            }]
        );
        for (text, pattern, sensitive, expected) in [
            (
                "aaaaa",
                "aa",
                true,
                vec![Match { start: 0, end: 2 }, Match { start: 2, end: 4 }],
            ),
            (
                "ætail🙂tail",
                "tail",
                true,
                vec![Match { start: 1, end: 5 }, Match { start: 6, end: 10 }],
            ),
            (
                "KkK",
                "k",
                false,
                vec![
                    Match { start: 0, end: 1 },
                    Match { start: 1, end: 2 },
                    Match { start: 2, end: 3 },
                ],
            ),
        ] {
            let query = SearchQuery::new(pattern, sensitive, false, false);
            assert_eq!(query.find_all_rope(&ropey::Rope::from_str(text)), expected);
        }
        let text = ropey::Rope::from_str("one\ntwo");
        assert_eq!(
            SearchQuery::new("one\\ntwo", true, false, true).find_all_rope(&text),
            vec![Match { start: 0, end: 7 }]
        );
        assert!(SearchQuery::new("", false, false, false)
            .find_all_rope(&text)
            .is_empty());
    }

    #[test]
    fn literal_search_is_case_insensitive_by_default() {
        let query = SearchQuery::new("hello", false, false, false);
        let matches = query.find_all("Hello world, hello there");
        assert_eq!(matches.len(), 2);

        // Compare the fast path to the unchanged engine, including overlapping
        // candidates, literal regex punctuation and Unicode fold equivalents.
        for pattern in [
            "hello", "aa", "a.a", "[", "\n", "\0", "k", "s", "İ", "σ", "",
        ] {
            for case_sensitive in [false, true] {
                let query = SearchQuery::new(pattern, case_sensitive, false, false);
                let mut fallback = query.clone();
                fallback.literal = None;
                assert_eq!(
                    query.literal.is_some(),
                    !pattern.is_empty() && (case_sensitive || pattern.is_ascii())
                );
                for text in [
                    "",
                    "Hello hello HELLO",
                    "aaaaa",
                    "a.a [\n\0",
                    "kKK sSſ",
                    "İi σΣς",
                    "猫hello🙂",
                ] {
                    assert_eq!(
                        query.find_all(text),
                        fallback.find_all(text),
                        "pattern={pattern:?}, text={text:?}, case_sensitive={case_sensitive}"
                    );
                }
            }
        }
    }

    #[test]
    fn case_sensitive_search_matches_exact_case_only() {
        let query = SearchQuery::new("Hello", true, false, false);
        let matches = query.find_all("Hello world, hello there");
        assert_eq!(matches, vec![Match { start: 0, end: 5 }]);
    }

    #[test]
    fn whole_word_excludes_partial_matches() {
        let query = SearchQuery::new("the", false, true, false);
        assert!(query.literal.is_none());
        let matches = query.find_all("the other there");
        assert_eq!(matches, vec![Match { start: 0, end: 3 }]);
    }

    #[test]
    fn regex_search_finds_digit_runs() {
        let query = SearchQuery::new(r"\d+", false, false, true);
        assert!(query.literal.is_none());
        let matches = query.find_all("abc 123 def 456 ghi");
        assert_eq!(
            matches,
            vec![Match { start: 4, end: 7 }, Match { start: 12, end: 15 }]
        );
    }

    #[test]
    fn invalid_regex_reports_error_and_is_not_valid() {
        let query = SearchQuery::new("[invalid", false, false, true);
        assert!(query.has_error());
        assert!(!query.is_valid());
        assert!(query.find_all("[invalid text").is_empty());
    }

    #[test]
    fn empty_pattern_is_not_valid_and_has_no_error() {
        let query = SearchQuery::new("", false, false, false);
        assert!(!query.is_valid());
        assert!(!query.has_error());
        assert!(query.find_all("anything").is_empty());
    }

    #[test]
    fn char_offsets_account_for_multi_byte_characters() {
        // "é" is 2 bytes in UTF-8 but 1 char; offsets must be in chars.
        let query = SearchQuery::new("world", false, false, false);
        let matches = query.find_all("café world");
        assert_eq!(matches, vec![Match { start: 5, end: 10 }]);
    }

    #[test]
    fn rope_paths_match_authoritative_regex_across_adversarial_chunks() {
        // The long, mixed-width padding forces many Rope chunks and puts
        // candidates at different byte/char offsets and chunk boundaries.
        let padding = "a猫🙂β_".repeat(2_000);
        let text = format!("{padding}é🙂猫é foo foobar foo\nΣσς\n{padding}é🙂猫é");
        let rope = ropey::Rope::from_str(&text);
        assert!(rope.chunks().count() > 2);

        for (pattern, sensitive, whole_word, is_regex) in [
            ("é🙂猫é", true, false, false),
            ("foo", true, false, false),
            ("foo", true, true, false),
            ("σ", false, false, false),
            (r"(?m)^|$", true, false, true),
            (r"é🙂猫é|foo\b", true, false, true),
        ] {
            let query = SearchQuery::new(pattern, sensitive, whole_word, is_regex);
            let expected = query.find_all(&text);
            assert_eq!(
                query.find_all_rope(&rope),
                expected,
                "pattern={pattern:?}, sensitive={sensitive}, whole_word={whole_word}, regex={is_regex}"
            );

            for scope in [
                (0, 0),
                (padding.chars().count(), padding.chars().count() + 4),
                (1, text.chars().count() - 1),
            ] {
                let scoped: Vec<_> = expected
                    .iter()
                    .copied()
                    .filter(|m| m.start >= scope.0 && m.end <= scope.1)
                    .collect();
                assert_eq!(query.find_all_rope_in(&rope, Some(scope)), scoped);
            }
        }
    }
}
