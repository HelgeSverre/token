//! Editor area - manages multiple editor panes, tabs, and split views
//!
//! This module implements a hierarchical layout system for multiple editor panes,
//! tabs, and split views, allowing the same document to be viewed in multiple places.

use std::collections::HashMap;

use super::document::Document;
use super::editor::{EditorState, ScrollRevealMode};

// ============================================================================
// Identifiers
// ============================================================================

/// Unique identifier for a document
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DocumentId(pub u64);

/// Unique identifier for an editor view
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditorId(pub u64);

/// Unique identifier for an editor group (pane)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupId(pub u64);

/// Unique identifier for a tab
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TabId(pub u64);

// ============================================================================
// Layout Primitives
// ============================================================================

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
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
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
    pub is_preview: bool, // Preview tabs get replaced on next file open
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
    /// Layout info (set by parent during layout computation)
    pub rect: Rect,
}

impl EditorGroup {
    /// Get the currently active tab
    pub fn active_tab(&self) -> Option<&Tab> {
        self.tabs.get(self.active_tab_index)
    }

    /// Get the editor ID of the active tab
    pub fn active_editor_id(&self) -> Option<EditorId> {
        self.active_tab().map(|t| t.editor_id)
    }
}

// ============================================================================
// Layout Tree
// ============================================================================

/// Direction for splitting editor groups
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Children arranged left-to-right
    Horizontal,
    /// Children arranged top-to-bottom
    Vertical,
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

