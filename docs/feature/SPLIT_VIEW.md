# Split View & Multi-Pane Architecture

A hierarchical layout system for multiple editor panes, tabs, and split views.

---

## Overview

### Current Limitations

The editor currently has:

- Single `Document` in `AppModel`
- Single `EditorState` (cursor, viewport, selections)
- No concept of tabs or panes
- No way to view the same document in two places
- No split view capability

### Goals

- **Multiple editor panes** arranged in splits (horizontal/vertical)
- **Tabs within each pane** for switching between documents
- **Shared documents** - same file open in multiple views with synchronized edits
- **Independent view state** - each pane has its own cursor, scroll position, selections
- **Flexible layout tree** - arbitrary nesting of splits
- **Focus management** - clear indication of active pane

---

## Architecture

### Core Entities

Based on EDITOR_UI_REFERENCE.md Chapter 2:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│ Window                                                                      │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ EditorArea                                                              │ │
│ │ ┌─────────────────────────────────┬───────────────────────────────────┐ │ │
│ │ │ EditorGroup (Left)              │ EditorGroup (Right)               │ │ │
│ │ │ ┌─────────────────────────────┐ │ ┌───────────────────────────────┐ │ │ │
│ │ │ │ TabBar                      │ │ │ TabBar                        │ │ │ │
│ │ │ │ [main.rs*][lib.rs]          │ │ │ [main.rs]                     │ │ │ │
│ │ │ └─────────────────────────────┘ │ └───────────────────────────────┘ │ │ │
│ │ │ ┌─────────────────────────────┐ │ ┌───────────────────────────────┐ │ │ │
│ │ │ │ EditorPane                  │ │ │ EditorPane                    │ │ │ │
│ │ │ │ (view of main.rs)           │ │ │ (another view of main.rs)     │ │ │ │
│ │ │ │                             │ │ │                               │ │ │ │
│ │ │ │ Independent scroll/cursor   │ │ │ Independent scroll/cursor     │ │ │ │
│ │ │ └─────────────────────────────┘ │ └───────────────────────────────┘ │ │ │
│ │ └─────────────────────────────────┴───────────────────────────────────┘ │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
│ ┌─────────────────────────────────────────────────────────────────────────┐ │
│ │ StatusBar (global or per-pane)                                          │ │
│ └─────────────────────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────┘
```

### Data Structures

```rust
use std::collections::HashMap;

// ============================================================================
// Identifiers
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

// ============================================================================
// Document (shared content)
// ============================================================================

/// A document is the shared text content, independent of how it's displayed.
/// Multiple EditorStates can reference the same Document.
#[derive(Debug, Clone)]
pub struct Document {
    pub id: DocumentId,
    pub buffer: Rope,
    pub file_path: Option<PathBuf>,
    pub is_modified: bool,
    pub undo_stack: Vec<EditOperation>,
    pub redo_stack: Vec<EditOperation>,

    // Metadata
    pub language: Option<String>,  // "rust", "python", etc.
    pub encoding: Encoding,
    pub line_ending: LineEnding,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Encoding {
    #[default]
    Utf8,
    Utf16Le,
    Utf16Be,
    Latin1,
}

#[derive(Debug, Clone, Copy, Default)]
pub enum LineEnding {
    #[default]
    Lf,
    CrLf,
    Cr,
}

// ============================================================================
// Editor State (per-view state)
// ============================================================================

/// View-specific state for editing a document.
/// Each EditorState references a Document but has independent cursor/scroll.
#[derive(Debug, Clone)]
pub struct EditorState {
    pub id: EditorId,
    pub document_id: DocumentId,

    // View state (independent per editor)
    pub cursors: Vec<Cursor>,
    pub selections: Vec<Selection>,
    pub viewport: Viewport,
    pub scroll_padding: usize,

    // View-specific settings
    pub soft_wrap: bool,
    pub show_line_numbers: bool,
}

// ============================================================================
// Tabs
// ============================================================================

/// A tab represents an open editor in a group
#[derive(Debug, Clone)]
pub struct Tab {
    pub id: TabId,
    pub editor_id: EditorId,
    pub is_pinned: bool,
    pub is_preview: bool,  // Preview tabs get replaced on next file open
}

// ============================================================================
// Editor Group (pane with tabs)
// ============================================================================

/// An editor group contains a tab bar and displays one editor at a time
#[derive(Debug, Clone)]
pub struct EditorGroup {
    pub id: GroupId,
    pub tabs: Vec<Tab>,
    pub active_tab_index: usize,

