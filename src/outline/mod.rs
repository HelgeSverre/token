//! Code outline extraction
//!
//! Provides structural symbol extraction from tree-sitter parse trees.
//! Used by the outline panel to show a collapsible tree of document symbols.

mod extract;

pub use extract::extract_outline;

/// Symbol kind for display and categorization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutlineKind {
    Heading { level: u8 },
    Module,
    Class,
    Struct,
    Enum,
    Interface,
    Trait,
    Function,
    Method,
    Property,
    Field,
    Constant,
    EnumVariant,
    Impl,
    Namespace,
}

impl OutlineKind {
    /// Short label for rendering in the outline tree
    pub fn label(&self) -> &'static str {
        match self {
            OutlineKind::Heading { level: 1 } => "H1",
            OutlineKind::Heading { level: 2 } => "H2",
            OutlineKind::Heading { level: 3 } => "H3",
            OutlineKind::Heading { level: 4 } => "H4",
            OutlineKind::Heading { level: 5 } => "H5",
            OutlineKind::Heading { level: 6 } => "H6",
            OutlineKind::Heading { .. } => "H?",
            OutlineKind::Module => "mod",
            OutlineKind::Class => "class",
            OutlineKind::Struct => "struct",
            OutlineKind::Enum => "enum",
            OutlineKind::Interface => "iface",
            OutlineKind::Trait => "trait",
            OutlineKind::Function => "fn",
            OutlineKind::Method => "fn",
            OutlineKind::Property => "prop",
            OutlineKind::Field => "field",
            OutlineKind::Constant => "const",
            OutlineKind::EnumVariant => "var",
            OutlineKind::Impl => "impl",
            OutlineKind::Namespace => "ns",
        }
    }
}

/// A range in the document (line/col are 0-based)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OutlineRange {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

/// A single node in the outline tree
#[derive(Debug, Clone)]
pub struct OutlineNode {
    pub kind: OutlineKind,
    pub name: String,
    pub range: OutlineRange,
    pub children: Vec<OutlineNode>,
}

impl OutlineNode {
    /// Whether this node has children (can be expanded/collapsed)
    pub fn is_collapsible(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Complete outline for a document
#[derive(Debug, Clone)]
pub struct OutlineData {
    pub revision: u64,
    pub roots: Vec<OutlineNode>,
}

impl OutlineData {
    /// Create an empty outline
    pub fn empty(revision: u64) -> Self {
        Self {
            revision,
            roots: Vec::new(),
        }
    }

    /// Check if the outline has any symbols
    pub fn is_empty(&self) -> bool {
        self.roots.is_empty()
    }
}
