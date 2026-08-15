//! Solved layout output: the queryable geometry snapshot.
//!
//! Clay emits a flat render-command array; this adaptation emits a solved
//! tree that Token's imperative painters *query* (rects by key, draw
//! order for traversal) and that hit-testing and update-layer capacity
//! queries read — one geometry, three consumers.

use std::collections::HashMap;
use std::ops::Range;

use crate::layout::keys::UiKey;
use crate::layout::text::TextStyle;
use crate::model::editor_area::Rect;

/// One wrapped line of a solved text leaf: a byte range into the source
/// string plus its measured width.
#[derive(Clone, Debug, PartialEq)]
pub struct TextLine {
    pub range: Range<usize>,
    pub width: f32,
}

/// Solved payload of a `Content::RowList` leaf.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowListSolved {
    pub row_height: f32,
    pub count: usize,
    pub scroll_offset: usize,
}

/// Solved leaf content.
#[derive(Clone, Debug, Default)]
pub enum SolvedContent {
    #[default]
    None,
    Text {
        text: String,
        style: TextStyle,
        line_height: f32,
        lines: Vec<TextLine>,
    },
    RowList(RowListSolved),
}

/// One solved element.
#[derive(Clone, Debug)]
pub struct SolvedNode {
    pub key: Option<UiKey>,
    /// Border box in physical px.
    pub rect: Rect,
    /// `rect` minus padding.
    pub content_rect: Rect,
    /// Intersection of ancestor clip content-boxes — the scissor to apply
    /// when painting this node. `None` = unclipped.
    pub clip: Option<Rect>,
    /// Draw layer inherited from the nearest floating ancestor (0 = flow).
    pub z: i16,
    pub parent: Option<u32>,
    pub content: SolvedContent,
}

/// Snap a solved rect to integer pixels: edges round independently so
/// adjacent rects stay gap-free. Returns `(x, y, w, h)`.
pub fn snap(rect: Rect) -> (usize, usize, usize, usize) {
    let x0 = rect.x.round().max(0.0) as usize;
    let y0 = rect.y.round().max(0.0) as usize;
    let x1 = (rect.x + rect.width).round().max(0.0) as usize;
    let y1 = (rect.y + rect.height).round().max(0.0) as usize;
    (x0, y0, x1.saturating_sub(x0), y1.saturating_sub(y0))
}

/// The solved geometry of one tree: query rects by key, hit-test the
/// topmost element at a point, walk nodes in draw order.
#[derive(Default)]
pub struct LayoutSnapshot {
    pub(crate) nodes: Vec<SolvedNode>,
    pub(crate) by_key: HashMap<UiKey, u32>,
    /// Node indices in paint order: flow nodes in declaration order
    /// (parents before children), floating subtrees after, sorted by
    /// `(z, declaration order)`.
    pub(crate) draw_order: Vec<u32>,
}

impl LayoutSnapshot {
    pub fn node(&self, key: UiKey) -> Option<&SolvedNode> {
        self.by_key.get(&key).map(|&i| &self.nodes[i as usize])
    }

    pub fn rect(&self, key: UiKey) -> Option<Rect> {
        self.node(key).map(|n| n.rect)
    }

    pub fn content_rect(&self, key: UiKey) -> Option<Rect> {
        self.node(key).map(|n| n.content_rect)
    }

    /// Topmost keyed element containing `(x, y)`, honoring clip rects and
    /// draw order — the `Clay_PointerOver` equivalent. An unkeyed hit
    /// resolves to its nearest keyed ancestor.
    pub fn hit(&self, x: f32, y: f32) -> Option<UiKey> {
        for &index in self.draw_order.iter().rev() {
            let node = &self.nodes[index as usize];
            if let Some(clip) = node.clip {
                if !clip.contains(x, y) {
                    continue;
                }
            }
            if !node.rect.contains(x, y) {
                continue;
            }
            // Nearest keyed self-or-ancestor.
            let mut cursor = Some(index);
            while let Some(i) = cursor {
                let n = &self.nodes[i as usize];
                if let Some(key) = n.key {
                    return Some(key);
                }
                cursor = n.parent;
            }
            // Keyless subtree: keep scanning lower elements.
        }
        None
    }