    // Layout info (set by parent)
    pub rect: Rect,
}

impl EditorGroup {
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_index)
    }

    pub fn active_editor_id(&self) -> Option<EditorId> {
        self.active_tab().map(|t| t.editor_id)
    }
}

// ============================================================================
// Layout Tree
// ============================================================================

/// A node in the layout tree - either a group or a split container
#[derive(Debug, Clone)]
pub enum LayoutNode {
    Group(GroupId),
    Split(SplitContainer),
}

/// A container that splits space between children
#[derive(Debug, Clone)]
pub struct SplitContainer {
    pub direction: SplitDirection,
    pub children: Vec<LayoutNode>,
    /// Proportional sizes (0.0 to 1.0, must sum to 1.0)
    pub ratios: Vec<f32>,
    /// Minimum size in pixels for each child
    pub min_sizes: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    Horizontal,  // Children arranged left-to-right
    Vertical,    // Children arranged top-to-bottom
}

/// Rectangle for layout calculations
#[derive(Debug, Clone, Copy, Default)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width &&
        py >= self.y && py < self.y + self.height
    }
}

// ============================================================================
// Editor Area (top-level container)
// ============================================================================

/// The editor area manages all groups, documents, and the layout tree
#[derive(Debug, Clone)]
pub struct EditorArea {
    /// All open documents (shared across editors)
    pub documents: HashMap<DocumentId, Document>,

    /// All editor states (each references a document)
    pub editors: HashMap<EditorId, EditorState>,

    /// All editor groups (each contains tabs)
    pub groups: HashMap<GroupId, EditorGroup>,

    /// The layout tree root
    pub layout: LayoutNode,

    /// Currently focused group
    pub focused_group_id: GroupId,

    /// ID generators
    next_document_id: u64,
    next_editor_id: u64,
    next_group_id: u64,
    next_tab_id: u64,
}
```

### AppModel Changes

```rust
/// The complete application model
#[derive(Debug, Clone)]
pub struct AppModel {
    /// Editor area with all documents, editors, groups, and layout
    pub editor_area: EditorArea,

    /// UI state (status bar, cursor blink, etc.)
    pub ui: UiState,

    /// Theme
    pub theme: Theme,

    /// Window dimensions
    pub window_size: (u32, u32),

    /// Font metrics
    pub line_height: usize,
    pub char_width: f32,
}

impl AppModel {
    /// Get the currently focused document
    pub fn focused_document(&self) -> Option<&Document> {
        let group = self.editor_area.groups.get(&self.editor_area.focused_group_id)?;
        let editor_id = group.active_editor_id()?;
        let editor = self.editor_area.editors.get(&editor_id)?;
        self.editor_area.documents.get(&editor.document_id)
    }

    /// Get the currently focused editor state
    pub fn focused_editor(&self) -> Option<&EditorState> {
        let group = self.editor_area.groups.get(&self.editor_area.focused_group_id)?;
        let editor_id = group.active_editor_id()?;
        self.editor_area.editors.get(&editor_id)
    }

    /// Get mutable focused editor
    pub fn focused_editor_mut(&mut self) -> Option<&mut EditorState> {
        let group = self.editor_area.groups.get(&self.editor_area.focused_group_id)?;
        let editor_id = group.active_editor_id()?;
        self.editor_area.editors.get_mut(&editor_id)
    }
}
```

---

## Layout Algorithm

### Computing Rectangles

```rust
impl EditorArea {
    /// Compute layout rectangles for all groups
    pub fn compute_layout(&mut self, available_rect: Rect) {
        self.compute_layout_node(&self.layout.clone(), available_rect);
    }

    fn compute_layout_node(&mut self, node: &LayoutNode, rect: Rect) {
        match node {
            LayoutNode::Group(group_id) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.rect = rect;
                }
            }
            LayoutNode::Split(split) => {
                self.compute_split_layout(split, rect);
            }
        }
    }

    fn compute_split_layout(&mut self, split: &SplitContainer, rect: Rect) {
        let total_size = match split.direction {
            SplitDirection::Horizontal => rect.width,
            SplitDirection::Vertical => rect.height,
        };

        // Splitter bar width
        const SPLITTER_SIZE: f32 = 4.0;
        let total_splitters = (split.children.len().saturating_sub(1)) as f32 * SPLITTER_SIZE;
        let available = total_size - total_splitters;

        let mut offset = 0.0;

        for (i, (child, ratio)) in split.children.iter().zip(&split.ratios).enumerate() {
            let child_size = available * ratio;

            let child_rect = match split.direction {
                SplitDirection::Horizontal => Rect {
                    x: rect.x + offset,
                    y: rect.y,
                    width: child_size,
                    height: rect.height,
                },
                SplitDirection::Vertical => Rect {
                    x: rect.x,
                    y: rect.y + offset,
                    width: rect.width,
                    height: child_size,
                },
            };

            self.compute_layout_node(child, child_rect);

            offset += child_size;
            if i < split.children.len() - 1 {
                offset += SPLITTER_SIZE;
            }
        }
    }
}
```

### Hit Testing

```rust
impl EditorArea {
    /// Find which group contains a point
    pub fn group_at_point(&self, x: f32, y: f32) -> Option<GroupId> {
        self.group_at_point_in_node(&self.layout, x, y)
    }

