//! Ordered background file effects. A single queue prevents overlapping saves
//! (including Save As) from leaving older bytes on disk after a newer save.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};

use ropey::Rope;
use token::messages::{AppMsg, LayoutMsg, Msg};
use token::model::{
    Document, FileOpenRequest, FileOpenSource, FileRequest, PreparedFile, TabContent, ViewMode,
};

#[derive(Debug)]
pub(super) enum FileJob {
    Keymap {
        session: Arc<()>,
        save: Option<Box<token::keymap::preferences::KeymapSave>>,
    },
    InlineUsage(token::completion::statistics::UsageEvent),
    Open(FileOpenRequest),
    Write {
        target: FileRequest,
        path: PathBuf,
        content: Rope,
    },
    Read {
        target: FileRequest,
        path: PathBuf,
    },
}

/// CLI startup uses exactly the same preparation and tab installation as later
/// opens. This runs on the loader before the model is installed into the runtime.
pub(super) fn prepare_startup_files(model: &mut token::AppModel, paths: Vec<PathBuf>) {
    use token::update::update;

    let initial_tab = model
        .editor_area
        .focused_group()
        .and_then(|group| group.active_tab())
        .map(|tab| tab.id);
    let mut first_error = None;
    let mut failures = 0;
    for path in paths {
        if let Some(token::Cmd::PrepareFileOpen(request)) =
            update(model, Msg::Layout(LayoutMsg::OpenFileInNewTab(path)))
        {
            let reply = FileJob::Open(request).run(None);
            if let Msg::Layout(LayoutMsg::FilePrepared {
                result: Err(error), ..
            }) = &reply
            {
                failures += 1;
                first_error.get_or_insert_with(|| error.clone());
            }
            // Startup dispatches syntax/LSP work after installing this session
            // into the runtime. Redraws and FileOpenFinished have no consumers
            // in the loader; recent-file state is already updated by dispatch.
            update(model, reply);
        }
    }
    let opened = model
        .editor_area
        .documents
        .values()
        .filter(|doc| doc.file_path.is_some())
        .count();
    if opened > 0 {
        if let Some(tab) = initial_tab {
            update(model, Msg::Layout(LayoutMsg::CloseTab(tab)));
        }
        update(model, Msg::Layout(LayoutMsg::SwitchToTab(0)));
        model.ui.set_status(format!(
            "Opened {opened} file{}",
            if opened == 1 { "" } else { "s" }
        ));
    }
    if let Some(error) = first_error {
        model
            .ui
            .set_status(format!("Opened {opened} files; {failures} failed: {error}"));
    }
}

pub(super) struct FileWorker {
    sender: Option<mpsc::Sender<FileJob>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FileWorker {
    pub fn send(&self, job: FileJob) -> Result<(), mpsc::SendError<FileJob>> {
        self.sender
            .as_ref()
            .expect("sender exists until exclusive Drop")
            .send(job)
    }
}

impl Drop for FileWorker {
    fn drop(&mut self) {
        // Closing the queue drains all requested writes before application
        // teardown. Losing the UI reply receiver must not cancel disk writes.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                tracing::error!("File worker panicked during shutdown");
            }
        }
    }
}

