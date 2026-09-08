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
                offset_within_row: 0.0,
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
    offset_within_row: f32,
}

impl RowListView {
    /// A continuously scrolled form/list, using the same drawn-row and hit geometry
    /// as row-snapped panels. Offsets are physical pixels, not selectable rows.
    pub fn from_pixel_scroll(rect: Rect, row_height: f32, count: usize, offset: usize) -> Self {
        let mut view = Self {
            rect,
            solved: RowListSolved {
                row_height,
                count,
                scroll_offset: 0,
            },
            offset_within_row: 0.0,
        };
        if row_height > 0.0 {
            let offset = offset.min(view.max_scroll_pixels()) as f32;
            view.solved.scroll_offset = (offset / row_height).floor() as usize;
            view.offset_within_row = offset % row_height;
        }
        view
    }

    pub fn content_height_pixels(&self) -> usize {
        (self.solved.count as f32 * self.solved.row_height).ceil() as usize
    }

    pub fn max_scroll_pixels(&self) -> usize {
        self.content_height_pixels()
            .saturating_sub(self.rect.height.max(0.0) as usize)
    }

    pub fn scroll_offset_pixels(&self) -> usize {
        (self.solved.scroll_offset as f32 * self.solved.row_height + self.offset_within_row).round()
            as usize
    }

    /// Reveal a complete row with the smallest pixel movement; do not snap an
    /// already visible row. Oversized rows align at the viewport's top.
    pub fn scroll_to_reveal_pixels(&self, selected: usize) -> usize {
        let top = selected.min(self.solved.count.saturating_sub(1)) as f32 * self.solved.row_height;
        let bottom = top + self.solved.row_height;
        let offset = self.scroll_offset_pixels() as f32;
        let target = if top < offset || self.solved.row_height > self.rect.height {
            top
        } else if bottom > offset + self.rect.height {
            bottom - self.rect.height
        } else {
            offset
        };
        (target.ceil() as usize).min(self.max_scroll_pixels())
    }

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

    /// Rows intersecting the viewport, including partial rows at either edge.
    /// Painting clips these rows to the same viewport used by hit testing.
    pub fn drawn_range(&self) -> Range<usize> {
        if self.solved.row_height <= 0.0 || self.rect.height <= 0.0 {
            return self.solved.scroll_offset..self.solved.scroll_offset;
        }
        let drawn =
            ((self.rect.height + self.offset_within_row) / self.solved.row_height).ceil() as usize;
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
        let visual =
            ((y - self.rect.y + self.offset_within_row) / self.solved.row_height).floor() as usize;
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
            self.rect.y + visual as f32 * self.solved.row_height - self.offset_within_row,
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

    /// Minimal-reveal scrolling: the offset that keeps `selected` inside
    /// the visible window, moving `scroll_offset` only as far as needed
    /// (up to the selection, or down so it becomes the last full row). A
    /// zero-capacity box leaves the offset alone — THE one reveal formula,
    /// so Outline/Problems and future row lists never re-derive it.
    pub fn scroll_to_reveal(&self, scroll_offset: usize, selected: usize) -> usize {
        let capacity = self.visible_capacity();
        if capacity == 0 {
            return scroll_offset;
        }
        if selected < scroll_offset {
            selected
        } else if selected >= scroll_offset.saturating_add(capacity) {
            selected.saturating_add(1) - capacity
        } else {
            scroll_offset
        }
    }
}

#[cfg(test)]
mod pixel_scroll_tests {
    use super::*;

    #[test]
    fn pixel_scrolled_rows_share_range_hit_reveal_and_end_clamping() {
        let rect = Rect::new(10.0, 100.0, 200.0, 100.0);
        let view = RowListView::from_pixel_scroll(rect, 72.0, 4, 13);
        assert_eq!(view.scroll_offset_pixels(), 13);
        assert_eq!(view.drawn_range(), 0..2);
        assert_eq!(view.row_rect(0).unwrap().y, 87.0);
        assert_eq!(view.row_rect(1).unwrap().y, 159.0);
        assert_eq!(view.row_at_y(99.0), None);
        assert_eq!(view.row_at_y(100.0), Some(0));
        assert_eq!(view.row_at_y(159.0), Some(1));
        assert_eq!(view.row_at_y(200.0), None);
        assert_eq!(view.scroll_to_reveal_pixels(0), 0);
        assert_eq!(view.scroll_to_reveal_pixels(1), 44);
        let bottom = RowListView::from_pixel_scroll(rect, 72.0, 4, usize::MAX);
        assert_eq!(bottom.scroll_offset_pixels(), 188);
        assert_eq!(bottom.drawn_range(), 2..4);
        let last = bottom.row_rect(3).unwrap();
        assert_eq!(last.y + last.height, rect.y + rect.height);
        assert_eq!(bottom.scroll_to_reveal_pixels(3), 188);
        let empty = RowListView::from_pixel_scroll(rect, 72.0, 0, usize::MAX);
        assert_eq!(empty.scroll_offset_pixels(), 0);
        assert!(empty.drawn_range().is_empty());
        assert_eq!(empty.row_at_y(150.0), None);
    }
}
