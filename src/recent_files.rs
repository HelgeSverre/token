//! Recent file history and its picker
//!
//! Tracks files opened in the editor and persists them to disk.
//! Files are stored in MRU (most recently used) order with a capacity limit.
//! The picker owns filtering, date/pin grouping and selection preservation.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::editable::{EditConstraints, EditableState, StringBuffer};
use crate::model::COMMAND_PALETTE_MAX_VISIBLE;
use crate::view::overlay_surface::{resolve_scroll_for_selection, SectionShape};

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
    /// Whether the entry is pinned to the top of the Recent Files list
    /// (overlay-surface.md: `⌘.` toggle, Pinned section).
    #[serde(default)]
    pub pinned: bool,
}

/// Which section of the Recent Files modal an entry belongs in
/// (overlay-surface.md Phase 3: Pinned / Today / Yesterday / Earlier).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecentGroup {
    Pinned,
    Today,
    Yesterday,
    Earlier,
}

impl RecentGroup {
    fn title(&self) -> &'static str {
        match self {
            RecentGroup::Pinned => "Pinned",
            RecentGroup::Today => "Today",
            RecentGroup::Yesterday => "Yesterday",
            RecentGroup::Earlier => "Earlier",
        }
    }
}

impl RecentEntry {
    /// Create a new entry for the current time
    pub fn new(path: PathBuf, workspace: Option<PathBuf>) -> Self {
        Self {
            path,
            opened_at: now_epoch_secs(),
            workspace,
            open_count: 1,
            pinned: false,
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

    /// Which Recent Files section this entry belongs in: pinned entries
    /// always sort first regardless of recency; the rest are bucketed by
    /// calendar day (UTC — no timezone dependency in this crate).
    fn group(&self) -> RecentGroup {
        if self.pinned {
            return RecentGroup::Pinned;
        }
        let now = now_epoch_secs();
        let diff_days = (now / 86400).saturating_sub(self.opened_at / 86400);
        match diff_days {
            0 => RecentGroup::Today,
            1 => RecentGroup::Yesterday,
            _ => RecentGroup::Earlier,
        }
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
    /// Under `cfg(test)` this always returns empty — same isolation rule as
    /// `CommandHistory::load`: unit tests must not observe the developer's
    /// real `recent.json`.
    pub fn load() -> Self {
        #[cfg(test)]
        {
            Self::default()
        }
        #[cfg(not(test))]
        {
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

    /// Add a boundary-resolved path (or update if already present), without I/O.
    /// Callers with an open document should use its captured file identity.
    pub fn add(&mut self, path: PathBuf, workspace: Option<PathBuf>) {
        // Check if already in list
        if let Some(idx) = self.find_index(&path) {
            // Update existing entry and move to front
            self.entries[idx].touch();
            if let Some(ws) = workspace {
                self.entries[idx].workspace = Some(ws);
            }
            let entry = self.entries.remove(idx);
            self.entries.insert(0, entry);
        } else {
            // Add new entry at front
            let entry = RecentEntry::new(path, workspace);
            self.entries.insert(0, entry);
        }

        // Enforce capacity limit
        self.entries.truncate(MAX_ENTRIES);
    }

    /// Toggle the pinned flag for an entry, if present.
    fn toggle_pin(&mut self, path: &Path) {
        if let Some(idx) = self.find_index(path) {
            self.entries[idx].pinned = !self.entries[idx].pinned;
        }
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

/// Recent Files modal state. Owns filtering, section ordering and selection
/// preservation; callers never translate between display rows and MRU entries.
#[derive(Debug, Clone)]
pub struct RecentFilesState {
    pub selected_index: usize,
    pub editable: EditableState<StringBuffer>,
    pub scroll_offset: usize,
    entries: Vec<RecentEntry>,
    filtered_rows: Vec<usize>,
}

impl RecentFilesState {
    /// Keep the current file in MRU order, but preselect the previous file
    /// when the current file is first so immediate confirmation quick-switches.
    pub fn new(recent: &RecentFiles, current_file: Option<&Path>) -> Self {
        let entries = recent.entries.clone();
        let selected_entry = match current_file {
            Some(current)
                if entries.len() > 1
                    && entries
                        .first()
                        .is_some_and(|entry| entry.path.as_path() == current) =>
            {
                1
            }
            _ => 0,
        };
        let mut state = Self {
            selected_index: 0,
            entries,
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            scroll_offset: 0,
            filtered_rows: Vec::new(),
        };
        state.recompute_filtered_rows();
        state.selected_index = state
            .filtered_rows
            .iter()
            .position(|&i| i == selected_entry)
            .unwrap_or(0);
        state
    }

    pub fn input(&self) -> String {
        self.editable.text()
    }

    /// Refresh after editing the filter, starting selection and scrolling over.
    pub fn filter_changed(&mut self) {
        self.recompute_filtered_rows();
        self.selected_index = 0;
        self.scroll_offset = 0;
    }

    fn recompute_filtered_rows(&mut self) {
        let filter = self.input().to_lowercase();
        let mut order: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| filter.is_empty() || e.display_path().to_lowercase().contains(&filter))
            .map(|(i, _)| i)
            .collect();
        // Stable grouping preserves MRU order within each section.
        order.sort_by_key(|&i| self.entries[i].group() as u8);
        self.filtered_rows = order;
    }

    pub fn rows(&self) -> impl ExactSizeIterator<Item = &RecentEntry> {
        self.filtered_rows.iter().map(|&i| &self.entries[i])
    }

    /// Section boundaries shared by rendering, navigation and automation.
    pub fn groups(&self) -> Vec<(&'static str, Vec<&RecentEntry>)> {
        let mut groups: Vec<(&'static str, Vec<&RecentEntry>)> = Vec::new();
        for entry in self.rows() {
            let title = entry.group().title();
            match groups.last_mut() {
                Some((last_title, entries)) if *last_title == title => entries.push(entry),
                _ => groups.push((title, vec![entry])),
            }
        }
        groups
    }

    pub(crate) fn section_shapes(&self) -> Vec<SectionShape> {
        self.groups()
            .iter()
            .map(|(_, entries)| SectionShape {
                has_title: true,
                len: entries.len(),
            })
            .collect()
    }

    pub fn selected_entry(&self) -> Option<&RecentEntry> {
        self.filtered_rows
            .get(self.selected_index)
            .map(|&i| &self.entries[i])
    }

    /// Toggle the selected pin in the modal and persistent history, keeping
    /// the same file selected and visible after regrouping. Returns whether
    /// there was a selection to persist.
    pub fn toggle_selected_pin(&mut self, recent: &mut RecentFiles) -> bool {
        let Some(&entry_idx) = self.filtered_rows.get(self.selected_index) else {
            return false;
        };
        let entry = &mut self.entries[entry_idx];
        recent.toggle_pin(&entry.path);
        entry.pinned = !entry.pinned;
        self.recompute_filtered_rows();
        self.selected_index = self
            .filtered_rows
            .iter()
            .position(|&i| i == entry_idx)
            .expect("pinning does not change which entries match the filter");
        self.scroll_offset = resolve_scroll_for_selection(
            &self.section_shapes(),
            self.selected_index,
            COMMAND_PALETTE_MAX_VISIBLE,
            self.scroll_offset,
        );
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, workspace: Option<&str>) -> RecentEntry {
        RecentEntry {
            path: PathBuf::from(path),
            opened_at: 0,
            workspace: workspace.map(PathBuf::from),
            open_count: 1,
            pinned: false,
        }
    }

    fn make_recent(entries: Vec<RecentEntry>) -> RecentFiles {
        RecentFiles {
            version: 1,
            entries,
        }
    }

    #[test]
    fn test_recent_files_state_includes_current_file() {
        let recent = make_recent(vec![
            make_entry("/a.rs", None),
            make_entry("/b.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(Path::new("/a.rs")));
        assert_eq!(
            state
                .rows()
                .map(|entry| entry.path.clone())
                .collect::<Vec<_>>(),
            vec![
                PathBuf::from("/a.rs"),
                PathBuf::from("/b.rs"),
                PathBuf::from("/c.rs")
            ]
        );
    }

    #[test]
    fn test_recent_files_state_preselects_previous_file_when_current_is_first() {
        let recent = make_recent(vec![
            make_entry("/a.rs", None),
            make_entry("/b.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(Path::new("/a.rs")));
        assert_eq!(state.selected_index, 1);
    }

    #[test]
    fn test_recent_files_state_keeps_default_selection_when_current_is_only_entry() {
        let recent = make_recent(vec![make_entry("/a.rs", None)]);
        let state = RecentFilesState::new(&recent, Some(Path::new("/a.rs")));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_keeps_default_selection_when_current_not_first() {
        let recent = make_recent(vec![
            make_entry("/b.rs", None),
            make_entry("/a.rs", None),
            make_entry("/c.rs", None),
        ]);
        let state = RecentFilesState::new(&recent, Some(Path::new("/a.rs")));
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_no_current_file() {
        let recent = make_recent(vec![make_entry("/a.rs", None), make_entry("/b.rs", None)]);
        let state = RecentFilesState::new(&recent, None);
        assert_eq!(state.rows().len(), 2);
    }

    #[test]
    fn test_recent_files_state_filter() {
        let recent = make_recent(vec![
            make_entry("/project/src/main.rs", Some("/project")),
            make_entry("/project/Cargo.toml", Some("/project")),
            make_entry("/other/README.md", None),
        ]);
        let mut state = RecentFilesState::new(&recent, None);

        // No filter — all entries
        assert_eq!(state.rows().len(), 3);

        // Filter by "main"
        state.selected_index = 2;
        state.scroll_offset = 1;
        state.editable.set_content("main");
        state.filter_changed();
        assert_eq!(state.rows().len(), 1);
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.scroll_offset, 0);
        assert_eq!(
            state.selected_entry().unwrap().path,
            PathBuf::from("/project/src/main.rs")
        );
    }

    #[test]
    fn test_recent_files_state_filter_case_insensitive() {
        let recent = make_recent(vec![make_entry("/project/src/Main.rs", Some("/project"))]);
        let mut state = RecentFilesState::new(&recent, None);
        state.editable.set_content("main");
        state.filter_changed();
        assert_eq!(state.rows().len(), 1);
    }

    #[test]
    fn test_recent_files_state_empty() {
        let recent = RecentFiles::default();
        let state = RecentFilesState::new(&recent, None);
        assert_eq!(state.rows().len(), 0);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_initial_selection() {
        let recent = make_recent(vec![make_entry("/a.rs", None), make_entry("/b.rs", None)]);
        let state = RecentFilesState::new(&recent, None);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_recent_files_state_puts_pinned_first_preserving_mru_order() {
        let mut a = make_entry("/a.rs", None);
        a.pinned = true;
        let mut c = make_entry("/c.rs", None);
        c.pinned = true;
        let recent = make_recent(vec![
            make_entry("/z.rs", None), // unpinned, MRU-first
            a,                         // pinned
            make_entry("/y.rs", None), // unpinned
            c,                         // pinned
        ]);
        let state = RecentFilesState::new(&recent, None);

        // Pinned entries (a, c) sort before unpinned (z, y), each group
        // keeping its original MRU-relative order — not re-sorted by name.
        let ordered_paths: Vec<_> = state.rows().map(|entry| entry.path.clone()).collect();
        assert_eq!(
            ordered_paths,
            vec![
                PathBuf::from("/a.rs"),
                PathBuf::from("/c.rs"),
                PathBuf::from("/z.rs"),
                PathBuf::from("/y.rs"),
            ]
        );
    }

    #[test]
    fn test_recent_files_state_respects_query_filter() {
        let recent = make_recent(vec![
            make_entry("/project/main.rs", Some("/project")),
            make_entry("/project/lib.rs", Some("/project")),
        ]);
        let mut state = RecentFilesState::new(&recent, None);
        state.editable.set_content("main");
        state.filter_changed();

        assert_eq!(state.rows().len(), 1);
        assert_eq!(
            state.selected_entry().unwrap().path,
            PathBuf::from("/project/main.rs")
        );
    }

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
            pinned: false,
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
            pinned: false,
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

        recent.add(PathBuf::from("/a.rs"), Some(PathBuf::from("/workspace")));
        assert_eq!(
            recent.entries[0].workspace,
            Some(PathBuf::from("/workspace"))
        );
    }

    #[test]
    fn test_workspace_preserved_on_reopen_without_workspace() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), Some(PathBuf::from("/workspace")));
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
        assert_eq!(loaded.entries[1].workspace, Some(PathBuf::from("/project")));
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
            pinned: false,
        };
        assert_eq!(entry.time_ago(), "2 mins ago");

        // 1 minute (singular)
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 60,
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        assert_eq!(entry.time_ago(), "1 min ago");

        // Hours
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 7200,
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        assert_eq!(entry.time_ago(), "2 hours ago");

        // 1 hour (singular)
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 3600,
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        assert_eq!(entry.time_ago(), "1 hour ago");

        // Days
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 172800,
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        assert_eq!(entry.time_ago(), "2 days ago");

        // Weeks
        let entry = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now - 1209600,
            workspace: None,
            open_count: 1,
            pinned: false,
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
            pinned: false,
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

    #[test]
    fn test_group_pinned_wins_regardless_of_age() {
        let now = now_epoch_secs();
        let entry = RecentEntry {
            path: PathBuf::from("/old.rs"),
            opened_at: now.saturating_sub(1_000_000),
            workspace: None,
            open_count: 1,
            pinned: true,
        };
        assert_eq!(entry.group(), RecentGroup::Pinned);
    }

    #[test]
    fn test_group_buckets_by_calendar_day() {
        let now = now_epoch_secs();
        let today = RecentEntry {
            path: PathBuf::from("/t.rs"),
            opened_at: now,
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        let yesterday = RecentEntry {
            path: PathBuf::from("/y.rs"),
            opened_at: now.saturating_sub(86400),
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        let earlier = RecentEntry {
            path: PathBuf::from("/e.rs"),
            opened_at: now.saturating_sub(86400 * 5),
            workspace: None,
            open_count: 1,
            pinned: false,
        };
        assert_eq!(today.group(), RecentGroup::Today);
        assert_eq!(yesterday.group(), RecentGroup::Yesterday);
        assert_eq!(earlier.group(), RecentGroup::Earlier);
    }

    #[test]
    fn test_toggle_pin_flips_flag_and_is_idempotent_on_missing_path() {
        let mut recent = RecentFiles::default();
        recent.add(PathBuf::from("/a.rs"), None);
        let path = recent.entries[0].path.clone();

        recent.toggle_pin(&path);
        assert!(recent.entries[0].pinned);

        recent.toggle_pin(&path);
        assert!(!recent.entries[0].pinned);

        // Toggling a path that isn't in the list is a no-op, not a panic.
        recent.toggle_pin(&PathBuf::from("/does-not-exist.rs"));
        assert!(!recent.entries[0].pinned);
    }
}
