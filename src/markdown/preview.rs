//! Markdown preview pane state

use crate::model::editor_area::{DocumentId, GroupId, PreviewId, Rect};

/// State for a markdown preview pane
#[derive(Debug, Clone)]
pub struct PreviewPane {
    pub id: PreviewId,
    pub document_id: DocumentId,
    /// The group this preview is attached to
    pub group_id: GroupId,
    pub rendered_html: String,
    pub rendered_lines: Vec<RenderedLine>,
    pub scroll_offset: usize,
    pub scroll_sync_enabled: bool,
    pub rect: Rect,
    pub last_revision: u64,
}

/// A rendered line in the preview (for native text rendering)
#[derive(Debug, Clone)]
pub struct RenderedLine {
    pub segments: Vec<StyledSegment>,
    pub source_line: usize,
}

/// A styled segment of text
#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub text: String,
    pub style: MarkdownStyle,
}

/// Style variants for markdown elements
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkdownStyle {
    Normal,
    Heading1,
    Heading2,
    Heading3,
    Heading4,
    Heading5,
    Heading6,
    Bold,
    Italic,
    BoldItalic,
    Code,
    CodeBlock,
    Link,
    ListBullet,
    ListNumber,
    BlockquoteMarker,
    BlockquoteText,
    HorizontalRule,
}

impl PreviewPane {
    pub fn new(id: PreviewId, document_id: DocumentId, group_id: GroupId) -> Self {
        Self {
            id,
            document_id,
            group_id,
            rendered_html: String::new(),
            rendered_lines: Vec::new(),
            scroll_offset: 0,
            scroll_sync_enabled: true,
            rect: Rect::default(),
            last_revision: 0,
        }
    }

    pub fn needs_refresh(&self, document_revision: u64) -> bool {
        self.last_revision != document_revision
    }
}