impl FileJob {
    fn run(self, config_dir: Option<&Path>) -> Msg {
        match self {
            Self::Keymap { session, save } => Msg::Ui(token::messages::UiMsg::Settings(
                token::messages::SettingsMsg::KeymapResult {
                    session,
                    saved: save.is_some(),
                    result: super::keymap_settings::prepare(config_dir, save.as_deref())
                        .map(Box::new)
                        .map_err(|e| e.to_string()),
                },
            )),
            Self::InlineUsage(event) => {
                Msg::Completion(token::messages::CompletionMsg::InlineStatisticsSaved(
                    config_dir
                        .ok_or_else(|| {
                            io::Error::new(io::ErrorKind::NotFound, "No config directory available")
                        })
                        .and_then(|dir| {
                            super::inline_statistics::update(dir, Some(&event)).map(|_| ())
                        })
                        .map_err(|error| error.to_string()),
                ))
            }
            Self::Open(request) => {
                let result = prepare_open(&request, config_dir)
                    .map(Box::new)
                    .map_err(|error| format!("{error:#}"));
                Msg::Layout(LayoutMsg::FilePrepared { request, result })
            }
            Self::Write {
                target,
                path,
                content,
            } => {
                let result = (|| {
                    let mut file = BufWriter::new(File::create(&path)?);
                    for chunk in content.chunks() {
                        file.write_all(chunk.as_bytes())?;
                    }
                    file.flush()
                })()
                .map_err(|error: io::Error| error.to_string());
                let identity = result
                    .as_ref()
                    .ok()
                    .map(|_| token::util::FileIdentity::resolve(path.clone()));
                Msg::App(AppMsg::SaveCompleted {
                    target,
                    path,
                    content,
                    identity,
                    result,
                })
            }
            Self::Read { target, path } => {
                let result = std::fs::read_to_string(&path).map_err(|error| error.to_string());
                let identity = result
                    .as_ref()
                    .ok()
                    .map(|_| token::util::FileIdentity::resolve(path.clone()));
                Msg::App(AppMsg::FileLoaded {
                    target,
                    path,
                    identity,
                    result,
                })
            }
        }
    }

    pub fn failed(self, error: String) -> Msg {
        match self {
            Self::Keymap { session, save } => Msg::Ui(token::messages::UiMsg::Settings(
                token::messages::SettingsMsg::KeymapResult {
                    session,
                    saved: save.is_some(),
                    result: Err(error),
                },
            )),
            Self::InlineUsage(_) => Msg::Completion(
                token::messages::CompletionMsg::InlineStatisticsSaved(Err(error)),
            ),
            Self::Open(request) => Msg::Layout(LayoutMsg::FilePrepared {
                request,
                result: Err(error),
            }),
            Self::Write {
                target,
                path,
                content,
            } => Msg::App(AppMsg::SaveCompleted {
                target,
                path,
                content,
                identity: None,
                result: Err(error),
            }),
            Self::Read { target, path } => Msg::App(AppMsg::FileLoaded {
                target,
                path,
                identity: None,
                result: Err(error),
            }),
        }
    }
}

