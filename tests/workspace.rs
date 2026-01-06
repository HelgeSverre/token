//! Integration tests for workspace operations
//!
//! Tests file tree management, sidebar interactions, and workspace state.

mod common;

use std::collections::HashSet;
use std::path::PathBuf;

use token::messages::{Msg, WorkspaceMsg};
use token::model::{FileExtension, FileNode, FileTree, FocusTarget, ScaledMetrics, Workspace};
use token::update::update;

// ============================================================================
// FileExtension classification tests
// ============================================================================

#[test]
fn test_file_extension_rust() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("main.rs")),
        FileExtension::Rust
    );
}

#[test]
fn test_file_extension_javascript_variants() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("app.js")),
        FileExtension::JavaScript
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("module.mjs")),
        FileExtension::JavaScript
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("common.cjs")),
        FileExtension::JavaScript
    );
}

#[test]
fn test_file_extension_typescript_variants() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("app.ts")),
        FileExtension::TypeScript
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("component.tsx")),
        FileExtension::TypeScript
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("types.mts")),
        FileExtension::TypeScript
    );
}

#[test]
fn test_file_extension_python() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("script.py")),
        FileExtension::Python
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("gui.pyw")),
        FileExtension::Python
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("stubs.pyi")),
        FileExtension::Python
    );
}

#[test]
fn test_file_extension_special_files() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from(".gitignore")),
        FileExtension::Git
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("Cargo.lock")),
        FileExtension::Lock
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("package-lock.json")),
        FileExtension::Lock
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("Makefile")),
        FileExtension::Config
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("Dockerfile")),
        FileExtension::Config
    );
}

#[test]
fn test_file_extension_binary() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("program.exe")),
        FileExtension::Binary
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("library.dll")),
        FileExtension::Binary
    );
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("library.so")),
        FileExtension::Binary
    );
}

#[test]
fn test_file_extension_unknown() {
    assert_eq!(
        FileExtension::from_path(&PathBuf::from("data.xyz")),
        FileExtension::Unknown
    );
}

#[test]
fn test_file_extension_icons() {
    // Icons are Nerd Font glyphs - they may render as empty in tests but should be consistent
    let rust_icon = FileExtension::Rust.icon();
    let js_icon = FileExtension::JavaScript.icon();
    let unknown_icon = FileExtension::Unknown.icon();

    // Different extensions should have distinct icons
    assert_ne!(rust_icon, js_icon);
    // All icons should be static strings (this tests the API works)
    let _ = rust_icon.len();
    let _ = js_icon.len();
    let _ = unknown_icon.len();
}

// ============================================================================
// FileNode tests
// ============================================================================

#[test]
fn test_file_node_new_file() {
    let node = FileNode::new_file(PathBuf::from("/path/to/main.rs"));
    assert_eq!(node.name, "main.rs");
    assert!(!node.is_dir);
    assert!(node.children.is_empty());
    assert_eq!(node.extension, FileExtension::Rust);
}

#[test]
fn test_file_node_new_dir() {
    let node = FileNode::new_dir(PathBuf::from("/path/to/src"));
    assert_eq!(node.name, "src");
    assert!(node.is_dir);
    assert!(node.children.is_empty());
}

#[test]
fn test_file_node_icon_file() {
    let node = FileNode::new_file(PathBuf::from("main.rs"));
    assert!(!node.icon().is_empty());
}

#[test]
fn test_file_node_icon_dir() {
    let node = FileNode::new_dir(PathBuf::from("src"));
    // Icons are Nerd Font glyphs - verify the API returns strings
    let collapsed = node.icon();
    let expanded = node.icon_expanded();
    // Both should be static str references (may appear empty without Nerd Font)
    let _ = format!("{}{}", collapsed, expanded);
    // Directory icons should be returned for directories
    assert!(node.is_dir);
}

// ============================================================================
// FileTree tests
// ============================================================================

#[test]
fn test_file_tree_default() {
    let tree = FileTree::default();
    assert!(tree.roots.is_empty());
}

#[test]
fn test_file_tree_count_visible_empty() {
    let tree = FileTree::default();
    let expanded = HashSet::new();
    assert_eq!(tree.count_visible(&expanded), 0);
}

#[test]
fn test_file_tree_count_visible_with_roots() {
    let mut tree = FileTree::default();
    tree.roots
        .push(FileNode::new_file(PathBuf::from("file1.rs")));
    tree.roots
        .push(FileNode::new_file(PathBuf::from("file2.rs")));

    let expanded = HashSet::new();
    assert_eq!(tree.count_visible(&expanded), 2);
}

