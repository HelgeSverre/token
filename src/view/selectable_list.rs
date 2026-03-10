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
    pub fn compute(total_items: usize, selected_index: usize, max_visible_items: usize) -> Self {
        let selected_index = selected_index.min(total_items.saturating_sub(1));
        let visible_count = total_items.min(max_visible_items);
        let scroll_offset = if selected_index >= max_visible_items {
            selected_index + 1 - max_visible_items
        } else {
            0
        };
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
}
