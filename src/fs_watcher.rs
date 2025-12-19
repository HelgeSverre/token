//! File system watching for workspace file tree updates
//!
//! Uses the `notify` crate with debouncing to detect file system changes
//! and refresh the workspace file tree automatically.

use notify_debouncer_mini::{new_debouncer, DebouncedEventKind, Debouncer};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

/// Events emitted by the file system watcher
#[derive(Debug, Clone)]
pub enum FileSystemEvent {
    /// A file or directory was created
    Created(PathBuf),
    /// A file was modified
    Modified(PathBuf),
    /// A file or directory was deleted
    Deleted(PathBuf),
    /// Any change occurred (generic, for debounced events)
    Changed(PathBuf),
}

/// File system watcher with debouncing
///
/// Watches a directory recursively and emits debounced events
/// to avoid overwhelming the UI with rapid changes.
pub struct FileSystemWatcher {
    /// The debouncer handles watching and event coalescing
    _debouncer: Debouncer<notify::RecommendedWatcher>,
    /// Receiver for debounced events
    rx: Receiver<Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>>,
    /// Root path being watched
    root: PathBuf,
}

impl FileSystemWatcher {
    /// Create a new file system watcher for a directory
    ///
    /// Events are debounced with a 500ms delay to coalesce rapid changes
    /// (e.g., git operations, build processes).
    pub fn new(root: PathBuf) -> Result<Self, notify::Error> {
        let (tx, rx) = mpsc::channel();

        // 500ms debounce delay - balances responsiveness with avoiding spam
        let debounce_duration = Duration::from_millis(500);

        let mut debouncer = new_debouncer(debounce_duration, tx)?;

        // Watch the root directory recursively
        debouncer
            .watcher()
            .watch(&root, notify::RecursiveMode::Recursive)?;

        tracing::info!("Started file system watcher for: {}", root.display());

        Ok(Self {
            _debouncer: debouncer,
            rx,
            root,
        })
    }

    /// Get the root path being watched
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// Poll for pending file system events (non-blocking)
    ///
    /// Returns a list of paths that changed. The caller should refresh
    /// the file tree when this returns non-empty.
    pub fn poll_events(&self) -> Vec<FileSystemEvent> {
        let mut events = Vec::new();

        // Drain all pending events from the channel
        while let Ok(result) = self.rx.try_recv() {
            match result {
                Ok(debounced_events) => {
                    for event in debounced_events {
                        // Filter out events for ignored paths
                        if self.should_ignore(&event.path) {
                            continue;
                        }

                        let fs_event = match event.kind {
                            DebouncedEventKind::Any => FileSystemEvent::Changed(event.path),
                            DebouncedEventKind::AnyContinuous => {
                                // Continuous events during active changes - skip to avoid spam
                                continue;
                            }
                            // Handle any future variants (non_exhaustive enum)
                            _ => FileSystemEvent::Changed(event.path),
                        };

                        // Deduplicate: don't add the same path twice
                        if !events.iter().any(|e| match (e, &fs_event) {
                            (FileSystemEvent::Changed(p1), FileSystemEvent::Changed(p2)) => {
                                p1 == p2
                            }
                            _ => false,
                        }) {
                            events.push(fs_event);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("File system watcher error: {:?}", e);
                }
            }
        }

        if !events.is_empty() {
            tracing::debug!("File system watcher detected {} changes", events.len());
        }

        events
    }

    /// Check if a path should be ignored (hidden files, build artifacts, etc.)
    fn should_ignore(&self, path: &std::path::Path) -> bool {
        // Get the relative path components
        let relative = path.strip_prefix(&self.root).unwrap_or(path);

        for component in relative.components() {
            if let std::path::Component::Normal(name) = component {
                let name_str = name.to_string_lossy();

                // Ignore patterns matching FileTree::should_ignore
                if name_str.starts_with('.') && name_str != ".gitignore" {
                    return true;
                }

                if matches!(
                    name_str.as_ref(),
                    "target"
                        | "node_modules"
                        | "__pycache__"
                        | ".git"
                        | ".svn"
                        | ".hg"
                        | ".DS_Store"
                        | "Thumbs.db"
                        | ".idea"
                        | ".vscode"
                ) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::thread;
    use tempfile::tempdir;

    // ========================================================================
    // should_ignore tests - comprehensive path filtering
    // ========================================================================

    #[test]
    fn test_should_ignore_git_directory() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return; // Skip if watcher can't be created
        }
        let watcher = watcher.unwrap();

        assert!(watcher.should_ignore(&root.join(".git")));
        assert!(watcher.should_ignore(&root.join(".git/objects")));
        assert!(watcher.should_ignore(&root.join(".git/refs/heads")));
    }

    #[test]
    fn test_should_ignore_build_directories() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        assert!(watcher.should_ignore(&root.join("target")));
        assert!(watcher.should_ignore(&root.join("target/debug")));
        assert!(watcher.should_ignore(&root.join("target/release/binary")));
        assert!(watcher.should_ignore(&root.join("node_modules")));
        assert!(watcher.should_ignore(&root.join("node_modules/package/index.js")));
        assert!(watcher.should_ignore(&root.join("__pycache__")));
        assert!(watcher.should_ignore(&root.join("__pycache__/module.pyc")));
    }

    #[test]
    fn test_should_ignore_ide_directories() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        assert!(watcher.should_ignore(&root.join(".idea")));
        assert!(watcher.should_ignore(&root.join(".idea/workspace.xml")));
        assert!(watcher.should_ignore(&root.join(".vscode")));
        assert!(watcher.should_ignore(&root.join(".vscode/settings.json")));
    }

    #[test]
    fn test_should_ignore_os_files() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        assert!(watcher.should_ignore(&root.join(".DS_Store")));
        assert!(watcher.should_ignore(&root.join("Thumbs.db")));
    }

