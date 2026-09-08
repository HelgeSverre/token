//! Bounded workspace source chunks, using the existing language/outline registry.
//! Filesystem policy and cancellation belong to the runtime worker.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use super::recency::{ContextChunk, CHUNK_BYTES, FILENAME_BYTES};
use crate::syntax::{registry, LanguageId};
use crate::util::ByteSize;

pub const FILE_BYTES: ByteSize = ByteSize::kibibytes(256);
pub const INDEX_BYTES: ByteSize = ByteSize::mebibytes(8);
pub const MAX_FILES: usize = 512;
const MAX_FILE_CHUNKS: usize = 128;

struct Source {
    text: String,
    lines: usize,
    chunks: Vec<Range<usize>>,
}

/// Worker-local cache. Exact source equality avoids stale timestamp-based reuse.
#[derive(Default)]
pub struct RetrievalIndex {
    sources: BTreeMap<String, Source>,
    parser: tree_sitter::Parser,
}

impl RetrievalIndex {
    /// Evict anything not observed in the current ignore-aware traversal.
    pub fn retain(&mut self, filenames: &HashSet<String>) {
        self.sources.retain(|name, _| filenames.contains(name));
    }

    /// Cache declaration ranges, falling back to line windows without an outline.
    /// The caller bounds total file count/bytes before calling this method.
    pub fn update(&mut self, filename: String, text: String, lines: usize, cancelled: &AtomicBool) {
        if text.len() > FILE_BYTES.as_usize()
            || text.contains('\0')
            || !(1..=256).contains(&lines)
            || filename.len() > FILENAME_BYTES.as_usize()
            || filename
                .chars()
                .any(|c| c.is_control() || matches!(c, '<' | '>'))
        {
            self.sources.remove(&filename);
            return;
        }
        if self
            .sources
            .get(&filename)
            .is_some_and(|s| s.text == text && s.lines == lines)
        {
            return;
        }
        let language = LanguageId::from_path(std::path::Path::new(&filename));
        let definition = registry::language(language);
        let Some(grammar) = definition.parser.as_ref() else {
            return;
        };
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(byte, _)| byte + 1));
        let mut chunks = Vec::new();
        self.parser.reset();
        if self.parser.set_language(&(grammar.grammar)()).is_ok() {
            let deadline = std::time::Instant::now() + std::time::Duration::from_millis(25);
            let mut progress = |_: &tree_sitter::ParseState| {
                cancelled.load(Ordering::Relaxed) || std::time::Instant::now() >= deadline
            };
            let options = tree_sitter::ParseOptions::new().progress_callback(&mut progress);
            let tree = self.parser.parse_with_options(
                &mut |byte, _| &text.as_bytes()[byte..],
                None,
                Some(options),
            );
            if let Some(tree) = tree {
                let outline = definition.outline.extract(tree.root_node(), &text);
                let mut nodes: Vec<_> = outline.iter().rev().collect();
                while let Some(node) = nodes.pop() {
                    if chunks.len() >= MAX_FILE_CHUNKS || cancelled.load(Ordering::Relaxed) {
                        break;
                    }
                    let start = node.range.start_line.min(starts.len() - 1);
                    let end = (node.range.end_line + 1).min(start + lines);
                    chunks.push(starts[start]..starts.get(end).copied().unwrap_or(text.len()));
                    nodes.extend(node.children.iter().rev());
                }
            }
        }
        if chunks.is_empty() && !cancelled.load(Ordering::Relaxed) {
            for start in (0..starts.len()).step_by(lines).take(MAX_FILE_CHUNKS) {
                chunks
                    .push(starts[start]..starts.get(start + lines).copied().unwrap_or(text.len()));
            }
        }
        for chunk in &mut chunks {
            chunk.end = chunk.end.min(chunk.start + CHUNK_BYTES.as_usize());
            while !text.is_char_boundary(chunk.end) {
                chunk.end -= 1;
            }
        }
        chunks.retain(|range| !text[range.clone()].trim().is_empty());
        chunks.sort_unstable_by_key(|range| (range.start, range.end));
        chunks.dedup();
        if !cancelled.load(Ordering::Relaxed) {
            self.sources.insert(
                filename,
                Source {
                    text,
                    lines,
                    chunks,
                },
            );
        }
    }

    /// BM25 over exact identifiers near the cursor; zero-overlap chunks never
    /// leave the index. Stable ties and output order preserve prompt prefixes.
    pub fn select(
        &self,
        prefix: &str,
        suffix: &str,
        limit: usize,
        cancelled: &AtomicBool,
    ) -> Vec<ContextChunk> {
        let prefix_start = prefix
            .char_indices()
            .rev()
            .nth(1023)
            .map_or(0, |(byte, _)| byte);
        let suffix_end = suffix
            .char_indices()
            .nth(256)
            .map_or(suffix.len(), |(byte, _)| byte);
        let query: HashSet<_> = identifiers(&prefix[prefix_start..])
            .rev()
            .chain(identifiers(&suffix[..suffix_end]))
            .take(128)
            .collect();
        if query.is_empty() {
            return Vec::new();
        }
        let mut candidates = Vec::new();
        let mut frequencies: HashMap<&str, usize> = HashMap::new();
        let mut total_length = 0;
        let mut remaining = INDEX_BYTES.as_usize();
        for (filename, source) in &self.sources {
            for range in &source.chunks {
                if cancelled.load(Ordering::Relaxed) {
                    return Vec::new();
                }
                if range.len() > remaining {
                    continue;
                }
                remaining -= range.len();
                // Ordered terms also make floating-point score summation stable.
                let mut counts = BTreeMap::<&str, usize>::new();
                let mut length = 0;
                for term in identifiers(&source.text[range.clone()]) {
                    length += 1;
                    if query.contains(term) {
                        *counts.entry(term).or_default() += 1;
                    }
                }
                for term in counts.keys() {
                    *frequencies.entry(term).or_default() += 1;
                }
                total_length += length;
                candidates.push((filename, source, range, length, counts));
            }
        }
        let count = candidates.len() as f64;
        let average = (total_length as f64 / count).max(1.0);
        let mut ranked: Vec<_> = candidates
            .into_iter()
            .filter_map(|(filename, source, range, length, counts)| {
                let score: f64 = counts
                    .into_iter()
                    .map(|(term, frequency)| {
                        let df = frequencies[term] as f64;
                        // Positive Robertson IDF, k1=1.2, b=0.75.
                        let idf = (1.0 + (count - df + 0.5) / (df + 0.5)).ln();
                        let tf = frequency as f64;
                        idf * tf * 2.2 / (tf + 1.2 * (0.25 + 0.75 * length as f64 / average))
                    })
                    .sum();
                (score > 0.0).then_some((score, filename, source, range))
            })
            .collect();
        ranked.sort_by(|a, b| {
            b.0.total_cmp(&a.0)
                .then_with(|| a.1.cmp(b.1))
                .then_with(|| a.3.start.cmp(&b.3.start))
        });
        let mut selected: Vec<ContextChunk> = Vec::new();
        let mut regions: Vec<(&str, &Range<usize>)> = Vec::new();
        for (_, filename, source, range) in ranked {
            if selected.len() >= limit.min(super::recency::MAX_CHUNKS) {
                break;
            }
            let text = &source.text[range.clone()];
            if regions.iter().any(|(name, prior)| {
                *name == filename && prior.start < range.end && range.start < prior.end
            }) || selected.iter().any(|chunk| chunk.text == text)
            {
                continue;
            }
            regions.push((filename, range));
            selected.push(ContextChunk {
                filename: filename.clone(),
                text: text.to_owned(),
            });
        }
        selected.sort_by(|a, b| {
            a.filename
                .cmp(&b.filename)
                .then_with(|| a.text.cmp(&b.text))
        });
        selected
    }
}

