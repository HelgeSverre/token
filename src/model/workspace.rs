//! Workspace management - file tree sidebar and workspace tracking
//!
//! Provides VS Code-style workspace functionality:
//! - File tree sidebar with expand/collapse
//! - Workspace root directory tracking
//! - File type classification and icons

use std::collections::HashSet;
use std::path::{Path, PathBuf};

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

    /// Get a text-based icon for the file type
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Rust => "󱘗",
            Self::JavaScript => "",
            Self::TypeScript => "",
            Self::Python => "",
            Self::Go => "",
            Self::C | Self::Cpp | Self::Header => "",
            Self::Json => "",
            Self::Yaml | Self::Toml => "",
            Self::Markdown => "",
            Self::Html => "",
            Self::Css | Self::Scss => "",
            Self::Sql => "",
            Self::Shell => "",
            Self::Git => "",
            Self::Lock => "",
            Self::Config => "",
            Self::Text => "",
            Self::Binary => "",
            Self::Unknown => "",
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
            "" // Folder icon
        } else {
            self.extension.icon()
        }
    }

    /// Get expanded folder icon
    pub fn icon_expanded(&self) -> &'static str {
        if self.is_dir {
            "" // Open folder icon
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
    pub fn from_directory(root: &Path) -> std::io::Result<Self> {
        if !root.is_dir() {
            return Ok(Self { roots: Vec::new() });
        }

        // Scan the contents of the root directory
        let mut children = Self::scan_directory(root, 0)?;
        Self::sort_nodes(&mut children);

        // Create the root folder node with the workspace directory itself
        let mut root_node = FileNode::new_dir(root.to_path_buf());
        root_node.children = children;

        Ok(Self {
            roots: vec![root_node],
        })
    }

    /// Scan a directory recursively (up to max depth)
    fn scan_directory(dir: &Path, depth: usize) -> std::io::Result<Vec<FileNode>> {
        const MAX_DEPTH: usize = 20;

        if depth > MAX_DEPTH {
            return Ok(Vec::new());
        }

        let mut nodes = Vec::new();

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            // Check if should ignore
            if Self::should_ignore(&path) {
                continue;
            }

            let node = if path.is_dir() {
                let mut dir_node = FileNode::new_dir(path.clone());
                // Recursively scan children
                dir_node.children = Self::scan_directory(&path, depth + 1)?;
                Self::sort_nodes(&mut dir_node.children);
                dir_node
            } else {
                FileNode::new_file(path)
            };

            // Skip empty directories? (optional, currently keeping them)
            nodes.push(node);
        }

        Ok(nodes)
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
    pub fn refresh(&mut self, root: &Path) -> std::io::Result<()> {
        *self = Self::from_directory(root)?;
        Ok(())
    }

    /// Incrementally update the tree for changed paths.
    ///
    /// Instead of rescanning the entire tree, this method only refreshes
    /// the parent directories of the changed paths. This is much faster
    /// for typical file system changes (creating/deleting/modifying files).
    pub fn update_paths(&mut self, root: &Path, paths: &[PathBuf]) -> std::io::Result<()> {
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
            return self.refresh(root);
        }

        // Refresh each parent directory incrementally
        for parent in &parents_to_refresh {
            self.refresh_directory(root, parent)?;
        }

        Ok(())
    }

    /// Refresh a single directory's children in the tree.
    fn refresh_directory(&mut self, root: &Path, dir_path: &Path) -> std::io::Result<()> {
        // Special case: refreshing the root itself
        if dir_path == root {
            return self.refresh(root);
        }

        // Find the node for this directory and update its children
        for tree_root in &mut self.roots {
            if Self::refresh_node_directory(tree_root, dir_path)? {
                return Ok(());
            }
        }

        // Directory not found in tree - might be new, do full refresh
        self.refresh(root)
    }

    /// Recursively find and refresh a directory node.
    /// Returns true if the directory was found and refreshed.
    fn refresh_node_directory(node: &mut FileNode, target_dir: &Path) -> std::io::Result<bool> {
        if node.path == target_dir && node.is_dir {
            // Found it! Refresh this node's children
            let mut new_children = Self::scan_directory(&node.path, 0)?;
            Self::sort_nodes(&mut new_children);
            node.children = new_children;
            return Ok(true);
        }

        // Recurse into children
        if node.is_dir && target_dir.starts_with(&node.path) {
            for child in &mut node.children {
                if Self::refresh_node_directory(child, target_dir)? {
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
        self.roots
            .iter()
            .map(|node| Self::count_visible_node(node, expanded))
            .sum()
    }

    fn count_visible_node(node: &FileNode, expanded: &HashSet<PathBuf>) -> usize {
        let mut count = 1; // This node

        if node.is_dir && expanded.contains(&node.path) {
            for child in &node.children {
                count += Self::count_visible_node(child, expanded);
            }
        }

        count
    }

    /// Get the nth visible item (for hit testing)
    pub fn get_visible_item(&self, index: usize, expanded: &HashSet<PathBuf>) -> Option<&FileNode> {
        let mut current = 0;
        for node in &self.roots {
            if let Some(found) = Self::get_visible_item_node(node, index, &mut current, expanded) {
                return Some(found);
            }
        }
        None
    }

    /// Get the nth visible item with its depth (for hit testing with chevron area detection)
    pub fn get_visible_item_with_depth(
        &self,
        index: usize,
        expanded: &HashSet<PathBuf>,
    ) -> Option<(&FileNode, usize)> {
        let mut current = 0;
        for node in &self.roots {
            if let Some(found) =
                Self::get_visible_item_node_with_depth(node, index, &mut current, 0, expanded)
            {
                return Some(found);
            }
        }
        None
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
        for node in &self.roots {
            if let Some(found) = Self::get_visible_item_by_path_node(node, path, expanded) {
                return Some(found);
            }
        }
        None
    }

    fn get_visible_item_by_path_node<'a>(
        node: &'a FileNode,
        target: &PathBuf,
        expanded: &HashSet<PathBuf>,
    ) -> Option<&'a FileNode> {
        if &node.path == target {
            return Some(node);
        }

        // Only recurse into expanded directories
        if node.is_dir && expanded.contains(&node.path) {
            for child in &node.children {
                if let Some(found) = Self::get_visible_item_by_path_node(child, target, expanded) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn get_visible_item_node<'a>(
        node: &'a FileNode,
        target: usize,
        current: &mut usize,
        expanded: &HashSet<PathBuf>,
    ) -> Option<&'a FileNode> {
        if *current == target {
            return Some(node);
        }
        *current += 1;

        if node.is_dir && expanded.contains(&node.path) {
            for child in &node.children {
                if let Some(found) = Self::get_visible_item_node(child, target, current, expanded) {
                    return Some(found);
                }
            }
        }

        None
    }

    fn get_visible_item_node_with_depth<'a>(
        node: &'a FileNode,
        target: usize,
        current: &mut usize,
        depth: usize,
        expanded: &HashSet<PathBuf>,
    ) -> Option<(&'a FileNode, usize)> {
        if *current == target {
            return Some((node, depth));
        }
        *current += 1;

        if node.is_dir && expanded.contains(&node.path) {
            for child in &node.children {
                if let Some(found) = Self::get_visible_item_node_with_depth(
                    child,
                    target,
                    current,
                    depth + 1,
                    expanded,
                ) {
                    return Some(found);
                }
            }
        }

        None
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
}

impl Workspace {
    /// Create a new workspace from a directory
    pub fn new(root: PathBuf, metrics: &ScaledMetrics) -> std::io::Result<Self> {
        // Canonicalize the root path to get the full absolute path
        // This ensures file_name() works correctly (e.g., "." becomes "/path/to/dir")
        let root = std::fs::canonicalize(&root)?;

        let file_tree = FileTree::from_directory(&root)?;

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
        })
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
            self.expanded_folders.remove(path);
        } else {
            self.expanded_folders.insert(path.to_path_buf());
        }
    }

    /// Expand a folder (no-op if already expanded)
    pub fn expand_folder(&mut self, path: &Path) {
        self.expanded_folders.insert(path.to_path_buf());
    }

    /// Collapse a folder (no-op if already collapsed)
    pub fn collapse_folder(&mut self, path: &Path) {
        self.expanded_folders.remove(path);
    }

    /// Check if a folder is expanded
    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded_folders.contains(path)
    }

    /// Refresh the file tree from disk (full rescan)
    pub fn refresh(&mut self) -> std::io::Result<()> {
        self.file_tree.refresh(&self.root)
    }

    /// Incrementally update the file tree for specific changed paths.
    /// Much faster than full refresh for typical file operations.
    pub fn update_paths(&mut self, paths: &[PathBuf]) -> std::io::Result<()> {
        self.file_tree.update_paths(&self.root, paths)
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
        // Expand all parent folders
        let mut current = path.parent();
        while let Some(parent) = current {
            if parent.starts_with(&self.root) && parent != self.root {
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
        };

        let folder = Path::new("/test/src");

        assert!(!ws.is_expanded(folder));
        ws.toggle_folder(folder);
        assert!(ws.is_expanded(folder));
        ws.toggle_folder(folder);
        assert!(!ws.is_expanded(folder));
    }
}
