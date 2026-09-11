//! Settings-form presentation for OverlaySurface. Painting and input consume
//! the same resolved rectangles; metadata and scrolling remain shared.
use super::*;
use crate::view::{TextFieldOptions, TextFieldRenderer};

fn item_height(
    row: &DisplayRow<'_>,
    sf: f64,
    viewport_height: usize,
    width: usize,
    form: bool,
) -> usize {
    match row {
        DisplayRow::Row(
            Row {
                accessory:
                    Accessory::SettingInput {
                        content,
                        line_height,
                        ..
                    },
                ..
            },
            _,
        ) => {
            let lines = if content.constraints.allow_multiline {
                3
            } else {
                1
            };
            let padding = scaled(
                if width >= scaled(440.0, sf) {
                    24.0
                } else {
                    60.0
                },
                sf,
            );
            let minimum = padding + line_height;
            (padding + lines * line_height).min(viewport_height.max(minimum))
        }
        DisplayRow::Row(
            Row {
                accessory: Accessory::Choices { labels, .. },
                ..
            },
            _,
        ) => {
            if form {
                return row_height(sf);
            }
            let rect = WidgetRect {
                x: 0,
                y: 0,
                w: width,
                h: row_height(sf),
            };
            preset_rects(&rect, labels, sf)
                .last()
                .map_or(row_height(sf), |last| {
                    row_height(sf).max(last.y + last.h + scaled(8.0, sf))
                })
        }
        _ => row_height(sf),
    }
}

fn input_rect(rect: Rect, sf: f64, browse: bool) -> Rect {
    if rect.width >= scaled(440.0, sf) as f32 {
        let label = scaled(124.0, sf) as f32;
        let action = if browse {
            scaled(120.0, sf) as f32
        } else {
            0.0
        };
        return Rect::new(
            rect.x + label,
            rect.y + scaled(12.0, sf) as f32,
            (rect.width - label - action - scaled(8.0, sf) as f32).max(0.0),
            (rect.height - scaled(24.0, sf) as f32).max(0.0),
        );
    }
    Rect::new(
        rect.x + scaled(8.0, sf) as f32,
        rect.y + scaled(48.0, sf) as f32,
        (rect.width - scaled(16.0, sf) as f32).max(0.0),
        (rect.height - scaled(60.0, sf) as f32).max(0.0),
    )
}

pub(super) fn field_options(row: &Row, rect: Rect, sf: f64) -> Option<TextFieldOptions> {
    let Accessory::SettingInput {
        content,
        line_height,
        char_width,
        browse,
        ..
    } = row.accessory
    else {
        return None;
    };
    Some(TextFieldOptions::for_text_area(
        content,
        input_rect(rect, sf, browse),
        line_height,
        char_width,
    ))
}
fn choice_width(label: &str, sf: f64) -> usize {
    scaled(label.chars().count() as f32 * 7.0 + 16.0, sf)
}

fn is_checkbox(labels: &[&str]) -> bool {
    labels == ["Off", "On"]
}

fn is_disclosure(labels: &[&str]) -> bool {
    labels == ["Show"] || labels == ["Hide"]
}

fn checkbox_rect(row: &WidgetRect, sf: f64) -> WidgetRect {
    let size = scaled(14.0, sf);
    WidgetRect {
        x: row.x + row.w.saturating_sub(size + scaled(4.0, sf)),
        y: row.y + scaled(10.0, sf),
        w: size,
        h: size,
    }
}

fn select_rect(row: &WidgetRect, sf: f64) -> WidgetRect {
    let inset = if row.w >= scaled(440.0, sf) {
        scaled(124.0, sf)
    } else {
        0
    };
    WidgetRect {
        x: row.x + inset,
        y: row.y + scaled(if inset == 0 { 26.0 } else { 4.0 }, sf),
        w: row.w.saturating_sub(inset),
        h: scaled(29.0, sf),
    }
}

/// Dropdown options share one layout for painting and pointer selection.
fn select_options<'a>(
    spec: &'a OverlaySpec,
    layout: &OverlayLayout,
) -> Vec<(FlatIndex, usize, &'a str, WidgetRect)> {
    let Some(index) = collection(spec).and_then(|collection| collection.open_select) else {
        return Vec::new();
    };
    let Body::List { sections, .. } = &spec.body else {
        return Vec::new();
    };
    let rows = flatten_rows(sections);
    let Some((rect, labels)) =
        layout
            .rows
            .iter()
            .zip(&layout.settings_items)
            .find_map(|(rect, (display, _))| match rows.get(*display) {
                Some(DisplayRow::Row(
                    Row {
                        accessory: Accessory::Choices { labels, .. },
                        ..
                    },
                    row,
                )) if row.0 == index => Some((rect, labels)),
                _ => None,
            })
    else {
        return Vec::new();
    };
    let sf = layout.scale_factor;
    let anchor = select_rect(rect, sf);
    let bottom = layout
        .footer
        .map_or(layout.panel.y + layout.panel.h, |footer| footer.y);
    let height = scaled(28.0, sf).min(bottom.saturating_sub(layout.panel.y) / labels.len().max(1));
    let total = height * labels.len();
    let y = (anchor.y + anchor.h)
        .min(bottom.saturating_sub(total))
        .max(layout.panel.y);
    labels
        .iter()
        .enumerate()
        .map(|(choice, label)| {
            (
                FlatIndex(index),
                choice,
                *label,
                WidgetRect {
                    x: anchor.x,
                    y: y + choice * height,
                    w: anchor.w,
                    h: height,
                },
            )
        })
        .collect()
}

fn draw_checkbox(
    frame: &mut Frame,
    painter: &mut TextPainter,
    rect: WidgetRect,
    enabled: bool,
    colors: &Palette,
    sf: f64,
) {
    frame.draw_bordered_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        if enabled {
            colors.accent
        } else {
            colors.recessed_wash
        },
        colors.hairline,
    );
    if enabled {
        text(
            frame,
            painter,
            &rect,
            "✓",
            size_px(12.0, sf),
            colors.text_bright,
        );
    }
}

fn preset_rects(row: &WidgetRect, labels: &[&str], scale_factor: f64) -> Vec<WidgetRect> {
    if labels.is_empty() {
        return Vec::new();
    }
    let control = controls(row, scale_factor);
    let budget = if row.w < scaled(400.0, scale_factor) {
        row.w
    } else {
        row.w * 2 / 3
    };
    let gap = scaled(dims::CHIP_GAP, scale_factor);
    let widths: Vec<_> = labels
        .iter()
        .map(|label| choice_width(label, scale_factor))
        .collect();
    let total = widths.iter().sum::<usize>() + gap * labels.len().saturating_sub(1);
    if total > budget {
        // Long selectors wrap below the label/description instead of squeezing
        // every option into indistinguishable fragments. Height, paint and hit
        // testing all consume these same rectangles.
        let mut x = row.x;
        let mut y = row.y
            + scaled(
                if row.w < scaled(400.0, scale_factor) {
                    26.0
                } else {
                    50.0
                },
                scale_factor,
            );
        let h = scaled(22.0, scale_factor);
        return widths
            .into_iter()
            .map(|width| {
                let w = width.min(row.w);
                if x > row.x && x + w > row.x + row.w {
                    x = row.x;
                    y += h + gap;
                }
                let rect = WidgetRect { x, y, w, h };
                x += w + gap;
                rect
            })
            .collect();
    }
    let mut x = control.x + control.w.saturating_sub(total);
    let h = scaled(22.0, scale_factor).min(control.h);
    widths
        .into_iter()
        .map(|w| {
            let rect = WidgetRect {
                x,
                y: control.y + control.h.saturating_sub(h) / 2,
                w,
                h,
            };
            x += w + gap;
            rect
        })
        .collect()
}

