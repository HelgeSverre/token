//! Persistent recent files list
//!
//! Tracks files opened in the editor and persists them to disk.
//! Files are stored in MRU (most recently used) order with a capacity limit.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Maximum number of entries to keep
const MAX_ENTRIES: usize = 50;

/// A single entry in the recent files list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentEntry {
    /// Absolute path to the file
    pub path: PathBuf,
    /// Timestamp when last opened (Unix epoch seconds)
    pub opened_at: u64,
    /// Workspace root when file was opened (if any)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace: Option<PathBuf>,
    /// Number of times file has been opened (for ranking)
    #[serde(default)]
    pub open_count: u32,
}

impl RecentEntry {
    /// Create a new entry for the current time
    pub fn new(path: PathBuf, workspace: Option<PathBuf>) -> Self {
        Self {
            path,
            opened_at: now_epoch_secs(),
            workspace,
            open_count: 1,
        }
    }

    /// Update entry for re-opening
    pub fn touch(&mut self) {
        self.opened_at = now_epoch_secs();
        self.open_count += 1;
    }

    /// Get display path (relative to workspace if available, otherwise filename)
    pub fn display_path(&self) -> String {
        if let Some(ws) = &self.workspace {
            if let Ok(relative) = self.path.strip_prefix(ws) {
                return relative.to_string_lossy().to_string();
            }
        }
        self.path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| self.path.to_string_lossy().to_string())
    }

    /// Get human-readable time since opened
    pub fn time_ago(&self) -> String {
        let now = now_epoch_secs();
        let diff = now.saturating_sub(self.opened_at);

        if diff < 60 {
            "just now".to_string()
        } else if diff < 3600 {
            let mins = diff / 60;
            format!("{} min{} ago", mins, if mins == 1 { "" } else { "s" })
        } else if diff < 86400 {
            let hours = diff / 3600;
            format!("{} hour{} ago", hours, if hours == 1 { "" } else { "s" })
        } else if diff < 604800 {
            let days = diff / 86400;
            format!("{} day{} ago", days, if days == 1 { "" } else { "s" })
        } else {
            let weeks = diff / 604800;
            format!("{} week{} ago", weeks, if weeks == 1 { "" } else { "s" })
        }
    }

    /// Check if file still exists
    pub fn exists(&self) -> bool {
        self.path.exists()
    }
}

fn now_epoch_secs() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Persistent recent files list
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecentFiles {
    /// Schema version for forward compatibility
    #[serde(default)]
    pub version: u32,
    /// Recent file entries, most recent first
    pub entries: Vec<RecentEntry>,
}

impl RecentFiles {
    pub const CURRENT_VERSION: u32 = 1;

