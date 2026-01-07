# Enhanced File Finder

Enhance the existing File Finder (Cmd+Shift+O) with persistent file indexing and improved fuzzy matching for sub-100ms search in large workspaces.

> **Status:** Future Enhancement
> **Priority:** P2
> **Effort:** M (Medium, 8-12 hours)
> **Created:** 2025-01-08
> **Milestone:** Performance & Scale

---

## Table of Contents

1. [Overview](#overview)
2. [Current Implementation](#current-implementation)
3. [Proposed Enhancements](#proposed-enhancements)
4. [Architecture](#architecture)
5. [Data Structures](#data-structures)
6. [Implementation Plan](#implementation-plan)
7. [Testing Strategy](#testing-strategy)
8. [Performance Goals](#performance-goals)
9. [References](#references)

---

## Overview

### Current State

The File Finder (`Cmd+Shift+O`) is **already implemented** with:

✅ **Implemented:**
- Modal UI with search input and result list
- Fuzzy matching using `nucleo` matcher library
- File icon display with path truncation
- Keyboard navigation (Up/Down, Enter, Escape)
- Opens selected file in new tab
- Reads files from workspace file tree on modal open
- Limits results to 50 items

**Current Implementation:**
- **Files:** `src/model/ui.rs` (FileFinderState, FileMatch)
- **Matching:** `src/update/ui.rs` (fuzzy_match_files function)
- **Rendering:** `src/view/mod.rs` (render_modals - FileFinder case)
- **Keybindings:** Arrow keys navigate, Enter opens, Escape closes
- **Messages:** `UiMsg::OpenFuzzyFileFinder`, `ModalMsg::Confirm`

**Performance Characteristics:**
- Re-scans file tree on every modal open
- Searches all files on every keystroke
- Works well for small-to-medium workspaces (<5,000 files)
- Can be slow for large monorepos (>10,000 files)

### Limitations

The current implementation has performance and scalability issues:

1. **Full scan on open** - Reads all files from file tree every time modal opens
2. **No persistence** - Can't cache results between invocations
3. **Limited to 50 results** - Hard-coded limit regardless of query quality
4. **Filename-only matching** - Doesn't search in file paths
5. **No recent files priority** - No special handling for recently opened files
6. **No file system watching** - Can't track changes after modal opens

### Goals

1. **Persistent index** - Build file index once per workspace, persist in memory
2. **Sub-100ms search** - Instant results even for 10,000+ file workspaces
3. **Path matching** - Search in file paths, not just names (e.g., "src/model" matches)
4. **Better ranking** - Prioritize exact matches, prefixes, and recent files
5. **Incremental updates** - Update index when files are added/removed
6. **Larger result sets** - Show more results when query is specific

### Non-Goals

- Full-text search within files (separate feature: find in files)
- Go to symbol/definition (LSP feature)
- Remote file systems or network shares
- Changing the existing UI or UX (enhancement is backend-only)

---

## Current Implementation

### File Finder Flow (v0.3.13)

```
User presses Cmd+Shift+O
        ↓
UiMsg::OpenFuzzyFileFinder
        ↓
Get files from workspace.file_tree.get_all_file_paths()
        ↓
Create FileFinderState with Vec<PathBuf>
        ↓
User types → fuzzy_match_files() on every keystroke
        ↓
nucleo::Matcher scores each file against query
        ↓
Sort by score, display top 50 results
```

### Performance Bottlenecks

1. **Modal open:** O(n) file tree traversal
2. **Every keystroke:** O(n) fuzzy matching across all files
3. **No caching:** Metadata (size, modified date) recomputed every time
4. **Filename-only:** Can't search by path components

### Code Locations

| Component | File | Lines |
|-----------|------|-------|
| State | `src/model/ui.rs` | 278-312 |
| Fuzzy matching | `src/update/ui.rs` | 1086-1130 |
| Modal opening | `src/update/ui.rs` | 127-152 |
| Rendering | `src/view/mod.rs` | 1873-1996 |
| Key handling | `src/runtime/input.rs` | 194-245 |

---

## Proposed Enhancements

### Enhancement 1: Persistent File Index

**Problem:** Every time the modal opens, we iterate through the entire file tree.

**Solution:** Build a persistent `FileIndex` when workspace opens, store in `AppModel`.

**Benefits:**
- Build once, search many times
- Enables advanced matching strategies
- Can add metadata (size, modified time, etc.)

### Enhancement 2: Path Matching

**Problem:** Can only search filenames, not paths (e.g., can't find "src/model/editor.rs" by typing "src/model").

**Solution:** Match against full relative path, bonus for path component boundaries.

```rust
fn match_file(&self, query: &str, file: &IndexedFile) -> Option<SearchResult> {
    // Try exact match first
    if file.filename_lower == query { return Some(exact_match()); }
    
    // Try prefix match
    if file.filename_lower.starts_with(query) { return Some(prefix_match()); }
    
    // Try path matching (e.g., "src/ed" matches "src/model/editor.rs")
    if file.path_lower.contains(query) { return Some(path_match()); }
    
    // Try fuzzy match with trigrams
    fuzzy_match_with_trigrams(query, file)
}
```

### Enhancement 3: Improved Ranking

**Problem:** All results sorted by generic fuzzy score, no special handling for common patterns.

**Solution:** Multi-tier scoring system:

```rust
enum MatchQuality {
    ExactMatch,        // 10000 points - "editor.rs" == "editor.rs"
    PrefixMatch,       // 5000 points - "editor.rs" starts with "ed"
    PathPrefixMatch,   // 4000 points - "src/editor.rs" path matches "src/ed"
    SubstringMatch,    // 3000 points - "my_editor.rs" contains "editor"
    FuzzyMatch,        // 0-1000 points - fuzzy algorithm score
}

fn compute_score(match_quality: MatchQuality, file: &IndexedFile, recent: bool) -> u32 {
    let mut score = match_quality.base_score();
    
    // Bonus for recent files
    if recent { score += 2000; }
    
    // Bonus for shorter filenames (more specific)
    score += (200 - file.filename.len() as u32).max(0);
    
    // Bonus for common file types
    if matches!(file.extension.as_deref(), Some("rs" | "md" | "toml")) {
        score += 50;
    }
    
    score
}
```

### Enhancement 4: Recent Files Priority

**Problem:** No special handling for recently opened files.

**Solution:** Track recent files in `AppModel`, boost their score.

```rust
// Add to AppModel
pub struct AppModel {
    // ... existing fields ...
    
    /// Recently opened files (most recent first)
    pub recent_files: Vec<PathBuf>,  // Limit to 20
}

impl AppModel {
    pub fn mark_file_opened(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(20);
    }
}
```

### Enhancement 5: Incremental Index Updates

**Problem:** Index becomes stale when files are added/removed.

**Solution:** Add incremental update methods to `FileIndex`.

```rust
impl FileIndex {
    pub fn add_file(&mut self, path: PathBuf) -> Result<()> {
        let file = IndexedFile::new(path)?;
        let idx = self.files.len();
        
        // Update trigram index
        for trigram in &file.trigrams {
            self.trigram_index.entry(*trigram).or_default().push(idx);
        }
        
        // Update name index
        self.by_name.entry(file.filename_lower.clone()).or_default().push(idx);
        
        self.files.push(file);
        Ok(())
    }
    
    pub fn remove_file(&mut self, path: &Path) {
        // Remove from indices and rebuild affected entries
    }
}
```

**Integration with workspace file tree:**
- When `Workspace::add_file_to_tree()` is called, also call `file_index.add_file()`
- When files are deleted, call `file_index.remove_file()`

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                    File Finder Enhancement Architecture                      │
├─────────────────────────────────────────────────────────────────────────────┤
│                                                                              │
│  ┌────────────────────────────────────────────────────────────────────────┐ │
│  │                          File Index (NEW)                               │ │
│  │  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────────────┐ │ │
│  │  │  Workspace  │───▶│   Build     │───▶│  FileIndex                  │ │ │
│  │  │  Opened     │    │   Index     │    │  - files: Vec<IndexedFile>  │ │ │
│  │  └─────────────┘    └─────────────┘    │  - by_name: BTreeMap        │ │ │
│  │         ▲                              │  - by_ext: HashMap          │ │ │
│  │         │                              │  - mru_cache: LruCache      │ │ │
│  │         │ FS Events                    └─────────────────────────────┘ │ │
│  │         │ (incremental)                              │                   │ │
│  └─────────┼───────────────────────────────────────────┼───────────────────┘ │
│            │                                           │                     │
│            │                                           ▼                     │
│  ┌─────────┴─────────────────────────────────────────────────────────────┐  │
│  │                    Existing File Finder Modal                          │  │
│  │                                                                        │  │
│  │  Query: [ project.rs              ]                                   │  │
│  │  ├─────────────────────────────────────────────────────────────────┤  │  │
│  │  │  ★ src/project.rs              (recently opened, +5000 score)   │  │  │
│  │  │    tests/test_project.rs       (fuzzy match, +1200 score)       │  │  │
│  │  │    benches/project_bench.rs    (fuzzy match, +900 score)        │  │  │
│  │  │    docs/PROJECT.md             (fuzzy match, +600 score)        │  │  │
│  └────────────────────────────────────────────────────────────────────────┘  │
│                                                                              │
└──────────────────────────────────────────────────────────────────────────────┘
```

### Key Improvements

1. **Persistent Index:** Build once, keep in memory
2. **Incremental Updates:** Add/remove files on FS events
3. **Pre-computed Metadata:** Trigrams, size, modified time
4. **Path Search:** Match against full path, not just filename
5. **MRU Boost:** Recently opened files get score multiplier
6. **Parallel Search:** Use rayon for multi-threaded search (optional)

---

## Data Structures

### New: FileIndex (src/file_index.rs)

```rust
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Pre-indexed file for fast searching
#[derive(Debug, Clone)]
pub struct IndexedFile {
    /// Path relative to workspace root
    pub path: PathBuf,
    /// Filename only (e.g., "editor.rs")
    pub filename: String,
    /// Lowercase filename for case-insensitive matching
    pub filename_lower: String,
    /// Full path as lowercase string (e.g., "src/model/editor.rs")
    pub path_lower: String,
    /// File extension without dot
    pub extension: Option<String>,
    /// File size in bytes
    pub size: u64,
    /// Character trigrams for fuzzy matching
    pub trigrams: Vec<[char; 3]>,
}

/// File index for fast searching
#[derive(Debug)]
pub struct FileIndex {
    /// All indexed files
    pub files: Vec<IndexedFile>,
    /// Trigram -> file indices
    pub trigram_index: HashMap<[char; 3], Vec<usize>>,
    /// Filename (lowercase) -> file indices
    pub by_name: BTreeMap<String, Vec<usize>>,
    /// Workspace root
    pub root: PathBuf,
    /// When index was built
    pub built_at: Instant,
    /// Number of files
    pub file_count: usize,
    /// Build time in milliseconds
    pub build_time_ms: u64,
}

impl FileIndex {
    /// Build index from workspace root
    pub fn build(root: &Path) -> std::io::Result<Self> {
        // Scan directory recursively
        // Build trigram and name indices
        // Return FileIndex
    }
    
    /// Search for files matching query
    pub fn search(&self, query: &str, recent_files: &[PathBuf], limit: usize) -> Vec<SearchResult> {
        // Try exact, prefix, path, and fuzzy matches
        // Boost scores for recent files
        // Sort by score descending
        // Return top N results
    }
    
    /// Add file to index (incremental update)
    pub fn add_file(&mut self, path: PathBuf, size: u64) {
        // Add to files vector
        // Update trigram index
        // Update name index
    }
    
    /// Remove file from index (incremental update)
    pub fn remove_file(&mut self, path: &Path) {
        // Remove from files vector
        // Update indices
    }
}

#[derive(Debug, Clone)]
pub struct SearchResult {
    pub index: usize,
    pub file: IndexedFile,
    pub score: u32,
    pub matched_ranges: Vec<(usize, usize)>,
}
```

### Updates to AppModel

```rust
// src/model/mod.rs

pub struct AppModel {
    // ... existing fields ...
    
    /// File index for enhanced quick file finder (None if no workspace)
    pub file_index: Option<FileIndex>,
    
    /// Recently opened files (most recent first, limit 20)
    pub recent_files: Vec<PathBuf>,
}

impl AppModel {
    pub fn rebuild_file_index(&mut self) {
        if let Some(workspace) = &self.workspace {
            match FileIndex::build(&workspace.root) {
                Ok(index) => {
                    tracing::info!(
                        "Built file index: {} files in {}ms",
                        index.file_count,
                        index.build_time_ms
                    );
                    self.file_index = Some(index);
                }
                Err(e) => {
                    tracing::error!("Failed to build file index: {}", e);
                }
            }
        }
    }
    
    pub fn mark_file_opened(&mut self, path: PathBuf) {
        // Remove if already present
        self.recent_files.retain(|p| p != &path);
        // Add at front
        self.recent_files.insert(0, path);
        // Keep only 20 most recent
        self.recent_files.truncate(20);
    }
}
```

### Update FileFinderState

```rust
// Update src/model/ui.rs

pub struct FileFinderState {
    /// Editable state for the search input field
    pub editable: EditableState<StringBuffer>,
    /// Index of selected file in filtered results
    pub selected_index: usize,
    /// Filtered and ranked file results
    pub results: Vec<FileMatch>,
    /// All files in workspace (cached when modal opens) - DEPRECATED
    pub all_files: Vec<PathBuf>,
    /// Workspace root path (for computing relative paths)
    pub workspace_root: PathBuf,
    /// Whether to use enhanced search (when FileIndex is available)
    pub use_enhanced: bool,
}
```

---

## Implementation Plan

### Phase 1: File Index Module (2-3 hours)

**Create:** `src/file_index.rs`

- [ ] Create `IndexedFile` struct with metadata
- [ ] Implement `FileIndex::build()` with recursive scan
- [ ] Add ignore patterns (reuse from workspace)
- [ ] Implement trigram computation
- [ ] Implement exact, prefix, and substring matching
- [ ] Add unit tests for index building

**Test:** Index 10,000 files in < 500ms

### Phase 2: AppModel Integration (1-2 hours)

**Modify:** `src/model/mod.rs`

- [ ] Add `file_index: Option<FileIndex>` to AppModel
- [ ] Add `recent_files: Vec<PathBuf>` to AppModel
- [ ] Implement `rebuild_file_index()` method
- [ ] Call `rebuild_file_index()` when workspace opens
- [ ] Implement `mark_file_opened()` method
- [ ] Call `mark_file_opened()` when file is opened

**Test:** File index is built when workspace opens

### Phase 3: Enhanced Search (2-3 hours)

**Modify:** `src/update/ui.rs`

- [ ] Update `fuzzy_match_files()` to use `FileIndex::search()` if available
- [ ] Fall back to current implementation if no index
- [ ] Pass recent files to search function
- [ ] Increase result limit from 50 to 100
- [ ] Add path matching support

**Test:** Search returns results in < 10ms for 10k files

### Phase 4: Incremental Updates (2 hours)

**Modify:** `src/model/workspace.rs`, `src/update/workspace.rs`

- [ ] When file is added to workspace, call `file_index.add_file()`
- [ ] When file is removed, call `file_index.remove_file()`
- [ ] Handle file renames (remove old + add new)
- [ ] Add integration tests

**Test:** Creating a file makes it searchable immediately

### Phase 5: UI Enhancements (1-2 hours)

**Modify:** `src/view/mod.rs`

- [ ] Show file path highlights for path matches
- [ ] Add indicator for recent files (★ icon)
- [ ] Show "X results" footer
- [ ] Show build time in status when index is built

**Test:** Visual inspection of enhanced UI

### Phase 6: Performance Testing (1 hour)

- [ ] Add benchmark for index building (various sizes)
- [ ] Add benchmark for search (various query types)
- [ ] Test with 10,000+ file workspace
- [ ] Profile memory usage

**Target:** < 100ms index build for 10k files, < 10ms search

---

## Testing Strategy

### Unit Tests

```rust
// src/file_index.rs

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_trigram_computation() {
        let trigrams = compute_trigrams("test");
        assert_eq!(trigrams.len(), 2); // "tes", "est"
    }

    #[test]
    fn test_exact_match() {
        let index = create_test_index();
        let results = index.search("main.rs", &[], 10);
        assert_eq!(results[0].file.filename, "main.rs");
        assert!(results[0].score > 9000); // Near-exact score
    }
    
    #[test]
    fn test_prefix_match() {
        let index = create_test_index();
        let results = index.search("mod", &[], 10);
        assert!(results.iter().any(|r| r.file.filename == "mod.rs"));
    }
    
    #[test]
    fn test_path_match() {
        let index = create_test_index();
        let results = index.search("src/model", &[], 10);
        assert!(results.iter().any(|r| r.file.path.starts_with("src/model")));
    }
    
    #[test]
    fn test_recent_priority() {
        let index = create_test_index();
        let recent = vec![PathBuf::from("src/lib.rs")];
        let results = index.search("lib", &recent, 10);
        assert_eq!(results[0].file.filename, "lib.rs");
        assert!(results[0].score > 7000); // Boosted score
    }
    
    #[test]
    fn test_incremental_add() {
        let mut index = create_test_index();
        let initial_count = index.file_count;
        index.add_file(PathBuf::from("new_file.rs"), 1024);
        assert_eq!(index.file_count, initial_count + 1);
        
        let results = index.search("new_file", &[], 10);
        assert_eq!(results[0].file.filename, "new_file.rs");
    }
    
    #[test]
    fn test_incremental_remove() {
        let mut index = create_test_index();
        let path = PathBuf::from("lib.rs");
        index.remove_file(&path);
        
        let results = index.search("lib.rs", &[], 10);
        assert!(results.is_empty() || results[0].file.filename != "lib.rs");
    }
}
```

### Integration Tests

```rust
// tests/file_finder_tests.rs

#[test]
fn test_file_finder_uses_index() {
    let mut model = create_model_with_workspace();
    model.rebuild_file_index();
    assert!(model.file_index.is_some());

    // Open file finder
    let cmd = update(&mut model, Msg::Ui(UiMsg::OpenFuzzyFileFinder));
    assert!(model.ui.has_modal());
    
    // Should use index, not scan file tree
    // (verify by checking FileFinderState.use_enhanced flag)
}

#[test]
fn test_search_performance() {
    let mut model = create_model_with_large_workspace(10_000);
    model.rebuild_file_index();
    
    let start = Instant::now();
    if let Some(ref index) = model.file_index {
        let results = index.search("test", &[], 50);
        let elapsed = start.elapsed();
        assert!(elapsed.as_millis() < 100, "Search took {}ms", elapsed.as_millis());
        assert!(!results.is_empty());
    }
}
```

### Performance Benchmarks

```rust
// benches/file_index.rs

use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_index_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_build");
    
    for size in [1_000, 10_000, 100_000].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            let temp_dir = create_temp_workspace(size);
            b.iter(|| {
                FileIndex::build(&temp_dir).unwrap()
            });
        });
    }
    
    group.finish();
}

fn bench_search(c: &mut Criterion) {
    let temp_dir = create_temp_workspace(10_000);
    let index = FileIndex::build(&temp_dir).unwrap();
    
    let mut group = c.benchmark_group("search");
    
    for query in ["test", "mod.rs", "xyz123"].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(query), query, |b, &query| {
            b.iter(|| {
                index.search(query, &[], 100)
            });
        });
    }
    
    group.finish();
}

criterion_group!(benches, bench_index_build, bench_search);
criterion_main!(benches);
```

---

## Performance Goals

| Metric | Current | Target | Notes |
|--------|---------|--------|-------|
| Index Build (1k files) | N/A | < 50ms | One-time cost on workspace open |
| Index Build (10k files) | N/A | < 500ms | Large workspace |
| Search (exact match) | ~20ms | < 5ms | With index lookup |
| Search (fuzzy match) | ~50ms | < 20ms | With trigram filtering |
| Memory (1k files) | N/A | < 1MB | ~1KB per file |
| Memory (10k files) | N/A | < 10MB | Acceptable overhead |

---

## Migration Path

The enhancement is **backward compatible** - the existing implementation continues to work:

1. If `AppModel.file_index` is `None`, use current `fuzzy_match_files()` implementation
2. If `AppModel.file_index` is `Some`, use new `FileIndex::search()` method
3. Gradual rollout: enable index for workspaces with > 500 files first

This allows testing the new implementation without breaking existing functionality.

---

## Future Enhancements

Beyond this enhancement:

- **File system watching:** Use `notify` crate to detect file changes automatically
- **Symbol search:** Add "Go to Symbol" with LSP integration (separate feature)
- **Workspace-wide search:** Full-text search across all files
- **Index persistence:** Save index to disk for instant startup
- **Ignore file support:** Respect `.gitignore` patterns

---

## References

- **Current Implementation:** `src/update/ui.rs:1086-1130` - `fuzzy_match_files()`
- **Nucleo Matcher:** Using `nucleo::Matcher` from `nucleo` crate
- **File Tree:** `src/model/workspace.rs` - `FileTree` and file traversal
- **Modal System:** `src/model/ui.rs` - `FileFinderState` and modal rendering
- **Similar Projects:** 
  - VS Code's Quick Open uses a persistent index
  - Sublime Text's fuzzy finder pre-computes file metadata
  - IntelliJ's "Search Everywhere" uses trigram indexing