    /// Row-list math for a `Content::RowList` element. Returns `None` when
    /// the key isn't in this snapshot (e.g. the panel isn't the active one
    /// of any dock) — callers treat that as "not visible".
    pub fn row_list(&self, key: UiKey) -> Option<RowListView> {
        let node = self.node(key)?;
        match node.content {
            SolvedContent::RowList(solved) => Some(RowListView {
                rect: node.rect,
                solved,
            }),
            _ => None,
        }
    }

    /// Visit solved nodes in paint order.
    pub fn visit_draw_order(&self, mut f: impl FnMut(&SolvedNode)) {
        for &index in &self.draw_order {
            f(&self.nodes[index as usize]);
        }
    }
}

/// THE row geometry authority for uniform-row panels: capacity, drawn
/// range, hit mapping, and scroll clamping all derive from one box and one
/// row height, so they cannot disagree (the class of bug where
/// `visible_capacity` floors but `row_index_at_y` accepts a sliver row that
/// was never drawn).
#[derive(Clone, Copy, Debug)]
pub struct RowListView {
    rect: Rect,
    solved: RowListSolved,
}

impl RowListView {
    pub fn rect(&self) -> Rect {
        self.rect
    }

    pub fn row_height(&self) -> f32 {
        self.solved.row_height
    }

    pub fn count(&self) -> usize {
        self.solved.count
    }

    pub fn scroll_offset(&self) -> usize {
        self.solved.scroll_offset
    }

    /// Fully visible rows: `floor(height / row_height)`. This is the
    /// capacity used for scroll math.
    pub fn visible_capacity(&self) -> usize {
        if self.solved.row_height <= 0.0 {
            return 0;
        }
        (self.rect.height / self.solved.row_height).floor() as usize
    }

    /// Rows actually painted: `ceil(height / row_height)` starting at the
    /// scroll offset — a partial bottom row is drawn (clipped by the
    /// panel's scissor) and therefore hittable.
    pub fn drawn_range(&self) -> Range<usize> {
        if self.solved.row_height <= 0.0 {
            return self.solved.scroll_offset..self.solved.scroll_offset;
        }
        let drawn = (self.rect.height / self.solved.row_height).ceil() as usize;
        let start = self.solved.scroll_offset.min(self.solved.count);
        let end = start.saturating_add(drawn).min(self.solved.count);
        start..end
    }

    /// The absolute row index at pixel `y`, if it lands on a drawn row.
    pub fn row_at_y(&self, y: f32) -> Option<usize> {
        if y < self.rect.y || y >= self.rect.y + self.rect.height {
            return None;
        }
        if self.solved.row_height <= 0.0 {
            return None;
        }
        let visual = ((y - self.rect.y) / self.solved.row_height).floor() as usize;
        let index = self.solved.scroll_offset.saturating_add(visual);
        (index < self.solved.count).then_some(index)
    }

    /// The rect of an absolute row index, if it's within the drawn range.
    pub fn row_rect(&self, index: usize) -> Option<Rect> {
        if !self.drawn_range().contains(&index) {
            return None;
        }
        let visual = index - self.solved.scroll_offset;
        Some(Rect::new(
            self.rect.x,
            self.rect.y + visual as f32 * self.solved.row_height,
            self.rect.width,
            self.solved.row_height,
        ))
    }

    /// Maximum useful scroll offset: `count - visible_capacity`, floored at
    /// zero — THE one clamp formula.
    pub fn max_scroll(&self) -> usize {
        self.solved.count.saturating_sub(self.visible_capacity())
    }

    pub fn clamp_scroll(&self, offset: usize) -> usize {
        offset.min(self.max_scroll())
    }
}