const ROW: f32 = 56.0;
const TOP: f32 = 49.0;
const FOOT: f32 = 52.0;
const NAV_STEP: f32 = 33.0;

fn row_height(scale: f64) -> usize {
    scaled(ROW, scale)
}

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
    let top = scaled(TOP, sf);
    let categories = crate::settings::categories().len();
    let sidebar = if wide(p, sf)
        && p.h.saturating_sub(top + scaled(FOOT, sf)) >= categories * scaled(NAV_STEP, sf)
    {
        scaled(169.0, sf)
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
    let nav_height = categories.div_ceil(columns) * scaled(NAV_STEP, sf);
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
        header_y: p.y + scaled(12.0, sf),
        header_h: scaled(30.0, sf),
    }
}

fn collection<'a>(spec: &'a OverlaySpec) -> Option<&'a SettingsCollection<'a>> {
    match &spec.anchor {
        Anchor::Settings { collection, .. } => collection.as_ref(),
        _ => None,
    }
}

struct ManagerRects {
    heading: WidgetRect,
    add: WidgetRect,
    master: WidgetRect,
    records: Rect,
    detail: Rect,
}

fn manager_rects(p: &WidgetRect, sf: f64) -> ManagerRects {
    let chrome = chrome(p, sf);
    let x = p.x + chrome.sidebar;
    let w = p.w.saturating_sub(chrome.sidebar);
    let top = chrome.body_top;
    let heading_h = scaled(104.0, sf);
    let rail = scaled(221.0, sf).min(w / 3);
    let bottom = p.y + p.h.saturating_sub(scaled(FOOT, sf));
    let body_y = (top + heading_h).min(bottom);
    ManagerRects {
        master: WidgetRect {
            x: x + scaled(26.0, sf),
            y: top + scaled(77.0, sf),
            w: scaled(14.0, sf),
            h: scaled(14.0, sf),
        },
        heading: WidgetRect {
            x: x + scaled(26.0, sf),
            y: top + scaled(22.0, sf),
            w: w.saturating_sub(scaled(150.0, sf)),
            h: heading_h,
        },
        add: WidgetRect {
            x: x + w.saturating_sub(scaled(100.0, sf)),
            y: top + scaled(24.0, sf),
            w: scaled(74.0, sf),
            h: scaled(29.0, sf),
        },
        records: Rect::new(
            x as f32,
            body_y as f32,
            rail as f32,
            bottom.saturating_sub(body_y) as f32,
        ),
        detail: Rect::new(
            (x + rail) as f32,
            body_y as f32,
            w.saturating_sub(rail) as f32,
            bottom.saturating_sub(body_y) as f32,
        ),
    }
}

/// Pixel viewport shared by rendering, hit testing, scrolling and row reveal.
pub(crate) fn scroll_viewport(
    width: usize,
    height: usize,
    sf: f64,
    content_height: usize,
    offset: usize,
) -> crate::layout::RowListView {
    let p = panel(width, height, sf);
    let chrome = chrome(&p, sf);
    crate::layout::RowListView::from_pixel_scroll(
        Rect::new(
            (p.x + chrome.sidebar) as f32,
            chrome.body_top as f32,
            p.w.saturating_sub(chrome.sidebar) as f32,
            (p.y + p.h).saturating_sub(chrome.body_top + scaled(FOOT, sf)) as f32,
        ),
        1.0,
        content_height,
        offset,
    )
}

/// Full-row capacity for keyboard Page Up/Down, not pointer scrolling.
pub fn visible_count(width: usize, height: usize, sf: f64) -> usize {
    (scroll_viewport(width, height, sf, 0, 0).visible_capacity() / row_height(sf).max(1)).max(1)
}

pub(super) fn layout(
    spec: &OverlaySpec,
    out: &mut OverlayLayout,
    width: usize,
    height: usize,
    sf: f64,
) {
    let p = panel(width, height, sf);
    let pad = scaled(16.0, sf).min(p.w / 8);
    let chrome = chrome(&p, sf);
    let manager = collection(spec).map(|_| manager_rects(&p, sf));
    let content_pad = if manager.is_some() {
        scaled(26.0, sf)
    } else {
        pad
    };
    let sidebar = chrome.sidebar;
    let detail_sidebar = sidebar
        + manager
            .as_ref()
            .map_or(0, |rects| rects.records.width as usize);
    let nav_top = chrome.nav_top;
    out.panel = p;
    out.header = Some(WidgetRect {
        x: p.x + sidebar.max(scaled(108.0, sf)) + pad,
        y: chrome.header_y,
        w: p.w.saturating_sub(sidebar.max(scaled(108.0, sf)) + pad * 2),
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
                            y: nav_top + i * scaled(NAV_STEP, sf),
                            w: sidebar - pad,
                            h: scaled(28.0, sf),
                        }
                    } else {
                        let w = p.w.saturating_sub(pad * 2) / chrome.nav_columns;
                        WidgetRect {
                            x: p.x + pad + (i % chrome.nav_columns) * w,
                            y: nav_top + (i / chrome.nav_columns) * scaled(NAV_STEP, sf),
                            w,
                            h: scaled(28.0, sf),
                        }
                    }
                })
                .collect()
        })
        .unwrap_or_default();
    let (items, offset) = match &spec.body {
        Body::List {
            sections, scroll, ..
        } => (flatten_rows(sections), *scroll),
        _ => (Vec::new(), 0),
    };
    let mut total = if manager.is_some() {
        scaled(16.0, sf)
    } else {
        0
    };
    let viewport_height = manager
        .as_ref()
        .map_or_else(
            || scroll_viewport(width, height, sf, 0, 0).rect(),
            |manager| manager.detail,
        )
        .height
        .max(0.0) as usize;
    let positions: Vec<_> = items
        .iter()
        .map(|item| {
            let start = total;
            let footer_action = matches!((item, &spec.anchor), (DisplayRow::Row(_, index), Anchor::Settings { actions_row: Some(row), .. }) if index.0 == *row);
            if !footer_action { total += item_height(
                item,
                sf,
                viewport_height,
                p.w.saturating_sub(detail_sidebar + content_pad * 2),
                manager.is_some(),
            ); }
            start..total
        })
        .collect();
    out.settings_positions = items
        .iter()
        .zip(&positions)
        .filter_map(|(item, range)| matches!(item, DisplayRow::Row(..)).then_some(range.clone()))
        .collect();
    let viewport = if let (Some(rects), Some(records)) = (&manager, collection(spec)) {
        let record_view = crate::layout::RowListView::from_pixel_scroll(
            rects.records,
            scaled(64.0, sf) as f32,
            records.records.len(),
            records.scroll,
        );
        let rect = rects.records;
        out.settings_records_scrollbar = (record_view.max_scroll_pixels() > 0).then(|| {
            list_scrollbar(
                WidgetRect {
                    x: rect.x as usize,
                    y: rect.y as usize,
                    w: rect.width as usize,
                    h: rect.height as usize,
                },
                ScrollbarState::new(
                    record_view.content_height_pixels(),
                    rect.height as usize,
                    record_view.scroll_offset_pixels(),
                ),
                sf,
            )
        });
        out.settings_records_viewport = Some(record_view);
        crate::layout::RowListView::from_pixel_scroll(rects.detail, 1.0, total, offset)
    } else {
        scroll_viewport(width, height, sf, total, offset)
    };
    let body = viewport.rect();
    out.settings_viewport = Some(viewport);
    out.row_height = row_height(sf);
    let visible = viewport.drawn_range();
    out.settings_items = positions
        .iter()
        .enumerate()
        .filter(|(_, range)| {
            !range.is_empty() && range.start < visible.end && range.end > visible.start
        })
        .map(|(index, range)| {
            (
                index,
                Rect::new(
                    (p.x + detail_sidebar + content_pad) as f32,
                    body.y + range.start as f32 - viewport.scroll_offset_pixels() as f32,
                    p.w.saturating_sub(detail_sidebar + content_pad * 2) as f32,
                    range.len() as f32,
                ),
            )
        })
        .collect();
    out.rows = out
        .settings_items
        .iter()
        .map(|(_, rect)| WidgetRect {
            x: rect.x as usize,
            y: rect.y.max(0.0) as usize,
            w: rect.width as usize,
            h: rect.height as usize,
        })
        .collect();
    out.footer = Some(WidgetRect {
        x: p.x,
        y: p.y + p.h.saturating_sub(scaled(FOOT, sf)),
        w: p.w,
        h: scaled(FOOT, sf),
    });
    out.scrollbar = if viewport.max_scroll_pixels() > 0 {
        let body = WidgetRect {
            x: body.x as usize,
            y: body.y as usize,
            w: body.width as usize,
            h: body.height as usize,
        };
        Some(list_scrollbar(
            body,
            ScrollbarState::new(
                viewport.content_height_pixels(),
                body.h,
                viewport.scroll_offset_pixels(),
            ),
            sf,
        ))
    } else {
        None
    };
}

