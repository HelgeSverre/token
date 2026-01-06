# Quick Open

Fuzzy file search across workspace with Shift+Cmd+O shortcut.

> **Status:** Planned
> **Priority:** P1
> **Effort:** L
> **Created:** 2025-12-19
> **Milestone:** 1 - Navigation
> **Keybinding:** Shift+Cmd+O (replaces former Open Folder binding)

---

## Table of Contents

1. [Overview](#overview)
2. [Architecture](#architecture)
3. [Data Structures](#data-structures)
4. [Keybindings](#keybindings)
5. [Implementation Plan](#implementation-plan)
6. [Testing Strategy](#testing-strategy)
7. [References](#references)

---

## Overview

### Current State

The editor has a workspace concept (`src/model/workspace.rs`) with file tree sidebar. Opening files requires:
- Navigating the sidebar tree manually
- Using `LayoutMsg::OpenFileInNewTab(PathBuf)` directly
- Dropping files onto the window

There is no quick file search/open functionality.

### Goals

1. **Fuzzy file search** - Search workspace files by name with Cmd+P
2. **Instant results** - Sub-100ms response time even for large workspaces
3. **Smart ranking** - Prioritize recently opened files and better matches
4. **Preview on select** - Show file preview while navigating results
5. **Integration with recent files** - Include recently opened files (see F-040)

### Non-Goals

- Full-text search within files (separate feature: find in files)
- Go to symbol/definition (LSP feature)
- Remote file systems or network shares

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           Quick Open Architecture                            │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          File Index Cache                               │ │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────────┐ │ │
│  │  │  Workspace  │───▶│  Indexer    │───▶│  FileIndex                  │ │ │
│  │  │  Root       │    │  (async)    │    │  - files: Vec<IndexedFile>  │ │ │
│  │  └─────────────┘    └─────────────┘    │  - by_name: BTreeMap        │ │ │
│  │         ▲                              │  - trigrams: HashMap        │ │ │
│  │         │ FileSystemChange             └─────────────────────────────┘ │ │
│  │         │ (invalidate)                              │                   │ │
│  └─────────┼───────────────────────────────────────────┼───────────────────┘ │
│            │                                           │                     │
│            │                                           ▼                     │
│  ┌─────────┴─────────────────────────────────────────────────────────────┐  │
│  │                         Quick Open Modal                               │  │
│  │                                                                        │  │
│  │  ┌─────────────────────────────────────────────────────────────────┐  │  │
│  │  │  Query: [ project.rs                                      ]     │  │  │
│  │  ├─────────────────────────────────────────────────────────────────┤  │  │
│  │  │  ★ src/project.rs              (recently opened)               │  │  │
│  │  │    tests/test_project.rs                                       │  │  │
│  │  │    benches/project_bench.rs                                    │  │  │
│  │  │    docs/PROJECT.md                                             │  │  │
│  │  └─────────────────────────────────────────────────────────────────┘  │  │
│  │                                                                        │  │
│  │  [Enter] Open   [Cmd+Enter] Open in Split   [Esc] Cancel              │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### File Indexing Flow

```
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│  Workspace    │────▶│  Background   │────▶│  FileIndex    │
│  Opened       │     │  Scan Thread  │     │  Ready        │
└───────────────┘     └───────────────┘     └───────────────┘
                             │
                             ▼
                      ┌───────────────┐
                      │  Msg::Index   │
                      │  Complete     │
                      └───────────────┘
                             │
                             ▼
┌───────────────┐     ┌───────────────┐
│  FS Watcher   │────▶│  Incremental  │
│  Event        │     │  Update       │
└───────────────┘     └───────────────┘
```

### Module Structure

```
src/
├── model/
│   ├── ui.rs                    # Add QuickOpenState
│   └── workspace.rs             # File tree (existing)
├── file_index.rs                # NEW: File indexing and search
└── update/
    └── modal.rs                 # Quick open modal handling
```

---

## Data Structures

### File Index

```rust
// src/file_index.rs

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// An indexed file for quick searching
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// Full path from workspace root
    pub path: PathBuf,
    /// File name only (for display and primary matching)
    pub name: String,
    /// Lowercased name for case-insensitive matching
    pub name_lower: String,
    /// Path components for path matching
    pub path_components: Vec<String>,
    /// File extension (without dot)
    pub extension: Option<String>,
    /// Size in bytes (for display)
    pub size: u64,
    /// Pre-computed trigrams for fast fuzzy matching
    pub trigrams: HashSet<[char; 3]>,
}

impl IndexedFile {
    /// Create from path relative to workspace root
    pub fn new(path: PathBuf, size: u64) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let name_lower = name.to_lowercase();

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());

        let path_components: Vec<String> = path
            .components()
            .filter_map(|c| c.as_os_str().to_str())
            .map(|s| s.to_string())
            .collect();

        let trigrams = compute_trigrams(&name_lower);

        Self {
            path,
            name,
            name_lower,
            path_components,
            extension,
            size,
            trigrams,
        }
    }
}

/// Compute character trigrams for fuzzy matching
fn compute_trigrams(s: &str) -> HashSet<[char; 3]> {
    let chars: Vec<char> = s.chars().collect();
    let mut trigrams = HashSet::new();

    if chars.len() >= 3 {
        for window in chars.windows(3) {
            trigrams.insert([window[0], window[1], window[2]]);
        }
    }

    trigrams
}

/// Complete file index for a workspace
#[derive(Debug, Clone)]
pub struct FileIndex {
    /// All indexed files
    pub files: Vec<IndexedFile>,
    /// Index by file name for prefix matching
    pub by_name: BTreeMap<String, Vec<usize>>,
    /// Workspace root for resolving absolute paths
    pub root: PathBuf,
    /// When the index was last built
    pub built_at: Instant,
    /// Number of files indexed
    pub file_count: usize,
    /// Indexing duration in milliseconds
    pub index_time_ms: u64,
}

impl FileIndex {
    /// Build index from workspace root
    pub fn build(root: &Path) -> std::io::Result<Self> {
        let start = Instant::now();
        let mut files = Vec::new();

        Self::scan_directory(root, root, &mut files)?;

        // Build name index
        let mut by_name: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        for (idx, file) in files.iter().enumerate() {
            by_name
                .entry(file.name_lower.clone())
                .or_default()
                .push(idx);
        }

        let file_count = files.len();
        let index_time_ms = start.elapsed().as_millis() as u64;

        Ok(Self {
            files,
            by_name,
            root: root.to_path_buf(),
            built_at: Instant::now(),
            file_count,
            index_time_ms,
        })
    }

    /// Recursively scan directory
    fn scan_directory(
        root: &Path,
        dir: &Path,
        files: &mut Vec<IndexedFile>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Skip hidden files and common ignore patterns
            if name_str.starts_with('.') {
                continue;
            }
            if matches!(
                name_str.as_ref(),
                "target" | "node_modules" | "__pycache__" | ".git" | "build" | "dist"
            ) {
                continue;
            }

            if path.is_dir() {
                Self::scan_directory(root, &path, files)?;
            } else if path.is_file() {
                let relative = path.strip_prefix(root).unwrap_or(&path);
                let metadata = entry.metadata()?;
                files.push(IndexedFile::new(relative.to_path_buf(), metadata.len()));
            }
        }

        Ok(())
    }

    /// Search for files matching query
    pub fn search(&self, query: &str, limit: usize) -> Vec<SearchResult> {
        if query.is_empty() {
            // Return first N files (sorted by path)
            return self
                .files
                .iter()
                .take(limit)
                .enumerate()
                .map(|(i, f)| SearchResult {
                    index: i,
                    file: f.clone(),
                    score: 0,
                    matched_ranges: Vec::new(),
                })
                .collect();
        }

        let query_lower = query.to_lowercase();
        let query_chars: Vec<char> = query_lower.chars().collect();

        let mut results: Vec<SearchResult> = self
            .files
            .iter()
            .enumerate()
            .filter_map(|(idx, file)| {
                let (score, ranges) = self.match_file(&query_lower, &query_chars, file);
                if score > 0 {
                    Some(SearchResult {
                        index: idx,
                        file: file.clone(),
                        score,
                        matched_ranges: ranges,
                    })
                } else {
                    None
                }
            })
            .collect();

        // Sort by score (descending), then by name length (shorter = better)
        results.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.file.name.len().cmp(&b.file.name.len()))
        });

        results.truncate(limit);
        results
    }

    /// Match a file against query, returning (score, matched ranges)
    fn match_file(
        &self,
        query: &str,
        query_chars: &[char],
        file: &IndexedFile,
    ) -> (u32, Vec<(usize, usize)>) {
        let name = &file.name_lower;

        // Exact match
        if name == query {
            return (10000, vec![(0, name.len())]);
        }

        // Prefix match
        if name.starts_with(query) {
            return (5000 + (100 - name.len() as u32).max(0), vec![(0, query.len())]);
        }

        // Substring match
        if let Some(pos) = name.find(query) {
            return (
                3000 + (100 - pos as u32).max(0),
                vec![(pos, pos + query.len())],
            );
        }

        // Fuzzy match - all characters in order
        let name_chars: Vec<char> = name.chars().collect();
        let mut matched_ranges = Vec::new();
        let mut query_idx = 0;
        let mut last_match = None;
        let mut score = 0u32;
        let mut consecutive = 0u32;

        for (i, &c) in name_chars.iter().enumerate() {
            if query_idx < query_chars.len() && c == query_chars[query_idx] {
                // Start or extend a range
                match last_match {
                    Some(last) if last + 1 == i => {
                        // Extend current range
                        if let Some((_, end)) = matched_ranges.last_mut() {
                            *end = i + 1;
                        }
                        consecutive += 1;
                        score += 10 + consecutive * 5;
                    }
                    _ => {
                        // Start new range
                        matched_ranges.push((i, i + 1));
                        consecutive = 0;
                        score += 10;

                        // Bonus for matching at word boundary
                        if i == 0 || !name_chars[i - 1].is_alphanumeric() {
                            score += 20;
                        }
                    }
                }
                last_match = Some(i);
                query_idx += 1;
            }
        }

        // All query characters must match
        if query_idx == query_chars.len() {
            // Bonus for shorter file names (more specific match)
            score += (200 - name.len() as u32).max(0);
            (score, matched_ranges)
        } else {
            (0, Vec::new())
        }
    }

    /// Add a file to the index (for incremental updates)
    pub fn add_file(&mut self, path: PathBuf, size: u64) {
        let relative = path.strip_prefix(&self.root).unwrap_or(&path);
        let file = IndexedFile::new(relative.to_path_buf(), size);
        let idx = self.files.len();

        self.by_name
            .entry(file.name_lower.clone())
            .or_default()
            .push(idx);
        self.files.push(file);
        self.file_count += 1;
    }

    /// Remove a file from the index
    pub fn remove_file(&mut self, path: &Path) {
        let relative = path.strip_prefix(&self.root).unwrap_or(path);
        if let Some(idx) = self.files.iter().position(|f| f.path == relative) {
            let file = &self.files[idx];
            if let Some(indices) = self.by_name.get_mut(&file.name_lower) {
                indices.retain(|&i| i != idx);
            }
            self.files.remove(idx);
            self.file_count -= 1;

            // Update indices in by_name map
            for indices in self.by_name.values_mut() {
                for i in indices.iter_mut() {
                    if *i > idx {
                        *i -= 1;
                    }
                }
            }
        }
    }
}

/// A search result with match information
#[derive(Debug, Clone)]
pub struct SearchResult {
    /// Index into FileIndex.files
    pub index: usize,
    /// The matched file
    pub file: IndexedFile,
    /// Match score (higher = better)
    pub score: u32,
    /// Character ranges that matched (for highlighting)
    pub matched_ranges: Vec<(usize, usize)>,
}
```

### Quick Open Modal State

```rust
// Add to src/model/ui.rs

use crate::file_index::{FileIndex, SearchResult};

/// State for the quick open modal (Cmd+P)
#[derive(Debug, Clone)]
pub struct QuickOpenState {
    /// Editable state for the search input
    pub editable: EditableState<StringBuffer>,
    /// Index of selected result
    pub selected_index: usize,
    /// Current search results
    pub results: Vec<SearchResult>,
    /// Whether we're showing recently opened files
    pub showing_recent: bool,
}

impl Default for QuickOpenState {
    fn default() -> Self {
        Self {
            editable: EditableState::new(StringBuffer::new(), EditConstraints::single_line()),
            selected_index: 0,
            results: Vec::new(),
            showing_recent: true,
        }
    }
}

impl QuickOpenState {
    /// Update search results based on current query
    pub fn update_results(
        &mut self,
        index: &FileIndex,
        recent_files: &[PathBuf],
        limit: usize,
    ) {
        let query = self.editable.text();

        if query.is_empty() {
            // Show recent files first, then alphabetical
            self.showing_recent = true;
            self.results = recent_files
                .iter()
                .filter_map(|path| {
                    let relative = path.strip_prefix(&index.root).ok()?;
                    index.files.iter().enumerate().find_map(|(i, f)| {
                        if f.path == relative {
                            Some(SearchResult {
                                index: i,
                                file: f.clone(),
                                score: 10000, // High score for recent
                                matched_ranges: Vec::new(),
                            })
                        } else {
                            None
                        }
                    })
                })
                .take(limit / 2)
                .chain(index.search("", limit / 2))
                .collect();
        } else {
            self.showing_recent = false;
            self.results = index.search(&query, limit);
        }

        // Reset selection to first result
        self.selected_index = 0;
    }

    /// Get the currently selected file path
    pub fn selected_path(&self, index: &FileIndex) -> Option<PathBuf> {
        self.results
            .get(self.selected_index)
            .map(|r| index.root.join(&r.file.path))
    }
}

/// Add QuickOpen to ModalId
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalId {
    CommandPalette,
    GotoLine,
    FindReplace,
    ThemePicker,
    QuickOpen, // NEW
}

/// Add to ModalState
#[derive(Debug, Clone)]
pub enum ModalState {
    CommandPalette(CommandPaletteState),
    GotoLine(GotoLineState),
    FindReplace(FindReplaceState),
    ThemePicker(ThemePickerState),
    QuickOpen(QuickOpenState), // NEW
}
```

### AppModel Integration

```rust
// Updates to src/model/mod.rs

pub struct AppModel {
    // ... existing fields ...

    /// File index for quick open (None if no workspace)
    pub file_index: Option<FileIndex>,
}

impl AppModel {
    /// Rebuild file index from workspace
    pub fn rebuild_file_index(&mut self) {
        if let Some(workspace) = &self.workspace {
            match FileIndex::build(&workspace.root) {
                Ok(index) => {
                    tracing::info!(
                        "Indexed {} files in {}ms",
                        index.file_count,
                        index.index_time_ms
                    );
                    self.file_index = Some(index);
                }
                Err(e) => {
                    tracing::error!("Failed to build file index: {}", e);
                }
            }
        }
    }
}
```

---

## Keybindings

| Action | Mac | Windows/Linux | Notes |
|--------|-----|---------------|-------|
| Open Quick Open | Shift+Cmd+O | Shift+Ctrl+O | Main shortcut (was Cmd+P) |
| Navigate up | Up Arrow | Up Arrow | Select previous result |
| Navigate down | Down Arrow | Down Arrow | Select next result |
| Open file | Enter | Enter | Open in current group |
| Open in split | Cmd+Enter | Ctrl+Enter | Open in new split pane |
| Preview file | - | - | Auto-preview on selection |
| Close | Escape | Escape | Close modal |

---

## Implementation Plan

### Phase 1: File Indexing

**Files:** `src/file_index.rs`

- [ ] Create `IndexedFile` struct with path, name, trigrams
- [ ] Implement `FileIndex::build()` with recursive directory scan
- [ ] Add ignore patterns (node_modules, target, .git, etc.)
- [ ] Implement basic substring search
- [ ] Add benchmarks for indexing performance

**Test:** Index 10,000 files in < 500ms.

### Phase 2: Fuzzy Matching

**Files:** `src/file_index.rs`

- [ ] Implement fuzzy matching algorithm
- [ ] Add word boundary bonuses
- [ ] Add consecutive character bonuses
- [ ] Return matched character ranges for highlighting
- [ ] Add unit tests for matching edge cases

**Test:** Query "mod.rs" matches "src/model/mod.rs" with high score.

### Phase 3: Modal State

**Files:** `src/model/ui.rs`

- [ ] Add `QuickOpenState` struct
- [ ] Add `QuickOpen` to `ModalId` and `ModalState`
- [ ] Implement `update_results()` with recent files integration
- [ ] Add result limit (default: 20 results)

**Test:** Empty query shows recent files first.

### Phase 4: Message Handling

**Files:** `src/messages.rs`, `src/update/modal.rs`

- [ ] Add `ModalMsg::OpenQuickOpen` message
- [ ] Handle input changes → trigger search
- [ ] Handle navigation (up/down)
- [ ] Handle confirm → open file
- [ ] Handle Cmd+Enter → open in split

**Test:** Selecting a file and pressing Enter opens it.

### Phase 5: Rendering

**Files:** `src/view/modal.rs`

- [ ] Render quick open modal with search input
- [ ] Render result list with file names and paths
- [ ] Highlight matched characters in results
- [ ] Show recent indicator (star) for recently opened files
- [ ] Show file size/icon indicators

**Test:** Matched characters are visually highlighted.

### Phase 6: Index Updates

**Files:** `src/file_index.rs`, `src/update/workspace.rs`

- [ ] Integrate with file system watcher
- [ ] Implement incremental `add_file()` and `remove_file()`
- [ ] Debounce index updates during rapid file changes
- [ ] Rebuild index on workspace change

**Test:** Creating a new file adds it to search results.

### Phase 7: Polish

- [ ] Add loading indicator during initial index build
- [ ] Add "no results" message
- [ ] Keyboard shortcut hint in footer
- [ ] Path preview on hover (full path in tooltip)
- [ ] Integration with recent files persistence (F-040)

---

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exact_match() {
        let index = FileIndex::build(Path::new("./test_fixtures")).unwrap();
        let results = index.search("Cargo.toml", 10);
        assert!(!results.is_empty());
        assert_eq!(results[0].file.name, "Cargo.toml");
    }

    #[test]
    fn test_fuzzy_match() {
        let index = FileIndex::build(Path::new("./test_fixtures")).unwrap();
        let results = index.search("crgtml", 10);
        // Should match Cargo.toml
        assert!(results.iter().any(|r| r.file.name == "Cargo.toml"));
    }

    #[test]
    fn test_path_matching() {
        let index = FileIndex::build(Path::new("./test_fixtures")).unwrap();
        let results = index.search("src/lib", 10);
        assert!(results.iter().any(|r| r.file.path.starts_with("src/")));
    }

    #[test]
    fn test_ignore_patterns() {
        let index = FileIndex::build(Path::new("./test_fixtures")).unwrap();
        // Should not include files from node_modules or target
        assert!(!index.files.iter().any(|f| {
            f.path.starts_with("node_modules/") || f.path.starts_with("target/")
        }));
    }

    #[test]
    fn test_match_ranges() {
        let index = FileIndex::build(Path::new("./test_fixtures")).unwrap();
        let results = index.search("main", 10);
        let main_rs = results.iter().find(|r| r.file.name == "main.rs").unwrap();
        assert!(!main_rs.matched_ranges.is_empty());
    }
}
```

### Integration Tests

```rust
// tests/quick_open_tests.rs

#[test]
fn test_quick_open_flow() {
    // Open quick open modal
    // Type partial filename
    // Verify results appear
    // Select and open file
    // Verify file opened in editor
}

#[test]
fn test_quick_open_recent_priority() {
    // Open several files
    // Open quick open with empty query
    // Verify recently opened files appear first
}

#[test]
fn test_quick_open_split_open() {
    // Open quick open
    // Select file
    // Press Cmd+Enter
    // Verify file opened in new split
}
```

### Performance Tests

```rust
// benches/quick_open.rs

#[bench]
fn bench_index_1000_files(b: &mut Bencher) {
    // Create temp directory with 1000 files
    b.iter(|| FileIndex::build(&temp_dir));
}

#[bench]
fn bench_search_1000_files(b: &mut Bencher) {
    let index = FileIndex::build(&test_dir).unwrap();
    b.iter(|| index.search("config", 20));
}
```

---

## References

- **Workspace:** `src/model/workspace.rs` - Existing workspace and file tree
- **Modal system:** `src/model/ui.rs` - Modal state management
- **File watching:** `src/fs_watcher.rs` - File system event handling
- **VS Code:** Quick Open (Cmd+P) behavior and ranking
- **fzf:** Fuzzy matching algorithm inspiration
- **Recent files:** F-040 feature for integration
