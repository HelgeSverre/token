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
    // TODO: is_pinned is currently unused. Implement pinned tab functionality
    // to prevent accidental closure of important tabs.
    pub is_pinned: bool,
    // TODO: is_preview is currently unused. Implement preview tab behavior
    // where opening a new file replaces the preview tab instead of creating a new one.
    pub is_preview: bool,
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
    // TODO: min_sizes is currently not enforced in compute_layout_node().
    // Implement enforcement to prevent panes from being resized below usable size.
    // This is a UX enhancement, not a crash risk (Rust bounds checking handles edge cases).
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

    /// Counter for generating unique untitled document names
    next_untitled_number: u32,

    /// Last layout rect used for compute_layout (for splitter drag calculations)
    pub last_layout_rect: Option<Rect>,
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
            next_untitled_number: 1,
            last_layout_rect: None,
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

    /// Check if a group is the currently focused group
    #[inline]
    pub fn is_group_focused(&self, group_id: GroupId) -> bool {
        self.focused_group_id == group_id
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

    /// Generate the next untitled document name (e.g., "Untitled", "Untitled-2", etc.)
    pub fn next_untitled_name(&mut self) -> String {
        let n = self.next_untitled_number;
        self.next_untitled_number += 1;
        if n == 1 {
            "Untitled".to_string()
        } else {
            format!("Untitled-{}", n)
        }
    }

    /// Get all editor IDs that are viewing a specific document
    pub fn editors_for_document(&self, doc_id: DocumentId) -> Vec<EditorId> {
        self.editors
            .iter()
            .filter(|(_, editor)| editor.document_id == Some(doc_id))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Find if a file is already open by its path
    /// Returns the document ID and group/tab info if found
    pub fn find_open_file(&self, path: &std::path::Path) -> Option<(DocumentId, GroupId, usize)> {
        // Canonicalize the input path for comparison
        let canonical_path = path.canonicalize().ok()?;

        for (doc_id, doc) in &self.documents {
            if let Some(ref doc_path) = doc.file_path {
                if let Ok(doc_canonical) = doc_path.canonicalize() {
                    if doc_canonical == canonical_path {
                        // Find which group/tab has this document
                        for (group_id, group) in &self.groups {
                            for (tab_idx, tab) in group.tabs.iter().enumerate() {
                                if let Some(editor) = self.editors.get(&tab.editor_id) {
                                    if editor.document_id == Some(*doc_id) {
                                        return Some((*doc_id, *group_id, tab_idx));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }

    /// Check if a file is already open (quick check without returning details)
    pub fn is_file_open(&self, path: &std::path::Path) -> bool {
        self.find_open_file(path).is_some()
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
    ///
    /// Uses `ViewportGeometry` methods for canonical calculations to ensure
    /// consistent viewport sizing across the codebase.
    pub fn sync_all_viewports(&mut self, line_height: usize, char_width: f32) {
        use super::ViewportGeometry;

        // Collect group rects and their editor IDs
        let group_info: Vec<(Vec<EditorId>, u32, u32)> = self
            .groups
            .values()
            .map(|group| {
                let editor_ids: Vec<EditorId> = group.tabs.iter().map(|t| t.editor_id).collect();
                (
                    editor_ids,
                    group.rect.width as u32,
                    group.rect.height as u32,
                )
            })
            .collect();

        // Update each editor's viewport based on its group's dimensions
        for (editor_ids, width, height) in group_info {
            // Use canonical ViewportGeometry methods for consistent calculations
            let visible_lines = ViewportGeometry::compute_visible_lines(height, line_height, 0);
            let visible_columns = ViewportGeometry::compute_visible_columns(width, char_width);

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
    /// Uses the default SPLITTER_WIDTH constant.
    pub fn compute_layout(&mut self, available: Rect) -> Vec<SplitterBar> {
        self.compute_layout_scaled(available, SPLITTER_WIDTH)
    }

    /// Compute layout with a custom splitter width (for HiDPI scaling).
    pub fn compute_layout_scaled(
        &mut self,
        available: Rect,
        splitter_width: f32,
    ) -> Vec<SplitterBar> {
        // Store the rect for splitter drag calculations
        self.last_layout_rect = Some(available);

        let mut splitters = Vec::new();
        self.compute_layout_node(
            &self.layout.clone(),
            available,
            &mut splitters,
            splitter_width,
        );
        splitters
    }

    /// Recursively compute layout for a node
    fn compute_layout_node(
        &mut self,
        node: &LayoutNode,
        rect: Rect,
        splitters: &mut Vec<SplitterBar>,
        splitter_width: f32,
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
                                    rect.x + offset + child_size - splitter_width / 2.0,
                                    rect.y,
                                    splitter_width,
                                    rect.height,
                                ),
                                index: i,
                            },
                            SplitDirection::Vertical => SplitterBar {
                                direction: container.direction,
                                rect: Rect::new(
                                    rect.x,
                                    rect.y + offset + child_size - splitter_width / 2.0,
                                    rect.width,
                                    splitter_width,
                                ),
                                index: i,
                            },
                        };
                        splitters.push(splitter);
                    }

                    // Recursively layout child
                    self.compute_layout_node(child, child_rect, splitters, splitter_width);

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
// Debug Invariant Validation
// ============================================================================

impl EditorArea {
    /// Validate internal invariants in debug builds.
    ///
    /// This function checks that:
    /// - focused_group_id points to an existing group
    /// - All groups have valid active_tab_index values
    /// - All tabs reference existing editors
    /// - All editors reference existing documents (if document_id is Some)
    ///
    /// Panics in debug builds if any invariant is violated.
    #[cfg(debug_assertions)]
    pub fn assert_invariants(&self) {
        // Check focused group exists
        assert!(
            self.groups.contains_key(&self.focused_group_id),
            "focused_group_id {:?} does not exist in groups",
            self.focused_group_id
        );

        // Check each group
        for (group_id, group) in &self.groups {
            // Check active_tab_index is valid
            if !group.tabs.is_empty() {
                assert!(
                    group.active_tab_index < group.tabs.len(),
                    "Group {:?} has active_tab_index {} but only {} tabs",
                    group_id,
                    group.active_tab_index,
                    group.tabs.len()
                );
            }

            // Check each tab references a valid editor
            for tab in &group.tabs {
                assert!(
                    self.editors.contains_key(&tab.editor_id),
                    "Tab {:?} references non-existent editor {:?}",
                    tab.id,
                    tab.editor_id
                );
            }
        }

        // Check each editor references a valid document (if document_id is Some)
        for (editor_id, editor) in &self.editors {
            if let Some(doc_id) = editor.document_id {
                assert!(
                    self.documents.contains_key(&doc_id),
                    "Editor {:?} references non-existent document {:?}",
                    editor_id,
                    doc_id
                );
            }
        }
    }

    /// No-op in release builds
    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn assert_invariants(&self) {}
}
