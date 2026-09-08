//! Bounded, cancelable directory suggestions. Never queued ahead of file saves.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc};
use std::time::{Duration, Instant};

use token::completion::path::{PathDirectory, PathEntry, PathRequest, PathResults};
use token::messages::{CompletionMsg, Msg};
use token::util::ByteSize;

pub(super) type PathWorker = super::latest_worker::LatestWorker<Arc<PathRequest>>;

const MAX_ENTRIES: usize = 500;
const MAX_SCANNED: usize = 20_000;
const MAX_NAMES: ByteSize = ByteSize::mebibytes(1);
const SCAN_BUDGET: Duration = Duration::from_millis(50);

pub(super) fn start(
    sender: Sender<Msg>,
    wake: Option<Arc<dyn Fn() + Send + Sync>>,
) -> std::io::Result<PathWorker> {
    PathWorker::start(
        "path-completion",
        sender,
        wake,
        |request, cancelled| {
            Msg::Completion(CompletionMsg::PathsReady {
                request: Arc::clone(request),
                result: collect(request, cancelled).map_err(|error| error.to_string()),
            })
        },
        |request| {
            Msg::Completion(CompletionMsg::PathsReady {
                request,
                result: Err("Path worker panicked".into()),
            })
        },
    )
}

fn collect(request: &PathRequest, cancelled: &AtomicBool) -> std::io::Result<PathResults> {
    let started = Instant::now();
    let directory = match &request.context.directory {
        PathDirectory::Local(path) => path.clone(),
        PathDirectory::HomeRelative(path) => dirs::home_dir()
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, "Home directory unavailable")
            })?
            .join(path),
    };
    if cancelled.load(Ordering::Relaxed) {
        return Ok(PathResults::default());
    }
    let query = request.context.query.to_ascii_lowercase();
    let show_hidden = query.starts_with('.');
    let mut results = PathResults::default();
    let mut name_bytes = 0;
    for (scanned, entry) in std::fs::read_dir(directory)?.enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Ok(PathResults::default());
        }
        if scanned >= MAX_SCANNED || started.elapsed() >= SCAN_BUDGET {
            results.truncated = true;
            break;
        }
        let entry = entry?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if (!show_hidden && name.starts_with('.')) || !name.to_ascii_lowercase().starts_with(&query)
        {
            continue;
        }
        let file_type = match entry.file_type() {
            Ok(kind) => kind,
            Err(_) => continue,
        };
        let (is_directory, is_file) = if file_type.is_symlink() {
            match entry.path().metadata() {
                Ok(metadata) => (metadata.is_dir(), metadata.is_file()),
                Err(_) => continue,
            }
        } else {
            (file_type.is_dir(), file_type.is_file())
        };
        if !is_directory && !is_file {
            continue;
        }
        let item = PathEntry { name, is_directory };
        if !item.is_directory
            && item.name == request.context.query
            && request
                .cursors
                .get(request.active_cursor_index)
                .is_some_and(|cursor| {
                    cursor.line == request.context.end.line
                        && cursor.column == request.context.end.column
                })
        {
            continue;
        }
        if request.context.insertion(&item).is_none() {
            continue;
        }
        if results.entries.len() == MAX_ENTRIES
            || name_bytes + item.name.len() > MAX_NAMES.as_usize()
        {
            results.truncated = true;
            break;
        }
        name_bytes += item.name.len();
        results.entries.push(item);
    }
    results.entries.sort_by(|left, right| {
        (!left.is_directory, &left.name).cmp(&(!right.is_directory, &right.name))
    });
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use token::{model::AppModel, Cmd};

    fn request(root: &std::path::Path, query: &str) -> Arc<PathRequest> {
        fn extract(command: Cmd) -> Option<Arc<PathRequest>> {
            match command {
                Cmd::CompletePaths(request) => Some(request),
                Cmd::Batch(commands) => commands.into_iter().find_map(extract),
                _ => None,
            }
        }
        let mut model = AppModel::new(1000, 700, 1.0);
        model.document_mut().buffer = format!("./{query}").into();
        model.document_mut().file_path = Some(root.join("current.txt"));
        model.editor_mut().cursors[0] = token::model::Cursor::at(0, 2 + query.chars().count());
        model.editor_mut().clear_selection();
        extract(
            token::update::update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu)).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn path_completion_directory_reads_prefixes_hidden_names_and_unicode() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("assets")).unwrap();
        for name in [
            "asset.txt",
            "other.txt",
            ".hidden",
            "héllo.txt",
            "has space.txt",
        ] {
            std::fs::write(temp.path().join(name), "").unwrap();
        }
        let cancelled = AtomicBool::new(false);
        let found = collect(&request(temp.path(), "as"), &cancelled).unwrap();
        assert_eq!(
            found.entries,
            vec![
                PathEntry {
                    name: "assets".into(),
                    is_directory: true
                },
                PathEntry {
                    name: "asset.txt".into(),
                    is_directory: false
                },
            ]
        );
        assert_eq!(
            collect(&request(temp.path(), ".h"), &cancelled)
                .unwrap()
                .entries[0]
                .name,
            ".hidden"
        );
        assert_eq!(
            collect(&request(temp.path(), "hé"), &cancelled)
                .unwrap()
                .entries[0]
                .name,
            "héllo.txt"
        );
        let all = collect(&request(temp.path(), ""), &cancelled).unwrap();
        assert!(
            collect(&request(temp.path(), "asset.txt"), &cancelled)
                .unwrap()
                .entries
                .is_empty(),
            "already-complete filenames must not consume Tab for a no-op"
        );
        assert!(!all
            .entries
            .iter()
            .any(|entry| entry.name.starts_with('.') || entry.name.contains(' ')));
    }

    #[test]
    fn path_completion_directory_limits_and_cancellation_are_bounded() {
        let temp = tempfile::tempdir().unwrap();
        for index in 0..MAX_ENTRIES + 10 {
            std::fs::write(temp.path().join(format!("item{index}")), "").unwrap();
        }
        let request = request(temp.path(), "item");
        let found = collect(&request, &AtomicBool::new(false)).unwrap();
        assert!(found.truncated);
        assert!(found.entries.len() <= MAX_ENTRIES);
        assert!(collect(&request, &AtomicBool::new(true))
            .unwrap()
            .entries
            .is_empty());
        let missing = tempfile::tempdir().unwrap();
        let missing_request = self::request(missing.path(), "");
        missing.close().unwrap();
        assert!(collect(&missing_request, &AtomicBool::new(false)).is_err());
        assert!(collect(&missing_request, &AtomicBool::new(true)).is_ok());
    }

    #[cfg(unix)]
    #[test]
    fn path_completion_directory_handles_symlinks() {
        use std::os::unix::fs::symlink;
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("target")).unwrap();
        symlink("target", temp.path().join("alias")).unwrap();
        symlink("missing", temp.path().join("broken")).unwrap();
        let found = collect(&request(temp.path(), ""), &AtomicBool::new(false)).unwrap();
        assert!(found
            .entries
            .iter()
            .any(|entry| entry.name == "alias" && entry.is_directory));
        assert!(!found.entries.iter().any(|entry| entry.name == "broken"));
        assert_eq!(found.entries.len(), 2);
    }

    // APFS rejects invalid UTF-8 filenames at creation time. Exercise this
    // filesystem boundary on Linux, where such names can actually exist.
    #[cfg(target_os = "linux")]
    #[test]
    fn path_completion_directory_omits_non_utf8_names() {
        use std::os::unix::ffi::OsStringExt;
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(std::ffi::OsString::from_vec(vec![0xff])),
            "",
        )
        .unwrap();
        assert!(collect(&request(temp.path(), ""), &AtomicBool::new(false))
            .unwrap()
            .entries
            .is_empty());
    }
}
