//! Settings-form presentation for OverlaySurface. Painting and input consume
//! the same resolved rectangles; metadata and scrolling remain shared.
use super::*;

const ROW: f32 = 72.0;
const TOP: f32 = 108.0;
const FOOT: f32 = 36.0;

fn panel(width: usize, height: usize, sf: f64) -> WidgetRect {
    let margin = scaled(16.0, sf).min(width / 8).min(height / 8);
    let w = width.saturating_sub(margin * 2).min(scaled(1160.0, sf));
    WidgetRect {
        x: (width - w) / 2,
        y: margin,
        w,
        h: height.saturating_sub(margin * 2),
    }
}

fn wide(rect: &WidgetRect, sf: f64) -> bool {
    rect.w >= scaled(680.0, sf)
}

/// Shared responsive chrome geometry. Short windows reflow all categories into
/// a grid so none disappear behind the footer.
struct Chrome {
    nav_top: usize,
    nav_columns: usize,
    sidebar: usize,
    body_top: usize,
    header_y: usize,
    header_h: usize,
}

fn chrome(p: &WidgetRect, sf: f64) -> Chrome {
    let short = p.h < scaled(400.0, sf);
    let top = scaled(if short { 84.0 } else { TOP }, sf);
    let categories = crate::settings::categories().len();
    let sidebar = if wide(p, sf)
        && p.h.saturating_sub(top + scaled(FOOT, sf)) >= categories * scaled(36.0, sf)
    {
        scaled(210.0, sf)
    } else {
        0
    };
    let columns = if sidebar > 0 {
        1
    } else if wide(p, sf) {
        categories
    } else {
        3
    };
    let nav_height = categories.div_ceil(columns) * scaled(34.0, sf);
    Chrome {
        nav_top: p.y + top,
        nav_columns: columns,
        sidebar,
        body_top: p.y
            + top
            + if sidebar == 0 {
                nav_height + scaled(8.0, sf)
            } else {
                0
            },
        header_y: p.y + scaled(if short { 44.0 } else { 54.0 }, sf),
        header_h: scaled(if short { 30.0 } else { 36.0 }, sf),
    }
}

/// Display-row capacity shared by update, rendering, and screenshot navigation.
pub fn visible_count(width: usize, height: usize, sf: f64) -> usize {
    let p = panel(width, height, sf);
    let top = chrome(&p, sf).body_top - p.y;
    p.h.saturating_sub(top + scaled(FOOT, sf))
        .checked_div(scaled(ROW, sf))
        .unwrap_or(0)
        .max(1)
}