    fn group_at_point_in_node(&self, node: &LayoutNode, x: f32, y: f32) -> Option<GroupId> {
        match node {
            LayoutNode::Group(group_id) => {
                let group = self.groups.get(group_id)?;
                if group.rect.contains(x, y) {
                    Some(*group_id)
                } else {
                    None
                }
            }
            LayoutNode::Split(split) => {
                for child in &split.children {
                    if let Some(id) = self.group_at_point_in_node(child, x, y) {
                        return Some(id);
                    }
                }
                None
            }
        }
    }
}
```

---

## Messages

```rust
/// Editor group / layout messages
#[derive(Debug, Clone)]
pub enum LayoutMsg {
    // Focus
    FocusGroup(GroupId),
    FocusNextGroup,
    FocusPreviousGroup,

    // Tabs
    OpenFile { path: PathBuf, group_id: Option<GroupId> },
    CloseTab { group_id: GroupId, tab_id: TabId },
    CloseActiveTab,
    NextTab,
    PreviousTab,
    ActivateTab { group_id: GroupId, tab_index: usize },
    MoveTabToGroup { tab_id: TabId, target_group_id: GroupId },

    // Splits
    SplitHorizontal,  // Split focused group horizontally
    SplitVertical,    // Split focused group vertically
    CloseGroup(GroupId),
    ResizeSplit { direction: SplitDirection, delta: f32 },

    // Layout
    ResetLayout,  // Back to single pane
}

// Updated top-level Msg
#[derive(Debug, Clone)]
pub enum Msg {
    Editor(EditorMsg),
    Document(DocumentMsg),
    Ui(UiMsg),
    App(AppMsg),
    Layout(LayoutMsg),  // NEW
}
```

---

## Document Synchronization

When the same document is open in multiple editors:

```rust
impl EditorArea {
    /// Apply an edit to a document and notify all editors viewing it
    pub fn apply_edit(&mut self, document_id: DocumentId, edit: EditOperation)
        -> Vec<EditorId>
    {
        // Apply to document
        if let Some(doc) = self.documents.get_mut(&document_id) {
            match &edit {
                EditOperation::Insert { position, text, .. } => {
                    doc.buffer.insert(*position, text);
                }
                EditOperation::Delete { position, text, .. } => {
                    doc.buffer.remove(*position..*position + text.chars().count());
                }
            }
            doc.push_edit(edit.clone());
        }

        // Find all editors viewing this document
        let affected_editors: Vec<EditorId> = self.editors
            .iter()
            .filter(|(_, e)| e.document_id == document_id)
            .map(|(id, _)| *id)
            .collect();

        // Each affected editor may need cursor/scroll adjustment
        for editor_id in &affected_editors {
            if let Some(editor) = self.editors.get_mut(editor_id) {
                // Adjust cursor positions based on edit
                self.adjust_cursors_for_edit(editor, &edit);
            }
        }

        affected_editors
    }

    fn adjust_cursors_for_edit(&self, editor: &mut EditorState, edit: &EditOperation) {
        // TODO: Implement cursor adjustment logic
        // - If edit is before cursor, shift cursor
        // - If edit is at cursor, decide behavior (typically cursor moves with insert)
        // - If edit is after cursor, no change needed
    }
}
```

---

## Rendering

### Render Pipeline

```rust
impl Renderer {
    pub fn render(&mut self, model: &AppModel, perf: &PerfStats) -> Result<()> {
        // Clear background
        let bg_color = model.theme.editor.background.to_argb_u32();
        self.buffer.fill(bg_color);

        // Compute layout
        let editor_rect = Rect::new(
            0.0,
            0.0,
            model.window_size.0 as f32,
            model.window_size.1 as f32 - STATUS_BAR_HEIGHT,
        );

        // Render each group
        self.render_layout_node(&model.editor_area.layout, model)?;

        // Render splitter bars
        self.render_splitters(&model.editor_area.layout, model)?;

        // Render status bar
        self.render_status_bar(model)?;

        Ok(())
    }

