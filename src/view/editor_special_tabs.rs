//! Non-text editor tab rendering.

use crate::model::editor::BinaryPlaceholderState;
use crate::model::AppModel;

use super::frame::{Frame, TextPainter};
use super::{button, geometry};

/// Render an image viewer tab.
pub fn render_image_tab(
    frame: &mut Frame,
    _painter: &mut TextPainter,
    model: &AppModel,
    img_state: &crate::image::ImageState,
    layout: &geometry::GroupLayout,
) {
    let content_rect = layout.content_rect;
    let bg = model.theme.editor.background.to_argb_u32();
    frame.fill_rect(content_rect, bg);

    let padding = model.metrics.padding_large * 2;
    let dest_x = content_rect.x as usize + padding;
    let dest_y = content_rect.y as usize + padding;
    let dest_w = (content_rect.width as usize).saturating_sub(padding * 2);
    let dest_h = (content_rect.height as usize).saturating_sub(padding * 2);

    if dest_w > 0 && dest_h > 0 {
        let ip = &model.theme.image_preview;
        let check_size = ip.checkerboard_size;
        let light = ip.checkerboard_light.to_argb_u32();
        let dark = ip.checkerboard_dark.to_argb_u32();
        for cy in 0..dest_h {
            for cx in 0..dest_w {
                let px = dest_x + cx;
                let py = dest_y + cy;
                let checker = ((cx / check_size) + (cy / check_size)).is_multiple_of(2);
                frame.set_pixel(px, py, if checker { light } else { dark });
            }
        }

        frame.blit_rgba_scaled(
            &img_state.pixels,
            img_state.width,
            img_state.height,
            dest_x,
            dest_y,
            dest_w,
            dest_h,
        );
    }
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
    use super::format_file_size;

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
}