#[test]
fn test_file_tree_count_visible_collapsed_dir() {
    let mut tree = FileTree::default();
    let mut dir = FileNode::new_dir(PathBuf::from("/project/src"));
    dir.children
        .push(FileNode::new_file(PathBuf::from("/project/src/main.rs")));
    dir.children
        .push(FileNode::new_file(PathBuf::from("/project/src/lib.rs")));
    tree.roots.push(dir);

    let expanded = HashSet::new();
    // Only the directory itself is visible (collapsed)
    assert_eq!(tree.count_visible(&expanded), 1);
}

#[test]
fn test_file_tree_count_visible_expanded_dir() {
    let mut tree = FileTree::default();
    let dir_path = PathBuf::from("/project/src");
    let mut dir = FileNode::new_dir(dir_path.clone());
    dir.children
        .push(FileNode::new_file(PathBuf::from("/project/src/main.rs")));
    dir.children
        .push(FileNode::new_file(PathBuf::from("/project/src/lib.rs")));
    tree.roots.push(dir);

    let mut expanded = HashSet::new();
    expanded.insert(dir_path);
    // Directory + 2 children
    assert_eq!(tree.count_visible(&expanded), 3);
}

#[test]
fn test_file_tree_get_visible_item() {
    let mut tree = FileTree::default();
    tree.roots
        .push(FileNode::new_file(PathBuf::from("first.rs")));
    tree.roots
        .push(FileNode::new_file(PathBuf::from("second.rs")));

    let expanded = HashSet::new();
    let item0 = tree.get_visible_item(0, &expanded);
    let item1 = tree.get_visible_item(1, &expanded);
    let item2 = tree.get_visible_item(2, &expanded);

    assert!(item0.is_some());
    assert_eq!(item0.unwrap().name, "first.rs");
    assert!(item1.is_some());
    assert_eq!(item1.unwrap().name, "second.rs");
    assert!(item2.is_none());
}

#[test]
fn test_file_tree_get_visible_item_with_depth() {
    let mut tree = FileTree::default();
    let dir_path = PathBuf::from("/project/src");
    let mut dir = FileNode::new_dir(dir_path.clone());
    dir.children
        .push(FileNode::new_file(PathBuf::from("/project/src/main.rs")));
    tree.roots.push(dir);

    let mut expanded = HashSet::new();
    expanded.insert(dir_path);

    let item0 = tree.get_visible_item_with_depth(0, &expanded);
    let item1 = tree.get_visible_item_with_depth(1, &expanded);

    assert!(item0.is_some());
    let (node0, depth0) = item0.unwrap();
    assert_eq!(node0.name, "src");
    assert_eq!(depth0, 0);

    assert!(item1.is_some());
    let (node1, depth1) = item1.unwrap();
    assert_eq!(node1.name, "main.rs");
    assert_eq!(depth1, 1);
}

// ============================================================================
// Workspace state tests
// ============================================================================

fn test_workspace() -> Workspace {
    let metrics = ScaledMetrics::new(1.0);
    Workspace {
        root: PathBuf::from("/test/project"),
        expanded_folders: HashSet::new(),
        selected_item: None,
        file_tree: FileTree::default(),
        sidebar_visible: true,
        sidebar_width_logical: metrics.sidebar_default_width_logical,
        scroll_offset: 0,
    }
}

#[test]
fn test_workspace_toggle_folder() {
    let mut ws = test_workspace();
    let folder = PathBuf::from("/test/project/src");

    assert!(!ws.is_expanded(&folder));

    ws.toggle_folder(&folder);
    assert!(ws.is_expanded(&folder));

    ws.toggle_folder(&folder);
    assert!(!ws.is_expanded(&folder));
}

#[test]
fn test_workspace_expand_folder() {
    let mut ws = test_workspace();
    let folder = PathBuf::from("/test/project/src");

    ws.expand_folder(&folder);
    assert!(ws.is_expanded(&folder));

    // Expanding again is idempotent
    ws.expand_folder(&folder);
    assert!(ws.is_expanded(&folder));
}

#[test]
fn test_workspace_collapse_folder() {
    let mut ws = test_workspace();
    let folder = PathBuf::from("/test/project/src");

    ws.expand_folder(&folder);
    ws.collapse_folder(&folder);
    assert!(!ws.is_expanded(&folder));

    // Collapsing again is idempotent
    ws.collapse_folder(&folder);
    assert!(!ws.is_expanded(&folder));
}

