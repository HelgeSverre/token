//! Benchmarks for workspace and file tree operations
//!
//! Run with: cargo bench --bench workspace

mod support;

use std::collections::HashSet;
use std::path::PathBuf;

use token::model::{FileExtension, FileNode, FileTree, ScaledMetrics, Workspace};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// ============================================================================
// File extension classification benchmarks
// ============================================================================

#[divan::bench]
fn classify_rust_file() {
    let path = PathBuf::from("main.rs");
    divan::black_box(FileExtension::from_path(&path));
}

#[divan::bench]
fn classify_typescript_file() {
    let path = PathBuf::from("component.tsx");
    divan::black_box(FileExtension::from_path(&path));
}

#[divan::bench]
fn classify_special_file() {
    let path = PathBuf::from("Cargo.lock");
    divan::black_box(FileExtension::from_path(&path));
}

#[divan::bench(args = [10, 100, 1000])]
fn classify_many_files(count: usize) {
    let paths: Vec<PathBuf> = (0..count)
        .map(|i| PathBuf::from(format!("file{}.rs", i)))
        .collect();

    for path in &paths {
        divan::black_box(FileExtension::from_path(path));
    }
}

// ============================================================================
// FileNode creation benchmarks
// ============================================================================

#[divan::bench]
fn create_file_node() {
    let path = PathBuf::from("/project/src/main.rs");
    divan::black_box(FileNode::new_file(path));
}

#[divan::bench]
fn create_dir_node() {
    let path = PathBuf::from("/project/src");
    divan::black_box(FileNode::new_dir(path));
}

#[divan::bench(args = [10, 100, 500])]
fn create_many_file_nodes(count: usize) {
    for i in 0..count {
        let path = PathBuf::from(format!("/project/src/file{}.rs", i));
        divan::black_box(FileNode::new_file(path));
    }
}

// ============================================================================
// FileTree traversal benchmarks
// ============================================================================

fn create_flat_tree(count: usize) -> FileTree {
    let mut tree = FileTree::default();
    for i in 0..count {
        tree.roots
            .push(FileNode::new_file(PathBuf::from(format!("file{}.rs", i))));
    }
    tree
}

fn create_nested_tree(depth: usize, files_per_dir: usize) -> FileTree {
    fn create_nested_dir(base: &str, depth: usize, files_per_dir: usize) -> FileNode {
        let mut dir = FileNode::new_dir(PathBuf::from(base));

        // Add files
        for i in 0..files_per_dir {
            dir.children.push(FileNode::new_file(PathBuf::from(format!(
                "{}/file{}.rs",
                base, i
            ))));
        }

        // Add subdirectory if depth > 0
        if depth > 0 {
            dir.children.push(create_nested_dir(
                &format!("{}/sub", base),
                depth - 1,
                files_per_dir,
            ));
        }

        dir
    }

    let mut tree = FileTree::default();
    tree.roots
        .push(create_nested_dir("/project", depth, files_per_dir));
    tree
}

#[divan::bench(args = [10, 100, 500, 1000])]
fn count_visible_flat(count: usize) {
    let tree = create_flat_tree(count);
    let expanded = HashSet::new();
    divan::black_box(tree.count_visible(&expanded));
}

#[divan::bench(args = [3, 5, 7])]
fn count_visible_nested_collapsed(depth: usize) {
    let tree = create_nested_tree(depth, 10);
    let expanded = HashSet::new();
    divan::black_box(tree.count_visible(&expanded));
}

#[divan::bench(args = [3, 5, 7])]
fn count_visible_nested_all_expanded(depth: usize) {
    let tree = create_nested_tree(depth, 10);

    // Expand all directories
    let mut expanded = HashSet::new();
    fn collect_dirs(node: &FileNode, expanded: &mut HashSet<PathBuf>) {
        if node.is_dir {
            expanded.insert(node.path.clone());
            for child in &node.children {
                collect_dirs(child, expanded);
            }
        }
    }
    for root in &tree.roots {
        collect_dirs(root, &mut expanded);
    }

    divan::black_box(tree.count_visible(&expanded));
}

#[divan::bench(args = [10, 100, 500])]
fn get_visible_item_first(count: usize) {
    let tree = create_flat_tree(count);
    let expanded = HashSet::new();
    divan::black_box(tree.get_visible_item(0, &expanded));
}

#[divan::bench(args = [10, 100, 500])]
fn get_visible_item_middle(count: usize) {
    let tree = create_flat_tree(count);
    let expanded = HashSet::new();
    divan::black_box(tree.get_visible_item(count / 2, &expanded));
}

#[divan::bench(args = [10, 100, 500])]
fn get_visible_item_last(count: usize) {
    let tree = create_flat_tree(count);
    let expanded = HashSet::new();
    divan::black_box(tree.get_visible_item(count - 1, &expanded));
}

#[divan::bench(args = [10, 100, 500])]
fn get_visible_item_with_depth(count: usize) {
    let tree = create_flat_tree(count);
    let expanded = HashSet::new();
    divan::black_box(tree.get_visible_item_with_depth(count / 2, &expanded));
}

// ============================================================================
// Workspace operations benchmarks
// ============================================================================

