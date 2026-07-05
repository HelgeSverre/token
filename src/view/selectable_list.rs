//! Shared rendering helpers for simple selectable row lists.

use super::frame::Frame;

#[derive(Debug, Clone, Copy)]
pub struct SelectableListColors {
    pub selection_bg: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct SelectableListLayout {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub row_height: usize,
    pub max_visible_items: usize,
    pub selection_inset: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelectableListViewport {
    pub selected_index: usize,
    pub scroll_offset: usize,
    pub visible_count: usize,
    pub items_after: usize,
}

impl SelectableListViewport {
    /// Computes a viewport assuming no prior scroll position (window starts at
    /// the top). Kept for callers that don't track scroll state across
    /// renders; prefer [`Self::compute_from`] when a previous scroll offset is
    /// available, since it implements minimal-reveal scrolling instead of
    /// always pinning to an edge.
    pub fn compute(total_items: usize, selected_index: usize, max_visible_items: usize) -> Self {
        Self::compute_from(total_items, selected_index, max_visible_items, 0)
    }

    /// Minimal-reveal scrolling: the visible window is only moved when
    /// `selected_index` falls outside `[previous_scroll_offset,
    /// previous_scroll_offset + max_visible_items)`. When it does, the window
    /// moves by the minimum amount needed to bring the selection back into
    /// view — scrolling up just enough if the selection moved above the
    /// window, or down just enough if it moved below — rather than
    /// unconditionally recomputing from scratch and pinning the selection to
    /// an edge.
    pub fn compute_from(
        total_items: usize,
        selected_index: usize,
        max_visible_items: usize,
        previous_scroll_offset: usize,
    ) -> Self {
        let selected_index = selected_index.min(total_items.saturating_sub(1));
        let visible_count = total_items.min(max_visible_items);

        let max_scroll_offset = total_items.saturating_sub(max_visible_items);
        let mut scroll_offset = previous_scroll_offset.min(max_scroll_offset);

        if selected_index < scroll_offset {
            // Selection moved above the visible window: scroll up just enough.
            scroll_offset = selected_index;
        } else if selected_index >= scroll_offset + max_visible_items {
            // Selection moved below the visible window: scroll down just enough.
            scroll_offset = selected_index + 1 - max_visible_items;
        }

        let items_after = total_items.saturating_sub(scroll_offset + max_visible_items);

        Self {
            selected_index,
            scroll_offset,
            visible_count,
            items_after,
        }
    }
}

pub fn render_selectable_list<T, F>(
    frame: &mut Frame,
    items: &[T],
    selected_index: usize,
    layout: &SelectableListLayout,
    colors: &SelectableListColors,
    mut render_row: F,
) -> SelectableListViewport
where
    F: FnMut(&mut Frame, &T, usize, usize, bool),
{
    let viewport =
        SelectableListViewport::compute(items.len(), selected_index, layout.max_visible_items);

    for (i, item) in items
        .iter()
        .skip(viewport.scroll_offset)
        .take(layout.max_visible_items)
        .enumerate()
    {
        let actual_index = viewport.scroll_offset + i;
        let item_y = layout.y + i * layout.row_height;
        let is_selected = actual_index == viewport.selected_index;

        if is_selected {
            let highlight_width = layout
                .width
                .saturating_sub(layout.selection_inset.saturating_mul(2));
            frame.fill_rect_px(
                layout.x + layout.selection_inset,
                item_y,
                highlight_width,
                layout.row_height,
                colors.selection_bg,
            );
        }

        render_row(frame, item, actual_index, item_y, is_selected);
    }

    viewport
}

#[cfg(test)]
mod tests {
    use super::SelectableListViewport;

    #[test]
    fn viewport_clamps_selection_without_scroll() {
        let viewport = SelectableListViewport::compute(3, 10, 8);
        assert_eq!(viewport.selected_index, 2);
        assert_eq!(viewport.scroll_offset, 0);
        assert_eq!(viewport.visible_count, 3);
        assert_eq!(viewport.items_after, 0);
    }

    #[test]
    fn viewport_scrolls_to_keep_selection_visible() {
        let viewport = SelectableListViewport::compute(15, 12, 8);
        assert_eq!(viewport.selected_index, 12);
        assert_eq!(viewport.scroll_offset, 5);
        assert_eq!(viewport.visible_count, 8);
        assert_eq!(viewport.items_after, 2);
    }

    /// M12 regression: moving the selection down one row at a time should
    /// only nudge the scroll offset by exactly one row at a time, once the
    /// selection actually leaves the visible window — never jump/pin
    /// unconditionally.
    #[test]
    fn compute_from_scrolls_down_minimally_one_row_at_a_time() {
        let total = 20;
        let max_visible = 8;
        let mut offset = 0usize;
        let mut changes = 0;

        for selected in 0..total {
            let viewport =
                SelectableListViewport::compute_from(total, selected, max_visible, offset);
            if viewport.scroll_offset != offset {
                changes += 1;
                assert_eq!(
                    viewport.scroll_offset,
                    offset + 1,
                    "scroll offset should move by exactly one row when the selection \
                     leaves the window from below"
                );
            }
            offset = viewport.scroll_offset;
        }

        // Once selection reaches the last item, the window should be pinned
        // just enough to show it (20 - 8 = 12), and it should only have
        // scrolled once per row past the initial page.
        assert_eq!(offset, total - max_visible);
        assert_eq!(changes, total - max_visible);
    }

    /// M12 regression: after scrolling down to the bottom, moving the
    /// selection back up should also only move the window by the minimum
    /// amount needed — and once it settles back within a stable window,
    /// further moves within that window must not change scroll_offset at all.
    #[test]
    fn compute_from_scrolls_up_minimally_and_holds_steady_within_window() {
        let total = 20;
        let max_visible = 8;

        // Start from the bottom-pinned window (as if the user had scrolled
        // all the way down previously).
        let bottom = SelectableListViewport::compute_from(total, total - 1, max_visible, 0);
        assert_eq!(bottom.scroll_offset, 12);

        // Move the selection up one row at a time and ensure the offset only
        // decreases by exactly one row at a time, right when selection
        // leaves the window from above.
        let mut offset = bottom.scroll_offset;
        let mut changes = 0;
        for selected in (0..total).rev() {
            let viewport =
                SelectableListViewport::compute_from(total, selected, max_visible, offset);
            if viewport.scroll_offset != offset {
                changes += 1;
                assert_eq!(
                    viewport.scroll_offset,
                    offset - 1,
                    "scroll offset should move by exactly one row when the selection \
                     leaves the window from above"
                );
            }
            offset = viewport.scroll_offset;
        }
        assert_eq!(offset, 0);
        assert_eq!(changes, total - max_visible);

        // Within a stable window, moving selection but staying inside the
        // visible range must not touch scroll_offset at all.
        let steady = SelectableListViewport::compute_from(total, 12, max_visible, 10);
        assert_eq!(steady.scroll_offset, 10, "selection stays within window");
    }

    /// M12 regression: this is the exact bug scenario. After the window has
    /// scrolled down to the bottom, jumping the selection directly to a row
    /// far above the window must scroll up by only the minimum amount needed
    /// (so the selection lands at the top edge of the window), not reset the
    /// window all the way back to the start the way the old
    /// "recompute from scratch and pin to an edge" logic did.
    #[test]
    fn compute_from_jump_above_window_scrolls_minimally_not_to_start() {
        let total = 20;
        let max_visible = 8;

        // Window pinned at the bottom: [12, 20).
        let previous_offset = 12;

        // Jump the selection up to row 5, which is above the window but far
        // from the very top of the list.
        let viewport = SelectableListViewport::compute_from(total, 5, max_visible, previous_offset);

        // Minimal reveal: offset should move to exactly the selected row so
        // that it sits at the top edge of the new window, not jump to 0.
        assert_eq!(viewport.scroll_offset, 5);
        assert_ne!(
            viewport.scroll_offset, 0,
            "must not unconditionally pin to the start of the list"
        );
    }
}