/// A node in the layout tree - either a group or a split container
#[derive(Debug, Clone)]
pub enum LayoutNode {
    Group(GroupId),
    Split(SplitContainer),
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

impl EditorArea {
    /// Create a new editor area with a single document and editor.
    /// This is the migration path from the old single-pane architecture.
    pub fn single_document(mut document: Document, mut editor: EditorState) -> Self {
        let doc_id = DocumentId(1);
        let editor_id = EditorId(1);
        let group_id = GroupId(1);
        let tab_id = TabId(1);

        // Assign IDs to document and editor
        document.id = Some(doc_id);
        editor.id = Some(editor_id);
        editor.document_id = Some(doc_id);

        let mut documents = HashMap::new();
        let mut editors = HashMap::new();
        let mut groups = HashMap::new();

        documents.insert(doc_id, document);
        editors.insert(editor_id, editor);

        let tab = Tab {
            id: tab_id,
            editor_id,
            is_pinned: false,
            is_preview: false,
        };

        groups.insert(
            group_id,
            EditorGroup {
                id: group_id,
                tabs: vec![tab],
                active_tab_index: 0,
                rect: Rect::default(),
            },
        );

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

    /// Get the currently focused group
    pub fn focused_group(&self) -> Option<&EditorGroup> {
        self.groups.get(&self.focused_group_id)
    }

    /// Get the currently focused group mutably
    pub fn focused_group_mut(&mut self) -> Option<&mut EditorGroup> {
        self.groups.get_mut(&self.focused_group_id)
    }

    /// Get the editor ID of the focused group's active tab
    pub fn focused_editor_id(&self) -> Option<EditorId> {
        self.focused_group().and_then(|g| g.active_editor_id())
    }

    /// Get the focused editor state
    pub fn focused_editor(&self) -> Option<&EditorState> {
        self.focused_editor_id()
            .and_then(|id| self.editors.get(&id))
    }

    /// Get the focused editor state mutably
    pub fn focused_editor_mut(&mut self) -> Option<&mut EditorState> {
        let editor_id = self.focused_editor_id()?;
        self.editors.get_mut(&editor_id)
    }

    /// Get the document ID of the focused editor
    pub fn focused_document_id(&self) -> Option<DocumentId> {
        self.focused_editor().and_then(|e| e.document_id)
    }

    /// Get the focused document
    pub fn focused_document(&self) -> Option<&Document> {
        self.focused_document_id()
            .and_then(|id| self.documents.get(&id))
    }

    /// Get the focused document mutably
    pub fn focused_document_mut(&mut self) -> Option<&mut Document> {
        let doc_id = self.focused_document_id()?;
        self.documents.get_mut(&doc_id)
    }

    /// Ensure the focused editor's cursor is visible.
    /// This method works around borrow checker issues by getting both doc and editor
    /// within the same scope.
    pub fn ensure_focused_cursor_visible(&mut self, mode: ScrollRevealMode) {
        let doc_id = match self.focused_document_id() {
            Some(id) => id,
            None => return,
        };
        let editor_id = match self.focused_editor_id() {
            Some(id) => id,
            None => return,
        };

        // Get raw pointers to work around borrow checker
        // Safety: We only read from doc while mutating editor, and they don't overlap
        let doc_ptr = self.documents.get(&doc_id).unwrap() as *const Document;
        let editor = self.editors.get_mut(&editor_id).unwrap();
        let doc = unsafe { &*doc_ptr };
        editor.ensure_cursor_visible_with_mode(doc, mode);
    }

    /// Generate a new document ID
    pub fn next_document_id(&mut self) -> DocumentId {
        let id = DocumentId(self.next_document_id);
        self.next_document_id += 1;
        id
    }

    /// Generate a new editor ID
    pub fn next_editor_id(&mut self) -> EditorId {
        let id = EditorId(self.next_editor_id);
        self.next_editor_id += 1;
        id
    }

    /// Generate a new group ID
    pub fn next_group_id(&mut self) -> GroupId {
        let id = GroupId(self.next_group_id);
        self.next_group_id += 1;
        id
    }

    /// Generate a new tab ID
    pub fn next_tab_id(&mut self) -> TabId {
        let id = TabId(self.next_tab_id);
        self.next_tab_id += 1;
        id
    }

    /// Get all editor IDs that are viewing a specific document
    pub fn editors_for_document(&self, doc_id: DocumentId) -> Vec<EditorId> {
        self.editors
            .iter()
            .filter(|(_, editor)| editor.document_id == Some(doc_id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Adjust cursors in all editors (except the specified one) viewing the same document
    /// after an edit operation.
    ///
    /// - `exclude_editor`: The editor that performed the edit (already has correct cursors)
    /// - `doc_id`: The document that was edited
    /// - `edit_line`: The line where the edit occurred
    /// - `edit_column`: The column where the edit occurred
    /// - `lines_delta`: Change in line count (positive = lines added, negative = lines removed)
    /// - `column_delta`: Change in column on the edit line (for same-line edits)
    pub fn adjust_other_editors_cursors(
        &mut self,
        exclude_editor: EditorId,
        doc_id: DocumentId,
        edit_line: usize,
        edit_column: usize,
        lines_delta: isize,
        column_delta: isize,
    ) {
        let editor_ids = self.editors_for_document(doc_id);

        for editor_id in editor_ids {
            if editor_id == exclude_editor {
                continue;
            }

            if let Some(editor) = self.editors.get_mut(&editor_id) {
                for (cursor, selection) in
                    editor.cursors.iter_mut().zip(editor.selections.iter_mut())
                {
                    // Adjust cursor
                    adjust_position_for_edit(
                        &mut cursor.line,
                        &mut cursor.column,
                        edit_line,
                        edit_column,
                        lines_delta,
                        column_delta,
                    );

                    // Adjust selection anchor
                    adjust_position_for_edit(
                        &mut selection.anchor.line,
                        &mut selection.anchor.column,
                        edit_line,
                        edit_column,
                        lines_delta,
                        column_delta,
                    );

                    // Adjust selection head
                    adjust_position_for_edit(
                        &mut selection.head.line,
                        &mut selection.head.column,
                        edit_line,
                        edit_column,
                        lines_delta,
                        column_delta,
                    );
                }
            }
        }
    }

    /// Sync all editor viewports based on their group's rect.
    /// Should be called after compute_layout() or after split/close operations.
    pub fn sync_all_viewports(&mut self, line_height: usize, char_width: f32) {
        // Collect group rects and their editor IDs
        let group_info: Vec<(Vec<EditorId>, f32, f32)> = self
            .groups
            .values()
            .map(|group| {
                let editor_ids: Vec<EditorId> = group.tabs.iter().map(|t| t.editor_id).collect();
                (editor_ids, group.rect.width, group.rect.height)
            })
            .collect();

        // Update each editor's viewport based on its group's dimensions
        for (editor_ids, width, height) in group_info {
            // Calculate visible lines from group height
            let visible_lines = if line_height > 0 {
                (height as usize) / line_height
            } else {
                25 // fallback
            };

            // Calculate visible columns from group width
            // Account for gutter/line numbers (estimate ~50px)
            let gutter_width = 50.0;
            let available_width = (width - gutter_width).max(0.0);
            let visible_columns = if char_width > 0.0 {
                (available_width / char_width).floor() as usize
            } else {
                80 // fallback
            };

            for editor_id in editor_ids {
                if let Some(editor) = self.editors.get_mut(&editor_id) {
                    editor.resize_viewport(visible_lines, visible_columns);
                }
            }
        }
    }

    /// Compute layout for all groups given the available rectangle.
    /// Updates the `rect` field of each EditorGroup.
    /// Returns a list of splitter bar positions for rendering/hit testing.
    pub fn compute_layout(&mut self, available: Rect) -> Vec<SplitterBar> {
        let mut splitters = Vec::new();
        self.compute_layout_node(&self.layout.clone(), available, &mut splitters);
        splitters
    }

    /// Recursively compute layout for a node
    fn compute_layout_node(
        &mut self,
        node: &LayoutNode,
        rect: Rect,
        splitters: &mut Vec<SplitterBar>,
    ) {
        match node {
            LayoutNode::Group(group_id) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.rect = rect;
                }
            }
            LayoutNode::Split(container) => {
                let children = &container.children;
                let ratios = &container.ratios;

                if children.is_empty() {
                    return;
                }

                // Calculate child rects based on direction and ratios
                let mut offset = 0.0;
                let total_size = match container.direction {
                    SplitDirection::Horizontal => rect.width,
                    SplitDirection::Vertical => rect.height,
                };

                for (i, child) in children.iter().enumerate() {
                    let ratio = ratios
                        .get(i)
                        .copied()
                        .unwrap_or(1.0 / children.len() as f32);
                    let child_size = total_size * ratio;

                    let child_rect = match container.direction {
                        SplitDirection::Horizontal => {
                            Rect::new(rect.x + offset, rect.y, child_size, rect.height)
                        }
                        SplitDirection::Vertical => {
                            Rect::new(rect.x, rect.y + offset, rect.width, child_size)
                        }
                    };

                    // Add splitter bar between children (not after last child)
                    if i < children.len() - 1 {
                        let splitter = match container.direction {
                            SplitDirection::Horizontal => SplitterBar {
                                direction: container.direction,
                                rect: Rect::new(
                                    rect.x + offset + child_size - SPLITTER_WIDTH / 2.0,
                                    rect.y,
                                    SPLITTER_WIDTH,
                                    rect.height,
                                ),
                                index: i,
                            },
                            SplitDirection::Vertical => SplitterBar {
                                direction: container.direction,
                                rect: Rect::new(
                                    rect.x,
                                    rect.y + offset + child_size - SPLITTER_WIDTH / 2.0,
                                    rect.width,
                                    SPLITTER_WIDTH,
                                ),
                                index: i,
                            },
                        };
                        splitters.push(splitter);
                    }

                    // Recursively layout child
                    self.compute_layout_node(child, child_rect, splitters);

                    offset += child_size;
                }
            }
        }
    }

    /// Find the group at a given point (for mouse clicks)
    pub fn group_at_point(&self, x: f32, y: f32) -> Option<GroupId> {
        self.group_at_point_node(&self.layout, x, y)
    }

    /// Recursively search for group at point
    fn group_at_point_node(&self, node: &LayoutNode, x: f32, y: f32) -> Option<GroupId> {
        match node {
            LayoutNode::Group(group_id) => {
                if let Some(group) = self.groups.get(group_id) {
                    if group.rect.contains(x, y) {
                        return Some(*group_id);
                    }
                }
                None
            }
            LayoutNode::Split(container) => {
                for child in &container.children {
                    if let Some(id) = self.group_at_point_node(child, x, y) {
                        return Some(id);
                    }
                }
                None
            }
        }
    }

    /// Find splitter bar at a given point (for drag handling)
    pub fn splitter_at_point(&self, splitters: &[SplitterBar], x: f32, y: f32) -> Option<usize> {
        for (i, splitter) in splitters.iter().enumerate() {
            if splitter.rect.contains(x, y) {
                return Some(i);
            }
        }
        None
    }
}

// ============================================================================
// Layout Constants and Types
// ============================================================================

/// Width of splitter bars in pixels
pub const SPLITTER_WIDTH: f32 = 6.0;

/// Adjust a cursor/selection position based on an edit that occurred.
///
/// This is used to synchronize cursors across multiple views of the same document.
/// When an edit happens at (edit_line, edit_column):
/// - If the cursor is before the edit point: no change
/// - If the cursor is on the same line, at or after the edit column: adjust column
/// - If the cursor is on a later line: adjust line number
fn adjust_position_for_edit(
    pos_line: &mut usize,
    pos_column: &mut usize,
    edit_line: usize,
    edit_column: usize,
    lines_delta: isize,
    column_delta: isize,
) {
    if *pos_line < edit_line {
        // Position is before the edit line - no adjustment needed
        return;
    }

    if *pos_line == edit_line {
        // Same line as edit
        if *pos_column >= edit_column {
            // At or after edit column - adjust column
            if column_delta >= 0 {
                *pos_column = pos_column.saturating_add(column_delta as usize);
            } else {
                *pos_column = pos_column.saturating_sub((-column_delta) as usize);
            }

            // If lines were added/removed, we might need to adjust both
            if lines_delta > 0 {
                // Newline was inserted - cursor moves to new line
                // Column becomes: pos_column - edit_column (position on new line)
                *pos_line = pos_line.saturating_add(lines_delta as usize);
                *pos_column = pos_column.saturating_sub(edit_column);
            } else if lines_delta < 0 {
                // Lines were joined - no line adjustment for same-line positions
            }
        }
        // Before edit column on same line - no adjustment
    } else {
        // Position is on a line after the edit line
        if lines_delta >= 0 {
            *pos_line = pos_line.saturating_add(lines_delta as usize);
        } else {
            *pos_line = pos_line.saturating_sub((-lines_delta) as usize);
        }
    }
}

/// Represents a draggable splitter bar between editor groups
#[derive(Debug, Clone, Copy)]
pub struct SplitterBar {
    /// Direction of the split this bar controls
    pub direction: SplitDirection,
    /// The hit-testing rectangle for this splitter
    pub rect: Rect,
    /// Index of this splitter within its parent container
    pub index: usize,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_editor_area() -> EditorArea {
        let document = Document::new();
        let editor = EditorState::new();
        EditorArea::single_document(document, editor)
    }

    #[test]
    fn test_rect_contains() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);

        // Inside
        assert!(rect.contains(50.0, 40.0));
        assert!(rect.contains(10.0, 20.0)); // Top-left corner

        // Outside
        assert!(!rect.contains(5.0, 40.0)); // Left of rect
        assert!(!rect.contains(150.0, 40.0)); // Right of rect
        assert!(!rect.contains(50.0, 10.0)); // Above rect
        assert!(!rect.contains(50.0, 80.0)); // Below rect

        // Edge cases (exclusive upper bounds)
        assert!(!rect.contains(110.0, 40.0)); // At right edge
        assert!(!rect.contains(50.0, 70.0)); // At bottom edge
    }

