//! Bounded, replaceable reference previews. Navigation targets never depend on
//! preview availability; unopened files are read only on this worker.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use token::messages::{LspMsg, Msg, ReferencesOutcome};
use token::update::navigation::LocationItem;
use token::util::ByteSize;

use super::super::latest_worker::LatestWorker;
use super::PendingReferences;

const FILE_READ_LIMIT: ByteSize = ByteSize::mebibytes(1);
const BATCH_READ_LIMIT: ByteSize = ByteSize::mebibytes(4);
const PREVIEW_CHARS: usize = 240;
const PREVIEW_TIMEOUT: Duration = Duration::from_millis(250);

struct PreviewJob {
    pending: PendingReferences,
    items: Vec<LocationItem>,
    buffers: HashMap<PathBuf, ropey::Rope>,
    outcome: ReferencesOutcome,
}

struct PreviewReply {
    generation: Arc<()>,
    message: Msg,
}

impl PreviewJob {
    fn reply(&self, items: Vec<LocationItem>) -> PreviewReply {
        PreviewReply {
            generation: Arc::clone(&self.pending.generation),
            message: Msg::Lsp(LspMsg::ReferencesResolved {
                target: self.pending.target.clone(),
                document_id: self.pending.document_id,
                revision: self.pending.revision,
                cursor: self.pending.cursor,
                items,
                outcome: self.outcome.clone(),
            }),
        }
    }
}

#[derive(Default)]
pub(super) struct ReferencePreviews {
    generation: Arc<()>,
    worker: Option<LatestWorker<PreviewJob, PreviewReply>>,
    replies: Option<mpsc::Receiver<PreviewReply>>,
    fallback: Option<(Instant, PreviewReply)>,
}

impl ReferencePreviews {
    /// The token spans both the LSP round trip and preview preparation. Identity
    /// is allocation-based, so a reused cursor/revision cannot admit an old reply.
    pub(super) fn begin(&mut self) -> Arc<()> {
        self.generation = Arc::new(());
        self.fallback = None;
        if let Some(worker) = &self.worker {
            worker.cancel();
        }
        Arc::clone(&self.generation)
    }