    fn render_layout_node(&mut self, node: &LayoutNode, model: &AppModel) -> Result<()> {
        match node {
            LayoutNode::Group(group_id) => {
                if let Some(group) = model.editor_area.groups.get(group_id) {
                    self.render_editor_group(group, model)?;
                }
            }
            LayoutNode::Split(split) => {
                for child in &split.children {
                    self.render_layout_node(child, model)?;
                }
            }
        }
        Ok(())
    }

    fn render_editor_group(&mut self, group: &EditorGroup, model: &AppModel) -> Result<()> {
        let rect = group.rect;
        let is_focused = group.id == model.editor_area.focused_group_id;

        // Tab bar height
        const TAB_BAR_HEIGHT: f32 = 28.0;

        // Render tab bar
        let tab_rect = Rect::new(rect.x, rect.y, rect.width, TAB_BAR_HEIGHT);
        self.render_tab_bar(group, &tab_rect, model)?;

        // Render editor content
        if let Some(editor_id) = group.active_editor_id() {
            if let Some(editor) = model.editor_area.editors.get(&editor_id) {
                if let Some(document) = model.editor_area.documents.get(&editor.document_id) {
                    let content_rect = Rect::new(
                        rect.x,
                        rect.y + TAB_BAR_HEIGHT,
                        rect.width,
                        rect.height - TAB_BAR_HEIGHT,
                    );
                    self.render_editor_content(editor, document, &content_rect, is_focused, model)?;
                }
            }
        }

        // Focus indicator border
        if is_focused {
            self.render_focus_border(&rect, model)?;
        }

        Ok(())
    }
}
```

### Tab Bar Rendering

```rust
fn render_tab_bar(&mut self, group: &EditorGroup, rect: &Rect, model: &AppModel) -> Result<()> {
    // Background
    let bg_color = model.theme.tab_bar.background.to_argb_u32();
    self.fill_rect(rect, bg_color);

    let mut x = rect.x + 4.0;

    for (i, tab) in group.tabs.iter().enumerate() {
        let is_active = i == group.active_tab_index;
        let editor = model.editor_area.editors.get(&tab.editor_id);
        let document = editor.and_then(|e| model.editor_area.documents.get(&e.document_id));

        let label = document
            .and_then(|d| d.file_path.as_ref())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled");

        let modified = document.map(|d| d.is_modified).unwrap_or(false);
        let display = if modified {
            format!("{} •", label)
        } else {
            label.to_string()
        };

        // Tab background
        let tab_bg = if is_active {
            model.theme.tab_bar.active_background.to_argb_u32()
        } else {
            model.theme.tab_bar.inactive_background.to_argb_u32()
        };

        let tab_width = (display.len() as f32 * self.char_width) + 16.0;
        let tab_rect = Rect::new(x, rect.y, tab_width, rect.height);
        self.fill_rect(&tab_rect, tab_bg);

        // Tab text
        let text_color = if is_active {
            model.theme.tab_bar.active_foreground.to_argb_u32()
        } else {
            model.theme.tab_bar.inactive_foreground.to_argb_u32()
        };

        self.draw_text(x + 8.0, rect.y + 6.0, &display, text_color)?;

        x += tab_width + 2.0;
    }

    Ok(())
}
```

---

## Theme Extensions

```rust
// In theme.rs
pub struct Theme {
    pub name: String,
    pub editor: EditorTheme,
    pub gutter: GutterTheme,
    pub status_bar: StatusBarTheme,
    pub tab_bar: TabBarTheme,      // NEW
    pub splitter: SplitterTheme,   // NEW
}

pub struct TabBarTheme {
    pub background: Color,
    pub active_background: Color,
    pub active_foreground: Color,
    pub inactive_background: Color,
    pub inactive_foreground: Color,
    pub border: Color,
    pub modified_indicator: Color,
    pub close_button: Color,
    pub close_button_hover: Color,
}

