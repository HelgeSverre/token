//! Workspace management - file tree sidebar and workspace tracking
//!
//! Provides VS Code-style workspace functionality:
//! - File tree sidebar with expand/collapse
//! - Workspace root directory tracking
//! - File type classification and icons

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::util::{visible_tree_count, visible_tree_row_at_index, visible_tree_row_matching};

use super::ScaledMetrics;

// ============================================================================
// File Extension Classification
// ============================================================================

/// File type based on extension for icon and syntax highlighting purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileExtension {
    Rust,
    JavaScript,
    TypeScript,
    Python,
    Go,
    C,
    Cpp,
    Header,
    Json,
    Yaml,
    Toml,
    Markdown,
    Html,
    Css,
    Scss,
    Sql,
    Shell,
    Git,
    Lock,
    Config,
    Text,
    Binary,
    Unknown,
}

impl FileExtension {
    /// Classify a file based on its extension
    pub fn from_path(path: &Path) -> Self {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase());

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_lowercase());

        // Check special filenames first
        if let Some(name) = &filename {
            match name.as_str() {
                ".gitignore" | ".gitattributes" | ".gitmodules" => return Self::Git,
                "cargo.lock" | "package-lock.json" | "yarn.lock" | "pnpm-lock.yaml" => {
                    return Self::Lock
                }
                "makefile" | "dockerfile" | ".dockerignore" => return Self::Config,
                _ => {}
            }
        }

        match extension.as_deref() {
            Some("rs") => Self::Rust,
            Some("js" | "mjs" | "cjs") => Self::JavaScript,
            Some("ts" | "tsx" | "mts" | "cts") => Self::TypeScript,
            Some("py" | "pyw" | "pyi") => Self::Python,
            Some("go") => Self::Go,
            Some("c") => Self::C,
            Some("cpp" | "cc" | "cxx") => Self::Cpp,
            Some("h" | "hpp" | "hxx") => Self::Header,
            Some("json" | "jsonc") => Self::Json,
            Some("yaml" | "yml") => Self::Yaml,
            Some("toml") => Self::Toml,
            Some("md" | "markdown") => Self::Markdown,
            Some("html" | "htm") => Self::Html,
            Some("css") => Self::Css,
            Some("scss" | "sass") => Self::Scss,
            Some("sql") => Self::Sql,
            Some("sh" | "bash" | "zsh" | "fish") => Self::Shell,
            Some("txt" | "text") => Self::Text,
            Some("exe" | "dll" | "so" | "dylib" | "o" | "a") => Self::Binary,
            _ => Self::Unknown,
        }
    }

    /// Get a Nerd Font icon for the file type
    ///
    /// Icons use codepoints from Nerd Fonts v3 (Devicons, Seti-UI, Material Design,
    /// Font Awesome, Codicons). Requires a Nerd Font patched font (e.g. JetBrainsMono NF).
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Rust => "\u{F1617}",      // 󱘗 nf-md-language_rust
            Self::JavaScript => "\u{E74E}", //  nf-dev-javascript
            Self::TypeScript => "\u{E628}", //  nf-seti-typescript
            Self::Python => "\u{E73C}",     //  nf-dev-python
            Self::Go => "\u{E627}",         //  nf-seti-go2
            Self::C => "\u{E61E}",          //  nf-custom-c
            Self::Cpp => "\u{E61D}",        //  nf-custom-cpp
            Self::Header => "\u{E61E}",     //  nf-custom-c (header)
            Self::Json => "\u{E60B}",       //  nf-seti-json
            Self::Yaml => "\u{E8EB}",       //  nf-dev-yaml
            Self::Toml => "\u{E6B2}",       //  nf-seti-config
            Self::Markdown => "\u{E609}",   //  nf-seti-markdown
            Self::Html => "\u{E736}",       //  nf-dev-html5
            Self::Css => "\u{E749}",        //  nf-dev-css3
            Self::Scss => "\u{E603}",       //  nf-seti-sass
            Self::Sql => "\u{E706}",        //  nf-dev-database
            Self::Shell => "\u{F489}",      //  nf-oct-terminal
            Self::Git => "\u{E702}",        //  nf-dev-git
            Self::Lock => "\u{F023}",       //  nf-fa-lock
            Self::Config => "\u{E615}",     //  nf-seti-config
            Self::Text => "\u{F0219}",      // 󰈙 nf-md-file_document
            Self::Binary => "\u{EAE8}",     //  nf-cod-file_binary
            Self::Unknown => "\u{EA7B}",    //  nf-cod-file
        }
    }
}