fn identifiers(text: &str) -> impl DoubleEndedIterator<Item = &str> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|term| term.len() > 2 && term.chars().any(char::is_alphabetic))
        .filter(|term| {
            !matches!(
                *term,
                "let"
                    | "mut"
                    | "pub"
                    | "self"
                    | "Self"
                    | "super"
                    | "crate"
                    | "use"
                    | "impl"
                    | "struct"
                    | "enum"
                    | "trait"
                    | "type"
                    | "return"
                    | "true"
                    | "false"
                    | "None"
                    | "Some"
                    | "const"
                    | "static"
                    | "async"
                    | "await"
                    | "match"
                    | "else"
                    | "for"
                    | "while"
                    | "loop"
                    | "function"
                    | "class"
                    | "def"
                    | "import"
                    | "from"
                    | "export"
                    | "public"
                    | "private"
                    | "void"
                    | "int"
                    | "new"
                    | "null"
                    | "this"
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retrieval_ranks_declarations_and_drops_unrelated_or_stale_source() {
        let cancelled = AtomicBool::new(false);
        let mut index = RetrievalIndex::default();
        index.update("helpers.rs".into(), "fn unrelated() { println!(\"noise\"); }\nfn parse_widget(widget: Widget) -> Widget { widget }\n".into(), 64, &cancelled);
        index.update(
            "other.rs".into(),
            "fn unrelated_again() {}\n".into(),
            64,
            &cancelled,
        );
        let chunks = index.select("let result = parse_widget(", "", 8, &cancelled);
        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].text,
            "fn parse_widget(widget: Widget) -> Widget { widget }\n"
        );
        assert!(index.select("let x = ", "", 8, &cancelled).is_empty());
        index.update(
            "helpers.rs".into(),
            "fn changed() {}\n".into(),
            64,
            &cancelled,
        );
        assert!(index.select("parse_widget", "", 8, &cancelled).is_empty());
        index.update(
            "helpers.rs".into(),
            format!(
                "fn parse_widget() {{ /* {} */ }}",
                "é".repeat(CHUNK_BYTES.as_usize())
            ),
            64,
            &cancelled,
        );
        let chunks = index.select("parse_widget", "", 8, &cancelled);
        assert_eq!(chunks.len(), 1);
        assert!(chunks[0].text.len() <= CHUNK_BYTES.as_usize());
        index.retain(&HashSet::new());
        assert!(index.select("parse_widget", "", 8, &cancelled).is_empty());
    }
}
