//! Opt-in, ignore-aware workspace context on the shared latest-request worker.

use std::collections::{BTreeMap, HashSet};
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

use token::completion::provider::InlineJob;
use token::completion::retrieval::{RetrievalIndex, FILE_BYTES, INDEX_BYTES, MAX_FILES};
use token::messages::{CompletionMsg, Msg};
use token::model::AppModel;
use token::syntax::LanguageId;

pub(super) type RetrievalWorker = super::latest_worker::LatestWorker<RetrievalJob>;

pub(super) struct RetrievalJob {
    job: Box<InlineJob>,
    root: Option<PathBuf>,
    active_path: Option<PathBuf>,
    // None deliberately masks disk content for oversized/special open buffers.
    buffers: BTreeMap<PathBuf, Option<ropey::Rope>>,
}

impl RetrievalJob {
    pub(super) fn capture(model: &AppModel, job: Box<InlineJob>) -> Self {
        let root = model.workspace_root().map(ToOwned::to_owned);
        let mut buffers: BTreeMap<_, _> = model
            .editor_area
            .documents
            .iter()
            .filter_map(|(id, doc)| {
                let path = doc.file_identity()?.path();
                if !root.as_ref().is_some_and(|root| path.starts_with(root)) {
                    return None;
                }
                let plain =
                    model.editor_area.editors.values().any(|editor| {
                        editor.document_id == Some(*id) && editor.is_plain_text_mode()
                    });
                Some((
                    path.to_owned(),
                    (plain && doc.buffer.len_bytes() <= FILE_BYTES.as_usize())
                        .then(|| doc.buffer.clone()),
                ))
            })
            .collect();
        let mut remaining = INDEX_BYTES.as_usize();
        for buffer in buffers.values_mut() {
            if let Some(rope) = buffer {
                if rope.len_bytes() > remaining {
                    *buffer = None;
                } else {
                    remaining -= rope.len_bytes();
                }
            }
        }
        Self {
            job,
            root,
            buffers,
            active_path: model
                .document()
                .file_identity()
                .map(|identity| identity.path().to_owned())
                .or_else(|| model.document().file_path.clone()),
        }
    }

    fn reply(&self, chunks: Vec<token::completion::recency::ContextChunk>) -> Msg {
        let mut job = self.job.clone();
        job.request.extra_context = chunks;
        Msg::Completion(CompletionMsg::InlineContextReady {
            job,
            root: self.root.clone(),
        })
    }
}

pub(super) fn start(
    sender: mpsc::Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) -> std::io::Result<RetrievalWorker> {
    let mut index = RetrievalIndex::default();
    let mut scope = None;
    RetrievalWorker::start(
        "inline-retrieval",
        sender,
        wake,
        move |request, cancelled| {
            if scope != request.root {
                index = RetrievalIndex::default();
                scope = request.root.clone();
            }
            collect(&mut index, request, cancelled)
        },
        |request| {
            Msg::Completion(CompletionMsg::InlineFailed {
                snapshot: request.job.request.snapshot,
                error: "Workspace context worker failed".into(),
            })
        },
    )
}

fn collect(index: &mut RetrievalIndex, request: &RetrievalJob, cancelled: &AtomicBool) -> Msg {
    let Some(root) = &request.root else {
        return request.reply(Vec::new());
    };
    let Ok(Some((limit, lines))) = request.job.provider.context.limits() else {
        return request.reply(Vec::new());
    };
    if root.canonicalize().ok().as_ref() != Some(root) {
        return request.reply(Vec::new());
    }
    if request
        .active_path
        .as_ref()
        .is_some_and(|path| !path.starts_with(root))
    {
        return request.reply(Vec::new());
    }
    let started = Instant::now();
    let mut remaining = INDEX_BYTES.as_usize();
    let mut names = HashSet::new();
    let mut walk = ignore::WalkBuilder::new(root);
    walk.require_git(false)
        .follow_links(false)
        .max_depth(Some(20))
        .max_filesize(Some(FILE_BYTES.as_u64()))
        .filter_entry(|entry| {
            entry.depth() == 0
                || entry.file_name().to_str().is_some_and(|name| {
                    !name.starts_with('.')
                        && !matches!(
                            name,
                            "target" | "node_modules" | "vendor" | "dist" | "build"
                        )
                })
        });
    for result in walk.build().take(20_000) {
        if cancelled.load(Ordering::Relaxed) {
            return request.reply(Vec::new());
        }
        if names.len() >= MAX_FILES
            || remaining == 0
            || started.elapsed() >= Duration::from_millis(250)
        {
            break;
        }
        // Never use a partial/broken ignore policy to select provider payloads.
        let Ok(entry) = result else {
            index.retain(&HashSet::new());
            return request.reply(Vec::new());
        };
        if entry.error().is_some() {
            index.retain(&HashSet::new());
            return request.reply(Vec::new());
        }
        if !entry.file_type().is_some_and(|kind| kind.is_file()) {
            continue;
        }
        let path = entry.path();
        if request.active_path.as_deref() == Some(path)
            || LanguageId::from_path(path) == LanguageId::PlainText
        {
            continue;
        }
        // Reject aliases and paths that changed to cross a symlink boundary.
        if path.canonicalize().ok().as_deref() != Some(path) {
            continue;
        }
        let Some(filename) = path.strip_prefix(root).ok().and_then(|path| path.to_str()) else {
            continue;
        };
        let text = match request.buffers.get(path) {
            Some(Some(buffer)) if buffer.len_bytes() <= remaining => {
                remaining -= buffer.len_bytes();
                Some(buffer.to_string())
            }
            Some(_) => None,
            None => read_source(path, &mut remaining),
        };
        let Some(text) = text else {
            continue;
        };
        names.insert(filename.to_owned());
        index.update(filename.to_owned(), text, lines, cancelled);
    }
    index.retain(&names);
    request.reply(index.select(
        &request.job.request.prefix,
        &request.job.request.suffix,
        limit,
        cancelled,
    ))
}