    /// Load recent files from disk
    pub fn load() -> Self {
        let Some(path) = crate::config_paths::recent_files_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => {
                let mut recent: Self = serde_json::from_str(&contents).unwrap_or_default();
                recent.prune_missing();
                recent
            }
            Err(_) => Self::default(),
        }
    }

    /// Save recent files to disk
    pub fn save(&self) -> std::io::Result<()> {
        let Some(path) = crate::config_paths::recent_files_path() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No config directory available",
            ));
        };
        crate::config_paths::ensure_all_config_dirs();
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)
    }

    /// Add a file to recent list (or update if already present)
    pub fn add(&mut self, path: PathBuf, workspace: Option<PathBuf>) {
        // Canonicalize path for consistent matching
        let canonical = path.canonicalize().unwrap_or(path);

        // Check if already in list
        if let Some(idx) = self.find_index(&canonical) {
            // Update existing entry and move to front
            self.entries[idx].touch();
            if let Some(ws) = workspace {
                self.entries[idx].workspace = Some(ws);
            }
            let entry = self.entries.remove(idx);
            self.entries.insert(0, entry);
        } else {
            // Add new entry at front
            let entry = RecentEntry::new(canonical, workspace);
            self.entries.insert(0, entry);
        }

        // Enforce capacity limit
        self.entries.truncate(MAX_ENTRIES);
    }

    /// Remove a file from recent list
    pub fn remove(&mut self, path: &Path) {
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        self.entries.retain(|e| e.path != canonical);
    }

    /// Clear all recent files
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Prune entries for files that no longer exist
    pub fn prune_missing(&mut self) {
        let original_len = self.entries.len();
        self.entries.retain(|e| e.exists());
        if self.entries.len() != original_len {
            tracing::debug!(
                "Pruned {} missing files from recent list",
                original_len - self.entries.len()
            );
        }
    }

    /// Find index of entry by path
    fn find_index(&self, path: &Path) -> Option<usize> {
        self.entries.iter().position(|e| e.path == path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_and_retrieve() {
        let mut recent = RecentFiles::default();
        let path = PathBuf::from("/test/file.rs");

        recent.add(path.clone(), None);

        assert_eq!(recent.entries.len(), 1);
        assert_eq!(recent.entries[0].path, path);
    }

    #[test]
    fn test_reopening_moves_to_front() {
        let mut recent = RecentFiles::default();

        recent.add(PathBuf::from("/first.rs"), None);
        recent.add(PathBuf::from("/second.rs"), None);
        recent.add(PathBuf::from("/first.rs"), None); // Reopen first

        assert_eq!(recent.entries[0].path, PathBuf::from("/first.rs"));
        assert_eq!(recent.entries.len(), 2); // No duplicate
    }

    #[test]
    fn test_capacity_limit() {
        let mut recent = RecentFiles::default();

        for i in 0..100 {
            recent.add(PathBuf::from(format!("/file{}.rs", i)), None);
        }

        assert_eq!(recent.entries.len(), MAX_ENTRIES);
    }

    #[test]
    fn test_time_ago() {
        let entry = RecentEntry::new(PathBuf::from("/test.rs"), None);
        assert_eq!(entry.time_ago(), "just now");
    }

    #[test]
    fn test_display_path_with_workspace() {
        let entry = RecentEntry {
            path: PathBuf::from("/project/src/main.rs"),
            opened_at: 0,
            workspace: Some(PathBuf::from("/project")),
            open_count: 1,
        };

        assert_eq!(entry.display_path(), "src/main.rs");
    }

    #[test]
    fn test_display_path_without_workspace() {
        let entry = RecentEntry {
            path: PathBuf::from("/project/src/main.rs"),
            opened_at: 0,
            workspace: None,
            open_count: 1,
        };

        assert_eq!(entry.display_path(), "main.rs");
    }

    #[test]
    fn test_remove() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        recent.add(PathBuf::from("/b.rs"), None);

        recent.remove(&PathBuf::from("/a.rs"));
        assert_eq!(recent.entries.len(), 1);
        assert_eq!(recent.entries[0].path, PathBuf::from("/b.rs"));
    }

    #[test]
    fn test_clear() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        recent.add(PathBuf::from("/b.rs"), None);

        recent.clear();
        assert!(recent.entries.is_empty());
    }

    #[test]
    fn test_open_count_increments() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        assert_eq!(recent.entries[0].open_count, 1);

        recent.add(PathBuf::from("/a.rs"), None);
        assert_eq!(recent.entries[0].open_count, 2);
    }

    #[test]
    fn test_workspace_updated_on_reopen() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        assert!(recent.entries[0].workspace.is_none());

        recent.add(
            PathBuf::from("/a.rs"),
            Some(PathBuf::from("/workspace")),
        );
        assert_eq!(
            recent.entries[0].workspace,
            Some(PathBuf::from("/workspace"))
        );
    }

    #[test]
    fn test_workspace_preserved_on_reopen_without_workspace() {
        let mut recent = RecentFiles::default();
        recent.add(
            PathBuf::from("/a.rs"),
            Some(PathBuf::from("/workspace")),
        );
        // Reopen without workspace — original workspace should be kept
        recent.add(PathBuf::from("/a.rs"), None);
        assert_eq!(
            recent.entries[0].workspace,
            Some(PathBuf::from("/workspace"))
        );
    }

    #[test]
    fn test_serialization_roundtrip() {
        let mut recent = RecentFiles {
            version: RecentFiles::CURRENT_VERSION,
            ..Default::default()
        };
        recent.add(PathBuf::from("/a.rs"), Some(PathBuf::from("/project")));
        recent.add(PathBuf::from("/b.rs"), None);

        let json = serde_json::to_string(&recent).unwrap();
        let loaded: RecentFiles = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.entries.len(), 2);
        assert_eq!(loaded.entries[0].path, PathBuf::from("/b.rs"));
        assert_eq!(loaded.entries[1].path, PathBuf::from("/a.rs"));
        assert_eq!(
            loaded.entries[1].workspace,
            Some(PathBuf::from("/project"))
        );
        assert_eq!(loaded.version, 1);
    }

    #[test]
    fn test_capacity_preserves_most_recent() {
        let mut recent = RecentFiles::default();
        for i in 0..100 {
            recent.add(PathBuf::from(format!("/file{}.rs", i)), None);
        }
        // Most recent (99) should be first, oldest kept should be 50
        assert_eq!(recent.entries[0].path, PathBuf::from("/file99.rs"));
        assert_eq!(
            recent.entries[MAX_ENTRIES - 1].path,
            PathBuf::from("/file50.rs")
        );
    }

    #[test]
    fn test_time_ago_ranges() {
        let now = now_epoch_secs();

        // Minutes
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 120,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "2 mins ago");

        // 1 minute (singular)
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 60,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "1 min ago");

        // Hours
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 7200,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "2 hours ago");

        // 1 hour (singular)
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 3600,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "1 hour ago");

        // Days
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 172800,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "2 days ago");

        // Weeks
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 1209600,
            workspace: None,
            open_count: 1,
        };
        assert_eq!(entry.time_ago(), "2 weeks ago");
    }

    #[test]
    fn test_display_path_no_filename() {
        let entry = RecentEntry {
            path: PathBuf::from("/"),
            opened_at: 0,
            workspace: None,
            open_count: 1,
        };
        // Root path has no file_name(), should fall back to full path
        assert_eq!(entry.display_path(), "/");
    }

    #[test]
    fn test_find_index() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        recent.add(PathBuf::from("/b.rs"), None);

        assert_eq!(recent.find_index(&PathBuf::from("/a.rs")), Some(1));
        assert_eq!(recent.find_index(&PathBuf::from("/b.rs")), Some(0));
        assert_eq!(recent.find_index(&PathBuf::from("/c.rs")), None);
    }

    #[test]
    fn test_default_has_empty_entries() {
        let recent = RecentFiles::default();
        assert!(recent.entries.is_empty());
        assert_eq!(recent.version, 0);
    }
}
