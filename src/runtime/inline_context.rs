//! Idle-committed, window-local recency context. No filesystem reads, retrieval
//! index, worker thread or full-document snapshots. Pending work is positions only.

use std::collections::VecDeque;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use token::completion::provider::InlineJob;
use token::completion::recency::{ContextChunk, CHUNK_BYTES, FILENAME_BYTES};
use token::config::ProviderConfig;
use token::model::{AppModel, Document, DocumentId};

const IDLE: Duration = Duration::from_millis(750);

#[derive(PartialEq, Eq)]
struct Scope {
    provider_name: String,
    provider: ProviderConfig,
    root: Option<PathBuf>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Visit {
    document: DocumentId,
    line: usize,
    column: usize,
    revision: u64,
}

struct Anchor {
    document: DocumentId,
    line: usize,
}

struct Stored {
    document: DocumentId,
    lines: std::ops::Range<usize>,
    path: Option<PathBuf>,
    chunk: IndexedChunk,
}

/// Byte ranges into our own immutable payload, sorted by token text. This
/// avoids rebuilding two allocating trees for every pairwise comparison.
/// Only the payload leaves runtime; indexes are neither transmitted nor cached.
struct IndexedChunk {
    payload: ContextChunk,
    tokens: Box<[Range<usize>]>,
}

impl IndexedChunk {
    fn new(payload: ContextChunk) -> Self {
        let text = &payload.text;
        let mut tokens = Vec::new();
        let mut start = None;
        for (byte, ch) in text.char_indices() {
            if ch.is_alphanumeric() || ch == '_' {
                start.get_or_insert(byte);
            } else if let Some(start) = start.take() {
                tokens.push(start..byte);
            }
        }
        if let Some(start) = start {
            tokens.push(start..text.len());
        }
        tokens.sort_unstable_by(|a, b| text[a.clone()].cmp(&text[b.clone()]));
        tokens.dedup_by(|a, b| text[a.clone()] == text[b.clone()]);
        Self {
            payload,
            tokens: tokens.into_boxed_slice(),
        }
    }

    fn similar(&self, other: &Self) -> bool {
        let total = self.tokens.len() + other.tokens.len();
        if total == 0 {
            return self.payload.text == other.payload.text;
        }
        let (mut left, mut right, mut intersection) = (0, 0, 0);
        while left < self.tokens.len() && right < other.tokens.len() {
            // Even matching every remaining token cannot exceed the strict
            // threshold. This is an exact bound, not approximate matching.
            let possible =
                intersection + (self.tokens.len() - left).min(other.tokens.len() - right);
            if possible * 10 <= (total - possible) * 9 {
                return false;
            }
            match self.payload.text[self.tokens[left].clone()]
                .cmp(&other.payload.text[other.tokens[right].clone()])
            {
                std::cmp::Ordering::Less => left += 1,
                std::cmp::Ordering::Greater => right += 1,
                std::cmp::Ordering::Equal => {
                    intersection += 1;
                    left += 1;
                    right += 1;
                }
            }
        }
        intersection * 10 > (total - intersection) * 9
    }
}

#[derive(Default)]
pub(super) struct InlineContextRing {
    scope: Option<Scope>,
    active: Option<Visit>,
    pending: VecDeque<Anchor>,
    chunks: VecDeque<Stored>,
    deadline: Option<Instant>,
}

impl InlineContextRing {
    pub(super) fn deadline(&self) -> Option<Instant> {
        self.deadline
    }

