//! Parent-directory subscriptions for open documents, independent of workspace
//! ignore rules. Owned and updated by the file worker, never by model updates.

use notify::Watcher;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use token::messages::{AppMsg, Msg};

pub(super) struct DocumentWatcher {
    watcher: notify::RecommendedWatcher,
    roots: HashSet<PathBuf>,
}

impl DocumentWatcher {
    pub fn new(
        replies: mpsc::Sender<Msg>,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> Result<Self, notify::Error> {
        let watcher =
            notify::recommended_watcher(move |result: Result<notify::Event, notify::Error>| {
                match result {
                    // Reads by the observation worker must not observe themselves.
                    Ok(event) if !matches!(event.kind, notify::EventKind::Access(_)) => {
                        let mut paths = event.paths;
                        paths.sort();
                        paths.dedup();
                        if !paths.is_empty() {
                            let _ = replies.send(Msg::App(AppMsg::FilesChanged(paths)));
                            if let Some(wake) = &wake {
                                wake();
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!("Open-file watcher: {error}"),
                }
            })?;
        Ok(Self {
            watcher,
            roots: HashSet::new(),
        })
    }

    pub fn sync(&mut self, paths: &[PathBuf]) {
        let desired: HashSet<_> = paths
            .iter()
            .flat_map(|path| {
                // A parent survives atomic file replacement; its parent also
                // catches deletion/recreation of the containing directory.
                path.ancestors()
                    .skip(1)
                    .filter(|parent| parent.is_dir())
                    .take(2)
                    .map(Path::to_path_buf)
            })
            .collect();
        for root in self.roots.difference(&desired) {
            let _ = self.watcher.unwatch(root);
        }
        self.roots.retain(|root| desired.contains(root));
        for root in desired {
            if !self.roots.contains(&root) {
                match self
                    .watcher
                    .watch(&root, notify::RecursiveMode::NonRecursive)
                {
                    Ok(()) => {
                        self.roots.insert(root);
                    }
                    Err(error) => tracing::warn!("Could not watch {}: {error}", root.display()),
                }
            }
        }
    }

    pub fn rearm_parent(&mut self, path: &Path) {
        let Some(parent) = path.parent().filter(|parent| parent.is_dir()) else {
            return;
        };
        if self.roots.contains(parent) {
            let _ = self.watcher.unwatch(parent);
        }
        match self
            .watcher
            .watch(parent, notify::RecursiveMode::NonRecursive)
        {
            Ok(()) => {
                self.roots.insert(parent.to_path_buf());
            }
            Err(error) => {
                self.roots.remove(parent);
                tracing::warn!("Could not rearm {}: {error}", parent.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn await_path(receiver: &mpsc::Receiver<Msg>, expected: &Path, stage: &str) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut observed = Vec::new();
        loop {
            let message = receiver
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|error| {
                    panic!("{stage}: waiting for {expected:?}: {error}; observed {observed:?}")
                });
            if let Msg::App(AppMsg::FilesChanged(paths)) = message {
                if paths.iter().any(|path| path == expected) {
                    return;
                }
                observed.extend(paths);
            }
        }
    }

    /// Establish delivery before testing a one-shot rename. FSEvents streams
    /// restart when subscriptions change; a parent/setup event is not evidence
    /// that events for the file itself can already reach this receiver.
    fn await_ready(receiver: &mpsc::Receiver<Msg>, marker: &Path) {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut attempt = 0;
        loop {
            std::fs::write(marker, attempt.to_string()).unwrap();
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "watcher not ready for {marker:?}");
            match receiver.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(Msg::App(AppMsg::FilesChanged(paths)))
                    if paths.iter().any(|path| path == marker) =>
                {
                    return
                }
                Ok(_) | Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(error) => panic!("watcher readiness for {marker:?}: {error}"),
            }
            attempt += 1;
        }
    }

    #[test]
    fn external_change_watcher_survives_atomic_file_and_parent_replacement() {
        let fixture = tempfile::tempdir().unwrap();
        let root = fixture.path().canonicalize().unwrap();
        let parent = root.join("documents");
        std::fs::create_dir(&parent).unwrap();
        let path = parent.join("open.txt");
        std::fs::write(&path, "original").unwrap();
        let (sender, receiver) = mpsc::channel();
        let mut watcher = DocumentWatcher::new(sender, None).unwrap();
        watcher.sync(std::slice::from_ref(&path));
        assert!(watcher.roots.contains(&parent));
        assert!(watcher.roots.contains(&root));
        await_ready(&receiver, &parent.join("initial-ready"));
        let replacement = parent.join("replacement.txt");
        std::fs::write(&replacement, "replaced").unwrap();
        std::fs::rename(&replacement, &path).unwrap();
        await_path(&receiver, &path, "atomic file replacement");

        // Losing the containing directory must not permanently lose coverage.
        std::fs::rename(&parent, root.join("old-documents")).unwrap();
        await_path(&receiver, &parent, "parent rename");
        std::fs::create_dir(&parent).unwrap();
        watcher.rearm_parent(&path);
        await_ready(&receiver, &parent.join("rearmed-ready"));
        std::fs::write(&path, "recreated").unwrap();
        await_path(&receiver, &path, "file creation after rearm");
        watcher.sync(&[]);
        assert!(watcher.roots.is_empty());
    }
}