    pub(super) fn prepare(
        &mut self,
        pending: PendingReferences,
        items: Vec<LocationItem>,
        buffers: HashMap<PathBuf, ropey::Rope>,
        outcome: ReferencesOutcome,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Option<Msg> {
        if !Arc::ptr_eq(&self.generation, &pending.generation) {
            return None;
        }
        let job = PreviewJob {
            pending,
            items,
            buffers,
            outcome,
        };
        if job.items.is_empty() {
            return Some(job.reply(Vec::new()).message);
        }
        if self.worker.is_none() {
            let (sender, replies) = mpsc::channel();
            match LatestWorker::start("reference-previews", sender, wake, compute, |job| {
                // A preview failure must not lose otherwise usable locations.
                job.reply(job.items.clone())
            }) {
                Ok(worker) => {
                    self.worker = Some(worker);
                    self.replies = Some(replies);
                }
                Err(error) => {
                    tracing::warn!(%error, "Cannot start reference previews; showing locations only");
                    return Some(job.reply(job.items.clone()).message);
                }
            }
        }
        if let Some(worker) = &self.worker {
            self.fallback = Some((
                Instant::now() + PREVIEW_TIMEOUT,
                job.reply(job.items.clone()),
            ));
            worker.submit(job);
        }
        None
    }

    pub(super) fn deadline(&self) -> Option<Instant> {
        self.fallback.as_ref().map(|(deadline, _)| *deadline)
    }

    pub(super) fn poll(&mut self) -> Option<Msg> {
        self.poll_at(Instant::now())
    }

    fn poll_at(&mut self, now: Instant) -> Option<Msg> {
        for reply in self.replies.iter().flat_map(mpsc::Receiver::try_iter) {
            if self.fallback.is_some() && Arc::ptr_eq(&self.generation, &reply.generation) {
                self.fallback = None;
                return Some(reply.message);
            }
        }
        if self.deadline().is_some_and(|deadline| now >= deadline) {
            if let Some(worker) = &self.worker {
                worker.cancel();
            }
            return self.fallback.take().map(|(_, reply)| reply.message);
        }
        None
    }
}

fn preview(chars: impl Iterator<Item = char>) -> String {
    let text: String = chars
        .skip_while(|c| c.is_whitespace())
        .take(PREVIEW_CHARS)
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    text.trim_end().to_owned()
}

/// The caller caches one file at a time (targets arrive in path order). Count
/// bytes against the batch budget even when reading fails partway through.
fn read_preview_file(path: &Path, remaining: &mut usize) -> Option<String> {
    if *remaining == 0 {
        return None;
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // A server may return a FIFO/device path. Never block opening a FIFO,
        // and check the opened handle rather than a racy path-level metadata call.
        options.custom_flags(libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let limit = (*remaining).min(FILE_READ_LIMIT.as_usize());
    let mut bytes = Vec::new();
    let result = file.take(limit as u64).read_to_end(&mut bytes);
    *remaining = remaining.saturating_sub(bytes.len());
    result.ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn compute(job: &PreviewJob, cancelled: &AtomicBool) -> PreviewReply {
    let mut items = job.items.clone();
    let mut remaining = BATCH_READ_LIMIT.as_usize();
    for group in items.chunk_by_mut(|a, b| a.path == b.path) {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        // chunk_by_mut only yields nonempty runs, already ordered by line.
        let buffer = job.buffers.get(&group[0].path);
        let text = buffer
            .is_none()
            .then(|| read_preview_file(&group[0].path, &mut remaining))
            .flatten();
        let mut lines = text
            .as_deref()
            .unwrap_or_default()
            .lines()
            .enumerate()
            .peekable();
        for item in group {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            let line = item.position.line as usize;
            item.preview = if let Some(buffer) = buffer {
                buffer
                    .get_line(line)
                    .map(|line| preview(line.chars()))
                    .unwrap_or_default()
            } else {
                while lines.peek().is_some_and(|(index, _)| *index < line) {
                    lines.next();
                }
                lines
                    .peek()
                    .filter(|(index, _)| *index == line)
                    .map(|(_, text)| preview(text.chars()))
                    .unwrap_or_default()
            };
        }
    }
    job.reply(items)
}

#[cfg(test)]
mod tests {
    use super::super::App;
    use super::*;
    use token::cli::{StartupConfig, StartupMode};

    fn item(path: impl Into<PathBuf>, line: u32, character: u32) -> LocationItem {
        LocationItem {
            path: path.into(),
            position: lsp_types::Position::new(line, character),
            preview: String::new(),
            route_hint: None,
        }
    }

    fn job(generation: Arc<()>, items: Vec<LocationItem>) -> PreviewJob {
        PreviewJob {
            pending: PendingReferences {
                target: token::model::usages::ReferencesTarget::Popup,
                document_id: token::model::editor_area::DocumentId(1),
                revision: 7,
                cursor: token::model::editor::Position::new(2, 3),
                generation,
            },
            items,
            buffers: HashMap::new(),
            outcome: ReferencesOutcome::Found,
        }
    }

    fn rows(message: Msg) -> Vec<LocationItem> {
        let Msg::Lsp(LspMsg::ReferencesResolved { items, .. }) = message else {
            panic!("expected reference locations")
        };
        items
    }

    #[test]
    fn references_previews_preserve_unsaved_unicode_buffers_and_unreadable_targets() {
        let dir = tempfile::tempdir().unwrap();
        let first = dir.path().join("a.rs");
        let second = dir.path().join("b.rs");
        std::fs::write(&first, "disk is stale").unwrap();
        std::fs::write(&second, "zero\n  disk preview\r\nthird\n").unwrap();
        let mut request = job(
            Arc::new(()),
            vec![
                item(&first, 0, 4),
                item(&first, 0, 8),
                item(&first, 50, 0),
                item(&second, 1, 3),
                item(&second, 1, 7),
                item(&second, 2, 0),
                item(dir.path().join("missing.rs"), 0, 0),
            ],
        );
        request
            .buffers
            .insert(first, format!("  {}\n", "🦀".repeat(400)).into());
        let result = rows(compute(&request, &AtomicBool::new(false)).message);
        assert_eq!(result.len(), 7);
        assert_eq!(result[0].preview, "🦀".repeat(PREVIEW_CHARS));
        assert_eq!(result[1].preview, result[0].preview);
        assert_eq!(result[2].preview, "");
        assert_eq!(result[3].preview, "disk preview");
        assert_eq!(result[4].preview, "disk preview");
        assert_eq!(result[5].preview, "third");
        assert_eq!(result[6].preview, "");
        assert_eq!(result[0].position.character, 4, "positions stay UTF-16");
    }

    #[test]
    fn references_previews_bound_file_and_batch_reads_and_skip_non_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.rs");
        std::fs::write(&path, "x".repeat(FILE_READ_LIMIT.as_usize() + 32)).unwrap();
        let mut remaining = BATCH_READ_LIMIT.as_usize();
        assert!(read_preview_file(dir.path(), &mut remaining).is_none());
        assert_eq!(remaining, BATCH_READ_LIMIT.as_usize());
        for _ in 0..4 {
            assert_eq!(
                read_preview_file(&path, &mut remaining).unwrap().len(),
                FILE_READ_LIMIT.as_usize()
            );
        }
        assert_eq!(remaining, 0);
        assert!(read_preview_file(&path, &mut remaining).is_none());
        let mut tail = 17;
        assert_eq!(read_preview_file(&path, &mut tail).unwrap(), "x".repeat(17));
        assert_eq!(tail, 0);
    }

    #[test]
    fn references_previews_cancel_before_reading_and_sanitize_control_characters() {
        let mut request = job(Arc::new(()), vec![item("/missing", 0, 0)]);
        request
            .buffers
            .insert("/missing".into(), "valid text".into());
        assert_eq!(
            rows(compute(&request, &AtomicBool::new(true)).message)[0].preview,
            ""
        );
        assert_eq!(preview("  a\tb\u{1b}c\r\n".chars()), "a b c");
    }

    #[test]
    fn references_previews_reject_superseded_lsp_and_already_queued_worker_replies() {
        let mut previews = ReferencePreviews::default();
        let old = job(previews.begin(), vec![item("/missing", 0, 0)]);
        let old_reply = old.reply(old.items.clone());
        let current_generation = previews.begin();
        assert!(previews
            .prepare(old.pending, old.items, old.buffers, old.outcome, None)
            .is_none());
        assert!(
            previews.worker.is_none(),
            "stale network replies cannot start work"
        );
        let current = job(current_generation, vec![item("/current", 0, 0)]);
        let (sender, replies) = mpsc::channel();
        previews.replies = Some(replies);
        previews.fallback = Some((
            Instant::now() + PREVIEW_TIMEOUT,
            current.reply(current.items.clone()),
        ));
        sender.send(old_reply).unwrap();
        assert!(previews.poll().is_none());
        sender.send(current.reply(current.items.clone())).unwrap();
        assert_eq!(
            rows(previews.poll().unwrap())[0].path,
            Path::new("/current")
        );
        assert!(previews.deadline().is_none());
    }

    #[test]
    fn references_previews_timeout_keeps_locations_and_discards_late_enrichment() {
        let mut previews = ReferencePreviews::default();
        let request = job(previews.begin(), vec![item("/slow", 0, 0)]);
        let deadline = Instant::now() + PREVIEW_TIMEOUT;
        previews.fallback = Some((deadline, request.reply(request.items.clone())));
        let (sender, replies) = mpsc::channel();
        previews.replies = Some(replies);
        assert!(previews
            .poll_at(deadline - Duration::from_millis(1))
            .is_none());
        assert_eq!(
            rows(previews.poll_at(deadline).unwrap())[0].path,
            Path::new("/slow")
        );
        sender.send(request.reply(request.items.clone())).unwrap();
        assert!(previews.poll_at(deadline + PREVIEW_TIMEOUT).is_none());
        assert!(previews.deadline().is_none());
    }

    #[test]
    fn references_previews_worker_wakes_and_keeps_origin_metadata() {
        let mut previews = ReferencePreviews::default();
        let mut request = job(previews.begin(), vec![item("/unsaved", 0, 0)]);
        request
            .buffers
            .insert("/unsaved".into(), "  unsaved".into());
        let (woke, wakes) = mpsc::channel();
        let caller = std::thread::current().id();
        let wake = Arc::new(move || {
            woke.send(std::thread::current().id()).unwrap();
        });
        assert!(previews
            .prepare(
                request.pending,
                request.items,
                request.buffers,
                request.outcome,
                Some(wake)
            )
            .is_none());
        assert_ne!(wakes.recv_timeout(Duration::from_secs(5)).unwrap(), caller);
        let Msg::Lsp(LspMsg::ReferencesResolved {
            document_id,
            revision,
            cursor,
            items,
            ..
        }) = previews.poll().unwrap()
        else {
            panic!("references reply")
        };
        assert_eq!(document_id, token::model::editor_area::DocumentId(1));
        assert_eq!(revision, 7);
        assert_eq!(cursor, token::model::editor::Position::new(2, 3));
        assert_eq!(items[0].preview, "unsaved");
    }

    #[test]
    fn references_previews_queued_intent_invalidates_ready_preview_before_update() {
        let mut app = App::new(
            800,
            600,
            StartupConfig {
                mode: StartupMode::Empty,
                initial_position: None,
                restore_session: false,
                wait_mode: false,
            },
            None,
            None,
            None,
        );
        app.model.config.lsp.enabled = false;
        app.model.document_mut().file_path = Some("/source.rs".into());
        let mut request = job(
            app.reference_previews.begin(),
            vec![item("/first.rs", 0, 0), item("/second.rs", 0, 0)],
        );
        request.pending.document_id = app.model.document().id.unwrap();
        request.pending.revision = app.model.document().revision;
        request.pending.cursor = app.model.editor().active_cursor().to_position();
        let now = Instant::now();
        let deadline = now + PREVIEW_TIMEOUT;
        app.reference_previews.fallback = Some((deadline, request.reply(request.items.clone())));
        let (sender, replies) = mpsc::channel();
        app.reference_previews.replies = Some(replies);
        sender.send(request.reply(request.items.clone())).unwrap();
        assert!(app.next_wake(now) <= deadline);
        app.msg_tx.send(Msg::Lsp(LspMsg::FindReferences)).unwrap();
        app.process_async_messages();
        assert!(app.model.ui.reference_list.is_none());
        assert!(app.model.ui.cursor_overlay.is_none());
    }

    #[test]
    fn references_targets_deduplicate_before_cap_sort_columns_and_preserve_routes() {
        let app = App::new(
            800,
            600,
            StartupConfig {
                mode: StartupMode::Empty,
                initial_position: None,
                restore_session: false,
                wait_mode: false,
            },
            None,
            None,
            None,
        );
        let uri: lsp_types::Uri = "file:///missing/reference.rs".parse().unwrap();
        let mut locations: Vec<_> = (0..250)
            .rev()
            .flat_map(|column| {
                let location = lsp_types::Location {
                    uri: uri.clone(),
                    range: lsp_types::Range::new(
                        lsp_types::Position::new(0, column),
                        lsp_types::Position::new(0, column + 1),
                    ),
                };
                [location.clone(), location]
            })
            .collect();
        locations.push(lsp_types::Location {
            uri: "untitled:buffer".parse().unwrap(),
            range: lsp_types::Range::default(),
        });
        let server = token::lsp::LspServerId::from("fixture");
        let items = app.build_reference_items(locations, &server, Path::new("/workspace"));
        assert_eq!(items.len(), super::super::MAX_REFERENCE_LOCATIONS);
        for (column, item) in items.iter().enumerate() {
            assert_eq!(item.position.character, column as u32);
            assert!(item.preview.is_empty());
            assert_eq!(
                item.route_hint,
                Some((server.clone(), PathBuf::from("/workspace")))
            );
        }
    }

    #[test]
    fn usages_panel_server_cleanup_settles_pending_search_and_preserves_other_roots() {
        let mut app = App::new(
            800,
            600,
            StartupConfig {
                mode: StartupMode::Empty,
                initial_position: None,
                restore_session: false,
                wait_mode: false,
            },
            None,
            None,
            None,
        );
        app.model.document_mut().file_path = Some("/source.rs".into());
        fn target(cmd: token::Cmd) -> Option<token::model::usages::ReferencesTarget> {
            match cmd {
                token::Cmd::LspRequestReferences { target, .. } => Some(target),
                token::Cmd::Batch(commands) => commands.into_iter().find_map(target),
                _ => None,
            }
        }
        let destination = target(
            token::update::update(&mut app.model, Msg::Lsp(LspMsg::FindUsagesInPanel)).unwrap(),
        )
        .unwrap();
        let doc = app.model.document();
        let id = doc.id.unwrap();
        let pending = PendingReferences {
            target: destination,
            document_id: id,
            revision: doc.revision,
            cursor: app.model.editor().active_cursor().to_position(),
            generation: Arc::new(()),
        };
        let server = token::lsp::LspServerId::from("fixture");
        let key = (server.clone(), PathBuf::from("/first"), 1);
        app.lsp.references.insert(key.clone(), id, pending);
        app.lsp.references.arm_deadline(key.clone());
        app.clear_pending_requests_for_roots(&server, &[PathBuf::from("/other")]);
        assert!(app.model.usages_panel.is_loading());
        assert!(app.lsp.references.requests.contains_key(&key));
        app.clear_pending_requests_for_roots(&server, &[PathBuf::from("/first")]);
        assert!(!app.model.usages_panel.is_loading());
        assert!(app.model.usages_panel.status.contains("unavailable"));
        assert!(app.lsp.references.requests.is_empty());
        assert!(app.lsp.references.deadlines.is_empty());
    }
}