fn create_workspace_with_tree(file_count: usize) -> Workspace {
    let metrics = ScaledMetrics::new(1.0);
    Workspace {
        root: PathBuf::from("/project"),
        expanded_folders: HashSet::new(),
        selected_item: None,
        file_tree: create_flat_tree(file_count),
        sidebar_visible: true,
        sidebar_width_logical: metrics.sidebar_default_width_logical,
        scroll_offset: 0,
    }
}

#[divan::bench(args = [10, 100, 500])]
fn workspace_visible_item_count(count: usize) {
    let ws = create_workspace_with_tree(count);
    divan::black_box(ws.visible_item_count());
}

#[divan::bench(args = [10, 100, 500])]
fn workspace_toggle_folder(count: usize) {
    let mut ws = create_workspace_with_tree(count);
    let folder = PathBuf::from("/project/src");

    for _ in 0..100 {
        ws.toggle_folder(&folder);
    }
    divan::black_box(&ws);
}

#[divan::bench]
fn workspace_is_expanded() {
    let mut ws = create_workspace_with_tree(100);
    let folder = PathBuf::from("/project/src");
    ws.expand_folder(&folder);

    for _ in 0..1000 {
        divan::black_box(ws.is_expanded(&folder));
    }
}

#[divan::bench]
fn workspace_depth_calculation() {
    let ws = create_workspace_with_tree(100);

    let paths = [
        PathBuf::from("/project"),
        PathBuf::from("/project/src"),
        PathBuf::from("/project/src/main.rs"),
        PathBuf::from("/project/src/lib/utils/helpers.rs"),
    ];

    for path in &paths {
        divan::black_box(ws.depth(path));
    }
}

#[divan::bench]
fn workspace_sidebar_width_conversion() {
    let ws = create_workspace_with_tree(100);
    let scale_factors = [1.0, 1.5, 2.0, 2.5, 3.0];

    for scale in scale_factors {
        divan::black_box(ws.sidebar_width(scale));
    }
}

// ============================================================================
// FileTree sorting benchmarks
// ============================================================================

#[divan::bench(args = [10, 100, 500])]
fn sort_mixed_files_and_dirs(count: usize) {
    let mut nodes: Vec<FileNode> = Vec::with_capacity(count);

    // Alternate between files and directories
    for i in 0..count {
        if i % 2 == 0 {
            nodes.push(FileNode::new_file(PathBuf::from(format!("file{}.rs", i))));
        } else {
            nodes.push(FileNode::new_dir(PathBuf::from(format!("dir{}", i))));
        }
    }

    // Sort using the FileTree method (dirs first, then alphabetical)
    nodes.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });

    divan::black_box(&nodes);
}

// ============================================================================
// Reveal file benchmarks (expanding parent folders)
// ============================================================================

#[divan::bench(args = [3, 5, 7])]
fn reveal_file_deep_path(depth: usize) {
    let mut ws = Workspace {
        root: PathBuf::from("/project"),
        expanded_folders: HashSet::new(),
        selected_item: None,
        file_tree: create_nested_tree(depth, 5),
        sidebar_visible: true,
        sidebar_width_logical: 250.0,
        scroll_offset: 0,
    };

    // Create a deep path
    let mut path = PathBuf::from("/project");
    for _ in 0..depth {
        path = path.join("sub");
    }
    path = path.join("file0.rs");

    ws.reveal_file(&path);
    divan::black_box(&ws);
}

// ============================================================================
// Large workspace simulation
// ============================================================================

fn create_large_tree(dirs: usize, files_per_dir: usize) -> FileTree {
    let mut tree = FileTree::default();
    let root = FileNode::new_dir(PathBuf::from("/project"));
    tree.roots.push(root);

    for d in 0..dirs {
        let mut dir = FileNode::new_dir(PathBuf::from(format!("/project/dir{}", d)));
        for f in 0..files_per_dir {
            dir.children.push(FileNode::new_file(PathBuf::from(format!(
                "/project/dir{}/file{}.rs",
                d, f
            ))));
        }
        tree.roots[0].children.push(dir);
    }

    tree
}

#[divan::bench(args = [(10, 10), (50, 20), (100, 50)])]
fn large_tree_count_visible(params: (usize, usize)) {
    let (dirs, files) = params;
    let tree = create_large_tree(dirs, files);

    // Expand all directories
    let mut expanded = HashSet::new();
    expanded.insert(PathBuf::from("/project"));
    for d in 0..dirs {
        expanded.insert(PathBuf::from(format!("/project/dir{}", d)));
    }

    divan::black_box(tree.count_visible(&expanded));
}

#[divan::bench(args = [(10, 10), (50, 20), (100, 50)])]
fn large_tree_get_item_at_end(params: (usize, usize)) {
    let (dirs, files) = params;
    let tree = create_large_tree(dirs, files);

    let mut expanded = HashSet::new();
    expanded.insert(PathBuf::from("/project"));
    for d in 0..dirs {
        expanded.insert(PathBuf::from(format!("/project/dir{}", d)));
    }

    let total = tree.count_visible(&expanded);
    divan::black_box(tree.get_visible_item(total - 1, &expanded));
}