// ============================================================================
// File Tree Nodes
// ============================================================================

/// A node in the file tree (either a file or directory)
#[derive(Debug, Clone)]
pub struct FileNode {
    /// File or directory name (not full path)
    pub name: String,
    /// Full path to the file/directory
    pub path: PathBuf,
    /// Whether this is a directory
    pub is_dir: bool,
    /// Children (only populated for directories)
    pub children: Vec<FileNode>,
    /// Cached file extension classification
    pub extension: FileExtension,
}

impl FileNode {
    /// Create a new file node
    pub fn new_file(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let extension = FileExtension::from_path(&path);

        Self {
            name,
            path,
            is_dir: false,
            children: Vec::new(),
            extension,
        }
    }

    /// Create a new directory node
    pub fn new_dir(path: PathBuf) -> Self {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        Self {
            name,
            path,
            is_dir: true,
            children: Vec::new(),
            extension: FileExtension::Unknown,
        }
    }

    /// Get icon for this node
    pub fn icon(&self) -> &'static str {
        if self.is_dir {
            "\u{F07B}" //  nf-fa-folder
        } else {
            self.extension.icon()
        }
    }

    /// Get expanded folder icon
    pub fn icon_expanded(&self) -> &'static str {
        if self.is_dir {
            "\u{F07C}" //  nf-fa-folder_open
        } else {
            self.extension.icon()
        }
    }
}

// ============================================================================
// File Tree
// ============================================================================

/// Patterns to ignore when scanning directories
const IGNORE_PATTERNS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "__pycache__",
    ".DS_Store",
    "Thumbs.db",
    ".idea",
    ".vscode",
    "*.pyc",
    "*.pyo",
    ".env",
];

/// How the file tree filters directory contents while scanning.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanOptions {
    /// Hide entries matched by `.gitignore` files (and `.git/info/exclude`).
    ///
    /// Off by default: gitignored files such as build output are shown
    /// unless the user asks to hide them. The built-in `IGNORE_PATTERNS`
    /// apply regardless of this flag.
    pub hide_gitignored: bool,
}

/// The complete file tree for a workspace
#[derive(Debug, Clone, Default)]
pub struct FileTree {
    /// Root nodes (files/directories at workspace root)
    pub roots: Vec<FileNode>,
}

impl FileTree {
    /// Create a file tree by scanning a directory
    ///
    /// The workspace root folder itself becomes the first (and only) root node,
    /// with its contents as children. This matches VS Code behavior where the
    /// project folder name is visible at the top of the file tree.
    pub fn from_directory(root: &Path, options: ScanOptions) -> std::io::Result<Self> {
        if !root.is_dir() {
            return Ok(Self { roots: Vec::new() });
        }

        // Create the root folder node with the workspace directory itself
        let mut root_node = FileNode::new_dir(root.to_path_buf());
        root_node.children = Self::scan_directory(root, options)?;

        Ok(Self {
            roots: vec![root_node],
        })
    }

    /// Scan a directory recursively (up to `MAX_DEPTH` levels below `dir`).
    ///
    /// Returns `dir`'s children, each directory populated with its own sorted
    /// subtree. Entries that fail to read (permissions, broken symlink loops)
    /// are skipped; only `dir` itself failing is an error.
    ///
    /// Scanning goes through the `ignore` walker so that, when
    /// `options.hide_gitignored` is set, nested `.gitignore` files and
    /// ancestors above `dir` are honoured the same way git does.
    fn scan_directory(dir: &Path, options: ScanOptions) -> std::io::Result<Vec<FileNode>> {
        const MAX_DEPTH: usize = 20;

        let mut walker = Self::walker(dir, options);
        walker.max_depth(Some(MAX_DEPTH + 1));

        // Pre-order walk: a directory arrives before its contents, so the
        // stack holds the open directories from `dir` (depth 0) down to the
        // parent of the current entry.
        let mut stack: Vec<FileNode> = vec![FileNode::new_dir(dir.to_path_buf())];
        for result in walker.build() {
            let entry = match result {
                Ok(entry) => entry,
                Err(err) if err.depth().is_some_and(|depth| depth > 0) => {
                    tracing::debug!("Skipping unreadable file tree entry: {err}");
                    continue;
                }
                Err(err) => {
                    return Err(err
                        .into_io_error()
                        .unwrap_or_else(|| std::io::Error::other(dir.display().to_string())));
                }
            };
            if entry.depth() == 0 {
                continue;
            }
            Self::close_directories(&mut stack, entry.depth());

            let is_dir = entry.file_type().is_some_and(|kind| kind.is_dir());
            if is_dir {
                stack.push(FileNode::new_dir(entry.into_path()));
            } else if let Some(parent) = stack.last_mut() {
                parent.children.push(FileNode::new_file(entry.into_path()));
            }
        }
        Self::close_directories(&mut stack, 1);

        let mut root = stack
            .pop()
            .unwrap_or_else(|| FileNode::new_dir(dir.to_path_buf()));
        Self::sort_nodes(&mut root.children);
        Ok(root.children)
    }