    #[test]
    fn test_single_group_layout() {
        let mut area = create_test_editor_area();
        let available = Rect::new(0.0, 0.0, 800.0, 600.0);

        let splitters = area.compute_layout(available);

        // Single group = no splitters
        assert!(splitters.is_empty());

        // Group should occupy entire area
        let group = area.focused_group().unwrap();
        assert_eq!(group.rect.x, 0.0);
        assert_eq!(group.rect.y, 0.0);
        assert_eq!(group.rect.width, 800.0);
        assert_eq!(group.rect.height, 600.0);
    }

    #[test]
    fn test_group_at_point_single() {
        let mut area = create_test_editor_area();
        let available = Rect::new(0.0, 0.0, 800.0, 600.0);
        area.compute_layout(available);

        // Point inside should find the group
        let group_id = area.group_at_point(400.0, 300.0);
        assert!(group_id.is_some());
        assert_eq!(group_id.unwrap(), area.focused_group_id);

        // Point outside should find nothing
        let outside = area.group_at_point(900.0, 300.0);
        assert!(outside.is_none());
    }

    #[test]
    fn test_horizontal_split_layout() {
        let mut area = create_test_editor_area();

        // Create a second group
        let group2_id = area.next_group_id();
        let doc_id = area.focused_document_id().unwrap();
        let editor2_id = area.next_editor_id();
        let tab2_id = area.next_tab_id();

        let mut editor2 = EditorState::new();
        editor2.id = Some(editor2_id);
        editor2.document_id = Some(doc_id);
        area.editors.insert(editor2_id, editor2);

        let tab2 = Tab {
            id: tab2_id,
            editor_id: editor2_id,
            is_pinned: false,
            is_preview: false,
        };

        area.groups.insert(
            group2_id,
            EditorGroup {
                id: group2_id,
                tabs: vec![tab2],
                active_tab_index: 0,
                rect: Rect::default(),
            },
        );

        // Create horizontal split
        let group1_id = area.focused_group_id;
        area.layout = LayoutNode::Split(SplitContainer {
            direction: SplitDirection::Horizontal,
            children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
            ratios: vec![0.5, 0.5],
            min_sizes: vec![100.0, 100.0],
        });

        let available = Rect::new(0.0, 0.0, 800.0, 600.0);
        let splitters = area.compute_layout(available);

        // Should have one splitter
        assert_eq!(splitters.len(), 1);
        assert_eq!(splitters[0].direction, SplitDirection::Horizontal);

        // Groups should be side by side
        let group1 = area.groups.get(&group1_id).unwrap();
        let group2 = area.groups.get(&group2_id).unwrap();

        assert_eq!(group1.rect.x, 0.0);
        assert_eq!(group1.rect.width, 400.0);

        assert_eq!(group2.rect.x, 400.0);
        assert_eq!(group2.rect.width, 400.0);
    }

