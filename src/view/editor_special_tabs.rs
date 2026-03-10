//! Non-text editor tab rendering.

use crate::model::editor::BinaryPlaceholderState;
use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::{button, geometry};

/// Render an image viewer tab.
pub fn render_image_tab(
    frame: &mut Frame,
    model: &AppModel,
    img_state: &crate::image::ImageState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect;
    crate::image::render::render_image(
        frame,
        img_state,
        &model.theme.image_preview,
        content_rect.x as usize,
        content_rect.y as usize,
        content_rect.width as usize,
        content_rect.height as usize,
    );
}

/// Render a binary file placeholder tab.
pub fn render_binary_placeholder(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    placeholder: &BinaryPlaceholderState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect;
    let bg = model.theme.editor.background.to_argb_u32();
    let fg = model.theme.editor.foreground.to_argb_u32();
    let dim_fg = model.theme.gutter.foreground.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    let char_width = painter.char_width();
    let line_height = painter.line_height();
    let btn_label = geometry::BINARY_PLACEHOLDER_BUTTON_LABEL;
    let bp_layout = geometry::binary_placeholder_layout(
        content_rect,
        line_height,
        char_width,
        model.metrics.padding_large,
        model.metrics.padding_medium,
        btn_label,
    );

    let filename = placeholder
        .path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let name_x = bp_layout
        .center_x
        .saturating_sub((filename.len() as f32 * char_width / 2.0) as usize);
    painter.draw(frame, name_x, bp_layout.name_y, &filename, fg);

    let size_str = format_file_size(placeholder.size_bytes);
    let size_x = bp_layout
        .center_x
        .saturating_sub((size_str.len() as f32 * char_width / 2.0) as usize);
    painter.draw(frame, size_x, bp_layout.size_y, &size_str, dim_fg);

    let btn_state = if model.ui.hover == crate::model::ui::HoverRegion::Button {
        button::ButtonState::Hovered
    } else {
        button::ButtonState::Normal
    };

    button::render_button(
        frame,
        painter,
        &model.theme,
        bp_layout.button_rect,
        btn_label,
        btn_state,
        true,
    );
}

fn format_file_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::{format_file_size, render_image_tab};
    use crate::commands::Cmd;
    use crate::image::ImageState;
    use crate::messages::{ImageMsg, Msg};
    use crate::model::{AppModel, Rect, ViewMode};
    use crate::update::update;
    use crate::view::frame::Frame;
    use crate::view::geometry::GroupLayout;

    fn make_image_model(content_width: u32, content_height: u32) -> AppModel {
        let mut model = AppModel::new(content_width, content_height, 1.0, vec![]);
        let tab_bar_height = model.metrics.tab_bar_height as f32;
        let group_id = model.editor_area.focused_group_id;
        model.editor_area.groups.get_mut(&group_id).unwrap().rect = Rect::new(
            0.0,
            0.0,
            content_width as f32,
            content_height as f32 + tab_bar_height,
        );

        let mut pixels = Vec::with_capacity(8 * 8 * 4);
        for y in 0..8 {
            for x in 0..8 {
                pixels.extend_from_slice(&[
                    (x * 31) as u8,
                    (y * 29) as u8,
                    ((x + y) * 17) as u8,
                    255,
                ]);
            }
        }
        model.editor_mut().view_mode = ViewMode::Image(Box::new(ImageState::new(
            pixels,
            8,
            8,
            0,
            "PNG".into(),
            content_width,
            content_height,
        )));

        model
    }

    fn render_image_buffer(model: &AppModel) -> Vec<u32> {
        let width = model.window_size.0 as usize;
        let height = model.window_size.1 as usize;
        let mut buffer = vec![0; width * height];
        let mut frame = Frame::new(&mut buffer, width, height);
        let group = model
            .editor_area
            .groups
            .get(&model.editor_area.focused_group_id)
            .unwrap();
        let image = model
            .editor_area
            .focused_editor()
            .unwrap()
            .view_mode
            .as_image()
            .unwrap();
        let layout = GroupLayout::new(group, model, 8.0);

        render_image_tab(&mut frame, model, image, &layout);
        buffer
    }

    #[test]
    fn formats_small_file_sizes() {
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1536), "1.5 KB");
    }

    #[test]
    fn formats_large_file_sizes() {
        assert_eq!(format_file_size(3 * 1024 * 1024), "3.0 MB");
        assert_eq!(format_file_size(5 * 1024 * 1024 * 1024), "5.0 GB");
    }

    #[test]
    fn image_tab_render_changes_when_zoom_changes() {
        let mut model = make_image_model(80, 60);
        let before = render_image_buffer(&model);

        let mouse_x = 40.0;
        let mouse_y = model.metrics.tab_bar_height as f64 + 30.0;
        let cmd = update(
            &mut model,
            Msg::Image(ImageMsg::Zoom {
                delta: 1.0,
                mouse_x,
                mouse_y,
            }),
        );

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));

        let after = render_image_buffer(&model);
        assert_ne!(
            before, after,
            "zoom state must change rendered pixels, not just status-bar state"
        );
    }

    #[test]
    fn image_tab_render_changes_when_panned() {
        let mut model = make_image_model(80, 60);
        let mouse_x = 40.0;
        let mouse_y = model.metrics.tab_bar_height as f64 + 30.0;
        update(
            &mut model,
            Msg::Image(ImageMsg::Zoom {
                delta: 1.0,
                mouse_x,
                mouse_y,
            }),
        );
        update(
            &mut model,
            Msg::Image(ImageMsg::StartPan {
                x: mouse_x,
                y: mouse_y,
            }),
        );
        let before = render_image_buffer(&model);

        let cmd = update(
            &mut model,
            Msg::Image(ImageMsg::UpdatePan {
                x: mouse_x + 12.0,
                y: mouse_y + 8.0,
            }),
        );

        assert!(cmd.as_ref().is_some_and(Cmd::needs_redraw));

        let after = render_image_buffer(&model);
        assert_ne!(
            before, after,
            "pan state must change rendered pixels, not just internal offsets"
        );
    }
}