    /// The directory walker every scan shares: built-in patterns and hidden
    /// files are decided by `should_ignore`, and `.gitignore` rules only
    /// apply when `options` ask for them.
    fn walker(dir: &Path, options: ScanOptions) -> ignore::WalkBuilder {
        let mut walker = ignore::WalkBuilder::new(dir);
        walker
            .standard_filters(false)
            .follow_links(true)
            .filter_entry(|entry| entry.depth() == 0 || !Self::should_ignore(entry.path()));
        if options.hide_gitignored {
            walker
                .git_ignore(true)
                .git_exclude(true)
                .parents(true)
                // Honour a `.gitignore` even when the workspace is not a git
                // checkout; the toggle is about the files, not the repository.
                .require_git(false);
        }
        walker
    }

    /// Pop finished directories off the walk stack until it holds `depth`
    /// entries, sorting each one and attaching it to its parent.
    fn close_directories(stack: &mut Vec<FileNode>, depth: usize) {
        while stack.len() > depth.max(1) {
            let Some(mut node) = stack.pop() else {
                break;
            };
            Self::sort_nodes(&mut node.children);
            if let Some(parent) = stack.last_mut() {
                parent.children.push(node);
            }
        }
    }

    /// Check if a path should be ignored
    fn should_ignore(path: &Path) -> bool {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check exact matches
        for pattern in IGNORE_PATTERNS {
            if let Some(ext) = pattern.strip_prefix("*.") {
                // Extension pattern
                if path.extension().and_then(|e| e.to_str()) == Some(ext) {
                    return true;
                }
            } else if name == *pattern {
                return true;
            }
        }

        // Hidden files (except .gitignore and similar)
        if name.starts_with('.')
            && !matches!(name, ".gitignore" | ".gitattributes" | ".editorconfig")
        {
            return true;
        }

        false
    }