    #[test]
    fn test_vertical_split_layout() {
        let mut area = create_test_editor_area();

        // Create a second group
        let group2_id = area.next_group_id();
        let doc_id = area.focused_document_id().unwrap();
        let editor2_id = area.next_editor_id();
        let tab2_id = area.next_tab_id();

        let mut editor2 = EditorState::new();
        editor2.id = Some(editor2_id);
        editor2.document_id = Some(doc_id);
        area.editors.insert(editor2_id, editor2);

        let tab2 = Tab {
            id: tab2_id,
            editor_id: editor2_id,
            is_pinned: false,
            is_preview: false,
        };

        area.groups.insert(
            group2_id,
            EditorGroup {
                id: group2_id,
                tabs: vec![tab2],
                active_tab_index: 0,
                rect: Rect::default(),
            },
        );

        // Create vertical split
        let group1_id = area.focused_group_id;
        area.layout = LayoutNode::Split(SplitContainer {
            direction: SplitDirection::Vertical,
            children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
            ratios: vec![0.5, 0.5],
            min_sizes: vec![100.0, 100.0],
        });

        let available = Rect::new(0.0, 0.0, 800.0, 600.0);
        let splitters = area.compute_layout(available);

        // Should have one splitter
        assert_eq!(splitters.len(), 1);
        assert_eq!(splitters[0].direction, SplitDirection::Vertical);

        // Groups should be stacked
        let group1 = area.groups.get(&group1_id).unwrap();
        let group2 = area.groups.get(&group2_id).unwrap();

        assert_eq!(group1.rect.y, 0.0);
        assert_eq!(group1.rect.height, 300.0);

        assert_eq!(group2.rect.y, 300.0);
        assert_eq!(group2.rect.height, 300.0);
    }