    /// Observe only editor state, not syntax replies, redraws or cursor blinking.
    /// Called before requests and after each batch of runtime events.
    pub(super) fn observe(&mut self, model: &AppModel, now: Instant) {
        let completion = &model.config.completion;
        let provider = completion
            .providers
            .get(&completion.inline.provider)
            .filter(|_| completion.enabled && completion.inline.enabled)
            .filter(|provider| {
                matches!(
                    provider.context,
                    token::completion::recency::ContextStrategy::RecencyRing { .. }
                )
            });
        let Some(provider) = provider else {
            *self = Self::default();
            return;
        };
        let same_scope = self.scope.as_ref().is_some_and(|scope| {
            scope.provider_name == completion.inline.provider
                && scope.provider == *provider
                && scope.root.as_ref() == model.workspace_root()
        });
        if !same_scope {
            *self = Self {
                scope: Some(Scope {
                    provider_name: completion.inline.provider.clone(),
                    provider: provider.clone(),
                    root: model.workspace_root().cloned(),
                }),
                ..Self::default()
            };
        }
        let Ok(Some((max_chunks, chunk_lines))) = provider.context.limits() else {
            return;
        };
        self.pending
            .retain(|anchor| eligible_document(model, anchor.document).is_some());
        self.chunks.retain(|stored| {
            eligible_document(model, stored.document)
                .is_some_and(|document| source_path(document) == stored.path.as_deref())
        });
        let current = model
            .editor_area
            .focused_document_id()
            .filter(|_| model.editor().is_plain_text_mode())
            .and_then(|id| eligible_document(model, id))
            .and_then(|document| {
                document.id.map(|id| {
                    let cursor = model.editor().active_cursor();
                    Visit {
                        document: id,
                        line: cursor.line,
                        column: cursor.column,
                        revision: document.revision,
                    }
                })
            });
        if current != self.active {
            if let Some(previous) = self.active {
                if current.is_none_or(|visit| {
                    visit.document != previous.document
                        || visit.line.abs_diff(previous.line) >= chunk_lines
                }) {
                    self.enqueue(previous.document, previous.line, max_chunks);
                }
            }
            if let Some(visit) = current {
                if self.active.is_none_or(|previous| {
                    previous.document != visit.document
                        || visit.line.abs_diff(previous.line) >= chunk_lines
                }) {
                    self.enqueue(visit.document, visit.line, max_chunks);
                }
            }
            self.active = current;
            if !self.pending.is_empty() {
                self.deadline = Some(now + IDLE);
            }
        }
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            while let Some(anchor) = self.pending.pop_front() {
                let Some(document) = eligible_document(model, anchor.document) else {
                    continue;
                };
                // Revisiting an overlapping region replaces its snapshot even
                // after a substantial rewrite (or deletion to empty text).
                let lines = line_range(document, anchor.line, chunk_lines);
                self.chunks.retain(|stored| {
                    stored.document != anchor.document
                        || stored.lines.start >= lines.end
                        || lines.start >= stored.lines.end
                });
                let Some(chunk) = capture(model, document, anchor.line, chunk_lines) else {
                    continue;
                };
                // Strict >0.9 Jaccard similarity; no ranking and no hash-only
                // equality. Index a captured snippet once, not once per pair.
                let chunk = IndexedChunk::new(chunk);
                self.chunks.retain(|stored| !stored.chunk.similar(&chunk));
                while self.chunks.len() >= max_chunks {
                    self.chunks.pop_front();
                }
                self.chunks.push_back(Stored {
                    document: anchor.document,
                    lines,
                    path: source_path(document).map(Path::to_path_buf),
                    chunk,
                });
            }
            self.deadline = None;
        }
        if self.pending.is_empty() {
            self.deadline = None;
        }
    }

    fn enqueue(&mut self, document: DocumentId, line: usize, limit: usize) {
        self.pending
            .retain(|anchor| !(anchor.document == document && anchor.line == line));
        while self.pending.len() >= limit {
            self.pending.pop_front();
        }
        self.pending.push_back(Anchor { document, line });
    }

    /// Successful saves can belong to a background pane. Capture its current
    /// open buffer at idle, never the full saved snapshot carried by the reply.
    pub(super) fn saved(&mut self, model: &AppModel, document: DocumentId, now: Instant) {
        self.observe(model, now);
        let Some(scope) = &self.scope else {
            return;
        };
        let Ok(Some((limit, _))) = scope.provider.context.limits() else {
            return;
        };
        let Some(editor) = model
            .editor_area
            .editors
            .values()
            .find(|editor| editor.document_id == Some(document) && editor.is_plain_text_mode())
        else {
            return;
        };
        if eligible_document(model, document).is_some() {
            self.enqueue(document, editor.active_cursor().line, limit);
            self.deadline = Some(now + IDLE);
        }
    }

    pub(super) fn attach(&mut self, model: &AppModel, job: &mut InlineJob, now: Instant) {
        self.observe(model, now);
        job.request.extra_context.clear();
        if self
            .scope
            .as_ref()
            .is_some_and(|scope| scope.provider == job.provider)
            && eligible_document(model, job.request.snapshot.document_id).is_some()
        {
            job.request.extra_context.extend(
                self.chunks
                    .iter()
                    .map(|stored| stored.chunk.payload.clone()),
            );
        }
    }
}

fn source_path(document: &Document) -> Option<&Path> {
    document
        .file_identity()
        .map(|identity| identity.path())
        .or(document.file_path.as_deref())
}