pub(super) fn layout(
    spec: &OverlaySpec,
    out: &mut OverlayLayout,
    width: usize,
    height: usize,
    sf: f64,
) {
    let p = panel(width, height, sf);
    let pad = scaled(20.0, sf).min(p.w / 8);
    let chrome = chrome(&p, sf);
    let sidebar = chrome.sidebar;
    let nav_top = chrome.nav_top;
    out.panel = p;
    out.header = Some(WidgetRect {
        x: p.x + pad,
        y: chrome.header_y,
        w: p.w.saturating_sub(pad * 2),
        h: chrome.header_h,
    });
    out.tab_rects = spec
        .tabs
        .as_ref()
        .map(|tabs| {
            tabs.tabs
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    if sidebar > 0 {
                        WidgetRect {
                            x: p.x + pad / 2,
                            y: nav_top + i * scaled(36.0, sf),
                            w: sidebar - pad,
                            h: scaled(32.0, sf),
                        }
                    } else {
                        let w = p.w.saturating_sub(pad * 2) / chrome.nav_columns;
                        WidgetRect {
                            x: p.x + pad + (i % chrome.nav_columns) * w,
                            y: nav_top + (i / chrome.nav_columns) * scaled(34.0, sf),
                            w,
                            h: scaled(30.0, sf),
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let top = chrome.body_top;
    let capacity = visible_count(width, height, sf);
    let (start, count, total) = match &spec.body {
        Body::List {
            sections, scroll, ..
        } => {
            let rows = flatten_rows(sections);
            let (start, count) = resolve_visible_window(&rows, *scroll, capacity);
            (start, count, rows.len())
        }
        _ => (0, 0, 0),
    };
    out.row_height = scaled(ROW, sf);
    out.rows = (0..count)
        .map(|i| WidgetRect {
            x: p.x + sidebar + pad,
            y: top + i * out.row_height,
            w: p.w.saturating_sub(sidebar + pad * 2),
            h: out.row_height,
        })
        .collect();
    out.footer = Some(WidgetRect {
        x: p.x + pad,
        y: p.y + p.h.saturating_sub(scaled(FOOT, sf)),
        w: p.w.saturating_sub(pad * 2),
        h: scaled(FOOT, sf),
    });
    out.scrollbar = if total > count {
        let track = count * out.row_height;
        let h = (track * count / total).max(scaled(20.0, sf)).min(track);
        Some(WidgetRect {
            x: p.x + p.w.saturating_sub(scaled(8.0, sf)),
            y: top + start * track.saturating_sub(h) / total.saturating_sub(count).max(1),
            w: scaled(3.0, sf),
            h,
        })
    } else {
        None
    };
}

fn close_rect(p: &WidgetRect, sf: f64) -> WidgetRect {
    let w = scaled(72.0, sf).min(p.w / 3);
    WidgetRect {
        x: p.x + p.w.saturating_sub(w + scaled(20.0, sf)),
        y: p.y + scaled(16.0, sf),
        w,
        h: scaled(28.0, sf),
    }
}

fn contains(r: &WidgetRect, x: usize, y: usize) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

fn controls(rect: &WidgetRect, sf: f64) -> WidgetRect {
    // At compact widths put controls below the label, leaving the entire
    // row width available to each. Descriptions remain in the footer.
    if rect.w < scaled(400.0, sf) {
        WidgetRect {
            x: rect.x,
            y: rect.y + scaled(34.0, sf),
            w: rect.w,
            h: scaled(30.0, sf),
        }
    } else {
        WidgetRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: scaled(40.0, sf),
        }
    }
}

fn value_rect(rect: &WidgetRect, sf: f64) -> WidgetRect {
    WidgetRect {
        x: rect.x,
        y: rect.y + scaled(36.0, sf),
        w: rect.w,
        h: scaled(24.0, sf),
    }
}

fn action_rect(rect: &WidgetRect, sf: f64) -> WidgetRect {
    WidgetRect {
        x: rect.x + rect.w.saturating_sub(scaled(112.0, sf)),
        y: rect.y + scaled(10.0, sf),
        w: scaled(112.0, sf).min(rect.w),
        h: scaled(24.0, sf).min(rect.h),
    }
}

fn preset_rects(rect: &WidgetRect, labels: &[&str], sf: f64) -> Vec<WidgetRect> {
    let control = controls(rect, sf);
    let budget = if rect.w < scaled(400.0, sf) {
        control.w
    } else {
        control.w * 2 / 3
    };
    choice_rects_with_budget(&control, labels, sf, 0, budget)
}

fn switch_rect(rect: &WidgetRect, sf: f64) -> WidgetRect {
    let r = controls(rect, sf);
    let w = scaled(34.0, sf).min(r.w);
    let h = scaled(20.0, sf).min(r.h);
    WidgetRect {
        x: r.x + r.w - w,
        y: r.y + (r.h - h) / 2,
        w,
        h,
    }
}

pub(super) fn hit_test(
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    x: usize,
    y: usize,
) -> OverlayHit {
    // The close button shares the modal's existing dismiss action.
    if contains(&close_rect(&layout.panel, layout.scale_factor), x, y) {
        return OverlayHit::Outside;
    }
    if !contains(&layout.panel, x, y) {
        return OverlayHit::Outside;
    }
    if layout
        .footer
        .as_ref()
        .is_some_and(|rect| contains(rect, x, y))
    {
        return OverlayHit::Inside;
    }
    if let Some(index) = layout.tab_rects.iter().position(|r| contains(r, x, y)) {
        return OverlayHit::Tab(index);
    }
    if let Body::List {
        sections,
        scroll,
        max_visible,
        ..
    } = &spec.body
    {
        let rows = flatten_rows(sections);
        let (start, _) = resolve_visible_window(&rows, *scroll, *max_visible);
        for (slot, rect) in layout.rows.iter().enumerate() {
            if !contains(rect, x, y) {
                continue;
            }
            if let Some(DisplayRow::Row(row, index)) = rows.get(start + slot) {
                if matches!(
                    row.accessory,
                    Accessory::SettingValue {
                        action: Some(_),
                        ..
                    }
                ) && contains(&action_rect(rect, layout.scale_factor), x, y)
                {
                    return OverlayHit::Choice {
                        row: *index,
                        choice: 0,
                    };
                }
                if let Accessory::Choices { labels, active } = &row.accessory {
                    if *labels == ["Off", "On"] {
                        if contains(&switch_rect(rect, layout.scale_factor), x, y) {
                            return OverlayHit::Choice {
                                row: *index,
                                choice: usize::from(*active != Some(1)),
                            };
                        }
                    } else {
                        for (choice, r) in preset_rects(rect, labels, layout.scale_factor)
                            .iter()
                            .enumerate()
                        {
                            if contains(r, x, y) {
                                return OverlayHit::Choice {
                                    row: *index,
                                    choice,
                                };
                            }
                        }
                    }
                }
                return OverlayHit::Row(*index);
            }
        }
    }
    OverlayHit::Inside
}

fn text(
    frame: &mut Frame,
    painter: &mut TextPainter,
    rect: &WidgetRect,
    value: &str,
    size: f32,
    color: u32,
) {
    let value = truncate_tail(painter, size, value, rect.w as f32);
    painter.draw_sized(frame, rect.x, rect.y, &value, size, 0.0, color);
}

// Mirrors the shared overlay renderer's explicit paint context.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    masks: &mut RoundedRectMaskCache,
    colors: &Palette,
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    sf: f64,
    cursor: bool,
) {
    let p = layout.panel;
    let pad = scaled(20.0, sf).min(p.w / 8);
    frame.dim(130);
    frame.fill_rounded_rect(p.x, p.y, p.w, p.h, scaled(8.0, sf), colors.panel_bg, masks);
    frame.push_clip(Rect::new(p.x as f32, p.y as f32, p.w as f32, p.h as f32));
    let close = close_rect(&p, sf);
    text(
        frame,
        painter,
        &close,
        "Close ×",
        size_px(11.0, sf),
        colors.text_dim,
    );
    let title = WidgetRect {
        x: p.x + pad,
        y: p.y + scaled(18.0, sf),
        w: p.w.saturating_sub(pad * 2),
        h: scaled(28.0, sf),
    };
    text(
        frame,
        painter,
        &title,
        "Settings",
        size_px(20.0, sf),
        colors.text_bright,
    );
    if let Some(header) = &layout.header {
        frame.fill_rounded_rect(
            header.x,
            header.y,
            header.w,
            header.h,
            scaled(5.0, sf),
            colors.recessed_wash,
            masks,
        );
    }
    if let Some(header) = &spec.header {
        render_header(frame, painter, colors, header, layout, sf, cursor);
    }
    let chrome = chrome(&p, sf);
    if chrome.sidebar > 0 {
        frame.fill_rect_px(
            p.x + chrome.sidebar,
            chrome.nav_top,
            scaled(1.0, sf),
            (p.y + p.h).saturating_sub(chrome.nav_top + scaled(FOOT, sf)),
            colors.hairline,
        );
    }
    if let Some(tabs) = &spec.tabs {
        for (i, (r, (label, _))) in layout.tab_rects.iter().zip(tabs.tabs).enumerate() {
            if i == tabs.active {
                frame.fill_rounded_rect(
                    r.x,
                    r.y,
                    r.w,
                    r.h,
                    scaled(4.0, sf),
                    colors.selection_wash,
                    masks,
                );
            }
            let label_rect = WidgetRect {
                x: r.x + scaled(10.0, sf),
                y: r.y + scaled(8.0, sf),
                w: r.w.saturating_sub(scaled(20.0, sf)),
                h: r.h,
            };
            text(
                frame,
                painter,
                &label_rect,
                label,
                size_px(12.0, sf),
                if i == tabs.active {
                    colors.text_bright
                } else {
                    colors.text_dim
                },
            );
        }
    }
    if let Body::List {
        sections,
        scroll,
        max_visible,
        selected,
    } = &spec.body
    {
        let rows = flatten_rows(sections);
        let (start, _) = resolve_visible_window(&rows, *scroll, *max_visible);
        for (slot, rect) in layout.rows.iter().enumerate() {
            match rows.get(start + slot) {
                Some(DisplayRow::SectionHeader(title)) => {
                    let r = WidgetRect {
                        y: rect.y + scaled(18.0, sf),
                        ..*rect
                    };
                    text(
                        frame,
                        painter,
                        &r,
                        title,
                        size_px(18.0, sf),
                        colors.text_bright,
                    );
                }
                Some(DisplayRow::Row(row, index)) => {
                    if *index == *selected {
                        frame.fill_rect_px(
                            rect.x.saturating_sub(scaled(8.0, sf)),
                            rect.y + scaled(10.0, sf),
                            scaled(2.0, sf),
                            scaled(24.0, sf),
                            colors.accent,
                        );
                    }
                    let compact = rect.w < scaled(400.0, sf);
                    let control = controls(rect, sf);
                    let reserve = if let Accessory::SettingValue {
                        action: Some(_), ..
                    } = row.accessory
                    {
                        scaled(120.0, sf)
                    } else if compact {
                        0
                    } else {
                        match &row.accessory {
                            Accessory::Choices { labels, .. } if *labels == ["Off", "On"] => {
                                scaled(54.0, sf)
                            }
                            Accessory::Choices { labels, .. } => preset_rects(rect, labels, sf)
                                .first()
                                .map(|r| rect.x + rect.w - r.x + scaled(16.0, sf))
                                .unwrap_or(0),
                            Accessory::DimText(_) => scaled(120.0, sf),
                            Accessory::Keycaps(steps) => {
                                keycaps_width(painter, steps, sf) + scaled(16.0, sf)
                            }
                            _ => 0,
                        }
                    };
                    let label = WidgetRect {
                        x: rect.x,
                        y: rect.y + scaled(10.0, sf),
                        w: rect.w.saturating_sub(reserve),
                        h: rect.h,
                    };
                    text(
                        frame,
                        painter,
                        &label,
                        row.label,
                        size_px(13.0, sf),
                        colors.text_primary,
                    );
                    if !compact {
                        if let Some(detail) = row.detail {
                            let r = WidgetRect {
                                y: rect.y + scaled(36.0, sf),
                                w: rect.w,
                                ..label
                            };
                            text(
                                frame,
                                painter,
                                &r,
                                detail,
                                size_px(11.0, sf),
                                colors.text_dim,
                            );
                        }
                    }
                    match &row.accessory {
                        Accessory::SettingValue {
                            text: value,
                            action,
                        } => {
                            let value_rect = value_rect(rect, sf);
                            text(
                                frame,
                                painter,
                                &value_rect,
                                value,
                                size_px(11.0, sf),
                                colors.text_dim,
                            );
                            if let Some(action) = action {
                                let r = action_rect(rect, sf);
                                text(
                                    frame,
                                    painter,
                                    &r,
                                    action,
                                    size_px(11.0, sf),
                                    colors.accent_bright,
                                );
                            }
                        }
                        Accessory::Choices { labels, active } if *labels == ["Off", "On"] => {
                            let r = switch_rect(rect, sf);
                            let on = *active == Some(1);
                            frame.fill_rounded_rect(
                                r.x,
                                r.y,
                                r.w,
                                r.h,
                                r.h / 2,
                                if on {
                                    colors.accent
                                } else {
                                    colors.keycap_border
                                },
                                masks,
                            );
                            let inset = scaled(3.0, sf).min(r.h / 2);
                            let d = r.h.saturating_sub(inset * 2);
                            let x = if on {
                                r.x + r.w.saturating_sub(d + inset)
                            } else {
                                r.x + inset
                            };
                            frame.fill_rounded_rect(
                                x,
                                r.y + inset,
                                d,
                                d,
                                d / 2,
                                colors.text_bright,
                                masks,
                            );
                        }
                        Accessory::Choices { labels, active } => {
                            for (i, r) in preset_rects(rect, labels, sf).iter().enumerate() {
                                frame.fill_rounded_rect(
                                    r.x,
                                    r.y,
                                    r.w,
                                    r.h,
                                    scaled(4.0, sf),
                                    if *active == Some(i) {
                                        colors.selection_wash
                                    } else {
                                        colors.keycap_bg
                                    },
                                    masks,
                                );
                                let label = WidgetRect {
                                    x: r.x + scaled(6.0, sf),
                                    y: r.y + scaled(4.0, sf),
                                    w: r.w.saturating_sub(scaled(12.0, sf)),
                                    h: r.h,
                                };
                                text(
                                    frame,
                                    painter,
                                    &label,
                                    labels[i],
                                    size_px(11.0, sf),
                                    if *active == Some(i) {
                                        colors.accent_bright
                                    } else {
                                        colors.text_dim
                                    },
                                );
                            }
                        }
                        Accessory::Keycaps(steps) => {
                            let width = keycaps_width(painter, steps, sf).min(control.w);
                            let mut x = control.x + control.w - width;
                            let y = control.y + scaled(8.0, sf);
                            frame.push_clip(Rect::new(
                                control.x as f32,
                                control.y as f32,
                                control.w as f32,
                                control.h as f32,
                            ));
                            for (i, step) in steps.iter().enumerate() {
                                if i > 0 {
                                    x += scaled(dims::CHIP_STEP_GAP, sf);
                                }
                                for (j, chip) in step.iter().enumerate() {
                                    if j > 0 {
                                        x += scaled(dims::CHIP_GAP, sf);
                                    }
                                    x += super::super::frame::draw_keycap(
                                        frame,
                                        painter,
                                        masks,
                                        x,
                                        y,
                                        &chip.label,
                                        colors.keycap_bg,
                                        colors.keycap_border,
                                        colors.keycap_fg,
                                        sf,
                                    );
                                }
                            }
                            frame.pop_clip();
                        }
                        Accessory::DimText(value) => {
                            let r = WidgetRect {
                                x: control.x + control.w.saturating_sub(scaled(112.0, sf)),
                                y: control.y + scaled(10.0, sf),
                                w: scaled(112.0, sf).min(control.w),
                                h: control.h,
                            };
                            text(
                                frame,
                                painter,
                                &r,
                                value,
                                size_px(11.0, sf),
                                colors.accent_bright,
                            );
                        }
                        _ => {}
                    }
                    frame.fill_rect_px(
                        rect.x,
                        rect.y + rect.h - scaled(1.0, sf),
                        rect.w,
                        scaled(1.0, sf),
                        colors.hairline,
                    );
                }
                _ => {}
            }
        }
        if rows.is_empty() {
            let r = WidgetRect {
                x: p.x + chrome.sidebar + pad,
                y: chrome.body_top + scaled(20.0, sf),
                w: p.w / 2,
                h: scaled(30.0, sf),
            };
            text(
                frame,
                painter,
                &r,
                "No matching settings",
                size_px(13.0, sf),
                colors.text_dim,
            );
        }
    }
    if let Some(r) = &layout.scrollbar {
        frame.fill_rect_px(r.x, r.y, r.w, r.h, colors.text_dim);
    }
    if let (Some(footer), Some(rect)) = (&spec.footer, layout.footer) {
        render_footer(frame, painter, colors, footer, rect, sf, 0, masks);
    }
    frame.pop_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_page_keeps_spacious_categories_and_shared_control_hits() {
        for (width, height, scale) in [
            (1100, 720, 1.0),
            (400, 750, 1.0),
            (800, 300, 1.0),
            (1600, 1100, 2.0),
        ] {
            let model = crate::model::AppModel::new(width, height, scale, vec![]);
            let state = crate::model::ui::SettingsState::default();
            crate::view::modal::with_settings_spec(&model, &state, |spec| {
                assert!(matches!(spec.anchor, Anchor::Settings { .. }));
                let geometry = super::super::layout(spec, width as usize, height as usize, scale);
                assert!(geometry.panel.h > height as usize * 4 / 5);
                assert_eq!(geometry.row_height, scaled(ROW, scale));
                let tabs = spec.tabs.as_ref().unwrap();
                assert_eq!(tabs.tabs[0].0, "All Settings");
                assert!(tabs.tabs.iter().any(|(label, _)| *label == "Appearance"));
                for (index, rect) in geometry.tab_rects.iter().enumerate() {
                    assert_eq!(
                        hit_test(spec, &geometry, rect.x + rect.w / 2, rect.y + rect.h / 2),
                        OverlayHit::Tab(index)
                    );
                }
                if width == 1100 {
                    assert!(geometry.tab_rects[0].x < geometry.rows[0].x);
                    assert!(geometry.tab_rects[1].y > geometry.tab_rects[0].y);
                }
            });
            for query in ["Theme", "scrollbar"] {
                let mut state = crate::model::ui::SettingsState::default();
                state.editable.set_content(query);
                state.refilter();
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let geometry =
                        super::super::layout(spec, width as usize, height as usize, scale);
                    let row = geometry
                        .rows
                        .iter()
                        .find(|row| {
                            hit_test(spec, &geometry, row.x + 1, row.y + 1)
                                == OverlayHit::Row(FlatIndex(0))
                        })
                        .unwrap();
                    let control = if query == "Theme" {
                        action_rect(row, scale)
                    } else {
                        switch_rect(row, scale)
                    };
                    assert!(matches!(
                        hit_test(
                            spec,
                            &geometry,
                            control.x + control.w / 2,
                            control.y + control.h / 2
                        ),
                        OverlayHit::Choice {
                            row: FlatIndex(0),
                            ..
                        }
                    ));
                });
            }
        }
    }

    #[test]
    fn compact_settings_rows_paint_values_beneath_the_label() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        for (query, expected) in [
            ("Theme", "custom-theme"),
            ("rust-analyzer command", "custom-analyzer"),
        ] {
            let mut model = crate::model::AppModel::new(400, 750, 1.0, vec![]);
            model.config.theme = "custom-theme".into();
            model
                .config
                .lsp
                .servers
                .entry("rust-analyzer".into())
                .or_default()
                .command = Some("custom-analyzer".into());
            let mut state = crate::model::ui::SettingsState::default();
            state.editable.set_content(query);
            state.refilter();
            let mut glyph_cache = crate::view::GlyphCache::default();
            let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
            let mut buffer = vec![0; 400 * 750];
            let mut frame = Frame::new(&mut buffer, 400, 750);
            frame.push_clip(Rect::new(0.0, 0.0, 400.0, 750.0));
            let colors = Palette::from_theme(&OverlayTheme::default_dark());
            crate::view::modal::with_settings_spec(&model, &state, |spec| {
                let Body::List { sections, .. } = &spec.body else {
                    panic!()
                };
                assert!(matches!(&sections[0].rows[0].accessory,
                    Accessory::SettingValue { text, .. } if *text == expected));
                let geometry = super::super::layout(spec, 400, 750, 1.0);
                let row = geometry
                    .rows
                    .iter()
                    .find(|r| {
                        hit_test(spec, &geometry, r.x + 1, r.y + 1) == OverlayHit::Row(FlatIndex(0))
                    })
                    .unwrap();
                let value = value_rect(row, 1.0);
                assert!(value.y + value.h <= row.y + row.h);
                render(
                    &mut frame,
                    &mut painter,
                    &mut RoundedRectMaskCache::new(),
                    &colors,
                    spec,
                    &geometry,
                    1.0,
                    true,
                );
                // The page and shared header must leave their enclosing clip intact.
                frame.pop_clip();
                let mut ink = 0;
                for y in value.y..value.y + value.h {
                    for x in value.x..value.x + value.w {
                        ink += usize::from(frame.get_pixel(x, y) != colors.panel_bg);
                    }
                }
                assert!(
                    ink > 20,
                    "configuration value must actually be painted: {expected}"
                );
                assert!(spec.footer.as_ref().unwrap().leading.contains(expected));
            });
        }
    }
}