    #[test]
    fn test_group_at_point_split() {
        let mut area = create_test_editor_area();

        // Create a second group with horizontal split
        let group2_id = area.next_group_id();
        let doc_id = area.focused_document_id().unwrap();
        let editor2_id = area.next_editor_id();
        let tab2_id = area.next_tab_id();

        let mut editor2 = EditorState::new();
        editor2.id = Some(editor2_id);
        editor2.document_id = Some(doc_id);
        area.editors.insert(editor2_id, editor2);

        let tab2 = Tab {
            id: tab2_id,
            editor_id: editor2_id,
            is_pinned: false,
            is_preview: false,
        };

        area.groups.insert(
            group2_id,
            EditorGroup {
                id: group2_id,
                tabs: vec![tab2],
                active_tab_index: 0,
                rect: Rect::default(),
            },
        );

        let group1_id = area.focused_group_id;
        area.layout = LayoutNode::Split(SplitContainer {
            direction: SplitDirection::Horizontal,
            children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
            ratios: vec![0.5, 0.5],
            min_sizes: vec![100.0, 100.0],
        });

        let available = Rect::new(0.0, 0.0, 800.0, 600.0);
        area.compute_layout(available);

        // Left side should be group1
        assert_eq!(area.group_at_point(100.0, 300.0), Some(group1_id));

        // Right side should be group2
        assert_eq!(area.group_at_point(600.0, 300.0), Some(group2_id));
    }

