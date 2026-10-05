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

/// Immutable match offsets, using half the payload of `Match` on 64-bit
/// platforms whenever the document's character-offset space fits in `u32`.
#[derive(Debug, Clone)]
pub struct MatchList {
    storage: MatchStorage,
}

impl Default for MatchList {
    fn default() -> Self {
        Self::new(0)
    }
}

#[derive(Debug, Clone)]
enum MatchStorage {
    Compact(Box<[[u32; 2]]>),
    Wide(Box<[Match]>),
}

impl MatchList {
    fn new(document_chars: usize) -> Self {
        Self {
            storage: if document_chars <= u32::MAX as usize {
                MatchStorage::Compact(Box::default())
            } else {
                MatchStorage::Wide(Box::default())
            },
        }
    }

    fn from_matches(document_chars: usize, matches: impl IntoIterator<Item = Match>) -> Self {
        let mut result = Self::new(document_chars);
        match &mut result.storage {
            MatchStorage::Compact(values) => {
                *values = matches
                    .into_iter()
                    .map(|m| {
                        [
                            u32::try_from(m.start).expect("offset fits document"),
                            u32::try_from(m.end).expect("offset fits document"),
                        ]
                    })
                    .collect()
            }
            MatchStorage::Wide(values) => *values = matches.into_iter().collect(),
        }
        result
    }

    pub fn len(&self) -> usize {
        match &self.storage {
            MatchStorage::Compact(values) => values.len(),
            MatchStorage::Wide(values) => values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, index: usize) -> Option<Match> {
        match &self.storage {
            MatchStorage::Compact(values) => values.get(index).map(|v| Match {
                start: v[0] as usize,
                end: v[1] as usize,
            }),
            MatchStorage::Wide(values) => values.get(index).copied(),
        }
    }

    pub fn first(&self) -> Option<Match> {
        self.get(0)
    }

    pub fn last(&self) -> Option<Match> {
        self.len().checked_sub(1).and_then(|index| self.get(index))
    }

    pub fn iter(&self) -> impl DoubleEndedIterator<Item = Match> + ExactSizeIterator + '_ {
        // Range::nth lets viewport consumers skip directly to a binary-searched
        // offset; a default Iterator::nth would walk every preceding match.
        (0..self.len()).map(|index| self.get(index).expect("index in bounds"))
    }

    pub fn partition_point(&self, mut predicate: impl FnMut(Match) -> bool) -> usize {
        let mut left = 0;
        let mut right = self.len();
        while left < right {
            let mid = left + (right - left) / 2;
            if predicate(self.get(mid).expect("index in bounds")) {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    }

    pub fn binary_search_by_key<K: Ord>(
        &self,
        key: &K,
        mut f: impl FnMut(Match) -> K,
    ) -> Result<usize, usize> {
        let mut left = 0;
        let mut right = self.len();
        while left < right {
            let mid = left + (right - left) / 2;
            match f(self.get(mid).expect("index in bounds")).cmp(key) {
                std::cmp::Ordering::Less => left = mid + 1,
                std::cmp::Ordering::Greater => right = mid,
                std::cmp::Ordering::Equal => return Ok(mid),
            }
        }
        Err(left)
    }

    #[cfg(test)]
    fn offset_payload_bytes(&self) -> usize {
        match &self.storage {
            MatchStorage::Compact(values) => values.len() * std::mem::size_of::<[u32; 2]>(),
            MatchStorage::Wide(values) => values.len() * std::mem::size_of::<Match>(),
        }
    }
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

    pub(crate) fn find_all_rope_compact(
        &self,
        text: &ropey::Rope,
        scope: Option<(usize, usize)>,
    ) -> MatchList {
        let document_chars = text.len_chars();
        if !self.is_valid() {
            return MatchList::new(document_chars);
        }
        if self.literal.is_some() && (self.case_sensitive || text.len_bytes() == document_chars) {
            MatchList::from_matches(document_chars, self.find_all_rope_iter(text, scope))
        } else {
            let contiguous = text.to_string();
            MatchList::from_matches(document_chars, self.find_all_iter(&contiguous, scope))
        }
    }

    /// As `find_all_rope`, but only allocates matches fully contained in `scope`.
    /// Searching remains document-wide so regex anchors and boundaries retain
    /// their authoritative meaning.
    pub(crate) fn find_all_rope_in(
        &self,
        text: &ropey::Rope,
        scope: Option<(usize, usize)>,
    ) -> Vec<Match> {
        self.find_all_rope_iter(text, scope).collect()
    }

    fn find_all_rope_iter<'a>(
        &'a self,
        text: &'a ropey::Rope,
        scope: Option<(usize, usize)>,
    ) -> Box<dyn Iterator<Item = Match> + 'a> {
        if !self.is_valid() {
            return Box::new(std::iter::empty());
        }
        if let Some(literal) = &self.literal {
            // Every non-ASCII UTF-8 character occupies more than one byte.
            // Rope stores both counts, so this proof does not scan the document.
            let ascii = text.len_bytes() == text.len_chars();
            if self.case_sensitive || ascii {
                return Box::new(
                    literal
                        .stream_find_iter(crate::util::text::RopeReader::new(text))
                        .map(move |result| {
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
                        .filter(move |m| {
                            scope.is_none_or(|(start, end)| m.start >= start && m.end <= end)
                        }),
                );
            }
        }
        Box::new(self.find_all_in(&text.to_string(), scope).into_iter())
    }

    /// Find all matches in `text`, converting the regex crate's byte
    /// offsets to char offsets incrementally (one pass over the matched
    /// spans, not a re-scan of the whole text per match).
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        self.find_all_in(text, None)
    }

    fn find_all_in(&self, text: &str, scope: Option<(usize, usize)>) -> Vec<Match> {
        self.find_all_iter(text, scope).collect()
    }

    fn find_all_iter<'a>(
        &'a self,
        text: &'a str,
        scope: Option<(usize, usize)>,
    ) -> Box<dyn Iterator<Item = Match> + 'a> {
        let Some(re) = &self.compiled else {
            return Box::new(std::iter::empty());
        };