fn read_source(path: &std::path::Path, remaining: &mut usize) -> Option<String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path).ok()?;
    let metadata = file.metadata().ok()?;
    let limit = (*remaining).min(FILE_BYTES.as_usize());
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return None;
    }
    let mut bytes = Vec::new();
    let result = file.take(limit as u64 + 1).read_to_end(&mut bytes);
    *remaining = remaining.saturating_sub(bytes.len());
    result.ok()?;
    let text = String::from_utf8(bytes).ok()?;
    (text.len() <= limit
        && !text.contains('\0')
        && path.canonicalize().ok().as_deref() == Some(path))
    .then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::completion::recency::ContextStrategy;

    #[test]
    fn retrieval_honors_ignores_buffers_scope_and_refreshes() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = "fn parse_widget() {}\n";
        for name in [
            "helper.rs",
            "active.rs",
            ".hidden.rs",
            "ignored.rs",
            "binary.rs",
        ] {
            std::fs::write(root.join(name), source).unwrap();
        }
        std::fs::write(root.join("binary.rs"), b"\0fn parse_widget() {}").unwrap();
        std::fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
        std::fs::create_dir(root.join("node_modules")).unwrap();
        std::fs::write(root.join("node_modules/dependency.rs"), source).unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("external.rs"), source).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink(outside.path(), root.join("linked")).unwrap();
        let mut document = token::model::Document::with_text("parse_widget(");
        document.id = Some(token::model::DocumentId(1));
        let job = Box::new(InlineJob {
            request: token::completion::inline::build_request(
                &document,
                (0, 13),
                1,
                Some("rust".into()),
                true,
            )
            .unwrap(),
            provider: token::config::ProviderConfig {
                context: ContextStrategy::WorkspaceRetrieval {
                    max_chunks: 8,
                    chunk_lines: 64,
                },
                ..Default::default()
            },
            context: None,
        });
        let mut request = RetrievalJob {
            job,
            root: Some(root.clone()),
            active_path: Some(root.join("active.rs")),
            buffers: BTreeMap::new(),
        };
        let mut index = RetrievalIndex::default();
        let cancelled = AtomicBool::new(false);
        let chunks = |message| match message {
            Msg::Completion(CompletionMsg::InlineContextReady { job, .. }) => {
                job.request.extra_context
            }
            _ => panic!("context reply"),
        };
        let result = chunks(collect(&mut index, &request, &cancelled));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].filename, "helper.rs");
        request.active_path = Some(outside.path().join("external.rs"));
        assert!(chunks(collect(&mut index, &request, &cancelled)).is_empty());
        request.active_path = Some(root.join("active.rs"));
        let mut budget = FILE_BYTES.as_usize();
        assert!(read_source(&root.join("binary.rs"), &mut budget).is_none());
        assert!(
            budget < FILE_BYTES.as_usize(),
            "rejected source still consumes the read budget"
        );
        request.buffers.insert(
            root.join("helper.rs"),
            Some("fn parse_widget() { unsaved(); }\n".into()),
        );
        assert!(chunks(collect(&mut index, &request, &cancelled))[0]
            .text
            .contains("unsaved"));
        std::fs::write(root.join(".gitignore"), "ignored.rs\nhelper.rs\n").unwrap();
        assert!(
            chunks(collect(&mut index, &request, &cancelled)).is_empty(),
            "new ignore rules also remove cached/open source"
        );
        std::fs::write(root.join(".gitignore"), "ignored.rs\n").unwrap();
        request.buffers.insert(root.join("helper.rs"), None);
        assert!(
            chunks(collect(&mut index, &request, &cancelled)).is_empty(),
            "excluded buffers cannot fall back to old disk content"
        );
        request.buffers.clear();
        std::fs::remove_file(root.join("helper.rs")).unwrap();
        assert!(chunks(collect(&mut index, &request, &cancelled)).is_empty());
        cancelled.store(true, Ordering::Relaxed);
        assert!(chunks(collect(&mut index, &request, &cancelled)).is_empty());
    }
}