    #[test]
    fn test_splitter_at_point() {
        let mut area = create_test_editor_area();

        // Create horizontal split
        let group2_id = area.next_group_id();
        let doc_id = area.focused_document_id().unwrap();
        let editor2_id = area.next_editor_id();
        let tab2_id = area.next_tab_id();

        let mut editor2 = EditorState::new();
        editor2.id = Some(editor2_id);
        editor2.document_id = Some(doc_id);
        area.editors.insert(editor2_id, editor2);

        let tab2 = Tab {
            id: tab2_id,
            editor_id: editor2_id,
            is_pinned: false,
            is_preview: false,
        };

        area.groups.insert(
            group2_id,
            EditorGroup {
                id: group2_id,
                tabs: vec![tab2],
                active_tab_index: 0,
                rect: Rect::default(),
            },
        );

        let group1_id = area.focused_group_id;
        area.layout = LayoutNode::Split(SplitContainer {
            direction: SplitDirection::Horizontal,
            children: vec![LayoutNode::Group(group1_id), LayoutNode::Group(group2_id)],
            ratios: vec![0.5, 0.5],
            min_sizes: vec![100.0, 100.0],
        });

        let available = Rect::new(0.0, 0.0, 800.0, 600.0);
        let splitters = area.compute_layout(available);

        // Splitter should be at x=400 (middle)
        assert!(area.splitter_at_point(&splitters, 400.0, 300.0).is_some());

        // Away from splitter should return None
        assert!(area.splitter_at_point(&splitters, 100.0, 300.0).is_none());
        assert!(area.splitter_at_point(&splitters, 600.0, 300.0).is_none());
    }
}