        if let Some(literal) = self.literal.as_ref().filter(|_| text.is_ascii()) {
            // Byte and character offsets coincide only under this text gate.
            return Box::new(
                literal
                    .find_iter(text)
                    .map(|m| Match {
                        start: m.start(),
                        end: m.end(),
                    })
                    .filter(move |m| {
                        scope.is_none_or(|(start, end)| m.start >= start && m.end <= end)
                    }),
            );
        }

        let mut previous = (0, 0);
        Box::new(re.find_iter(text).filter_map(move |m| {
            previous.1 += text[previous.0..m.start()].chars().count();
            let start = previous.1;
            previous.1 += text[m.start()..m.end()].chars().count();
            let result = Match {
                start,
                end: previous.1,
            };
            previous.0 = m.end();
            scope
                .is_none_or(|(scope_start, scope_end)| {
                    result.start >= scope_start && result.end <= scope_end
                })
                .then_some(result)
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_list_selects_representation_at_u32_document_boundary() {
        let edge = Match {
            start: u32::MAX as usize - 1,
            end: u32::MAX as usize,
        };
        let compact = MatchList::from_matches(u32::MAX as usize, [edge]);
        assert!(matches!(compact.storage, MatchStorage::Compact(_)));
        assert_eq!(compact.get(0), Some(edge));
        assert_eq!(compact.offset_payload_bytes(), 8);

        let Some(beyond_offset) = (u32::MAX as usize).checked_add(1) else {
            return;
        };
        let beyond = Match {
            start: u32::MAX as usize,
            end: beyond_offset,
        };
        let wide = MatchList::from_matches(beyond_offset, [beyond]);
        assert!(matches!(wide.storage, MatchStorage::Wide(_)));
        assert_eq!(wide.get(0), Some(beyond));
        if usize::BITS == 64 {
            assert_eq!(wide.offset_payload_bytes(), 16);
        }
    }

    #[test]
    #[cfg(target_pointer_width = "64")]
    fn compact_dense_collection_and_wide_navigation_are_identical() {
        let expected: Vec<_> = (0..100_000)
            .map(|start| Match {
                start,
                end: start + 1,
            })
            .collect();
        let compact = MatchList::from_matches(100_000, expected.iter().copied());
        let wide = MatchList::from_matches(u32::MAX as usize + 1, expected.iter().copied());
        assert_eq!(compact.offset_payload_bytes(), expected.len() * 8);
        assert_eq!(compact.iter().collect::<Vec<_>>(), expected);
        assert_eq!(
            compact.iter().rev().collect::<Vec<_>>(),
            wide.iter().rev().collect::<Vec<_>>()
        );
        assert_eq!(compact.first(), wide.first());
        assert_eq!(compact.last(), wide.last());
        for list in [&compact, &wide] {
            assert_eq!(list.partition_point(|m| m.start < 54_321), 54_321);
            assert_eq!(list.binary_search_by_key(&76_543, |m| m.start), Ok(76_543));
            assert_eq!(
                list.binary_search_by_key(&100_000, |m| m.start),
                Err(100_000)
            );
            assert_eq!(list.get(100_000), None);
        }
        assert_eq!(
            compact.partition_point(|m| m.start < 54_321),
            wide.partition_point(|m| m.start < 54_321)
        );
        assert_eq!(
            compact.binary_search_by_key(&76_543, |m| m.start),
            wide.binary_search_by_key(&76_543, |m| m.start)
        );
    }

    #[test]
    fn compact_search_collects_dense_results_and_preserves_scoped_regex_semantics() {
        let rope = ropey::Rope::from_str(&"a".repeat(100_000));
        let dense = SearchQuery::new("a", true, false, false).find_all_rope_compact(&rope, None);
        assert_eq!(dense.len(), 100_000);
        assert_eq!(dense.offset_payload_bytes(), 800_000);
        assert_eq!(
            dense.iter().nth(99_999),
            Some(Match {
                start: 99_999,
                end: 100_000
            })
        );
        let text = format!("{}\nAK café\r\n", "é".repeat(1024));
        let rope = ropey::Rope::from_str(&text);
        for (pattern, sensitive, word, regex) in [
            ("café", true, false, false),
            ("k", false, false, false),
            ("A", false, true, false),
            ("(?m)^|$", true, false, true),
            ("(", true, false, true),
        ] {
            let query = SearchQuery::new(pattern, sensitive, word, regex);
            for scope in [None, Some((1025, 1032)), Some((0, 0))] {
                let expected: Vec<_> = query
                    .find_all(&text)
                    .into_iter()
                    .filter(|m| scope.is_none_or(|(start, end)| m.start >= start && m.end <= end))
                    .collect();
                assert_eq!(
                    query
                        .find_all_rope_compact(&rope, scope)
                        .iter()
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
    }

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
