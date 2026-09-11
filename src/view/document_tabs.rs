//! Document-tab painting. Geometry remains owned by the editor tab-bar layout.
use super::{Frame, TextPainter};
use crate::model::{editor_area::EditorGroup, AppModel};

pub(crate) fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    group: &EditorGroup,
    layout: &crate::layout::editor::EditorTabBarLayout,
) {
    let metrics = &model.metrics;
    let (bar_x, bar_y, bar_w, bar_h) = crate::layout::snapshot::snap(layout.bar_rect());

    let tab_bar_bg = model.theme.tab_bar.background.to_argb_u32();
    frame.fill_rect_px(bar_x, bar_y, bar_w, bar_h, tab_bar_bg);

    let border_color = model.theme.tab_bar.border.to_argb_u32();
    frame.fill_rect_px(
        bar_x,
        bar_y + bar_h.saturating_sub(metrics.border_width),
        bar_w,
        metrics.border_width,
        border_color,
    );

    for (index, tab) in group.tabs.iter().enumerate() {
        let Some(tab_rect) = layout.tab_rect(tab.id) else {
            continue;
        };
        let (tab_x, tab_y, tab_w, tab_h) = crate::layout::snapshot::snap(tab_rect);
        let (bg_color, fg_color) = if index == group.active_tab_index {
            (
                model.theme.tab_bar.active_background.to_argb_u32(),
                model.theme.tab_bar.active_foreground.to_argb_u32(),
            )
        } else {
            (
                model.theme.tab_bar.inactive_background.to_argb_u32(),
                model.theme.tab_bar.inactive_foreground.to_argb_u32(),
            )
        };

        frame.fill_rect_px(tab_x, tab_y, tab_w, tab_h, bg_color);
        // Clip the title to the tab rect: edge tabs are width-clamped to
        // the group, and unclipped text would bleed into the next pane.
        frame.push_clip(tab_rect);
        let (text_x, text_y) = layout
            .title_origin(tab.id)
            .expect("every editor tab has a solved title origin");
        let title = model.editor_area.tab_display_name(tab);
        painter.draw(frame, text_x, text_y, &title, fg_color);
        frame.pop_clip();
    }
}
