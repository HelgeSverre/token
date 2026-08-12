//! Find/replace search engine: literal, case-sensitive, whole-word, and
//! regex matching, shared by find navigation and the decoration-pipeline
//! match highlighting (see `docs/feature/find-enhancements.md`).
//!
//! Offsets are char offsets, not byte offsets — the offset space used
//! throughout this codebase's document API (`Document::cursor_to_offset`),
//! unlike the `regex` crate which reports byte offsets on `&str`.

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
    /// Error message if regex compilation failed (invalid regex, or an
    /// invalid literal pattern once escaped with word boundaries).
    pub error: Option<String>,
}

impl SearchQuery {
    pub fn new(pattern: &str, case_sensitive: bool, whole_word: bool, is_regex: bool) -> Self {
        let mut query = Self {
            pattern: pattern.to_string(),
            compiled: None,
            error: None,
        };
        query.compile(case_sensitive, whole_word, is_regex);
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

    /// Find all matches in `text`, converting the regex crate's byte
    /// offsets to char offsets incrementally (one pass over the matched
    /// spans, not a re-scan of the whole text per match).
    pub fn find_all(&self, text: &str) -> Vec<Match> {
        let Some(re) = &self.compiled else {
            return Vec::new();
        };

        let mut matches = Vec::new();
        let mut prev_byte = 0;
        let mut prev_char = 0;
        for m in re.find_iter(text) {
            prev_char += text[prev_byte..m.start()].chars().count();
            let start = prev_char;
            prev_char += text[m.start()..m.end()].chars().count();
            let end = prev_char;
            prev_byte = m.end();
            matches.push(Match { start, end });
        }
        matches
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_search_is_case_insensitive_by_default() {
        let query = SearchQuery::new("hello", false, false, false);
        let matches = query.find_all("Hello world, hello there");
        assert_eq!(matches.len(), 2);
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
        let matches = query.find_all("the other there");
        assert_eq!(matches, vec![Match { start: 0, end: 3 }]);
    }

    #[test]
    fn regex_search_finds_digit_runs() {
        let query = SearchQuery::new(r"\d+", false, false, true);
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
}
