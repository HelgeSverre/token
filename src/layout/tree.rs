//! Declarative element tree (Clay's `CLAY(...)` macro, as a Rust builder).
//!
//! A tree is declared fresh for each solve — immediate-mode, like Clay —
//! with closure nesting instead of macro blocks:
//!
//! ```ignore
//! let mut t = UiTree::new();
//! t.node(
//!     ElementDecl { dir: Dir::Row, sizing: SizingAxes::grow(), ..Default::default() },
//!     |t| {
//!         t.leaf(ElementDecl { sizing: SizingAxes::fixed(200.0, 0.0), ..Default::default() });
//!         t.leaf(ElementDecl { sizing: SizingAxes::grow(), ..Default::default() });
//!     },
//! );
//! let snapshot = t.solve(root_rect, scale_factor, &mut measure);
//! ```

use crate::layout::algorithm;
use crate::layout::anchor::FloatDecl;
use crate::layout::keys::UiKey;
use crate::layout::sizing::{AlignX, AlignY, Dir, Padding, SizingAxes};
use crate::layout::snapshot::LayoutSnapshot;
use crate::layout::text::{TextMeasure, TextStyle};
use crate::model::editor_area::Rect;

/// Text wrapping mode. Only what Token's chrome needs — Clay's additional
/// modes (wrap-anywhere, no-trim) are deliberately not ported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Wrap {
    /// Single line per source line (`\n` still breaks).
    #[default]
    None,
    /// Word wrap at the solved width; over-long tokens hard-break
    /// (the `wrap_zone_text` semantics from the hover card).
    Words,
}

/// A measured text leaf.
#[derive(Clone, Debug)]
pub struct TextDecl {
    pub text: String,
    pub style: TextStyle,
    pub wrap: Wrap,
}

/// Token extension: a uniform-row list virtualized as a single leaf. The
/// engine solves the list's box; row geometry (capacity, row rects, hit
/// mapping, scroll clamping) derives from one formula in
/// [`crate::layout::snapshot::RowListView`] instead of per-row elements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowListDecl {
    pub row_height: f32,
    pub count: usize,
    /// Scroll offset in rows — owned by the app model, only read here.
    pub scroll_offset: usize,
}

/// Scroll offsets applied to a container's children (physical px). State is
/// owned by the app model (Elm purity) — the engine only reads it; Clay's
/// internal scroll containers and momentum are deliberately not ported.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScrollDecl {
    pub offset_x: f32,
    pub offset_y: f32,
}

/// Leaf content of an element.
#[derive(Clone, Debug, Default)]
pub enum Content {
    #[default]
    None,
    Text(TextDecl),
    RowList(RowListDecl),
}

/// One element's declaration (Clay's `Clay_ElementDeclaration`, layout
/// fields only — colors and borders are paint-time concerns in Token).
#[derive(Clone, Debug, Default)]
pub struct ElementDecl {
    pub key: Option<UiKey>,
    pub dir: Dir,
    pub sizing: SizingAxes,
    pub padding: Padding,
    /// Gap between adjacent children along `dir`.
    pub gap: f32,
    pub align: (AlignX, AlignY),
    /// Scissor children to this element's content box.
    pub clip: bool,
    pub scroll: Option<ScrollDecl>,
    /// Present ⇒ this element floats out of the normal flow.
    pub float: Option<FloatDecl>,
    pub content: Content,
}

/// Internal node storage: declaration plus tree links. Children indices are
/// collected eagerly so the solver can iterate a parent's children without
/// scanning (subtrees make siblings non-contiguous in declaration order).
pub(crate) struct Node {
    pub(crate) decl: ElementDecl,
    pub(crate) parent: Option<u32>,
    pub(crate) children: Vec<u32>,
}

/// Vec-backed tree builder. Parents always precede their descendants in
/// declaration order, which the solver's bottom-up (reverse) and top-down
/// (forward) passes rely on.
#[derive(Default)]
pub struct UiTree {
    pub(crate) nodes: Vec<Node>,
    open_stack: Vec<u32>,
}

impl UiTree {
    pub fn new() -> Self {
        Self::default()
    }

    fn push_node(&mut self, decl: ElementDecl) -> u32 {
        let index = self.nodes.len() as u32;
        let parent = self.open_stack.last().copied();
        self.nodes.push(Node {
            decl,
            parent,
            children: Vec::new(),
        });
        if let Some(p) = parent {
            self.nodes[p as usize].children.push(index);
        }
        index
    }

    /// Declare a container element; `children` declares its children.
    pub fn node(&mut self, decl: ElementDecl, children: impl FnOnce(&mut UiTree)) {
        let index = self.push_node(decl);
        self.open_stack.push(index);
        children(self);
        self.open_stack.pop();
    }

    /// Declare a childless element.
    pub fn leaf(&mut self, decl: ElementDecl) {
        self.push_node(decl);
    }

    /// Convenience: declare an unwrapped text leaf sized to fit.
    pub fn text(&mut self, key: Option<UiKey>, text: impl Into<String>, style: TextStyle) {
        self.leaf(ElementDecl {
            key,
            content: Content::Text(TextDecl {
                text: text.into(),
                style,
                wrap: Wrap::None,
            }),
            ..Default::default()
        });
    }

    /// Solve the tree into a queryable snapshot. `root` is the box the root
    /// element solves into (its declared sizing is ignored); `scale_factor`
    /// resolves logical-px anchor constants; `measure` is borrowed only for
    /// the duration of the solve.
    pub fn solve(
        self,
        root: Rect,
        scale_factor: f64,
        measure: &mut dyn TextMeasure,
    ) -> LayoutSnapshot {
        debug_assert!(
            self.open_stack.is_empty(),
            "UiTree::solve with unbalanced node() nesting"
        );
        algorithm::solve(self.nodes, root, scale_factor, measure)
    }
}