    #[test]
    fn test_should_ignore_hidden_files_except_gitignore() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        // Hidden files should be ignored
        assert!(watcher.should_ignore(&root.join(".hidden")));
        assert!(watcher.should_ignore(&root.join(".env")));
        assert!(watcher.should_ignore(&root.join(".cache")));

        // But .gitignore should NOT be ignored
        assert!(!watcher.should_ignore(&root.join(".gitignore")));
    }

    #[test]
    fn test_should_not_ignore_source_files() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        assert!(!watcher.should_ignore(&root.join("src/main.rs")));
        assert!(!watcher.should_ignore(&root.join("lib/utils.js")));
        assert!(!watcher.should_ignore(&root.join("Cargo.toml")));
        assert!(!watcher.should_ignore(&root.join("package.json")));
        assert!(!watcher.should_ignore(&root.join("README.md")));
    }

    #[test]
    fn test_should_not_ignore_nested_source_dirs() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if watcher.is_err() {
            return;
        }
        let watcher = watcher.unwrap();

        // Nested directories that look like build dirs but are in source
        assert!(!watcher.should_ignore(&root.join("docs/examples")));
        assert!(!watcher.should_ignore(&root.join("tests/fixtures")));
        assert!(!watcher.should_ignore(&root.join("benches/data")));
    }

    // ========================================================================
    // Watcher creation tests
    // ========================================================================

    #[test]
    fn test_watcher_creation_valid_dir() {
        let dir = tempdir().expect("Failed to create temp dir");
        let watcher = FileSystemWatcher::new(dir.path().to_path_buf());

        assert!(
            watcher.is_ok(),
            "Should be able to create watcher for valid directory"
        );
    }

    #[test]
    fn test_watcher_root_accessor() {
        let dir = tempdir().expect("Failed to create temp dir");
        let root = dir.path().to_path_buf();
        let watcher = FileSystemWatcher::new(root.clone());

        if let Ok(w) = watcher {
            assert_eq!(w.root(), &root);
        }
    }

    #[test]
    fn test_watcher_poll_events_empty_on_no_changes() {
        let dir = tempdir().expect("Failed to create temp dir");
        let watcher = FileSystemWatcher::new(dir.path().to_path_buf());

        if let Ok(w) = watcher {
            // Poll immediately - should return empty
            let events = w.poll_events();
            assert!(
                events.is_empty(),
                "Should have no events when nothing changed"
            );
        }
    }

    // ========================================================================
    // FileSystemEvent tests
    // ========================================================================

    #[test]
    fn test_file_system_event_clone() {
        let event = FileSystemEvent::Changed(PathBuf::from("/test/file.rs"));
        let cloned = event.clone();

        match (event, cloned) {
            (FileSystemEvent::Changed(p1), FileSystemEvent::Changed(p2)) => {
                assert_eq!(p1, p2);
            }
            _ => panic!("Clone should preserve variant"),
        }
    }

    #[test]
    fn test_file_system_event_debug() {
        let event = FileSystemEvent::Created(PathBuf::from("/test/new.rs"));
        let debug_str = format!("{:?}", event);
        assert!(debug_str.contains("Created"));
        assert!(debug_str.contains("new.rs"));
    }

    // ========================================================================
    // Integration tests (marked ignore for CI stability)
    // ========================================================================

    #[test]
    #[ignore] // Flaky in CI - file system event timing varies by platform
    fn test_watcher_detects_file_creation() {
        let dir = tempdir().expect("Failed to create temp dir");
        let watcher =
            FileSystemWatcher::new(dir.path().to_path_buf()).expect("Failed to create watcher");

        // Create a file
        let file_path = dir.path().join("test.txt");
        fs::write(&file_path, "hello").expect("Failed to write file");

        // Wait for debounce (500ms) plus margin
        thread::sleep(Duration::from_millis(1000));

        let events = watcher.poll_events();
        assert!(!events.is_empty(), "Should detect file creation");
    }

    #[test]
    #[ignore] // Flaky in CI - file system event timing varies by platform
    fn test_watcher_detects_file_modification() {
        let dir = tempdir().expect("Failed to create temp dir");

        // Create file first
        let file_path = dir.path().join("existing.txt");
        fs::write(&file_path, "initial content").expect("Failed to write file");

        // Start watching after file exists
        let watcher =
            FileSystemWatcher::new(dir.path().to_path_buf()).expect("Failed to create watcher");

        // Modify the file
        fs::write(&file_path, "modified content").expect("Failed to modify file");

        // Wait for debounce
        thread::sleep(Duration::from_millis(1000));

        let events = watcher.poll_events();
        assert!(!events.is_empty(), "Should detect file modification");
    }

    #[test]
    #[ignore] // Flaky in CI - file system event timing varies by platform
    fn test_watcher_ignores_target_directory() {
        let dir = tempdir().expect("Failed to create temp dir");
        let watcher =
            FileSystemWatcher::new(dir.path().to_path_buf()).expect("Failed to create watcher");

        // Create file in target directory (should be ignored)
        let target_dir = dir.path().join("target");
        fs::create_dir(&target_dir).expect("Failed to create target dir");
        fs::write(target_dir.join("output.bin"), "binary").expect("Failed to write");

        // Wait for debounce
        thread::sleep(Duration::from_millis(1000));

        let events = watcher.poll_events();
        // Events should be empty or not contain target paths
        for event in &events {
            match event {
                FileSystemEvent::Changed(p)
                | FileSystemEvent::Created(p)
                | FileSystemEvent::Modified(p)
                | FileSystemEvent::Deleted(p) => {
                    assert!(
                        !p.to_string_lossy().contains("target"),
                        "Should not report events from target directory"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore] // Flaky in CI - file system event timing varies by platform
    fn test_watcher_deduplicates_events() {
        let dir = tempdir().expect("Failed to create temp dir");
        let watcher =
            FileSystemWatcher::new(dir.path().to_path_buf()).expect("Failed to create watcher");

        // Create and immediately modify the same file multiple times
        let file_path = dir.path().join("rapid.txt");
        for i in 0..5 {
            fs::write(&file_path, format!("content {}", i)).expect("Failed to write");
            thread::sleep(Duration::from_millis(10));
        }

        // Wait for debounce
        thread::sleep(Duration::from_millis(1000));

        let events = watcher.poll_events();

        // Count events for the same path - should be deduplicated
        let rapid_events: Vec<_> = events
            .iter()
            .filter(|e| match e {
                FileSystemEvent::Changed(p) => p.ends_with("rapid.txt"),
                _ => false,
            })
            .collect();

        // Due to deduplication, should have at most 1 event for this file
        assert!(
            rapid_events.len() <= 1,
            "Should deduplicate events for same file"
        );
    }
}