pub struct SplitterTheme {
    pub background: Color,
    pub hover: Color,
    pub active: Color,  // While dragging
}
```

---

## Implementation Plan

### Phase 1: Core Data Structures

- [ ] Add ID types (`DocumentId`, `EditorId`, `GroupId`, `TabId`)
- [ ] Update `Document` to include `id: DocumentId`
- [ ] Update `EditorState` to include `id: EditorId` and `document_id: DocumentId`
- [ ] Add `Tab`, `EditorGroup`, `LayoutNode`, `SplitContainer`
- [ ] Add `EditorArea` with HashMaps and layout tree

### Phase 2: Layout System

- [ ] Add `Rect` type
- [ ] Implement `compute_layout()` for the layout tree
- [ ] Implement `group_at_point()` for hit testing
- [ ] Add splitter drag handling

### Phase 3: Update AppModel

- [ ] Replace single `Document`/`EditorState` with `EditorArea`
- [ ] Add convenience methods (`focused_document()`, `focused_editor()`)
- [ ] Update all `update_*` functions to work with focused editor

### Phase 4: Messages

- [ ] Add `LayoutMsg` enum
- [ ] Add `update_layout()` function
- [ ] Implement split/close/focus operations

### Phase 5: Rendering

- [ ] Update `Renderer` to iterate over groups
- [ ] Implement `render_editor_group()`
- [ ] Implement `render_tab_bar()`
- [ ] Implement splitter bar rendering
- [ ] Add focus indicator

### Phase 6: Document Synchronization

- [ ] Implement `apply_edit()` with multi-editor notification
- [ ] Implement cursor adjustment for edits in other views
- [ ] Handle undo/redo across shared documents

### Phase 7: Keyboard Shortcuts

- [ ] `Cmd+\` - Split horizontal
- [ ] `Cmd+Shift+\` - Split vertical
- [ ] `Cmd+W` - Close tab
- [ ] `Cmd+Shift+W` - Close group
- [ ] `Cmd+1/2/3` - Focus group by index
- [ ] `Cmd+Tab` - Next tab
- [ ] `Ctrl+Tab` - Next group

---

## Migration Path

To avoid breaking the existing single-pane functionality:

1. **Create `EditorArea` with single group** - Wrap existing Document/EditorState
2. **Keep `AppModel` convenience methods** - `focused_document()` returns what `model.document` used to
3. **Gradually move rendering** - First render single group, then add splits
4. **Add split commands only when rendering works**

```rust
// Initial migration - single group
impl EditorArea {
    pub fn single_document(document: Document, editor: EditorState) -> Self {
        let doc_id = DocumentId(1);
        let editor_id = EditorId(1);
        let group_id = GroupId(1);
        let tab_id = TabId(1);

        let mut documents = HashMap::new();
        let mut editors = HashMap::new();
        let mut groups = HashMap::new();

        documents.insert(doc_id, Document { id: doc_id, ..document });
        editors.insert(editor_id, EditorState {
            id: editor_id,
            document_id: doc_id,
            ..editor
        });

        let tab = Tab {
            id: tab_id,
            editor_id,
            is_pinned: false,
            is_preview: false,
        };

        groups.insert(group_id, EditorGroup {
            id: group_id,
            tabs: vec![tab],
            active_tab_index: 0,
            rect: Rect::default(),
        });

        Self {
            documents,
            editors,
            groups,
            layout: LayoutNode::Group(group_id),
            focused_group_id: group_id,
            next_document_id: 2,
            next_editor_id: 2,
            next_group_id: 2,
            next_tab_id: 2,
        }
    }
}
```

---

## Example Scenarios

### Split Current Editor Horizontally

```
Before:                          After:
┌────────────────────────┐       ┌───────────┬────────────┐
│ [main.rs]              │       │ [main.rs] │ [main.rs]  │
│                        │  ──►  │           │            │
│ fn main() {            │       │ fn main() │ fn main()  │
│     ...                │       │     ...   │     ...    │
└────────────────────────┘       └───────────┴────────────┘

- Creates new EditorGroup with same document
- New EditorState with independent cursor/scroll
- Layout changes from Group(1) to Split { children: [Group(1), Group(2)] }
```

### Open File in Split

```
User: Cmd+\ then opens lib.rs

┌────────────────────────┐       ┌───────────┬────────────┐
│ [main.rs]              │       │ [main.rs] │ [lib.rs]   │
│                        │  ──►  │           │            │
│ fn main() {            │       │ fn main() │ mod utils; │
└────────────────────────┘       └───────────┴────────────┘
```

### Edit Synchronized Across Views

```
┌───────────┬────────────┐
│ [main.rs] │ [main.rs]  │   Both views show same Document
│           │            │
│ fn foo()▌ │ fn foo()   │   Left view: cursor at line 1
│           │            │   Right view: scrolled to line 50
│           │            │
└───────────┴────────────┘

User types in left view:
- Document.buffer updated
- Left EditorState cursor advances
- Right EditorState notified, may need cursor adjustment
- Both views re-render
```