/// All validation, alias lookup, UTF-8 reading and image decoding runs on the
/// ordered worker. A known document is returned by identity, never reread.
fn prepare_open(
    request: &FileOpenRequest,
    config_dir: Option<&Path>,
) -> anyhow::Result<PreparedFile> {
    use anyhow::Context;
    use token::util::{
        filename_for_display, is_likely_binary, is_supported_image, validate_file_for_opening,
        FileOpenError,
    };

    let resolved;
    let path = match &request.source {
        FileOpenSource::Path(path) => path,
        FileOpenSource::Configuration(resource) => {
            resolved = super::configuration::prepare_resource(*resource, config_dir)
                .context("Could not open configuration resource")?;
            if *resource == token::commands::ConfigResource::Directory {
                return Ok(PreparedFile::Directory { path: resolved });
            }
            &resolved
        }
    };
    let identity = token::util::FileIdentity::resolve(path.clone());
    for known in &request.known_documents {
        let matches = known.path == *path || {
            let resolved;
            let known_identity = match known
                .identity
                .as_ref()
                .filter(|identity| identity.source() == known.path)
            {
                Some(identity) => identity,
                None => {
                    resolved = token::util::FileIdentity::resolve(known.path.clone());
                    &resolved
                }
            };
            known_identity.path() == identity.path()
        };
        if matches {
            return Ok(PreparedFile::Existing {
                document_id: known.document_id,
                path: known.path.clone(),
            });
        }
    }
    let mut view_mode = ViewMode::Text;
    let mut tab_content = TabContent::Text;
    let mut document = match validate_file_for_opening(path) {
        Ok(()) if is_supported_image(path) => {
            // Fit against the actual target pane when the reply is installed.
            let image = token::image::load_image(path, 0, 0)
                .with_context(|| format!("Error opening image: {}", filename_for_display(path)))?;
            view_mode = ViewMode::Image(Box::new(image));
            let mut doc = Document::new();
            doc.file_path = Some(path.clone());
            doc
        }
        Ok(()) if is_likely_binary(path) => {
            let size_bytes = std::fs::metadata(path)?.len();
            tab_content =
                TabContent::BinaryPlaceholder(token::model::editor::BinaryPlaceholderState {
                    path: path.clone(),
                    size_bytes,
                });
            let mut doc = Document::new();
            doc.file_path = Some(path.clone());
            doc
        }
        Ok(()) => Document::from_loaded_text(
            &std::fs::read_to_string(path)
                .with_context(|| format!("Error opening {}", path.display()))?,
            identity.clone(),
        ),
        Err(FileOpenError::NotFound)
            if request.policy == token::model::FileOpenPolicy::CreateOrOpen =>
        {
            Document::new_with_path(path.clone())
        }
        Err(error) => anyhow::bail!(error.user_message(&filename_for_display(path))),
    };
    document.set_file_identity(Some(identity));
    anyhow::ensure!(
        request.policy != token::model::FileOpenPolicy::ExistingText
            || (matches!(view_mode, ViewMode::Text) && matches!(tab_content, TabContent::Text)),
        "Workspace edits require an existing text file: {}",
        path.display()
    );
    Ok(PreparedFile::Loaded {
        document: Box::new(document),
        view_mode,
        tab_content,
    })
}

pub(super) fn start_worker(
    replies: mpsc::Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) -> io::Result<FileWorker> {
    start_worker_resolving_config(replies, wake, token::config_paths::config_dir)
}