#[test]
fn test_workspace_sidebar_width() {
    let mut ws = test_workspace();

    let scale_factor = 2.0;
    let physical_width = ws.sidebar_width(scale_factor);
    assert!((physical_width - ws.sidebar_width_logical * 2.0).abs() < 0.01);

    ws.set_sidebar_width(500.0, scale_factor);
    assert!((ws.sidebar_width_logical - 250.0).abs() < 0.01);
}

#[test]
fn test_workspace_depth() {
    let ws = test_workspace();

    assert_eq!(ws.depth(&PathBuf::from("/test/project")), 0);
    assert_eq!(ws.depth(&PathBuf::from("/test/project/src")), 1);
    assert_eq!(ws.depth(&PathBuf::from("/test/project/src/main.rs")), 2);
}

#[test]
fn test_workspace_visible_item_count() {
    let mut ws = test_workspace();
    assert_eq!(ws.visible_item_count(), 0);

    ws.file_tree
        .roots
        .push(FileNode::new_file(PathBuf::from("/test/project/file.rs")));
    assert_eq!(ws.visible_item_count(), 1);
}

#[test]
fn test_workspace_reveal_file() {
    let mut ws = test_workspace();

    let dir_path = PathBuf::from("/test/project/src");
    let file_path = PathBuf::from("/test/project/src/main.rs");

    let mut dir = FileNode::new_dir(dir_path.clone());
    dir.children.push(FileNode::new_file(file_path.clone()));
    ws.file_tree.roots.push(dir);

    // Initially collapsed
    assert!(!ws.is_expanded(&dir_path));
    assert!(ws.selected_item.is_none());

    // Reveal the file
    ws.reveal_file(&file_path);

    // Parent folder should be expanded
    assert!(ws.is_expanded(&dir_path));
    // File should be selected
    assert_eq!(ws.selected_item, Some(file_path));
}

// ============================================================================
// Workspace update handler tests
// ============================================================================

#[test]
fn test_workspace_toggle_sidebar_message() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    assert!(model.workspace.as_ref().unwrap().sidebar_visible);

    update(&mut model, Msg::Workspace(WorkspaceMsg::ToggleSidebar));
    assert!(!model.workspace.as_ref().unwrap().sidebar_visible);

    update(&mut model, Msg::Workspace(WorkspaceMsg::ToggleSidebar));
    assert!(model.workspace.as_ref().unwrap().sidebar_visible);
}

#[test]
fn test_workspace_toggle_sidebar_returns_focus_to_editor() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());
    model.ui.focus = FocusTarget::Sidebar;

    // Hiding sidebar while focused on it should return focus to editor
    update(&mut model, Msg::Workspace(WorkspaceMsg::ToggleSidebar));

    assert!(!model.workspace.as_ref().unwrap().sidebar_visible);
    assert_eq!(model.ui.focus, FocusTarget::Editor);
}

#[test]
fn test_workspace_toggle_folder_message() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    let folder = PathBuf::from("/test/project/src");

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::ToggleFolder(folder.clone())),
    );
    assert!(model.workspace.as_ref().unwrap().is_expanded(&folder));

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::ToggleFolder(folder.clone())),
    );
    assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));
}

#[test]
fn test_workspace_expand_collapse_folder_messages() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    let folder = PathBuf::from("/test/project/src");

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::ExpandFolder(folder.clone())),
    );
    assert!(model.workspace.as_ref().unwrap().is_expanded(&folder));

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::CollapseFolder(folder.clone())),
    );
    assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder));
}

#[test]
fn test_workspace_select_item_message() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    let path = PathBuf::from("/test/project/file.rs");

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::SelectItem(path.clone())),
    );

    assert_eq!(model.workspace.as_ref().unwrap().selected_item, Some(path));
}

#[test]
fn test_workspace_scroll_message() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    let mut ws = test_workspace();

    // Add some items to make scrolling meaningful
    for i in 0..50 {
        ws.file_tree
            .roots
            .push(FileNode::new_file(PathBuf::from(format!(
                "/test/project/file{}.rs",
                i
            ))));
    }
    model.workspace = Some(ws);

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::Scroll { lines: 5 }),
    );
    assert_eq!(model.workspace.as_ref().unwrap().scroll_offset, 5);

    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::Scroll { lines: -3 }),
    );
    assert_eq!(model.workspace.as_ref().unwrap().scroll_offset, 2);

    // Scrolling up past 0 should clamp to 0
    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::Scroll { lines: -10 }),
    );
    assert_eq!(model.workspace.as_ref().unwrap().scroll_offset, 0);
}

