//! Editor area - manages multiple editor panes, tabs, and split views
//!
//! This module implements a hierarchical layout system for multiple editor panes,
//! tabs, and split views, allowing the same document to be viewed in multiple places.

use std::collections::HashMap;

use super::document::Document;
use super::editor::{EditorState, ScrollRevealMode};
use crate::markdown::PreviewPane;

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
    /// Preview pane attached to this group (if any)
    pub attached_preview: Option<PreviewId>,
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

/// Unique identifier for a preview pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PreviewId(pub u64);

/// A node in the layout tree - either a group, a split container, or a preview pane
#[derive(Debug, Clone, Default)]
pub enum LayoutNode {
    #[default]
    Empty,
    Group(GroupId),
    Split(SplitContainer),
    Preview(PreviewId),
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

    /// All preview panes
    pub previews: HashMap<PreviewId, PreviewPane>,

    /// The layout tree root
    pub layout: LayoutNode,

    /// Currently focused group
    pub focused_group_id: GroupId,

    /// ID generators
    next_document_id: u64,
    next_editor_id: u64,
    next_group_id: u64,
    next_tab_id: u64,
    next_preview_id: u64,

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
                attached_preview: None,
            },
        );

        Self {
            documents,
            editors,
            groups,
            previews: HashMap::new(),
            layout: LayoutNode::Group(group_id),
            focused_group_id: group_id,
            next_document_id: 2,
            next_editor_id: 2,
            next_group_id: 2,
            next_tab_id: 2,
            next_preview_id: 1,
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

    /// Ensure the focused editor's cursor is visible without applying scroll padding.
    /// Use for mouse clicks where the clicked position is already on screen.
    pub fn ensure_focused_cursor_visible_no_padding(&mut self) {
        let doc_id = match self.focused_document_id() {
            Some(id) => id,
            None => return,
        };
        let editor_id = match self.focused_editor_id() {
            Some(id) => id,
            None => return,
        };

        let doc_ptr = self.documents.get(&doc_id).unwrap() as *const Document;
        let editor = self.editors.get_mut(&editor_id).unwrap();
        let doc = unsafe { &*doc_ptr };
        editor.ensure_cursor_visible_no_padding(doc);
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

    /// Generate a new preview ID
    pub fn next_preview_id(&mut self) -> PreviewId {
        let id = PreviewId(self.next_preview_id);
        self.next_preview_id += 1;
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

    // =========================================================================
    // Preview Pane Management (Group-Centric)
    // =========================================================================

    /// Check if a preview is open for a document
    pub fn has_preview_for_document(&self, doc_id: DocumentId) -> bool {
        self.previews.values().any(|p| p.document_id == doc_id)
    }

    /// Find preview pane for a document
    pub fn find_preview_for_document(&self, doc_id: DocumentId) -> Option<PreviewId> {
        self.previews
            .iter()
            .find(|(_, p)| p.document_id == doc_id)
            .map(|(id, _)| *id)
    }

    /// Find preview attached to a group
    pub fn find_preview_for_group(&self, group_id: GroupId) -> Option<PreviewId> {
        self.groups.get(&group_id)?.attached_preview
    }

    /// Get preview for the focused group
    pub fn focused_group_preview(&self) -> Option<PreviewId> {
        self.find_preview_for_group(self.focused_group_id)
    }

    /// Get preview pane by ID
    pub fn preview(&self, id: PreviewId) -> Option<&PreviewPane> {
        self.previews.get(&id)
    }

    /// Get preview pane mutably by ID
    pub fn preview_mut(&mut self, id: PreviewId) -> Option<&mut PreviewPane> {
        self.previews.get_mut(&id)
    }

    /// Get mutable preview for a group
    pub fn preview_for_group_mut(&mut self, group_id: GroupId) -> Option<&mut PreviewPane> {
        let preview_id = self.groups.get(&group_id)?.attached_preview?;
        self.previews.get_mut(&preview_id)
    }

    /// Open preview for the focused group's active document
    pub fn open_preview_for_focused_group(&mut self) -> Option<PreviewId> {
        let group_id = self.focused_group_id;
        self.open_preview_for_group(group_id)
    }

    /// Open preview for a specific group's active document
    pub fn open_preview_for_group(&mut self, group_id: GroupId) -> Option<PreviewId> {
        let group = self.groups.get(&group_id)?;
        let editor_id = group.active_editor_id()?;
        let editor = self.editors.get(&editor_id)?;
        let doc_id = editor.document_id?;

        // If a preview is already attached to this group, retarget it
        if let Some(existing_pid) = group.attached_preview {
            if let Some(preview) = self.previews.get_mut(&existing_pid) {
                preview.document_id = doc_id;
                preview.last_revision = 0;
                preview.scroll_offset = 0;
            }
            return Some(existing_pid);
        }

        // Create new preview pane
        let preview_id = self.next_preview_id();
        let preview = PreviewPane::new(preview_id, doc_id, group_id);
        self.previews.insert(preview_id, preview);

        // Attach to group
        if let Some(group) = self.groups.get_mut(&group_id) {
            group.attached_preview = Some(preview_id);
        }

        // Create a horizontal split with the group on the left and preview on the right
        self.wrap_group_with_preview(group_id, preview_id);

        Some(preview_id)
    }

    /// Close a preview pane and remove it from layout
    pub fn close_preview(&mut self, preview_id: PreviewId) {
        // Get group_id before removing
        let group_id = self.previews.get(&preview_id).map(|p| p.group_id);

        // Remove from previews map
        self.previews.remove(&preview_id);

        // Detach from group
        if let Some(gid) = group_id {
            if let Some(group) = self.groups.get_mut(&gid) {
                if group.attached_preview == Some(preview_id) {
                    group.attached_preview = None;
                }
            }
        }

        // Remove from layout
        self.remove_preview_from_layout(preview_id);
    }

    /// Toggle preview for the focused group
    /// Returns true if preview is now open, false if closed
    pub fn toggle_focused_preview(&mut self) -> bool {
        let group_id = self.focused_group_id;

        if let Some(preview_id) = self.find_preview_for_group(group_id) {
            self.close_preview(preview_id);
            false
        } else {
            self.open_preview_for_focused_group();
            true
        }
    }

    /// Called when the active tab changes in a group.
    /// Updates the preview to show the new document if it supports preview,
    /// otherwise closes the preview.
    pub fn on_group_active_tab_changed(&mut self, group_id: GroupId) {
        let preview_id = match self.find_preview_for_group(group_id) {
            Some(id) => id,
            None => return,
        };

        // Get the new active document
        let group = match self.groups.get(&group_id) {
            Some(g) => g,
            None => return,
        };
        let editor_id = match group.active_editor_id() {
            Some(id) => id,
            None => return,
        };
        let new_doc_id = match self.editors.get(&editor_id).and_then(|e| e.document_id) {
            Some(id) => id,
            None => return,
        };

        // Get the preview's document
        let preview_doc_id = match self.previews.get(&preview_id) {
            Some(p) => p.document_id,
            None => return,
        };

        // If the document changed, either retarget or close the preview
        if new_doc_id != preview_doc_id {
            // Check if the new document supports preview
            let supports_preview = self
                .documents
                .get(&new_doc_id)
                .map(|doc| doc.language.supports_preview())
                .unwrap_or(false);

            if supports_preview {
                // Retarget the preview to the new document
                if let Some(preview) = self.previews.get_mut(&preview_id) {
                    preview.document_id = new_doc_id;
                    preview.last_revision = 0; // Force refresh
                    preview.scroll_offset = 0;
                }
            } else {
                // Close the preview for unsupported file types
                self.close_preview(preview_id);
            }
        }
    }

    /// Close all previews for a document (called when document is closed)
    pub fn close_previews_for_document(&mut self, doc_id: DocumentId) {
        let preview_ids: Vec<PreviewId> = self
            .previews
            .iter()
            .filter_map(|(&pid, p)| {
                if p.document_id == doc_id {
                    Some(pid)
                } else {
                    None
                }
            })
            .collect();

        for pid in preview_ids {
            self.close_preview(pid);
        }
    }

    /// Find which group contains a document
    #[allow(dead_code)]
    fn find_group_for_document(&self, doc_id: DocumentId) -> Option<GroupId> {
        for (group_id, group) in &self.groups {
            for tab in &group.tabs {
                if let Some(editor) = self.editors.get(&tab.editor_id) {
                    if editor.document_id == Some(doc_id) {
                        return Some(*group_id);
                    }
                }
            }
        }
        None
    }

    /// Wrap a group in a horizontal split with a preview pane
    fn wrap_group_with_preview(&mut self, group_id: GroupId, preview_id: PreviewId) {
        let layout = std::mem::take(&mut self.layout);
        self.layout = replace_group_with_split(layout, group_id, preview_id);
    }

    /// Remove a preview pane from the layout, collapsing splits if needed
    fn remove_preview_from_layout(&mut self, preview_id: PreviewId) {
        let fallback_group = self.focused_group_id;
        let layout = std::mem::take(&mut self.layout);
        self.layout = remove_preview_node(layout, preview_id, fallback_group);
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
    pub fn sync_all_viewports(
        &mut self,
        line_height: usize,
        char_width: f32,
        tab_bar_height: usize,
    ) {
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
            // Subtract tab_bar_height because group rect includes the tab bar area,
            // but visible_lines should only count the text content area.
            let visible_lines =
                ViewportGeometry::compute_visible_lines(height, line_height, tab_bar_height);
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
            LayoutNode::Empty => {}
            LayoutNode::Group(group_id) => {
                if let Some(group) = self.groups.get_mut(group_id) {
                    group.rect = rect;
                }
            }
            LayoutNode::Preview(preview_id) => {
                if let Some(preview) = self.previews.get_mut(preview_id) {
                    preview.rect = rect;
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
            LayoutNode::Empty => None,
            LayoutNode::Group(group_id) => {
                if let Some(group) = self.groups.get(group_id) {
                    if group.rect.contains(x, y) {
                        return Some(*group_id);
                    }
                }
                None
            }
            LayoutNode::Preview(_) => {
                // Preview panes don't contain groups
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

    /// Find preview pane at a given point
    pub fn preview_at_point(&self, x: f32, y: f32) -> Option<PreviewId> {
        self.preview_at_point_node(&self.layout, x, y)
    }

    /// Recursively search for preview at point
    fn preview_at_point_node(&self, node: &LayoutNode, x: f32, y: f32) -> Option<PreviewId> {
        match node {
            LayoutNode::Empty => None,
            LayoutNode::Group(_) => None,
            LayoutNode::Preview(preview_id) => {
                if let Some(preview) = self.previews.get(preview_id) {
                    if preview.rect.contains(x, y) {
                        return Some(*preview_id);
                    }
                }
                None
            }
            LayoutNode::Split(container) => {
                for child in &container.children {
                    if let Some(id) = self.preview_at_point_node(child, x, y) {
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

// ============================================================================
// Layout Helper Functions (standalone to avoid borrow issues)
// ============================================================================

/// Recursively find a group node and replace it with a split containing a preview
fn replace_group_with_split(
    node: LayoutNode,
    target_group: GroupId,
    preview_id: PreviewId,
) -> LayoutNode {
    match node {
        LayoutNode::Empty => node,
        LayoutNode::Group(gid) if gid == target_group => LayoutNode::Split(SplitContainer {
            direction: SplitDirection::Horizontal,
            children: vec![LayoutNode::Group(gid), LayoutNode::Preview(preview_id)],
            ratios: vec![0.5, 0.5],
            min_sizes: vec![200.0, 200.0],
        }),
        LayoutNode::Group(_) => node,
        LayoutNode::Preview(_) => node,
        LayoutNode::Split(mut container) => {
            container.children = container
                .children
                .into_iter()
                .map(|child| replace_group_with_split(child, target_group, preview_id))
                .collect();
            LayoutNode::Split(container)
        }
    }
}

/// Recursively remove a preview node, collapsing single-child splits
fn remove_preview_node(node: LayoutNode, target: PreviewId, fallback_group: GroupId) -> LayoutNode {
    match node {
        LayoutNode::Empty => node,
        LayoutNode::Group(_) => node,
        LayoutNode::Preview(pid) if pid == target => {
            // Return empty - parent will clean this up
            LayoutNode::Empty
        }
        LayoutNode::Preview(_) => node,
        LayoutNode::Split(mut container) => {
            // Remove the target preview from children
            container
                .children
                .retain(|child| !matches!(child, LayoutNode::Preview(pid) if *pid == target));

            // Recursively process remaining children
            container.children = container
                .children
                .into_iter()
                .map(|child| remove_preview_node(child, target, fallback_group))
                .filter(|child| !matches!(child, LayoutNode::Empty))
                .collect();

            // Adjust ratios
            if !container.children.is_empty() {
                let ratio = 1.0 / container.children.len() as f32;
                container.ratios = vec![ratio; container.children.len()];
                container.min_sizes = vec![200.0; container.children.len()];
            }

            // Collapse if only one child remains
            if container.children.len() == 1 {
                container.children.pop().unwrap()
            } else if container.children.is_empty() {
                LayoutNode::Group(fallback_group)
            } else {
                LayoutNode::Split(container)
            }
        }
    }
}