fn breadcrumb_rect(p: &WidgetRect, sf: f64) -> WidgetRect {
    let w = scaled(100.0, sf).min(p.w / 3);
    WidgetRect {
        x: p.x + scaled(16.0, sf),
        y: p.y + scaled(12.0, sf),
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
            y: rect.y + scaled(26.0, sf),
            w: rect.w,
            h: scaled(22.0, sf),
        }
    } else {
        WidgetRect {
            x: rect.x,
            y: rect.y,
            w: rect.w,
            h: scaled(32.0, sf),
        }
    }
}

fn value_rect(rect: &WidgetRect, sf: f64) -> WidgetRect {
    WidgetRect {
        x: rect.x,
        y: rect.y + scaled(30.0, sf),
        w: rect.w,
        h: scaled(24.0, sf),
    }
}

fn action_rect(rect: &WidgetRect, sf: f64) -> WidgetRect {
    WidgetRect {
        x: rect.x + rect.w.saturating_sub(scaled(112.0, sf)),
        y: rect.y + scaled(4.0, sf),
        w: scaled(112.0, sf).min(rect.w),
        h: scaled(22.0, sf).min(rect.h),
    }
}

pub(super) fn hit_test(
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    x: usize,
    y: usize,
) -> OverlayHit {
    // On a draft, the Settings breadcrumb returns to the main page. There is
    // no header close button; Escape retains the modal's normal close action.
    if collection(spec).is_none()
        && matches!(spec.anchor, Anchor::Settings { subpage: true, .. })
        && contains(&breadcrumb_rect(&layout.panel, layout.scale_factor), x, y)
    {
        return OverlayHit::Close;
    }
    if !contains(&layout.panel, x, y) {
        return OverlayHit::Outside;
    }
    if collection(spec).is_some_and(|collection| collection.open_select.is_some()) {
        for (row, choice, _, rect) in select_options(spec, layout) {
            if contains(&rect, x, y) {
                return OverlayHit::Choice { row, choice };
            }
        }
        return OverlayHit::SettingsAction(crate::messages::SettingsCollectionAction::CloseSelect);
    }
    if collection(spec).is_some() {
        let manager = manager_rects(&layout.panel, layout.scale_factor);
        if contains(&manager.master, x, y) {
            return OverlayHit::SettingsAction(
                crate::messages::SettingsCollectionAction::ToggleMaster,
            );
        }
        if contains(&manager.add, x, y) {
            return OverlayHit::SettingsAction(crate::messages::SettingsCollectionAction::Add);
        }
        if let Some(viewport) = layout.settings_records_viewport {
            if viewport.rect().contains(x as f32, y as f32) {
                return viewport
                    .row_at_y(y as f32)
                    .map_or(OverlayHit::Inside, |index| {
                        OverlayHit::SettingsAction(
                            crate::messages::SettingsCollectionAction::Select(index),
                        )
                    });
            }
        }
    }
    if layout
        .footer
        .as_ref()
        .is_some_and(|rect| contains(rect, x, y))
    {
        for (row, choice, _, rect) in footer_actions(spec, layout) {
            if contains(&rect, x, y) {
                return OverlayHit::Choice { row, choice };
            }
        }
        return OverlayHit::Inside;
    }
    if let Some(index) = layout.tab_rects.iter().position(|r| contains(r, x, y)) {
        return OverlayHit::Tab(index);
    }
    if let (Body::List { sections, .. }, Some(viewport)) = (&spec.body, layout.settings_viewport) {
        if !viewport.rect().contains(x as f32, y as f32) {
            return OverlayHit::Inside;
        }
        let rows = flatten_rows(sections);
        for (rect, (display_index, raw)) in layout.rows.iter().zip(&layout.settings_items) {
            if !raw.contains(x as f32, y as f32) {
                continue;
            }
            if let Some(DisplayRow::Row(row, index)) = rows.get(*display_index) {
                if let Accessory::SettingInput { browse, .. } = row.accessory {
                    if browse && contains(&action_rect(rect, layout.scale_factor), x, y) {
                        return OverlayHit::Choice {
                            row: *index,
                            choice: 0,
                        };
                    }
                    if input_rect(*raw, layout.scale_factor, browse).contains(x as f32, y as f32) {
                        if let Some(opts) = field_options(row, *raw, layout.scale_factor) {
                            return OverlayHit::Input {
                                row: *index,
                                position: opts.position_at(x as f64, y as f64),
                            };
                        }
                    }
                }
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
                    if collection(spec).is_some() && is_disclosure(labels) {
                        return OverlayHit::Choice {
                            row: *index,
                            choice: 0,
                        };
                    }
                    if is_checkbox(labels) {
                        if contains(&checkbox_rect(rect, layout.scale_factor), x, y) {
                            return OverlayHit::Choice {
                                row: *index,
                                choice: usize::from(*active != Some(1)),
                            };
                        }
                        return OverlayHit::Row(*index);
                    }
                    if collection(spec).is_some() && labels.len() > 1 {
                        if contains(&select_rect(rect, layout.scale_factor), x, y) {
                            return OverlayHit::SettingsAction(
                                crate::messages::SettingsCollectionAction::ToggleSelect(index.0),
                            );
                        }
                        return OverlayHit::Row(*index);
                    }
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
    let value = painter.truncate_sized(value, size, rect.w as f32, EllipsisSide::End);
    painter.draw_sized(frame, rect.x, rect.y, &value, size, 0.0, color);
}

fn settings_button(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &crate::theme::Theme,
    rect: WidgetRect,
    label: &str,
    state: crate::view::button::ButtonState,
    sf: f64,
) {
    use crate::view::button::{render_button, ButtonStyle};
    render_button(
        frame,
        painter,
        theme,
        Rect::new(rect.x as f32, rect.y as f32, rect.w as f32, rect.h as f32),
        label,
        ButtonStyle {
            state,
            focused: false,
            text_size: Some(size_px(11.0, sf)),
        },
    );
}

fn select_button(
    frame: &mut Frame,
    painter: &mut TextPainter,
    colors: &Palette,
    rect: WidgetRect,
    label: &str,
    open: bool,
    sf: f64,
) {
    frame.draw_bordered_rect(
        rect.x,
        rect.y,
        rect.w,
        rect.h,
        colors.recessed_wash,
        if open { colors.accent } else { colors.hairline },
    );
    let label_rect = WidgetRect {
        x: rect.x + scaled(8.0, sf),
        y: rect.y + scaled(7.0, sf),
        w: rect.w.saturating_sub(scaled(32.0, sf)),
        ..rect
    };
    text(
        frame,
        painter,
        &label_rect,
        label,
        size_px(12.0, sf),
        colors.text_primary,
    );
    text(
        frame,
        painter,
        &WidgetRect {
            x: rect.x + rect.w.saturating_sub(scaled(20.0, sf)),
            w: scaled(14.0, sf),
            ..label_rect
        },
        "▾",
        size_px(12.0, sf),
        colors.text_dim,
    );
}

/// Fixed form actions use the same row/action identifiers as keyboard input.
/// This geometry is shared by painting and pointer hit testing.
fn footer_actions<'a>(
    spec: &'a OverlaySpec,
    layout: &OverlayLayout,
) -> Vec<(FlatIndex, usize, &'a str, WidgetRect)> {
    let Anchor::Settings {
        actions_row: Some(index),
        ..
    } = spec.anchor
    else {
        return Vec::new();
    };
    let (Body::List { sections, .. }, Some(footer)) = (&spec.body, layout.footer) else {
        return Vec::new();
    };
    let rows = flatten_rows(sections);
    let Some(DisplayRow::Row(
        Row {
            accessory: Accessory::Choices { labels, .. },
            ..
        },
        _,
    )) = rows
        .iter()
        .find(|row| matches!(row, DisplayRow::Row(_, i) if i.0 == index))
    else {
        return Vec::new();
    };
    let sf = layout.scale_factor;
    let mut right = footer.x + footer.w.saturating_sub(scaled(16.0, sf));
    labels
        .iter()
        .enumerate()
        .filter(|(_, label)| !label.is_empty())
        .map(|(choice, label)| {
            let w = choice_width(label, sf);
            let rect = WidgetRect {
                x: right.saturating_sub(w),
                y: footer.y + scaled(10.0, sf),
                w,
                h: scaled(24.0, sf),
            };
            right = rect.x.saturating_sub(scaled(6.0, sf));
            (FlatIndex(index), choice, *label, rect)
        })
        .collect()
}

// Mirrors the shared overlay renderer's explicit paint context.
#[allow(clippy::too_many_arguments)]
pub(super) fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    masks: &mut RoundedRectMaskCache,
    colors: &Palette,
    theme: &crate::theme::Theme,
    spec: &OverlaySpec,
    layout: &OverlayLayout,
    sf: f64,
    cursor: bool,
) {
    let p = layout.panel;
    let pad = scaled(16.0, sf).min(p.w / 8);
    let radius = scaled(8.0, sf);
    // Settings is a solid preferences page, even when other themed overlays
    // are translucent. Preserve the theme's RGB and only normalize opacity.
    let panel_bg = colors.panel_bg | 0xFF00_0000;
    render_backdrop(frame, p, radius, panel_bg, 130);
    frame.fill_rounded_rect(p.x, p.y, p.w, p.h, radius, panel_bg, masks);
    frame.push_clip(Rect::new(p.x as f32, p.y as f32, p.w as f32, p.h as f32));
    let top_height = scaled(TOP, sf);
    let chrome_bg = colors.panel_secondary | 0xFF00_0000;
    frame.fill_rect_top_rounded(p.x, p.y, p.w, top_height, radius, chrome_bg, masks);
    frame.fill_rect_px(
        p.x,
        p.y + top_height - scaled(1.0, sf),
        p.w,
        scaled(1.0, sf),
        colors.hairline,
    );
    let subpage = matches!(spec.anchor, Anchor::Settings { subpage: true, .. });
    let title = WidgetRect {
        x: p.x + pad,
        y: p.y + scaled(20.0, sf),
        w: scaled(104.0, sf),
        h: scaled(28.0, sf),
    };
    text(
        frame,
        painter,
        &title,
        if subpage && collection(spec).is_none() {
            "‹ Settings"
        } else {
            "Settings"
        },
        size_px(14.0, sf),
        colors.text_bright,
    );
    if !subpage {
        if let Some(header) = &layout.header {
            frame.fill_rounded_rect(
                header.x,
                header.y,
                header.w,
                header.h,
                scaled(2.0, sf),
                colors.recessed_wash,
                masks,
            );
        }
        if let Some(header) = &spec.header {
            render_header(frame, painter, colors, header, layout, sf, cursor);
        }
    } else if let (Some(header), Some(rect)) = (&spec.header, &layout.header) {
        let rect = WidgetRect {
            y: rect.y + scaled(8.0, sf),
            ..*rect
        };
        text(
            frame,
            painter,
            &rect,
            &collection(spec).map_or_else(
                || format!("› {}", header.text),
                |collection| format!("Preferences  /  {}", collection.title),
            ),
            size_px(13.0, sf),
            colors.text_primary,
        );
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
                    scaled(2.0, sf),
                    colors.keycap_bg,
                    masks,
                );
            }
            let label_rect = WidgetRect {
                x: r.x + scaled(10.0, sf),
                y: r.y + scaled(7.0, sf),
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
    if let Some(collection) = collection(spec) {
        let manager = manager_rects(&p, sf);
        draw_checkbox(
            frame,
            painter,
            manager.master,
            collection.enabled,
            colors,
            sf,
        );
        text(
            frame,
            painter,
            &WidgetRect {
                x: manager.master.x + scaled(23.0, sf),
                y: manager.master.y,
                w: manager.heading.w,
                h: manager.master.h,
            },
            collection.enable_label,
            size_px(12.0, sf),
            colors.text_primary,
        );
        text(
            frame,
            painter,
            &manager.heading,
            collection.title,
            size_px(20.0, sf),
            colors.text_bright,
        );
        text(
            frame,
            painter,
            &WidgetRect {
                y: manager.heading.y + scaled(31.0, sf),
                ..manager.heading
            },
            collection.description,
            size_px(12.0, sf),
            colors.text_dim,
        );
        settings_button(
            frame,
            painter,
            theme,
            manager.add,
            "+ Add",
            if collection.hovered == Some(crate::messages::SettingsCollectionAction::Add) {
                crate::view::button::ButtonState::Hovered
            } else {
                crate::view::button::ButtonState::Normal
            },
            sf,
        );
        let rect = manager.records;
        frame.fill_rect_px(
            rect.x as usize,
            rect.y as usize,
            rect.width as usize,
            rect.height as usize,
            colors.panel_secondary | 0xFF00_0000,
        );
        frame.fill_rect_px(
            rect.x as usize,
            rect.y as usize,
            (rect.width + manager.detail.width) as usize,
            scaled(1.0, sf),
            colors.hairline,
        );
        frame.fill_rect_px(
            (rect.x + rect.width) as usize,
            rect.y as usize,
            scaled(1.0, sf),
            rect.height as usize,
            colors.hairline,
        );
        if let Some(viewport) = layout.settings_records_viewport {
            frame.push_clip(viewport.rect());
            if collection.records.is_empty() {
                text(
                    frame,
                    painter,
                    &WidgetRect {
                        x: rect.x as usize + scaled(18.0, sf),
                        y: rect.y as usize + scaled(22.0, sf),
                        w: (rect.width as usize).saturating_sub(scaled(36.0, sf)),
                        h: scaled(20.0, sf),
                    },
                    "No saved entries yet",
                    size_px(11.0, sf),
                    colors.text_dim,
                );
            }
            for index in viewport.drawn_range() {
                let Some(record) = collection.records.get(index) else {
                    continue;
                };
                let Some(rect) = viewport.row_rect(index) else {
                    continue;
                };
                let r = WidgetRect {
                    x: rect.x as usize + scaled(8.0, sf),
                    y: rect.y.max(0.0) as usize + scaled(6.0, sf),
                    w: (rect.width as usize).saturating_sub(scaled(16.0, sf)),
                    h: scaled(52.0, sf),
                };
                if collection.selected == Some(record.id)
                    || collection.hovered
                        == Some(crate::messages::SettingsCollectionAction::Select(index))
                {
                    frame.fill_rounded_rect(
                        r.x,
                        r.y,
                        r.w,
                        r.h,
                        scaled(3.0, sf),
                        if collection.selected == Some(record.id) {
                            colors.selection_wash
                        } else {
                            colors.keycap_bg
                        },
                        masks,
                    );
                }
                text(
                    frame,
                    &mut painter.with_font(crate::view::FontRole::Code),
                    &WidgetRect {
                        x: r.x + scaled(10.0, sf),
                        y: r.y + scaled(8.0, sf),
                        w: r.w.saturating_sub(scaled(20.0, sf)),
                        ..r
                    },
                    record.id,
                    size_px(12.0, sf),
                    colors.text_primary,
                );
                text(
                    frame,
                    painter,
                    &WidgetRect {
                        x: r.x + scaled(10.0, sf),
                        y: r.y + scaled(30.0, sf),
                        w: r.w.saturating_sub(scaled(20.0, sf)),
                        ..r
                    },
                    &record.detail,
                    size_px(10.0, sf),
                    colors.text_dim,
                );
                if record.enabled {
                    frame.fill_rect_px(
                        r.x + r.w.saturating_sub(scaled(9.0, sf)),
                        r.y + scaled(12.0, sf),
                        scaled(3.0, sf),
                        scaled(3.0, sf),
                        colors.accent_bright,
                    );
                }
            }
            frame.pop_clip();
        }
        if let Some(bar) = &layout.settings_records_scrollbar {
            render_scrollbar(frame, bar, false, &ScrollbarColors::from(&theme.scrollbar));
        }
    }
    if let (
        Body::List {
            sections, selected, ..
        },
        Some(viewport),
    ) = (&spec.body, layout.settings_viewport)
    {
        frame.push_clip(viewport.rect());
        let rows = flatten_rows(sections);
        for (rect, (display_index, raw)) in layout.rows.iter().zip(&layout.settings_items) {
            match rows.get(*display_index) {
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
                        size_px(15.0, sf),
                        colors.text_bright,
                    );
                }
                Some(DisplayRow::Row(row, index)) => {
                    let disclosure = collection(spec).is_some()
                        && matches!(&row.accessory, Accessory::Choices {labels, ..} if is_disclosure(labels));
                    let button_state = |choice, active| {
                        use crate::view::button::ButtonState;
                        if active {
                            ButtonState::Pressed
                        } else if matches!(spec.anchor, Anchor::Settings { hovered_choice: Some((r, c)), .. } if r == index.0 && c == choice)
                        {
                            ButtonState::Hovered
                        } else {
                            ButtonState::Normal
                        }
                    };
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
                    let horizontal_field = rect.w >= scaled(440.0, sf)
                        && matches!(row.accessory, Accessory::SettingInput { .. });
                    let control = controls(rect, sf);
                    let reserve = if matches!(
                        row.accessory,
                        Accessory::SettingValue {
                            action: Some(_),
                            ..
                        } | Accessory::SettingInput { browse: true, .. }
                    ) {
                        scaled(120.0, sf)
                    } else if compact {
                        0
                    } else {
                        match &row.accessory {
                            Accessory::Choices { labels, .. } => preset_rects(rect, labels, sf)
                                .first()
                                .filter(|r| r.y < rect.y + scaled(28.0, sf))
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
                        y: rect.y + scaled(8.0, sf),
                        w: if horizontal_field
                            || (collection(spec).is_some()
                                && matches!(&row.accessory, Accessory::Choices {labels, ..} if labels.len() > 1 && !is_checkbox(labels)))
                        {
                            scaled(114.0, sf)
                        } else {
                            rect.w.saturating_sub(reserve)
                        },
                        h: rect.h,
                    };
                    text(
                        frame,
                        painter,
                        &label,
                        if disclosure {
                            if collection(spec).is_some_and(|collection| collection.advanced) {
                                "▾  Advanced"
                            } else {
                                "▸  Advanced"
                            }
                        } else if horizontal_field {
                            row.label.split(" (JSON").next().unwrap_or(row.label)
                        } else {
                            row.label
                        },
                        size_px(12.0, sf),
                        colors.text_primary,
                    );
                    if collection(spec).is_none()
                        && !horizontal_field
                        && (!compact || matches!(row.accessory, Accessory::SettingInput { .. }))
                    {
                        if let Some(detail) = row.detail {
                            let r = WidgetRect {
                                y: rect.y + scaled(30.0, sf),
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
                        Accessory::SettingInput {
                            content,
                            focused,
                            browse,
                            ..
                        } => {
                            let input = input_rect(*raw, sf, *browse);
                            let bg_y = (input.y - scaled(4.0, sf) as f32).max(0.0) as usize;
                            frame.draw_bordered_rect(
                                input.x as usize - scaled(4.0, sf),
                                bg_y,
                                input.width as usize + scaled(8.0, sf),
                                (input.y + input.height + scaled(4.0, sf) as f32).max(bg_y as f32)
                                    as usize
                                    - bg_y,
                                colors.recessed_wash,
                                if *focused {
                                    colors.accent
                                } else {
                                    colors.hairline
                                },
                            );
                            if *browse {
                                settings_button(
                                    frame,
                                    painter,
                                    theme,
                                    action_rect(rect, sf),
                                    "Browse…",
                                    button_state(0, false),
                                    sf,
                                );
                            }
                            if let Some(mut opts) = field_options(row, *raw, sf) {
                                opts.cursor_visible = cursor && *focused;
                                opts.text_color = colors.text_primary;
                                opts.cursor_color = colors.accent_bright;
                                opts.selection_color = colors.selection_wash;
                                frame.push_clip(input);
                                TextFieldRenderer::render(frame, painter, *content, &opts);
                                frame.pop_clip();
                            }
                        }
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
                                settings_button(
                                    frame,
                                    painter,
                                    theme,
                                    r,
                                    action,
                                    button_state(0, false),
                                    sf,
                                );
                            }
                        }
                        Accessory::Choices { labels, active } => {
                            if disclosure {
                                // The entire disclosure row is interactive.
                            } else if is_checkbox(labels) {
                                let r = checkbox_rect(rect, sf);
                                draw_checkbox(frame, painter, r, *active == Some(1), colors, sf);
                            } else if collection(spec).is_some() && labels.len() > 1 {
                                let r = select_rect(rect, sf);
                                select_button(
                                    frame,
                                    painter,
                                    colors,
                                    r,
                                    active
                                        .and_then(|index| labels.get(index))
                                        .copied()
                                        .unwrap_or("Select…"),
                                    collection(spec).is_some_and(|collection| {
                                        collection.open_select == Some(index.0)
                                    }),
                                    sf,
                                );
                            } else {
                                for (i, r) in preset_rects(rect, labels, sf).iter().enumerate() {
                                    settings_button(
                                        frame,
                                        painter,
                                        theme,
                                        *r,
                                        labels[i],
                                        button_state(i, *active == Some(i)),
                                        sf,
                                    );
                                }
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
                    if collection(spec).is_none() {
                        frame.fill_rect_px(
                            rect.x,
                            rect.y + rect.h - scaled(1.0, sf),
                            rect.w,
                            scaled(1.0, sf),
                            colors.hairline,
                        );
                    }
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
        frame.pop_clip();
    }
    render_list_scrollbar(frame, layout, colors);
    for (row, choice, label, rect) in select_options(spec, layout) {
        let hovered = matches!(spec.anchor, Anchor::Settings {hovered_choice: Some((r,c)), ..} if r == row.0 && c == choice)
            || collection(spec).is_some_and(|collection| collection.select_cursor == choice);
        settings_button(
            frame,
            painter,
            theme,
            rect,
            label,
            if hovered {
                crate::view::button::ButtonState::Hovered
            } else {
                crate::view::button::ButtonState::Normal
            },
            sf,
        );
    }
    if let (Some(footer), Some(rect)) = (&spec.footer, layout.footer) {
        let actions = footer_actions(spec, layout);
        let text_width = actions
            .iter()
            .map(|(_, _, _, rect)| rect.x.saturating_sub(p.x + scaled(12.0, sf)))
            .min()
            .unwrap_or(rect.w);
        let leading = fit_with_ellipsis(
            painter,
            footer.leading,
            size_px(SIZE_META, sf),
            text_width.saturating_sub(scaled(dims::HEADER_PAD_X * 2.0, sf)),
        );
        let footer = Footer {
            leading: &leading,
            trailing: if actions.is_empty() {
                footer.trailing
            } else {
                ""
            },
        };
        render_footer(
            frame,
            painter,
            colors,
            &footer,
            rect,
            sf,
            radius,
            colors.panel_secondary | 0xFF00_0000,
            masks,
        );
        for (row, choice, label, rect) in actions {
            use crate::view::button::ButtonState;
            let state = if matches!(spec.anchor, Anchor::Settings { hovered_choice: Some((r, c)), .. } if r == row.0 && c == choice)
            {
                ButtonState::Hovered
            } else if choice == 0 {
                ButtonState::Pressed
            } else {
                ButtonState::Normal
            };
            settings_button(frame, painter, theme, rect, label, state, sf);
        }
    }
    frame.pop_clip();
}

#[cfg(test)]
mod tests {
    #[test]
    fn settings_collection_dropdown_and_record_targets_match_layout() {
        use crate::settings::{forms::SettingsForm, SettingsState};
        for (width, height, scale) in [(1200, 900, 1.0), (700, 600, 1.0), (1200, 900, 2.0)] {
            let model = crate::model::AppModel::new(width, height, scale);
            let mut state = SettingsState {
                form: Some(SettingsForm::inline_provider(None, &model.config)),
                ..SettingsState::default()
            };
            state.refresh_entries(&model.config);
            let index = state
                .entries
                .iter()
                .position(|row| matches!(row.kind, crate::settings::RowKind::FormChoice(0)))
                .unwrap();
            state.form.as_mut().unwrap().open_select = Some(index);
            crate::view::modal::with_settings_spec(&model, &state, |spec| {
                let layout = super::super::layout(spec, width as usize, height as usize, scale);
                let options = select_options(spec, &layout);
                assert_eq!(options.len(), 5);
                for (row, choice, _, rect) in options {
                    assert_eq!(
                        super::super::hit_test(
                            spec,
                            &layout,
                            rect.x + rect.w / 2,
                            rect.y + rect.h / 2
                        ),
                        OverlayHit::Choice { row, choice }
                    );
                    assert!(rect.y + rect.h <= layout.footer.unwrap().y);
                }
            });
            state.form = Some(SettingsForm::language_server(
                Some("rust-analyzer"),
                &model.config,
            ));
            state.refresh_entries(&model.config);
            crate::view::modal::with_settings_spec(&model, &state, |spec| {
                let layout = super::super::layout(spec, width as usize, height as usize, scale);
                let viewport = layout.settings_records_viewport.unwrap();
                for index in viewport.drawn_range() {
                    let rect = viewport.row_rect(index).unwrap();
                    if !viewport
                        .rect()
                        .contains(rect.x + 10.0, rect.y + rect.height / 2.0)
                    {
                        continue;
                    }
                    assert_eq!(
                        hit_test(
                            spec,
                            &layout,
                            (rect.x + 10.0) as usize,
                            (rect.y + rect.height / 2.0) as usize
                        ),
                        OverlayHit::SettingsAction(
                            crate::messages::SettingsCollectionAction::Select(index)
                        )
                    );
                }
            });
        }
    }
    use super::*;

    #[test]
    fn settings_form_actions_stay_in_footer_at_every_scroll_offset() {
        for (width, height, scale) in [(1200, 900, 1.0), (400, 550, 1.0), (800, 1100, 2.0)] {
            let model = crate::model::AppModel::new(width, height, scale);
            let mut state = crate::settings::SettingsState::new(&model.config);
            state.form = Some(crate::settings::forms::SettingsForm::language_server(
                Some("rust-analyzer"),
                &model.config,
            ));
            state.refresh_entries(&model.config);
            for offset in [0, 51, 9000] {
                state.scroll_offset_px = offset;
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let geometry =
                        super::super::layout(spec, width as usize, height as usize, scale);
                    let footer = geometry.footer.unwrap();
                    let actions = footer_actions(spec, &geometry);
                    assert_eq!(actions.len(), 4);
                    for (row, choice, _, rect) in actions {
                        assert!(rect.x >= footer.x && rect.x + rect.w <= footer.x + footer.w);
                        assert!(rect.y >= footer.y && rect.y + rect.h <= footer.y + footer.h);
                        assert_eq!(
                            hit_test(spec, &geometry, rect.x + rect.w / 2, rect.y + rect.h / 2),
                            OverlayHit::Choice { row, choice }
                        );
                    }
                });
            }
        }
    }

    #[test]
    fn settings_long_choices_wrap_with_nonoverlapping_in_bounds_hit_targets() {
        let labels = &[
            "llama.cpp",
            "Ollama",
            "OpenAI-compatible",
            "Mistral FIM",
            "Tabby",
        ];
        let row = Row {
            icon: RowIcon::None,
            label: "Transport",
            detail: Some("API protocol"),
            detail_style: None,
            match_indices: &[],
            accessory: Accessory::Choices {
                labels,
                active: Some(0),
            },
        };
        for sf in [1.0, 2.0] {
            for width in [260.0, 620.0, 920.0] {
                let width = scaled(width, sf);
                let height = item_height(
                    &DisplayRow::Row(&row, FlatIndex(0)),
                    sf,
                    scaled(600.0, sf),
                    width,
                    false,
                );
                let rect = WidgetRect {
                    x: 20,
                    y: 30,
                    w: width,
                    h: height,
                };
                let choices = preset_rects(&rect, labels, sf);
                assert_eq!(choices.len(), labels.len());
                for (index, choice) in choices.iter().enumerate() {
                    assert!(choice.x >= rect.x && choice.x + choice.w <= rect.x + rect.w);
                    assert!(choice.y >= rect.y && choice.y + choice.h <= rect.y + rect.h);
                    assert_eq!(
                        choice.w,
                        choice_width(labels[index], sf),
                        "labels fit without truncation"
                    );
                    for other in choices.iter().skip(index + 1) {
                        assert!(
                            choice.x + choice.w <= other.x
                                || other.x + other.w <= choice.x
                                || choice.y + choice.h <= other.y
                                || other.y + other.h <= choice.y
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn settings_form_fields_share_multiline_caret_pointer_and_scrolled_geometry() {
        use crate::settings::{forms::SettingsForm, SettingsState};
        for scale in [1.0, 2.0] {
            let mut model = crate::model::AppModel::new(1000, 740, scale);
            model.char_width = 8.0 * scale as f32;
            model.line_height = (20.0 * scale) as usize;
            let mut form = SettingsForm::language_server(Some("rust-analyzer"), &model.config);
            form.advanced = true;
            form.fields[2]
                .input
                .set_content("{\n  \"check\": {\n    \"command\": \"clippy\"\n  }\n}");
            form.fields[2]
                .input
                .set_cursor_position(crate::editable::Position::new(2, 8), false);
            form.focused = Some(2);
            let mut state = SettingsState {
                form: Some(form),
                ..SettingsState::default()
            };
            state.refresh_entries(&model.config);
            let advanced_row = state
                .entries
                .iter()
                .position(|row| matches!(row.kind, crate::settings::RowKind::FormField(2)))
                .unwrap();
            state.selected_index = advanced_row;
            let mut advanced_hits = 0;
            let mut advanced_geometry = Vec::new();
            // Cover fractional-row offsets throughout the form rather than
            // tying caret visibility to one particular spacing design.
            for offset in (0..=1000).step_by(37) {
                state.scroll_offset_px = (offset as f64 * scale) as usize;
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let layout = super::super::layout(spec, 1000, 740, scale);
                    let back = breadcrumb_rect(&layout.panel, scale);
                    assert_eq!(
                        hit_test(spec, &layout, back.x + back.w / 2, back.y + back.h / 2),
                        OverlayHit::Inside
                    );
                    let viewport = layout.settings_viewport.unwrap();
                    for (display, raw) in &layout.settings_items {
                        let Body::List { sections, .. } = &spec.body else {
                            unreachable!()
                        };
                        let flattened = flatten_rows(sections);
                        let DisplayRow::Row(row, index) = &flattened[*display] else {
                            continue;
                        };
                        let Some(opts) = field_options(row, *raw, scale) else {
                            continue;
                        };
                        let Some(caret) = TextFieldRenderer::caret_rect(
                            match row.accessory {
                                Accessory::SettingInput { content, .. } => content,
                                _ => unreachable!(),
                            },
                            &opts,
                        ) else {
                            continue;
                        };
                        if index.0 == advanced_row {
                            advanced_geometry.push((offset, *raw, caret, viewport.rect()));
                        }
                        if !viewport.rect().contains(caret.x as f32, caret.y as f32) {
                            continue;
                        }
                        let hit = hit_test(spec, &layout, caret.x, caret.y);
                        let OverlayHit::Input {
                            row: actual,
                            position,
                        } = hit
                        else {
                            panic!("expected field at {caret:?}, got {hit:?}")
                        };
                        assert_eq!(actual, *index);
                        if index.0 == advanced_row {
                            advanced_hits += 1;
                        }
                        let Accessory::SettingInput { content, .. } = row.accessory else {
                            unreachable!()
                        };
                        assert_eq!(
                            position,
                            crate::editable::Position::new(
                                content.cursor().line,
                                content.cursor().column
                            )
                        );
                    }
                    assert_eq!(
                        viewport.scroll_offset_pixels(),
                        state.scroll_offset_px.min(viewport.max_scroll_pixels())
                    );
                });
            }
            assert!(
                advanced_hits > 0,
                "the multiline field must be exercised at scale {scale}: {advanced_geometry:?}"
            );
        }
    }

    #[test]
    fn settings_page_is_opaque_even_with_a_translucent_overlay_theme() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut model = crate::model::AppModel::new(400, 500, 1.0);
        let state = crate::settings::SettingsState::default();
        for background in [0xFF123456, 0xFFFEDCBA] {
            let mut reference = Vec::new();
            for alpha in [255, 128, 0] {
                model.theme.overlay.panel_background = crate::theme::Color::rgba(38, 40, 44, alpha);
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let layout = super::super::layout(spec, 400, 500, 1.0);
                    let mut pixels = vec![background; 400 * 500];
                    let mut frame = Frame::new(&mut pixels, 400, 500);
                    let mut cache = crate::view::GlyphCache::default();
                    let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
                    render(
                        &mut frame,
                        &mut painter,
                        &mut RoundedRectMaskCache::new(),
                        &Palette::from_theme(&model.theme),
                        &model.theme,
                        spec,
                        &layout,
                        1.0,
                        false,
                    );
                    if alpha == 255 {
                        let p = layout.panel;
                        assert_eq!(pixels[(p.y + p.h / 2) * 400 + p.x + 4], 0xFF26282C);
                        assert_ne!(pixels[0], background, "the visible backdrop remains dimmed");
                        reference = pixels;
                    } else {
                        assert_eq!(
                            pixels, reference,
                            "Settings ignores overlay panel alpha={alpha}"
                        );
                    }
                });
            }
        }
    }

    #[test]
    fn settings_partial_rows_share_pixel_scroll_geometry_and_clipping() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        for (width, height, scale) in [(1100, 720, 1.0), (400, 750, 1.0), (1600, 1100, 2.0)] {
            let model = crate::model::AppModel::new(width, height, scale);
            let mut reference = Vec::new();
            for offset in [0, 13, 71, 95, usize::MAX] {
                let state = crate::settings::SettingsState {
                    scroll_offset_px: offset,
                    ..Default::default()
                };
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let layout = super::super::layout(spec, width as usize, height as usize, scale);
                    let viewport = layout.settings_viewport.unwrap();
                    let clip = viewport.rect();
                    let Body::List { sections, .. } = &spec.body else {
                        panic!("list expected")
                    };
                    let rows = flatten_rows(sections);
                    assert_eq!(
                        viewport.scroll_offset_pixels(),
                        offset.min(viewport.max_scroll_pixels())
                    );
                    // General settings still have uniform rows. Use that independent
                    // projection to check the form's variable-height layout.
                    let uniform = crate::layout::RowListView::from_pixel_scroll(
                        clip,
                        row_height(scale) as f32,
                        rows.len(),
                        offset,
                    );
                    for (index, rect) in uniform.drawn_range().zip(&layout.rows) {
                        assert_eq!(rect.y, uniform.row_rect(index).unwrap().y as usize);
                    }
                    let x = layout.rows[0].x + 1;
                    for y in 0..height as usize {
                        let hit = hit_test(spec, &layout, x, y);
                        if let Some(index) = uniform.row_at_y(y as f32) {
                            let expected = match &rows[index] {
                                DisplayRow::Row(_, index) => OverlayHit::Row(*index),
                                _ => OverlayHit::Inside,
                            };
                            assert_eq!(hit, expected, "hit at y={y}, offset={offset}");
                        } else {
                            assert!(!matches!(
                                hit,
                                OverlayHit::Row(_) | OverlayHit::Choice { .. }
                            ));
                        }
                    }
                    let mut pixels = vec![0; width as usize * height as usize];
                    let mut frame = Frame::new(&mut pixels, width as usize, height as usize);
                    let mut cache = crate::view::GlyphCache::default();
                    let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
                    render(
                        &mut frame,
                        &mut painter,
                        &mut RoundedRectMaskCache::new(),
                        &Palette::from_theme(&model.theme),
                        &model.theme,
                        spec,
                        &layout,
                        scale,
                        true,
                    );
                    if offset == 0 {
                        reference = pixels;
                    } else {
                        for y in 0..height as usize {
                            if y < clip.y as usize || y >= (clip.y + clip.height) as usize {
                                let range = y * width as usize..(y + 1) * width as usize;
                                assert_eq!(
                                    pixels[range.clone()],
                                    reference[range],
                                    "chrome overwritten at y={y}"
                                );
                            }
                        }
                        assert_ne!(pixels, reference, "scrolling must move painted content");
                    }
                });
            }
        }
    }

    #[test]
    fn settings_scrollbar_paints_the_shared_theme_and_tracks_display_rows() {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        for (width, height, scale) in [(1100, 720, 1.0), (400, 750, 1.0), (1600, 1100, 2.0)] {
            let model = crate::model::AppModel::new(width, height, scale);
            for offset in [0, usize::MAX] {
                let state = crate::settings::SettingsState {
                    scroll_offset_px: offset,
                    ..Default::default()
                };
                crate::view::modal::with_settings_spec(&model, &state, |spec| {
                    let geometry =
                        super::super::layout(spec, width as usize, height as usize, scale);
                    let bar = geometry.scrollbar.unwrap();
                    assert_eq!(
                        bar.track_rect.width,
                        (SCROLLBAR_WIDTH_LOGICAL * scale) as f32
                    );
                    assert_eq!(
                        bar.state.position,
                        if offset == 0 {
                            0
                        } else {
                            bar.state.max_position()
                        }
                    );
                    assert!(
                        bar.state.total / geometry.row_height > state.rows.len(),
                        "section headings occupy real viewport space"
                    );
                    assert_eq!(
                        super::super::hit_test(
                            spec,
                            &geometry,
                            (bar.thumb_rect.x + 3.0) as usize,
                            (bar.thumb_rect.y + 3.0) as usize
                        ),
                        OverlayHit::Scrollbar
                    );
                    let mut actual = vec![0; width as usize * height as usize];
                    let mut expected = actual.clone();
                    let mut frame = Frame::new(&mut actual, width as usize, height as usize);
                    let mut cache = crate::view::GlyphCache::default();
                    let mut painter = TextPainter::new(&font, &mut cache, 14.0, 11.0, 8.0, 18);
                    render(
                        &mut frame,
                        &mut painter,
                        &mut RoundedRectMaskCache::new(),
                        &Palette::from_theme(&model.theme),
                        &model.theme,
                        spec,
                        &geometry,
                        scale,
                        true,
                    );
                    let mut reference = Frame::new(&mut expected, width as usize, height as usize);
                    render_scrollbar(
                        &mut reference,
                        &bar,
                        false,
                        &ScrollbarColors::from(&model.theme.scrollbar),
                    );
                    for y in bar.track_rect.y as usize
                        ..(bar.track_rect.y + bar.track_rect.height) as usize
                    {
                        for x in bar.track_rect.x as usize
                            ..(bar.track_rect.x + bar.track_rect.width) as usize
                        {
                            assert_eq!(frame.get_pixel(x, y), reference.get_pixel(x, y));
                        }
                    }
                });
            }
        }
    }

    #[test]
    fn settings_page_keeps_categories_and_shared_control_hits() {
        for (width, height, scale) in [
            (1100, 720, 1.0),
            (400, 750, 1.0),
            (800, 300, 1.0),
            (1600, 1100, 2.0),
        ] {
            let model = crate::model::AppModel::new(width, height, scale);
            let state = crate::settings::SettingsState::default();
            crate::view::modal::with_settings_spec(&model, &state, |spec| {
                assert!(matches!(spec.anchor, Anchor::Settings { .. }));
                let geometry = super::super::layout(spec, width as usize, height as usize, scale);
                assert!(geometry.panel.h > height as usize * 4 / 5);
                assert_eq!(geometry.row_height, scaled(ROW, scale));
                let footer = geometry.footer.unwrap();
                assert_eq!((footer.x, footer.w), (geometry.panel.x, geometry.panel.w));
                assert_eq!(footer.y + footer.h, geometry.panel.y + geometry.panel.h);
                let breadcrumb = breadcrumb_rect(&geometry.panel, scale);
                assert_eq!(
                    hit_test(spec, &geometry, breadcrumb.x + 1, breadcrumb.y + 1),
                    OverlayHit::Inside
                );
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
                let mut state = crate::settings::SettingsState::default();
                state.editable.set_content(query);
                state.resolve_rows();
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
                        checkbox_rect(row, scale)
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
            let mut model = crate::model::AppModel::new(400, 750, 1.0);
            model.config.theme = "custom-theme".into();
            model
                .config
                .lsp
                .servers
                .entry("rust-analyzer".into())
                .or_default()
                .command = Some("custom-analyzer".into());
            let mut state = crate::settings::SettingsState::default();
            state.editable.set_content(query);
            state.resolve_rows();
            let mut glyph_cache = crate::view::GlyphCache::default();
            let mut painter = TextPainter::new(&font, &mut glyph_cache, 14.0, 11.0, 8.0, 18);
            let mut buffer = vec![0; 400 * 750];
            let mut frame = Frame::new(&mut buffer, 400, 750);
            frame.push_clip(Rect::new(0.0, 0.0, 400.0, 750.0));
            let colors = Palette::from_theme(&model.theme);
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
                    &model.theme,
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