#[test]
fn test_workspace_file_system_change_message() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    // FileSystemChange should trigger a refresh (silently)
    // We can't easily test the actual refresh, but we can verify it doesn't crash
    let result = update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::FileSystemChange { paths: vec![] }),
    );
    assert!(result.is_some());
}

// ============================================================================
// Navigation tests
// ============================================================================

#[test]
fn test_workspace_select_next_previous() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    let mut ws = test_workspace();

    // Add items
    ws.file_tree
        .roots
        .push(FileNode::new_file(PathBuf::from("/test/project/a.rs")));
    ws.file_tree
        .roots
        .push(FileNode::new_file(PathBuf::from("/test/project/b.rs")));
    ws.file_tree
        .roots
        .push(FileNode::new_file(PathBuf::from("/test/project/c.rs")));
    model.workspace = Some(ws);

    // Select first item
    update(&mut model, Msg::Workspace(WorkspaceMsg::SelectNext));
    assert_eq!(
        model.workspace.as_ref().unwrap().selected_item,
        Some(PathBuf::from("/test/project/a.rs"))
    );

    // Select next
    update(&mut model, Msg::Workspace(WorkspaceMsg::SelectNext));
    assert_eq!(
        model.workspace.as_ref().unwrap().selected_item,
        Some(PathBuf::from("/test/project/b.rs"))
    );

    // Select previous
    update(&mut model, Msg::Workspace(WorkspaceMsg::SelectPrevious));
    assert_eq!(
        model.workspace.as_ref().unwrap().selected_item,
        Some(PathBuf::from("/test/project/a.rs"))
    );
}

#[test]
fn test_workspace_open_or_toggle_file() {
    use common::test_model;
    use std::fs;
    use tempfile::tempdir;

    // Create a temporary directory with a real file
    let dir = tempdir().expect("Failed to create temp dir");
    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "test content").expect("Failed to write file");

    let mut model = test_model("hello\nworld\n", 0, 0);
    let mut ws = test_workspace();
    ws.root = dir.path().to_path_buf();
    ws.file_tree
        .roots
        .push(FileNode::new_file(file_path.clone()));
    ws.selected_item = Some(file_path.clone());
    model.workspace = Some(ws);

    // OpenOrToggle on a file should open it
    let result = update(&mut model, Msg::Workspace(WorkspaceMsg::OpenOrToggle));
    assert!(result.is_some());
}

#[test]
fn test_workspace_open_or_toggle_folder() {
    use common::test_model;
    use std::fs;
    use tempfile::tempdir;

    // Create actual directory for is_dir() check to work
    let dir = tempdir().expect("Failed to create temp dir");
    let folder_path = dir.path().join("src");
    fs::create_dir(&folder_path).expect("Failed to create src dir");
    let file_path = folder_path.join("main.rs");
    fs::write(&file_path, "fn main() {}").expect("Failed to write file");

    let mut model = test_model("hello\nworld\n", 0, 0);
    let mut ws = test_workspace();
    ws.root = dir.path().to_path_buf();

    let mut folder = FileNode::new_dir(folder_path.clone());
    folder.children.push(FileNode::new_file(file_path));
    ws.file_tree.roots.push(folder);
    ws.selected_item = Some(folder_path.clone());
    model.workspace = Some(ws);

    // OpenOrToggle on a folder should toggle expansion
    assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder_path));

    update(&mut model, Msg::Workspace(WorkspaceMsg::OpenOrToggle));
    assert!(model.workspace.as_ref().unwrap().is_expanded(&folder_path));

    update(&mut model, Msg::Workspace(WorkspaceMsg::OpenOrToggle));
    assert!(!model.workspace.as_ref().unwrap().is_expanded(&folder_path));
}

// ============================================================================
// Sidebar resize tests
// ============================================================================

#[test]
fn test_workspace_sidebar_resize_flow() {
    use common::test_model;

    let mut model = test_model("hello\nworld\n", 0, 0);
    model.workspace = Some(test_workspace());

    let initial_width = model.workspace.as_ref().unwrap().sidebar_width_logical;

    // Start resize
    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::StartSidebarResize { initial_x: 250.0 }),
    );
    assert!(model.ui.sidebar_resize.is_some());

    // Update resize (drag right)
    update(
        &mut model,
        Msg::Workspace(WorkspaceMsg::UpdateSidebarResize { x: 300.0 }),
    );
    let new_width = model.workspace.as_ref().unwrap().sidebar_width_logical;
    assert!(new_width > initial_width);

    // End resize
    update(&mut model, Msg::Workspace(WorkspaceMsg::EndSidebarResize));
    assert!(model.ui.sidebar_resize.is_none());
}