    /// Sort nodes: directories first, then alphabetically (case-insensitive)
    fn sort_nodes(nodes: &mut [FileNode]) {
        nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        });
    }

    /// Refresh the file tree from disk (full rescan)
    pub fn refresh(&mut self, root: &Path, options: ScanOptions) -> std::io::Result<()> {
        *self = Self::from_directory(root, options)?;
        Ok(())
    }

    /// Incrementally update the tree for changed paths.
    ///
    /// Instead of rescanning the entire tree, this method only refreshes
    /// the parent directories of the changed paths. This is much faster
    /// for typical file system changes (creating/deleting/modifying files).
    pub fn update_paths(
        &mut self,
        root: &Path,
        paths: &[PathBuf],
        options: ScanOptions,
    ) -> std::io::Result<()> {
        // Collect unique parent directories that need updating
        let mut parents_to_refresh: HashSet<PathBuf> = HashSet::new();

        for path in paths {
            // Get the parent directory of the changed path
            if let Some(parent) = path.parent() {
                // Only include if within the workspace root
                if parent.starts_with(root) {
                    parents_to_refresh.insert(parent.to_path_buf());
                }
            }
        }

        // If no valid parents or too many changes, fall back to full refresh
        if parents_to_refresh.is_empty() || parents_to_refresh.len() > 10 {
            return self.refresh(root, options);
        }

        // Refresh each parent directory incrementally
        for parent in &parents_to_refresh {
            self.refresh_directory(root, parent, options)?;
        }

        Ok(())
    }

    /// Refresh a single directory's children in the tree.
    fn refresh_directory(
        &mut self,
        root: &Path,
        dir_path: &Path,
        options: ScanOptions,
    ) -> std::io::Result<()> {
        // Special case: refreshing the root itself
        if dir_path == root {
            return self.refresh(root, options);
        }

        // Find the node for this directory and update its children
        for tree_root in &mut self.roots {
            if Self::refresh_node_directory(tree_root, dir_path, options)? {
                return Ok(());
            }
        }

        // A directory the scan filter excludes (gitignored, `node_modules`,
        // ...) is never in the tree; changes inside it must not trigger a
        // full rescan, which with hiding on would fire for every log write.
        if Self::is_excluded(root, dir_path, options) {
            return Ok(());
        }

        // Directory not found in tree - might be new, do full refresh
        self.refresh(root, options)
    }

    /// Whether `dir` (inside `root`) is dropped by the scan filter.
    ///
    /// Walks only the chain of directories from `root` down to `dir`, so the
    /// cost is one directory listing per level rather than a tree scan.
    fn is_excluded(root: &Path, dir: &Path, options: ScanOptions) -> bool {
        let Ok(relative) = dir.strip_prefix(root) else {
            return false;
        };
        let depth = relative.components().count();
        if depth == 0 {
            return false;
        }
        let target = dir.to_path_buf();
        let mut walker = Self::walker(root, options);
        walker
            .max_depth(Some(depth))
            .filter_entry(move |entry| target.starts_with(entry.path()));
        !walker
            .build()
            .filter_map(Result::ok)
            .any(|entry| entry.path() == dir)
    }

    /// Recursively find and refresh a directory node.
    /// Returns true if the directory was found and refreshed.
    fn refresh_node_directory(
        node: &mut FileNode,
        target_dir: &Path,
        options: ScanOptions,
    ) -> std::io::Result<bool> {
        if node.path == target_dir && node.is_dir {
            // Found it! Refresh this node's children
            node.children = Self::scan_directory(&node.path, options)?;
            return Ok(true);
        }

        // Recurse into children
        if node.is_dir && target_dir.starts_with(&node.path) {
            for child in &mut node.children {
                if Self::refresh_node_directory(child, target_dir, options)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Get all file paths recursively (excludes directories)
    ///
    /// Returns a flat list of all files in the tree, useful for fuzzy file search.
    pub fn get_all_file_paths(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        for root in &self.roots {
            Self::collect_files_recursive(root, &mut files);
        }
        files
    }

    fn collect_files_recursive(node: &FileNode, files: &mut Vec<PathBuf>) {
        if node.is_dir {
            for child in &node.children {
                Self::collect_files_recursive(child, files);
            }
        } else {
            files.push(node.path.clone());
        }
    }

    /// Count total visible items (for scrolling calculations)
    pub fn count_visible(&self, expanded: &HashSet<PathBuf>) -> usize {
        visible_tree_count(&self.roots, |node: &FileNode| {
            node.is_dir && expanded.contains(&node.path)
        })
    }

    /// Get the nth visible item (for hit testing)
    pub fn get_visible_item(&self, index: usize, expanded: &HashSet<PathBuf>) -> Option<&FileNode> {
        visible_tree_row_at_index(&self.roots, index, |node: &FileNode| {
            node.is_dir && expanded.contains(&node.path)
        })
        .map(|row| row.node)
    }

    /// Get the nth visible item with its depth (for hit testing with chevron area detection)
    pub fn get_visible_item_with_depth(
        &self,
        index: usize,
        expanded: &HashSet<PathBuf>,
    ) -> Option<(&FileNode, usize)> {
        visible_tree_row_at_index(&self.roots, index, |node: &FileNode| {
            node.is_dir && expanded.contains(&node.path)
        })
        .map(|row| (row.node, row.depth))
    }

    /// Get a visible item by its path (for parent navigation)
    ///
    /// Returns the node if the path is visible in the current tree state.
    /// A node is visible if all its ancestor folders are expanded.
    pub fn get_visible_item_by_path(
        &self,
        path: &PathBuf,
        expanded: &HashSet<PathBuf>,
    ) -> Option<&FileNode> {
        visible_tree_row_matching(
            &self.roots,
            |node: &FileNode| node.is_dir && expanded.contains(&node.path),
            |node: &FileNode| &node.path == path,
        )
        .map(|row| row.node)
    }
}

// ============================================================================
// Workspace
// ============================================================================

/// Workspace state - manages file tree and sidebar
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Root directory of the workspace
    pub root: PathBuf,

    /// Expanded folder paths
    pub expanded_folders: HashSet<PathBuf>,

    /// Currently selected item in file tree
    pub selected_item: Option<PathBuf>,

    /// File tree cache
    pub file_tree: FileTree,

    /// Sidebar visibility
    pub sidebar_visible: bool,

    /// Sidebar width in logical pixels (scale-independent)
    pub sidebar_width_logical: f32,

    /// Scroll offset in the file tree (in items)
    pub scroll_offset: usize,

    /// Hide files matched by `.gitignore` from the tree (off by default).
    pub hide_gitignored: bool,
}

/// Last navigation synchronized with the explorer, independent of tree selection.
#[derive(Debug, Clone)]
pub(crate) struct AutoRevealTarget {
    pub editor_id: super::EditorId,
    pub path: PathBuf,
    pub workspace_root: PathBuf,
    pub explorer_visible: bool,
}

impl Workspace {
    /// Create a new workspace from a directory
    pub fn new(root: PathBuf, metrics: &ScaledMetrics) -> std::io::Result<Self> {
        // Canonicalize the root path to get the full absolute path
        // This ensures file_name() works correctly (e.g., "." becomes "/path/to/dir")
        let root = std::fs::canonicalize(&root)?;

        let file_tree = FileTree::from_directory(&root, ScanOptions::default())?;

        // Auto-expand the workspace root folder so its contents are visible
        let mut expanded_folders = HashSet::new();
        expanded_folders.insert(root.clone());

        Ok(Self {
            root,
            expanded_folders,
            selected_item: None,
            file_tree,
            sidebar_visible: true,
            sidebar_width_logical: metrics.sidebar_default_width_logical,
            scroll_offset: 0,
            hide_gitignored: false,
        })
    }

    /// Scan options derived from the workspace's current filter state.
    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            hide_gitignored: self.hide_gitignored,
        }
    }

    /// Flip gitignored-file hiding and rescan the tree under the new filter.
    ///
    /// Expansion and selection state are kept so that showing the files
    /// again restores the previous view.
    pub fn toggle_hide_gitignored(&mut self) -> std::io::Result<()> {
        self.hide_gitignored = !self.hide_gitignored;
        self.refresh()
    }

    /// Get sidebar width in physical pixels
    pub fn sidebar_width(&self, scale_factor: f64) -> f32 {
        self.sidebar_width_logical * scale_factor as f32
    }

    /// Set sidebar width from physical pixels
    pub fn set_sidebar_width(&mut self, physical_width: f32, scale_factor: f64) {
        self.sidebar_width_logical = physical_width / scale_factor as f32;
    }

    /// Toggle folder expansion
    pub fn toggle_folder(&mut self, path: &Path) {
        if self.expanded_folders.contains(path) {
            self.collapse_folder(path);
        } else {
            self.expanded_folders.insert(path.to_path_buf());
        }
    }

    /// Expand a folder (no-op if already expanded)
    pub fn expand_folder(&mut self, path: &Path) {
        self.expanded_folders.insert(path.to_path_buf());
    }

    /// Collapse a folder and every expanded folder beneath it.
    ///
    /// Collapsing a parent resets its subtree, so re-expanding it later
    /// shows collapsed children instead of restoring stale expansion state.
    /// This makes collapsing the top-level folders collapse everything.
    pub fn collapse_folder(&mut self, path: &Path) {
        self.expanded_folders
            .retain(|expanded| !expanded.starts_with(path));
    }

    /// Check if a folder is expanded
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_folders.contains(path)
    }

    /// Refresh the file tree from disk (full rescan)
    pub fn refresh(&mut self) -> std::io::Result<()> {
        self.file_tree.refresh(&self.root, self.scan_options())
    }

    /// Incrementally update the file tree for specific changed paths.
    /// Much faster than full refresh for typical file operations.
    pub fn update_paths(&mut self, paths: &[PathBuf]) -> std::io::Result<()> {
        self.file_tree
            .update_paths(&self.root, paths, self.scan_options())
    }

    /// Get visible item count (for scrollbar)
    pub fn visible_item_count(&self) -> usize {
        self.file_tree.count_visible(&self.expanded_folders)
    }

    /// Get the depth of a path relative to workspace root
    pub fn depth(&self, path: &Path) -> usize {
        path.strip_prefix(&self.root)
            .map(|rel| rel.components().count())
            .unwrap_or(0)
    }

    /// Reveal a file in the tree (expand parent folders and select)
    pub fn reveal_file(&mut self, path: &Path) {
        // Expand all parent folders, including the root itself — otherwise
        // revealing a file while the root row is collapsed leaves every
        // descendant hidden even though their own expansion state is set.
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.starts_with(&self.root) {
                self.expand_folder(parent);
            }
            current = parent.parent();
        }

        // Select the file
        self.selected_item = Some(path.to_path_buf());
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_extension_classification() {
        assert_eq!(
            FileExtension::from_path(Path::new("main.rs")),
            FileExtension::Rust
        );
        assert_eq!(
            FileExtension::from_path(Path::new("app.tsx")),
            FileExtension::TypeScript
        );
        assert_eq!(
            FileExtension::from_path(Path::new("Cargo.lock")),
            FileExtension::Lock
        );
        assert_eq!(
            FileExtension::from_path(Path::new(".gitignore")),
            FileExtension::Git
        );
    }

    #[test]
    fn test_file_tree_sorting() {
        let mut nodes = vec![
            FileNode::new_file(PathBuf::from("zebra.txt")),
            FileNode::new_dir(PathBuf::from("alpha")),
            FileNode::new_file(PathBuf::from("apple.txt")),
            FileNode::new_dir(PathBuf::from("beta")),
        ];

        FileTree::sort_nodes(&mut nodes);

        // Directories should come first
        assert!(nodes[0].is_dir);
        assert!(nodes[1].is_dir);
        assert!(!nodes[2].is_dir);
        assert!(!nodes[3].is_dir);

        // Within each group, should be alphabetical
        assert_eq!(nodes[0].name, "alpha");
        assert_eq!(nodes[1].name, "beta");
        assert_eq!(nodes[2].name, "apple.txt");
        assert_eq!(nodes[3].name, "zebra.txt");
    }

    #[test]
    fn test_ignore_patterns() {
        assert!(FileTree::should_ignore(Path::new(".git")));
        assert!(FileTree::should_ignore(Path::new("node_modules")));
        assert!(FileTree::should_ignore(Path::new("target")));
        assert!(FileTree::should_ignore(Path::new(".DS_Store")));
        assert!(!FileTree::should_ignore(Path::new(".gitignore")));
        assert!(!FileTree::should_ignore(Path::new("src")));
        assert!(!FileTree::should_ignore(Path::new("main.rs")));
    }

    /// A workspace with a root `.gitignore`, a nested one, and matching files.
    fn gitignored_fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::write(root.join(".gitignore"), "dist/\n*.log\n").unwrap();
        std::fs::create_dir(root.join("dist")).unwrap();
        std::fs::write(root.join("dist/bundle.js"), "").unwrap();
        std::fs::write(root.join("app.log"), "").unwrap();
        std::fs::create_dir(root.join("src")).unwrap();
        std::fs::write(root.join("src/.gitignore"), "generated.rs\n").unwrap();
        std::fs::write(root.join("src/generated.rs"), "").unwrap();
        std::fs::write(root.join("src/main.rs"), "").unwrap();
        dir
    }

    fn names(nodes: &[FileNode]) -> Vec<&str> {
        nodes.iter().map(|node| node.name.as_str()).collect()
    }

    fn child<'a>(nodes: &'a [FileNode], name: &str) -> &'a FileNode {
        nodes.iter().find(|node| node.name == name).unwrap()
    }

    #[test]
    fn gitignored_files_are_shown_by_default() {
        let dir = gitignored_fixture();
        let tree = FileTree::from_directory(dir.path(), ScanOptions::default()).unwrap();
        let children = &tree.roots[0].children;
        assert_eq!(names(children), ["dist", "src", ".gitignore", "app.log"]);
        assert_eq!(names(&child(children, "dist").children), ["bundle.js"]);
        assert_eq!(
            names(&child(children, "src").children),
            [".gitignore", "generated.rs", "main.rs"]
        );
    }

    #[test]
    fn hiding_gitignored_files_honours_root_and_nested_gitignore() {
        let dir = gitignored_fixture();
        let options = ScanOptions {
            hide_gitignored: true,
        };
        let tree = FileTree::from_directory(dir.path(), options).unwrap();
        let children = &tree.roots[0].children;
        assert_eq!(names(children), ["src", ".gitignore"]);
        assert_eq!(
            names(&child(children, "src").children),
            [".gitignore", "main.rs"]
        );
    }

    #[test]
    fn incremental_refresh_keeps_gitignored_files_hidden() {
        let dir = gitignored_fixture();
        let options = ScanOptions {
            hide_gitignored: true,
        };
        let mut tree = FileTree::from_directory(dir.path(), options).unwrap();
        std::fs::write(dir.path().join("src/other.log"), "").unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "").unwrap();

        let changed = vec![
            dir.path().join("src/other.log"),
            dir.path().join("src/lib.rs"),
        ];
        tree.update_paths(dir.path(), &changed, options).unwrap();
        let children = &tree.roots[0].children;
        assert_eq!(
            names(&child(children, "src").children),
            [".gitignore", "lib.rs", "main.rs"]
        );
    }

    #[test]
    fn changes_inside_an_excluded_directory_do_not_rescan_the_tree() {
        let dir = gitignored_fixture();
        let options = ScanOptions {
            hide_gitignored: true,
        };
        let mut tree = FileTree::from_directory(dir.path(), options).unwrap();
        // An unreported new file would only show up through a full rescan.
        std::fs::write(dir.path().join("src/unreported.rs"), "").unwrap();
        std::fs::write(dir.path().join("dist/other.js"), "").unwrap();

        let changed = vec![dir.path().join("dist/other.js")];
        tree.update_paths(dir.path(), &changed, options).unwrap();
        let children = &tree.roots[0].children;
        assert_eq!(names(children), ["src", ".gitignore"]);
        assert_eq!(
            names(&child(children, "src").children),
            [".gitignore", "main.rs"]
        );
    }

    #[test]
    fn changes_inside_a_new_visible_directory_still_rescan() {
        let dir = gitignored_fixture();
        let options = ScanOptions {
            hide_gitignored: true,
        };
        let mut tree = FileTree::from_directory(dir.path(), options).unwrap();
        std::fs::create_dir(dir.path().join("docs")).unwrap();
        std::fs::write(dir.path().join("docs/readme.md"), "").unwrap();

        let changed = vec![dir.path().join("docs/readme.md")];
        tree.update_paths(dir.path(), &changed, options).unwrap();
        let children = &tree.roots[0].children;
        assert_eq!(names(children), ["docs", "src", ".gitignore"]);
    }

    #[test]
    fn toggling_hide_gitignored_rescans_the_tree() {
        let dir = gitignored_fixture();
        let metrics = ScaledMetrics::new(1.0);
        let mut ws = Workspace::new(dir.path().to_path_buf(), &metrics).unwrap();
        assert!(!ws.hide_gitignored);
        assert_eq!(ws.file_tree.roots[0].children.len(), 4);

        ws.toggle_hide_gitignored().unwrap();
        assert!(ws.hide_gitignored);
        assert_eq!(
            names(&ws.file_tree.roots[0].children),
            ["src", ".gitignore"]
        );

        ws.toggle_hide_gitignored().unwrap();
        assert!(!ws.hide_gitignored);
        assert_eq!(ws.file_tree.roots[0].children.len(), 4);
    }

    #[test]
    fn test_workspace_folder_toggle() {
        let metrics = ScaledMetrics::new(1.0);
        let mut ws = Workspace {
            root: PathBuf::from("/test"),
            expanded_folders: HashSet::new(),
            selected_item: None,
            file_tree: FileTree::default(),
            sidebar_visible: true,
            sidebar_width_logical: metrics.sidebar_default_width_logical,
            scroll_offset: 0,
            hide_gitignored: false,
        };

        let folder = Path::new("/test/src");

        assert!(!ws.is_expanded(folder));
        ws.toggle_folder(folder);
        assert!(ws.is_expanded(folder));
        ws.toggle_folder(folder);
        assert!(!ws.is_expanded(folder));
    }
}