fn start_worker_resolving_config(
    replies: mpsc::Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
    config_dir: impl FnOnce() -> Option<PathBuf> + Send + 'static,
) -> io::Result<FileWorker> {
    let (sender, receiver) = mpsc::channel::<FileJob>();
    let thread = std::thread::Builder::new()
        .name("file-io".to_owned())
        .spawn(move || {
            let config_dir = config_dir();
            for job in receiver {
                let _ = replies.send(job.run(config_dir.as_deref()));
                if let Some(wake) = &wake {
                    wake();
                }
            }
        })?;
    Ok(FileWorker {
        sender: Some(sender),
        thread: Some(thread),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::update::update;
    use token::{AppModel, Cmd};

    fn open_request(path: PathBuf) -> FileOpenRequest {
        let mut model = AppModel::new(800, 600, 1.0);
        let Cmd::PrepareFileOpen(request) =
            update(&mut model, Msg::Layout(LayoutMsg::OpenFileInNewTab(path))).unwrap()
        else {
            panic!("open request")
        };
        request
    }

    #[test]
    fn startup_files_use_shared_special_tab_loading_and_preserve_cli_order() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("text.rs");
        let image = dir.path().join("image.png");
        let binary = dir.path().join("binary.dat");
        let missing = dir.path().join("new.rs");
        std::fs::write(&text, "fn café() {}\n").unwrap();
        std::fs::write(&binary, b"binary\0bytes").unwrap();
        image::RgbaImage::from_pixel(1200, 900, image::Rgba([10, 20, 30, 255]))
            .save(&image)
            .unwrap();
        let paths = vec![text.clone(), image.clone(), binary.clone(), missing.clone()];
        let mut model = AppModel::new(800, 600, 1.0);
        model.resize(800, 600);
        prepare_startup_files(&mut model, paths.clone());

        let group = model.editor_area.focused_group().unwrap();
        assert_eq!(group.tabs.len(), 4);
        assert_eq!(group.active_tab_index, 0);
        assert_eq!(model.editor_area.documents.len(), 4);
        assert_eq!(model.editor_area.editors.len(), 4);
        for (tab, path) in group.tabs.iter().zip(&paths) {
            let editor = &model.editor_area.editors[&tab.editor_id];
            let document = &model.editor_area.documents[&editor.document_id.unwrap()];
            assert_eq!(document.file_path.as_ref(), Some(path));
            assert!(document.file_identity().is_some());
            assert_eq!(
                model
                    .recent_files
                    .entries
                    .iter()
                    .filter(|entry| entry.path == document.file_identity().unwrap().path())
                    .count(),
                1
            );
            match path {
                path if path == &image => {
                    let ViewMode::Image(image) = &editor.view_mode else {
                        panic!("image tab")
                    };
                    assert!(
                        image.scale > 0.1 && image.scale < 1.0,
                        "fit actual startup pane"
                    );
                    assert!(!editor.is_plain_text_mode());
                }
                path if path == &binary => {
                    assert!(matches!(
                        editor.tab_content,
                        TabContent::BinaryPlaceholder(_)
                    ));
                    assert!(!editor.is_plain_text_mode());
                }
                path if path == &missing => {
                    assert!(document.is_modified);
                    assert!(!missing.exists(), "opening a new path must not create it");
                }
                _ => assert_eq!(document.buffer.to_string(), "fn café() {}\n"),
            }
        }
        assert!(!model.ui.is_loading);
    }

    #[test]
    fn startup_files_report_failures_without_discarding_successful_tabs() {
        let dir = tempfile::tempdir().unwrap();
        let bad_image = dir.path().join("corrupt.png");
        let good = dir.path().join("good.txt");
        std::fs::write(&bad_image, "not a PNG").unwrap();
        std::fs::write(&good, "kept").unwrap();
        let mut model = AppModel::new(800, 600, 1.0);
        prepare_startup_files(&mut model, vec![dir.path().into(), good.clone(), bad_image]);
        assert_eq!(model.editor_area.documents.len(), 1);
        assert_eq!(model.document().file_path.as_ref(), Some(&good));
        assert_eq!(model.document().buffer.to_string(), "kept");
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("2 failed"));
        assert!(!model.ui.is_loading);

        let mut failed = AppModel::new(800, 600, 1.0);
        let original = failed.document().id;
        prepare_startup_files(&mut failed, vec![dir.path().into()]);
        assert_eq!(failed.document().id, original);
        assert!(failed.document().file_path.is_none());
        assert_eq!(failed.editor_area.documents.len(), 1);
        assert!(failed
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("1 failed"));
        assert!(!failed.ui.is_loading);
    }

    #[cfg(unix)]
    #[test]
    fn startup_files_reuse_duplicate_and_symlink_paths_without_duplicate_recent_entries() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.rs");
        let alias = dir.path().join("alias.rs");
        std::fs::write(&real, "fn café() {}\n").unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let mut model = AppModel::new(800, 600, 1.0);
        prepare_startup_files(&mut model, vec![alias.clone(), real.clone(), alias.clone()]);
        assert_eq!(model.editor_area.documents.len(), 1);
        assert_eq!(model.editor_area.focused_group().unwrap().tabs.len(), 1);
        assert_eq!(model.document().file_path.as_ref(), Some(&alias));
        assert!(model
            .document()
            .matches_file_path(&real.canonicalize().unwrap()));
        assert_eq!(model.recent_files.entries.len(), 1);
        assert_eq!(model.recent_files.entries[0].open_count, 1);
    }

    #[test]
    fn file_open_worker_prepares_text_image_binary_and_new_files() {
        let dir = tempfile::tempdir().unwrap();
        let text = dir.path().join("text.rs");
        let binary = dir.path().join("binary.dat");
        let image = dir.path().join("image.png");
        let missing = dir.path().join("new.rs");
        std::fs::write(&text, "fn café() {}\n").unwrap();
        std::fs::write(&binary, b"binary\0bytes").unwrap();
        image::RgbaImage::from_pixel(2, 3, image::Rgba([10, 20, 30, 255]))
            .save(&image)
            .unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = start_worker(tx, None).unwrap();
        for path in [&text, &image, &binary, &missing] {
            worker
                .send(FileJob::Open(open_request(path.clone())))
                .unwrap();
        }
        drop(worker);
        let replies: Vec<_> = rx.try_iter().collect();
        assert_eq!(replies.len(), 4);
        for (index, reply) in replies.into_iter().enumerate() {
            let Msg::Layout(LayoutMsg::FilePrepared {
                result: Ok(prepared),
                ..
            }) = reply
            else {
                panic!("prepared")
            };
            let PreparedFile::Loaded {
                document,
                view_mode,
                tab_content,
                ..
            } = *prepared
            else {
                panic!("loaded")
            };
            match index {
                0 => {
                    assert_eq!(document.buffer.to_string(), "fn café() {}\n");
                    assert!(!document.is_modified);
                }
                1 => {
                    let ViewMode::Image(image) = view_mode else {
                        panic!("image")
                    };
                    assert_eq!((image.width, image.height), (2, 3));
                    assert_eq!(&image.pixels[..4], &[10, 20, 30, 255]);
                }
                2 => assert!(matches!(tab_content, TabContent::BinaryPlaceholder(_))),
                3 => {
                    assert_eq!(document.file_path.as_ref(), Some(&missing));
                    assert!(document.is_modified);
                    assert!(!missing.exists());
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn file_open_worker_rejects_bad_files_and_workspace_edit_placeholders() {
        let dir = tempfile::tempdir().unwrap();
        let invalid_utf8 = dir.path().join("invalid.txt");
        let broken_image = dir.path().join("broken.png");
        let oversized = dir.path().join("oversized.txt");
        std::fs::write(&invalid_utf8, [0xff, 0xfe]).unwrap();
        std::fs::write(&broken_image, "not an image").unwrap();
        File::create(&oversized)
            .unwrap()
            .set_len(token::util::ByteSize::mebibytes(51).as_u64())
            .unwrap();
        for path in [
            dir.path().to_path_buf(),
            invalid_utf8,
            broken_image,
            oversized,
        ] {
            assert!(prepare_open(&open_request(path), None).is_err());
        }
        let binary = dir.path().join("binary.bin");
        std::fs::write(&binary, b"\0binary").unwrap();
        for path in [binary, dir.path().join("missing.rs")] {
            let mut request = open_request(path);
            request.policy = token::model::FileOpenPolicy::ExistingText;
            assert!(prepare_open(&request, None).is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn file_identity_worker_does_not_reuse_a_lossy_filename_collision() {
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        let visible = dir.path().join("file-�.rs");
        std::fs::write(&visible, "keep this document").unwrap();
        let raw = dir
            .path()
            .join(std::ffi::OsString::from_vec(b"file-\xff.rs".to_vec()));
        let mut request = open_request(raw);
        request.known_documents.push(token::model::KnownFile {
            document_id: token::model::DocumentId(7),
            path: visible.clone(),
            identity: Some(token::util::FileIdentity::resolve(visible.clone())),
        });
        // Whether this filesystem permits raw names or rejects them, it must
        // never select the distinct Unicode replacement-character filename.
        assert!(!matches!(
            prepare_open(&request, None),
            Ok(PreparedFile::Existing { .. })
        ));
        assert_eq!(
            std::fs::read_to_string(visible).unwrap(),
            "keep this document"
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_open_worker_returns_existing_identity_for_an_alias_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("real.rs");
        let alias = dir.path().join("alias.rs");
        // Invalid UTF-8 would fail a read; an open buffer must be reused instead.
        std::fs::write(&real, [0xff, 0xfe]).unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let mut request = open_request(alias);
        let document_id = token::model::DocumentId(7);
        request.known_documents.push(token::model::KnownFile {
            document_id,
            path: real.clone(),
            identity: Some(token::util::FileIdentity::resolve(real.clone())),
        });
        assert!(
            matches!(prepare_open(&request, None).unwrap(), PreparedFile::Existing { document_id: found, path } if found == document_id && path == real)
        );
    }

    #[cfg(unix)]
    #[test]
    fn file_identity_worker_reuses_the_open_snapshot_after_a_symlink_changes() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("original.rs");
        let other = dir.path().join("other.rs");
        let alias = dir.path().join("link.rs");
        std::fs::write(&real, "original").unwrap();
        std::fs::write(&other, "other").unwrap();
        std::os::unix::fs::symlink(&real, &alias).unwrap();
        let identity = token::util::FileIdentity::resolve(alias.clone());
        std::fs::rename(&alias, dir.path().join("retired-link.rs")).unwrap();
        std::os::unix::fs::symlink(&other, &alias).unwrap();
        let document_id = token::model::DocumentId(7);
        let mut request = open_request(real);
        request.known_documents.push(token::model::KnownFile {
            document_id,
            path: alias.clone(),
            identity: Some(identity),
        });
        assert!(
            matches!(prepare_open(&request, None).unwrap(), PreparedFile::Existing { document_id: found, path }
            if found == document_id && path == alias)
        );
    }

    fn write_job(model: &mut AppModel, path: PathBuf, text: &str) -> FileJob {
        model.document_mut().file_path = Some(path);
        model.document_mut().buffer = text.into();
        model.config.format_on_save = false;
        let Cmd::SaveFile {
            target,
            path,
            content,
        } = update(model, Msg::App(AppMsg::SaveFile)).unwrap()
        else {
            panic!("write command")
        };
        FileJob::Write {
            target,
            path,
            content,
        }
    }

    fn config_request(
        model: &mut AppModel,
        resource: token::commands::ConfigResource,
    ) -> FileOpenRequest {
        let Cmd::PrepareFileOpen(request) =
            update(model, Msg::App(AppMsg::OpenConfigResource(resource))).unwrap()
        else {
            panic!("configuration request")
        };
        request
    }

    #[test]
    fn statistics_file_worker_merges_before_open_and_drains_after_receiver_closes() {
        use token::commands::ConfigResource;
        use token::completion::statistics::{Outcome, Statistics, UsageEvent};
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().to_owned();
        let (tx, rx) = mpsc::channel();
        let worker = start_worker_resolving_config(tx, None, move || Some(config)).unwrap();
        let accepted = || {
            FileJob::InlineUsage(UsageEvent {
                provider: "local".into(),
                outcome: Outcome::Accepted,
            })
        };
        worker.send(accepted()).unwrap();
        let mut model = AppModel::new(800, 600, 1.0);
        worker
            .send(FileJob::Open(config_request(
                &mut model,
                ConfigResource::InlineStatistics,
            )))
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
            Msg::Completion(token::messages::CompletionMsg::InlineStatisticsSaved(
                Ok(())
            ))
        ));
        let Msg::Layout(LayoutMsg::FilePrepared {
            result: Ok(prepared),
            ..
        }) = rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap()
        else {
            panic!("statistics resource reply")
        };
        let PreparedFile::Loaded { document, .. } = *prepared else {
            panic!("statistics resource document")
        };
        let statistics: Statistics = serde_json::from_str(&document.buffer.to_string()).unwrap();
        assert_eq!(statistics.providers["local"].accepted, 1);
        drop(rx);
        for _ in 0..4 {
            worker.send(accepted()).unwrap();
        }
        drop(worker);
        let statistics: Statistics = serde_json::from_slice(
            &std::fs::read(dir.path().join("inline-statistics.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(statistics.providers["local"].accepted, 5);
    }

    #[test]
    fn statistics_file_worker_reports_missing_configuration_and_queue_failure() {
        use token::completion::statistics::{Outcome, UsageEvent};
        let job = || {
            FileJob::InlineUsage(UsageEvent {
                provider: "local".into(),
                outcome: Outcome::Dismissed,
            })
        };
        for reply in [job().run(None), job().failed("worker unavailable".into())] {
            assert!(matches!(
                reply,
                Msg::Completion(token::messages::CompletionMsg::InlineStatisticsSaved(Err(
                    _
                )))
            ));
        }
    }

    #[test]
    fn config_resource_worker_prepares_and_loads_in_file_queue_order() {
        use token::commands::ConfigResource;
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("nested/config");
        let resolved = config.clone();
        let (tx, rx) = mpsc::channel();
        let worker = start_worker_resolving_config(tx, None, move || {
            assert_eq!(std::thread::current().name(), Some("file-io"));
            Some(resolved)
        })
        .unwrap();
        let mut model = AppModel::new(800, 600, 1.0);
        let mut writer = AppModel::new(800, 600, 1.0);
        let jobs = [
            FileJob::Open(config_request(&mut model, ConfigResource::Directory)),
            write_job(
                &mut writer,
                config.join("keymap.yaml"),
                "bindings: [] # keep custom\n",
            ),
            FileJob::Open(config_request(&mut model, ConfigResource::Keybindings)),
            FileJob::Open(config_request(&mut model, ConfigResource::Log)),
            write_job(
                &mut writer,
                config.join("logs/token.log.2026-09-06"),
                "worker log\n",
            ),
            FileJob::Open(config_request(&mut model, ConfigResource::Log)),
        ];
        for job in jobs {
            worker.send(job).unwrap();
        }
        drop(worker);
        let replies: Vec<_> = rx.try_iter().collect();
        assert_eq!(replies.len(), 6);
        for index in [1, 4] {
            assert!(matches!(
                &replies[index],
                Msg::App(AppMsg::SaveCompleted { result: Ok(()), .. })
            ));
        }
        assert!(
            matches!(&replies[3], Msg::Layout(LayoutMsg::FilePrepared { result: Err(error), .. })
            if error.contains("No log file is available"))
        );
        let Msg::Layout(LayoutMsg::FilePrepared {
            result: Ok(directory),
            ..
        }) = &replies[0]
        else {
            panic!("directory reply")
        };
        assert!(matches!(directory.as_ref(), PreparedFile::Directory { path } if path == &config));
        for (index, expected) in [(2, "bindings: [] # keep custom\n"), (5, "worker log\n")] {
            let Msg::Layout(LayoutMsg::FilePrepared {
                result: Ok(prepared),
                ..
            }) = &replies[index]
            else {
                panic!("loaded configuration file")
            };
            let PreparedFile::Loaded { document, .. } = prepared.as_ref() else {
                panic!("file loaded on worker")
            };
            assert_eq!(document.buffer.to_string(), expected);
        }
        assert!(config.join("themes").is_dir());
        assert_eq!(
            std::fs::read_to_string(config.join("keymap.yaml")).unwrap(),
            "bindings: [] # keep custom\n"
        );
    }

    #[test]
    fn config_resource_worker_failure_finishes_the_original_request() {
        use token::commands::ConfigResource;
        let mut model = AppModel::new(800, 600, 1.0);
        let original = model.document().id;
        let (tx, rx) = mpsc::channel();
        let worker = start_worker_resolving_config(tx, None, || None).unwrap();
        let mut ids = Vec::new();
        for resource in [
            ConfigResource::Directory,
            ConfigResource::Keybindings,
            ConfigResource::Log,
        ] {
            let request = config_request(&mut model, resource);
            ids.push(request.id());
            worker.send(FileJob::Open(request)).unwrap();
        }
        drop(worker);
        assert!(model.ui.is_loading);
        let replies: Vec<_> = rx.try_iter().collect();
        assert_eq!(replies.len(), ids.len());
        for (reply, id) in replies.into_iter().zip(ids) {
            assert!(
                matches!(&reply, Msg::Layout(LayoutMsg::FilePrepared { request, result: Err(error) })
                if request.id() == id && error.contains("No configuration directory"))
            );
            update(&mut model, reply);
        }
        assert!(!model.ui.is_loading);
        assert_eq!(model.document().id, original);
        assert_eq!(model.editor_area.documents.len(), 1);
    }

    #[test]
    fn file_io_worker_orders_writes_and_reads_and_drains_on_drop() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ordered.txt");
        let mut model = AppModel::new(800, 600, 1.0);
        let (tx, rx) = mpsc::channel();
        let worker = start_worker(tx, None).unwrap();
        for number in 0..20 {
            worker
                .send(write_job(
                    &mut model,
                    path.clone(),
                    &format!("snapshot {number} 🦀\n").repeat(1000),
                ))
                .unwrap();
        }
        let Cmd::LoadFile {
            target,
            path: read_path,
        } = update(&mut model, Msg::App(AppMsg::LoadFile(path.clone()))).unwrap()
        else {
            panic!("read command")
        };
        worker
            .send(FileJob::Read {
                target,
                path: read_path,
            })
            .unwrap();
        drop(worker); // joins after the last read, with no main-thread polling
        let replies: Vec<_> = rx.try_iter().collect();
        assert_eq!(replies.len(), 21);
        for (number, reply) in replies[..20].iter().enumerate() {
            let Msg::App(AppMsg::SaveCompleted {
                content,
                result,
                identity,
                ..
            }) = reply
            else {
                panic!("save reply")
            };
            assert!(result.is_ok());
            assert_eq!(
                identity.as_ref().unwrap().path(),
                path.canonicalize().unwrap()
            );
            assert_eq!(
                content.to_string(),
                format!("snapshot {number} 🦀\n").repeat(1000)
            );
        }
        let Msg::App(AppMsg::FileLoaded {
            result, identity, ..
        }) = &replies[20]
        else {
            panic!("read reply")
        };
        let expected = "snapshot 19 🦀\n".repeat(1000);
        assert_eq!(
            identity.as_ref().unwrap().path(),
            path.canonicalize().unwrap()
        );
        assert_eq!(result.as_ref().unwrap(), &expected);
        assert_eq!(std::fs::read_to_string(path).unwrap(), expected);
    }

    #[test]
    fn file_io_worker_keeps_writing_when_ui_receiver_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("drained.txt");
        let mut model = AppModel::new(800, 600, 1.0);
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let worker = start_worker(tx, None).unwrap();
        worker
            .send(write_job(&mut model, path.clone(), "first"))
            .unwrap();
        worker
            .send(write_job(&mut model, path.clone(), "last"))
            .unwrap();
        drop(worker);
        assert_eq!(std::fs::read_to_string(path).unwrap(), "last");
    }

    #[test]
    fn file_io_worker_reports_failure_and_continues_with_next_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("after-error.txt");
        let mut model = AppModel::new(800, 600, 1.0);
        let (tx, rx) = mpsc::channel();
        let worker = start_worker(tx, None).unwrap();
        worker
            .send(write_job(&mut model, dir.path().to_path_buf(), "fails"))
            .unwrap();
        worker
            .send(write_job(&mut model, path.clone(), "succeeds"))
            .unwrap();
        drop(worker);
        let replies: Vec<_> = rx.try_iter().collect();
        assert!(matches!(
            &replies[0],
            Msg::App(AppMsg::SaveCompleted { result: Err(_), .. })
        ));
        assert!(matches!(
            &replies[1],
            Msg::App(AppMsg::SaveCompleted { result: Ok(()), .. })
        ));
        assert_eq!(std::fs::read_to_string(path).unwrap(), "succeeds");
    }
}