fn eligible_document(model: &AppModel, id: DocumentId) -> Option<&Document> {
    let document = model.editor_area.documents.get(&id)?;
    if !model
        .editor_area
        .editors
        .values()
        .any(|editor| editor.document_id == Some(id) && editor.is_plain_text_mode())
    {
        return None;
    }
    if let Some(root) = model.workspace_root() {
        if document.file_path.is_some() {
            // Only a boundary-resolved path can prove symlinks stay in scope.
            if !document.file_identity()?.path().starts_with(root) {
                return None;
            }
        }
    }
    Some(document)
}

fn line_range(document: &Document, line: usize, lines: usize) -> std::ops::Range<usize> {
    let count = document.buffer.len_lines();
    let start = line.min(count.saturating_sub(1)).saturating_sub(lines / 2);
    start..(start + lines).min(count)
}

fn capture(
    model: &AppModel,
    document: &Document,
    line: usize,
    lines: usize,
) -> Option<ContextChunk> {
    let filename = match source_path(document) {
        Some(path) => match model.workspace_root() {
            Some(root) => path.strip_prefix(root).ok()?.to_str()?.to_owned(),
            None => format!("buffer-{}/{}", document.id?.0, path.file_name()?.to_str()?),
        },
        None => format!("untitled-{}", document.id?.0),
    };
    if filename.len() > FILENAME_BYTES.as_usize()
        || filename
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>'))
    {
        return None;
    }
    let count = document.buffer.len_lines();
    let range = line_range(document, line, lines);
    let start = document.buffer.line_to_char(range.start);
    let end = if range.end == count {
        document.buffer.len_chars()
    } else {
        document.buffer.line_to_char(range.end)
    };
    let mut text = String::with_capacity(CHUNK_BYTES.as_usize());
    for ch in document.buffer.slice(start..end).chars() {
        if text.len() + ch.len_utf8() > CHUNK_BYTES.as_usize() {
            break;
        }
        text.push(ch);
    }
    if text.trim().is_empty() {
        return None;
    }
    text.shrink_to_fit();
    Some(ContextChunk { filename, text })
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::completion::recency::ContextStrategy;

    // Harness-less benchmarks compile this module with cfg(test), but omit
    // #[test] functions. The helper remains used by the normal binary tests.
    #[allow(dead_code)]
    fn model() -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
        model.config.completion.enabled = true;
        model.config.completion.inline.enabled = true;
        model.config.completion.inline.provider = "recency-fixture".into();
        model.config.completion.providers.insert(
            "recency-fixture".into(),
            ProviderConfig {
                context: ContextStrategy::RecencyRing {
                    max_chunks: 8,
                    chunk_lines: 4,
                },
                ..Default::default()
            },
        );
        model.document_mut().buffer = "alpha beta gamma\n".into();
        model
    }

    #[test]
    fn recency_commits_only_after_editor_idle_and_keeps_a_stable_prefix() {
        let mut model = model();
        let mut ring = InlineContextRing::default();
        let start = Instant::now();
        ring.observe(&model, start);
        assert!(ring.chunks.is_empty());
        model.document_mut().buffer.insert(0, "edited ");
        model.document_mut().revision += 1;
        ring.observe(&model, start + IDLE / 2);
        ring.observe(&model, start + IDLE);
        assert!(ring.chunks.is_empty(), "editing extends the idle deadline");
        ring.observe(&model, start + IDLE * 2);
        assert_eq!(ring.chunks.len(), 1);
        let committed = ring.chunks[0].chunk.payload.clone();
        model.document_mut().buffer.insert(0, "later ");
        model.document_mut().revision += 1;
        ring.observe(&model, start + IDLE * 3);
        assert_eq!(ring.chunks[0].chunk.payload, committed);
        assert!(
            ring.deadline().is_none(),
            "no idle spin without pending captures"
        );
        ring.saved(&model, model.document().id.unwrap(), start + IDLE * 4);
        ring.observe(&model, start + IDLE * 5);
        assert!(ring
            .chunks
            .back()
            .unwrap()
            .chunk
            .payload
            .text
            .starts_with("later edited"));
        assert_eq!(
            ring.chunks.len(),
            1,
            "rewrites replace the same region even below the similarity threshold"
        );
        model.document_mut().buffer = "\n".into();
        model.document_mut().revision += 1;
        ring.saved(&model, model.document().id.unwrap(), start + IDLE * 6);
        ring.observe(&model, start + IDLE * 7);
        assert!(
            ring.chunks.is_empty(),
            "a deleted region cannot retain its old source"
        );
    }

    #[test]
    fn recency_switches_jumps_close_and_provider_changes_have_bounded_lifetimes() {
        let mut model = model();
        let first = model.document().id.unwrap();
        model.document_mut().buffer =
            "zero\none\ntwo\nthree\nfour\nfive\nsix\nseven\neight\n".into();
        let mut ring = InlineContextRing::default();
        let start = Instant::now();
        ring.observe(&model, start);
        ring.observe(&model, start + IDLE);
        model.editor_mut().cursors[0].line = 8;
        ring.observe(&model, start + IDLE * 2);
        assert_eq!(ring.pending.len(), 2, "both sides of a large jump");
        ring.observe(&model, start + IDLE * 3);
        assert_eq!(ring.chunks.len(), 2);
        let _ = token::update::update(
            &mut model,
            token::messages::Msg::Layout(token::messages::LayoutMsg::NewTab),
        );
        model.document_mut().buffer = "unrelated newly opened helper\n".into();
        ring.observe(&model, start + IDLE * 4);
        ring.observe(&model, start + IDLE * 5);
        assert_eq!(ring.chunks.len(), 3);
        model.editor_area.documents.remove(&first);
        ring.observe(&model, start + IDLE * 6);
        assert_eq!(ring.chunks.len(), 1);
        assert!(ring.chunks.iter().all(|stored| stored.document != first));
        model
            .config
            .completion
            .providers
            .get_mut("recency-fixture")
            .unwrap()
            .url
            .push_str("/other");
        ring.observe(&model, start + IDLE * 7);
        assert!(
            ring.chunks.is_empty(),
            "a new endpoint cannot inherit old context"
        );
        model.config.completion.inline.enabled = false;
        ring.observe(&model, start + IDLE * 8);
        assert!(ring.pending.is_empty());
        assert!(ring.scope.is_none());
        assert!(ring.deadline().is_none());
    }

    #[test]
    fn recency_similarity_is_strict_and_eviction_preserves_recency_order() {
        let similar = |a: &str, b: &str| {
            let chunk = |text: &str| {
                IndexedChunk::new(ContextChunk {
                    filename: String::new(),
                    text: text.into(),
                })
            };
            chunk(a).similar(&chunk(b))
        };
        assert!(!similar("0 1 2 3 4 5 6 7 8", "0 1 2 3 4 5 6 7 8 9"));
        assert!(similar("0 1 2 3 4 5 6 7 8 9", "0 1 2 3 4 5 6 7 8 9 10"));
        assert!(similar("a b a", "b a"));
        assert!(!similar("!!!", "???"));
        let model = model();
        let id = model.document().id.unwrap();
        let mut ring = InlineContextRing::default();
        for line in 0..100 {
            ring.enqueue(id, line, 8);
        }
        assert_eq!(ring.pending.len(), 8);
        assert_eq!(ring.pending[0].line, 92);
        ring.enqueue(id, 92, 8);
        assert_eq!(ring.pending.len(), 8);
        assert_eq!(ring.pending.back().unwrap().line, 92);
    }

    #[test]
    fn recency_revisit_replaces_overlaps_at_clamped_document_boundaries() {
        let mut model = model();
        model.document_mut().buffer = "zero\none\ntwo\nthree\nfour\nfive\nsix\n".into();
        let mut ring = InlineContextRing::default();
        let start = Instant::now();
        ring.observe(&model, start);
        ring.observe(&model, start + IDLE);
        assert_eq!(ring.chunks[0].lines, 0..4);
        model.editor_mut().cursors[0].line = 4;
        ring.observe(&model, start + IDLE * 2);
        ring.observe(&model, start + IDLE * 3);
        assert_eq!(
            ring.chunks.len(),
            1,
            "cursor distance alone misses overlap near the first line"
        );
        assert_eq!(ring.chunks[0].lines, 2..6);
        assert!(ring.chunks[0].chunk.payload.text.contains("four"));
    }

    #[test]
    fn recency_capture_bounds_unicode_and_excludes_workspace_symlink_escapes() {
        let mut model = model();
        model.document_mut().buffer = "🦀".repeat(10_000).as_str().into();
        let chunk = capture(&model, model.document(), 0, 4).unwrap();
        assert_eq!(chunk.text.len(), CHUNK_BYTES.as_usize());
        assert_eq!(chunk.text.chars().count(), CHUNK_BYTES.as_usize() / 4);
        assert!(!format!("{chunk:?}").contains('🦀'));
        let dir = tempfile::tempdir().unwrap();
        model.workspace =
            Some(token::model::Workspace::new(dir.path().into(), &model.metrics).unwrap());
        let root = model.workspace_root().unwrap().clone();
        let path = root.join("inside.rs");
        model.document_mut().file_path = Some(path.clone());
        let id = model.document().id.unwrap();
        assert!(
            eligible_document(&model, id).is_none(),
            "unresolved path cannot prove scope"
        );
        model
            .document_mut()
            .set_file_identity(Some(token::util::FileIdentity::from_resolved(
                path.clone(),
                &root.join("helper.rs"),
            )));
        assert!(eligible_document(&model, id).is_some());
        assert_eq!(
            capture(&model, model.document(), 0, 4).unwrap().filename,
            "helper.rs"
        );
        model
            .document_mut()
            .set_file_identity(Some(token::util::FileIdentity::from_resolved(
                path,
                &root.parent().unwrap().join("outside.rs"),
            )));
        assert!(
            eligible_document(&model, id).is_none(),
            "canonical symlink target escapes workspace"
        );
    }

    #[test]
    fn recency_attach_keeps_prefix_and_local_analysis_separate() {
        let model = model();
        let mut ring = InlineContextRing::default();
        let start = Instant::now();
        ring.observe(&model, start);
        let request = token::completion::inline::build_request(
            model.document(),
            (0, 3),
            1,
            Some("rust".into()),
            false,
        )
        .unwrap();
        let prefix = request.prefix.clone();
        let mut job = InlineJob {
            request,
            provider: model.config.completion.providers["recency-fixture"].clone(),
            context: None,
        };
        ring.attach(&model, &mut job, start + IDLE);
        assert_eq!(job.request.extra_context.len(), 1);
        assert_eq!(job.request.prefix, prefix);
        assert!(job.context.is_none());
        job.provider.url.push_str("/mismatch");
        ring.attach(&model, &mut job, start + IDLE * 2);
        assert!(job.request.extra_context.is_empty());
    }

    #[test]
    fn recency_indexed_similarity_matches_token_set_oracle() {
        use std::collections::BTreeSet;
        fn oracle(a: &str, b: &str) -> bool {
            let tokens = |text: &str| {
                text.split(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                    .filter(|token| !token.is_empty())
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>()
            };
            let a_tokens = tokens(a);
            let b_tokens = tokens(b);
            let intersection = a_tokens.intersection(&b_tokens).count();
            let union = a_tokens.len() + b_tokens.len() - intersection;
            if union == 0 {
                a == b
            } else {
                intersection * 10 > union * 9
            }
        }
        let vocabulary = [
            "α", "β", "🦀", "文", "a", "b", "a_b", "A", " ", "\r\n", "!", "a", "é", "e\u{301}",
        ];
        let mut seed = 17_u64;
        let mut make = || {
            let mut text = String::new();
            for _ in 0..80 {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                text.push_str(vocabulary[(seed >> 32) as usize % vocabulary.len()]);
                text.push(' ');
            }
            text
        };
        for case in 0..1000 {
            let a = make();
            let b = if case % 5 == 0 { a.clone() } else { make() };
            let expected = oracle(&a, &b);
            let a = IndexedChunk::new(ContextChunk {
                filename: String::new(),
                text: a,
            });
            let b = IndexedChunk::new(ContextChunk {
                filename: String::new(),
                text: b,
            });
            assert_eq!(a.similar(&b), expected);
            assert_eq!(b.similar(&a), expected);
            for chunk in [&a, &b] {
                assert!(chunk
                    .tokens
                    .windows(2)
                    .all(|pair| chunk.payload.text[pair[0].clone()]
                        < chunk.payload.text[pair[1].clone()]));
            }
        }
        for (a, b) in [
            ("", ""),
            ("!!!", "???"),
            ("\r\n", "\n"),
            ("🦀", "🦀"),
            ("a", ""),
        ] {
            let expected = oracle(a, b);
            let a = IndexedChunk::new(ContextChunk {
                filename: String::new(),
                text: a.into(),
            });
            let b = IndexedChunk::new(ContextChunk {
                filename: String::new(),
                text: b.into(),
            });
            assert_eq!(a.similar(&b), expected);
        }
    }
}
