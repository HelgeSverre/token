//! One Performance body for docked and floating hosts. Its private boxes use
//! local layout handles; only host chrome needs globally addressable keys.

#[cfg(debug_assertions)]
use crate::layout::{
    snapshot::snap, CellMeasure, Dir, ElementDecl, LayoutNodeId, Padding, RowListView, ScrollDecl,
    Sizing, SizingAxes, UiTree,
};
use crate::model::editor_area::Rect;
use crate::model::AppModel;
use crate::perf::PerfStats;
#[cfg(debug_assertions)]
use crate::theme::Color;

#[cfg(debug_assertions)]
use super::scrollbar::{render_scrollbar, ScrollbarColors, ScrollbarGeometry, ScrollbarState};
use super::{Frame, TextPainter};

#[cfg(debug_assertions)]
use super::FontRole;
#[cfg(debug_assertions)]
use crate::perf::{CompletedPerfView, ObservationMode, PerfStage};

#[cfg(debug_assertions)]
#[derive(Clone, Copy)]
enum BreakdownRow {
    Stage(PerfStage),
    Untracked,
}

#[cfg(debug_assertions)]
struct BodyLayout {
    frame: Rect,
    hero: Rect,
    history: Rect,
    stats: Rect,
    breakdown: Rect,
    stage_heading: Rect,
    stage_rows: Rect,
    cache: Rect,
    cache_heading: Rect,
    cache_summary: Rect,
    note: Rect,
    scroll: RowListView,
    scrollbar_track: Option<Rect>,
}

#[cfg(debug_assertions)]
fn section(height: f32, pad: f32) -> ElementDecl {
    ElementDecl {
        dir: Dir::Column,
        sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height)),
        padding: Padding::xy(pad, pad),
        ..Default::default()
    }
}

#[cfg(debug_assertions)]
fn band(height: f32) -> ElementDecl {
    ElementDecl {
        sizing: SizingAxes::new(Sizing::GROW, Sizing::Fixed(height)),
        ..Default::default()
    }
}

#[cfg(debug_assertions)]
impl BodyLayout {
    fn new(model: &AppModel, viewport: Rect, row_count: usize) -> Self {
        let scale = model.metrics.scale_factor as f32;
        let pad = model.metrics.padding_large as f32;
        let row = breakdown_row_height(model) as f32;
        let frame_height = 246.0 * scale;
        let heading_height = (model.line_height * 3) as f32;
        let breakdown_height = pad * 2.0 + heading_height + row * row_count.max(1) as f32;
        let cache_height = 112.0 * scale;
        let note_height = 54.0 * scale;
        let content_height =
            (frame_height + breakdown_height + cache_height + note_height).ceil() as usize;
        let scroll = RowListView::from_pixel_scroll(
            viewport,
            1.0,
            content_height,
            model.ui.performance_scroll_px,
        );
        let scrollbar_track = (scroll.max_scroll_pixels() > 0).then(|| {
            let width = (10.0 * scale).min(viewport.width.max(0.0));
            Rect::new(
                viewport.x + viewport.width - width,
                viewport.y,
                width,
                viewport.height,
            )
        });
        let content_viewport = Rect::new(
            viewport.x,
            viewport.y,
            (viewport.width - scrollbar_track.map_or(0.0, |track| track.width)).max(0.0),
            viewport.height,
        );
        let mut tree = UiTree::new();
        let (
            _,
            (
                frame,
                hero,
                history,
                stats,
                breakdown,
                stage_heading,
                stage_rows,
                cache,
                cache_heading,
                cache_summary,
                note,
            ),
        ) = tree.node_with(
            ElementDecl {
                dir: Dir::Column,
                sizing: SizingAxes::grow(),
                clip: true,
                scroll: Some(ScrollDecl {
                    offset_y: scroll.scroll_offset_pixels() as f32,
                    ..Default::default()
                }),
                ..Default::default()
            },
            |tree| {
                let (frame, (hero, history, stats)) =
                    tree.node_with(section(frame_height, pad), |tree| {
                        (
                            tree.leaf(band(72.0 * scale)),
                            tree.leaf(band(112.0 * scale)),
                            tree.leaf(band(34.0 * scale)),
                        )
                    });
                let (breakdown, (stage_heading, stage_rows)) =
                    tree.node_with(section(breakdown_height, pad), |tree| {
                        (
                            tree.leaf(band(heading_height)),
                            tree.leaf(band(row * row_count.max(1) as f32)),
                        )
                    });
                let (cache, (cache_heading, cache_summary)) = tree
                    .node_with(section(cache_height, pad), |tree| {
                        (tree.leaf(band(34.0 * scale)), tree.leaf(band(52.0 * scale)))
                    });
                let note = tree.leaf(band(note_height));
                (
                    frame,
                    hero,
                    history,
                    stats,
                    breakdown,
                    stage_heading,
                    stage_rows,
                    cache,
                    cache_heading,
                    cache_summary,
                    note,
                )
            },
        );
        let mut measure = CellMeasure {
            char_width: model.char_width,
            line_height: model.line_height as f32,
        };
        let solved = tree.solve(content_viewport, model.metrics.scale_factor, &mut measure);
        let rect = |id: LayoutNodeId| {
            solved
                .local_rect(id)
                .expect("the body tree declares every local section")
        };
        Self {
            hero: rect(hero),
            history: rect(history),
            stats: rect(stats),
            frame: rect(frame),
            stage_heading: rect(stage_heading),
            stage_rows: rect(stage_rows),
            breakdown: rect(breakdown),
            cache_heading: rect(cache_heading),
            cache_summary: rect(cache_summary),
            cache: rect(cache),
            note: rect(note),
            scroll,
            scrollbar_track,
        }
    }
}

#[cfg(debug_assertions)]
fn breakdown_row_height(model: &AppModel) -> usize {
    model.line_height * 2 + model.metrics.padding_large + model.metrics.padding_medium
}

#[cfg(debug_assertions)]
fn stage_color(stage: PerfStage, background: Color) -> u32 {
    let argb = stage.color();
    Color::rgb((argb >> 16) as u8, (argb >> 8) as u8, argb as u8)
        .with_min_contrast(background, 4.5)
        .to_argb_u32()
}

#[cfg(debug_assertions)]
pub fn scroll_limit(model: &AppModel, viewport: Rect, perf: &PerfStats) -> usize {
    let view = perf.completed_view();
    let row_count = visible_stages(view).len() + usize::from(view.untracked_mean().is_some());
    BodyLayout::new(model, viewport, row_count)
        .scroll
        .max_scroll_pixels()
}

#[cfg(not(debug_assertions))]
pub fn scroll_limit(_model: &AppModel, _viewport: Rect, _perf: &PerfStats) -> usize {
    0
}

#[cfg(debug_assertions)]
fn visible_stages(view: CompletedPerfView<'_>) -> Vec<PerfStage> {
    PerfStage::ALL
        .into_iter()
        .filter(|stage| {
            view.samples()
                .any(|sample| !sample.stages[stage.index()].is_zero())
        })
        .collect()
}

#[cfg(debug_assertions)]
pub fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    model: &AppModel,
    viewport: Rect,
    perf: &PerfStats,
) {
    // Hosts reach this from redraw paths whose painters sit in different
    // roles; the panel is chrome, so it owns its font rather than inherit one.
    let mut painter = painter.with_font(FontRole::Code);
    let painter = &mut *painter;
    let view = perf.completed_view();
    let stages = visible_stages(view);
    let has_untracked = view.untracked_mean().is_some();
    let layout = BodyLayout::new(model, viewport, stages.len() + usize::from(has_untracked));
    let fg = model.theme.sidebar.foreground.to_argb_u32();
    let muted = model.theme.tab_bar.inactive_foreground.to_argb_u32();
    let accent = model.theme.syntax.type_name.to_argb_u32();
    let border = model.theme.sidebar.border.to_argb_u32();
    let stage_colors =
        PerfStage::ALL.map(|stage| stage_color(stage, model.theme.sidebar.background));
    frame.push_clip(viewport);
    frame.fill_rect(viewport, model.theme.sidebar.background.to_argb_u32());

    let (x, y, width, _) = snap(layout.hero);
    label(frame, painter, x, y, "Frame time", muted);
    let latest = view.latest().map(|s| s.frame_time);
    let latest_text = latest.map_or_else(|| "—".to_owned(), |d| format!("{:.2}", ms(d)));
    let value_width = painter.draw_sized(
        frame,
        x,
        y + (20.0 * model.metrics.scale_factor as f32) as usize,
        &latest_text,
        28.0 * model.metrics.scale_factor as f32,
        0.0,
        fg,
    );
    label(
        frame,
        painter,
        x + value_width.ceil() as usize + model.metrics.padding_small,
        y + (39.0 * model.metrics.scale_factor as f32) as usize,
        "ms",
        muted,
    );
    let mode = match view.latest().map(|s| s.observation_mode) {
        Some(ObservationMode::Hidden) => "latest: hidden",
        Some(ObservationMode::DockOpen) => "latest: docked",
        Some(ObservationMode::FloatingOpen) => "latest: floating",
        None => "no samples",
    };
    // A narrow dock cannot fit the heading and the full label side by side, so
    // fall back to the bare mode name instead of letting them touch.
    let heading_width = painter.measure_width("Frame time").ceil() as usize;
    let gap = model.metrics.padding_large;
    let full_width = painter.measure_width(mode).ceil() as usize;
    let mode = if heading_width + gap + full_width <= width {
        mode
    } else {
        mode.strip_prefix("latest: ").unwrap_or(mode)
    };
    right_label(frame, painter, x + width, y, mode, muted);
    let (hx, hy, hw, hh) = snap(layout.history);
    frame.draw_sparkline(
        hx,
        hy,
        hw,
        hh.saturating_sub(model.line_height),
        view.samples().map(|s| s.frame_time),
        accent,
        model.theme.editor.background.to_argb_u32(),
    );
    label(
        frame,
        painter,
        hx,
        hy + hh.saturating_sub(model.line_height),
        &format!(
            "{} of {} rendered frames",
            view.sample_count(),
            crate::perf::PERF_HISTORY_SIZE
        ),
        muted,
    );
    let (sx, sy, sw, _) = snap(layout.stats);
    for (i, (name, value)) in [
        ("Mean", view.frame_mean()),
        ("Median", view.frame_median()),
        ("P95", view.frame_p95()),
    ]
    .into_iter()
    .enumerate()
    {
        let cell_x = sx + i * sw / 3;
        label(frame, painter, cell_x, sy, name, muted);
        painter.draw(frame, cell_x, sy + model.line_height, &format_ms(value), fg);
    }
    rule(frame, layout.frame, border);

    let (bx, by, bw, _) = snap(layout.stage_heading);
    label(frame, painter, bx, by, "Render breakdown", fg);
    right_label(
        frame,
        painter,
        bx + bw,
        by,
        &format!("{} frames", view.sample_count()),
        muted,
    );
    label(frame, painter, bx, by + model.line_height, "STAGE", muted);
    let compact = bw < (330.0 * model.metrics.scale_factor as f32) as usize;
    right_label(
        frame,
        painter,
        bx + bw,
        by + model.line_height,
        "LATEST",
        muted,
    );
    let bar_y = by + model.line_height * 2 + model.metrics.padding_small;
    match view.latest() {
        Some(sample) if !sample.frame_time.is_zero() && sample.untracked.is_some() => {
            let mut cursor = bx;
            let mut tracked_ns = 0_u128;
            let frame_ns = sample.frame_time.as_nanos();
            for stage in PerfStage::ALL {
                tracked_ns += sample.stages[stage.index()].as_nanos();
                let end = bx + ((tracked_ns * bw as u128 + frame_ns / 2) / frame_ns) as usize;
                frame.fill_rect_px(
                    cursor,
                    bar_y,
                    end.saturating_sub(cursor),
                    4,
                    stage_colors[stage.index()],
                );
                cursor = end;
            }
            if sample.untracked.is_some_and(|duration| !duration.is_zero()) {
                frame.fill_rect_px(cursor, bar_y, bx + bw - cursor, 4, muted);
            }
        }
        Some(sample) if sample.untracked.is_none() => {
            let message = if compact {
                "Stages overlap · no composition"
            } else {
                "Overlapping stage timings · no composition bar"
            };
            label(frame, painter, bx, bar_y, message, muted);
        }
        Some(_) => label(frame, painter, bx, bar_y, "Zero-duration frame", muted),
        None => label(frame, painter, bx, bar_y, "No completed frames yet", muted),
    }
    let (rx, ry, rw, _) = snap(layout.stage_rows);
    let row_height = breakdown_row_height(model).max(1);
    if stages.is_empty() && !has_untracked {
        label(frame, painter, rx, ry, "No stage samples yet", muted);
    }
    let rows = stages
        .into_iter()
        .map(BreakdownRow::Stage)
        .chain(has_untracked.then_some(BreakdownRow::Untracked));
    for (i, row) in rows.enumerate() {
        let row_y = ry + i * row_height;
        if row_y + row_height < viewport.y.max(0.0) as usize
            || row_y > (viewport.y + viewport.height).max(0.0) as usize
        {
            continue;
        }
        let (name, color, latest, mean) = match row {
            BreakdownRow::Stage(stage) => (
                if compact {
                    stage.short_label()
                } else {
                    stage.label()
                },
                stage_colors[stage.index()],
                view.latest().map(|sample| sample.stages[stage.index()]),
                view.stage_mean(stage)
                    .map(|duration| (duration, view.sample_count())),
            ),
            BreakdownRow::Untracked => (
                "Untracked",
                muted,
                view.latest().and_then(|sample| sample.untracked),
                view.untracked_mean(),
            ),
        };
        let name =
            painter.truncate_to_width(name, (rw * if compact { 55 } else { 46 } / 100) as f32);
        painter.draw(frame, rx, row_y, &name, color);
        right_label(frame, painter, rx + rw, row_y, &format_duration(latest), fg);
        if !compact {
            frame.draw_sparkline(
                rx,
                row_y + model.line_height + model.metrics.padding_small,
                rw / 3,
                model
                    .line_height
                    .saturating_sub(model.metrics.padding_large),
                view.samples().map(|sample| match row {
                    BreakdownRow::Stage(stage) => Some(sample.stages[stage.index()]),
                    BreakdownRow::Untracked => sample.untracked,
                }),
                color,
                model.theme.sidebar.background.to_argb_u32(),
            );
        }
        let mean_text = mean.map_or_else(
            || "No composable samples".to_owned(),
            |(duration, count)| {
                let denominator = match row {
                    BreakdownRow::Stage(_) => "frames",
                    BreakdownRow::Untracked => "composable",
                };
                format!(
                    "Mean {} · {count} {denominator}",
                    format_duration(Some(duration))
                )
            },
        );
        right_label(
            frame,
            painter,
            rx + rw,
            row_y + model.line_height,
            &mean_text,
            muted,
        );
    }
    rule(frame, layout.breakdown, border);

    let (cx, cy, cw, _) = snap(layout.cache_heading);
    label(frame, painter, cx, cy, "Glyph cache", fg);
    right_label(frame, painter, cx + cw, cy, "Lifetime rate", muted);
    let (cx, cy, cw, _) = snap(layout.cache_summary);
    let rate = view
        .lifetime_cache_hit_rate()
        .map_or_else(|| "—".to_owned(), |rate| format!("{rate:.1}%"));
    painter.draw_sized(
        frame,
        cx,
        cy,
        &rate,
        23.0 * model.metrics.scale_factor as f32,
        0.0,
        fg,
    );
    let (hits, misses) = view.lifetime_cache_counts();
    let details = view.latest().map_or_else(
        || "No lookups yet".to_owned(),
        |s| {
            format!(
                "{} entries   {} hits   {} misses",
                s.cache_entries, hits, misses
            )
        },
    );
    let details = painter.truncate_to_width(&details, cw as f32);
    label(frame, painter, cx, cy + 30, &details, muted);
    rule(frame, layout.cache, border);

    let (nx, ny, nw, _) = snap(layout.note);
    let mixed = view.latest().is_some_and(|latest| {
        view.samples()
            .any(|sample| sample.observation_mode != latest.observation_mode)
    });
    let note = if view.latest().is_none() {
        "Diagnostic · awaiting rendered frame"
    } else if mixed {
        "Diagnostic · mixed capture modes"
    } else if view
        .latest()
        .is_some_and(|sample| sample.observation_mode == ObservationMode::Hidden)
    {
        "Diagnostic · latest frame captured hidden"
    } else {
        "Diagnostic · panel paint included"
    };
    let note = painter.truncate_to_width(
        note,
        nw.saturating_sub(model.metrics.padding_large * 2) as f32,
    );
    label(
        frame,
        painter,
        nx + model.metrics.padding_large,
        ny + model.metrics.padding_large,
        &note,
        muted,
    );
    if let Some(track) = layout.scrollbar_track {
        let state = ScrollbarState::new(
            layout.scroll.content_height_pixels(),
            viewport.height.max(0.0) as usize,
            layout.scroll.scroll_offset_pixels(),
        );
        render_scrollbar(
            frame,
            &ScrollbarGeometry::vertical(track, &state),
            false,
            &ScrollbarColors::from(&model.theme.scrollbar),
        );
    }
    frame.pop_clip();
}

#[cfg(not(debug_assertions))]
pub fn render(
    _frame: &mut Frame,
    _painter: &mut TextPainter,
    _model: &AppModel,
    _viewport: Rect,
    _perf: &PerfStats,
) {
}

#[cfg(debug_assertions)]
fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

#[cfg(debug_assertions)]
fn format_ms(duration: Option<std::time::Duration>) -> String {
    duration.map_or_else(|| "—".to_owned(), |d| format!("{:.2} ms", ms(d)))
}

#[cfg(debug_assertions)]
fn format_duration(duration: Option<std::time::Duration>) -> String {
    duration.map_or_else(
        || "—".to_owned(),
        |value| {
            if value.as_millis() == 0 {
                format!("{:.0} µs", value.as_secs_f64() * 1_000_000.0)
            } else {
                format_ms(Some(value))
            }
        },
    )
}

#[cfg(debug_assertions)]
fn label(frame: &mut Frame, painter: &mut TextPainter, x: usize, y: usize, text: &str, color: u32) {
    painter
        .with_font(FontRole::Code)
        .draw(frame, x, y, text, color);
}

#[cfg(debug_assertions)]
fn right_label(
    frame: &mut Frame,
    painter: &mut TextPainter,
    right: usize,
    y: usize,
    text: &str,
    color: u32,
) {
    let mut ui = painter.with_font(FontRole::Code);
    let width = ui.measure_width(text).round().max(0.0) as usize;
    ui.draw(frame, right.saturating_sub(width), y, text, color);
}

#[cfg(debug_assertions)]
fn rule(frame: &mut Frame, rect: Rect, color: u32) {
    let (x, y, width, height) = snap(rect);
    frame.fill_rect_px(x, y + height.saturating_sub(1), width, 1, color);
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;
    use crate::view::GlyphCache;

    #[test]
    fn scrollable_body_reserves_a_gutter_at_wide_and_narrow_widths() {
        let mut model = AppModel::new(400, 600, 1.0);
        model.line_height = 20;
        for width in [250.0, 400.0] {
            let layout = BodyLayout::new(&model, Rect::new(0.0, 0.0, width, 500.0), 4);
            let track = layout.scrollbar_track.expect("body should scroll");
            assert!(layout.stage_rows.x + layout.stage_rows.width <= track.x);
            assert!(layout.cache_summary.x + layout.cache_summary.width <= track.x);
        }
    }

    #[test]
    fn paints_the_same_whatever_font_role_the_host_painter_is_in() {
        let code = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let ui = fontdue::Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let model = AppModel::new(400, 600, 1.0);
        let perf = PerfStats::default();
        let viewport = Rect::new(0.0, 0.0, 300.0, 400.0);
        let paint = |role| {
            let (mut code_cache, mut ui_cache) = (GlyphCache::default(), GlyphCache::default());
            let mut painter = TextPainter::new(&code, &mut code_cache, 14.0, 11.0, 8.0, 18)
                .with_ui_font(&ui, &mut ui_cache, role);
            let mut buffer = vec![0u32; 300 * 400];
            render(
                &mut Frame::new(&mut buffer, 300, 400),
                &mut painter,
                &model,
                viewport,
                &perf,
            );
            buffer
        };
        assert!(paint(FontRole::Code) == paint(FontRole::Ui));
    }

    #[test]
    fn a_short_viewport_never_paints_outside_itself() {
        let code = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let ui = fontdue::Font::from_bytes(
            include_bytes!("../../assets/Inter-Regular.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut perf = PerfStats::default();
        for _ in 0..8 {
            perf.record_stage_elapsed(PerfStage::TabBar, std::time::Duration::from_micros(300));
            perf.record_frame_time();
            perf.record_render_history();
        }
        let mut model = AppModel::new(400, 600, 1.0);
        for scroll in [0, 40, 10_000] {
            model.ui.performance_scroll_px = scroll;
            for height in [60.0, 150.0, 300.0] {
                let viewport = Rect::new(20.0, 100.0, 260.0, height);
                let (mut code_cache, mut ui_cache) = (GlyphCache::default(), GlyphCache::default());
                let mut painter = TextPainter::new(&code, &mut code_cache, 14.0, 11.0, 8.0, 18)
                    .with_ui_font(&ui, &mut ui_cache, FontRole::Ui);
                const SENTINEL: u32 = 0xFF01_0203;
                let mut buffer = vec![SENTINEL; 300 * 600];
                render(
                    &mut Frame::new(&mut buffer, 300, 600),
                    &mut painter,
                    &model,
                    viewport,
                    &perf,
                );
                for (index, pixel) in buffer.iter().enumerate() {
                    let (x, y) = ((index % 300) as f32, (index / 300) as f32);
                    if !viewport.contains(x, y) {
                        assert_eq!(
                            *pixel, SENTINEL,
                            "painted outside at ({x}, {y}) height={height} scroll={scroll}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn stage_duration_uses_microseconds_below_one_millisecond() {
        assert_eq!(
            format_duration(Some(std::time::Duration::from_micros(453))),
            "453 µs"
        );
        assert_eq!(
            format_duration(Some(std::time::Duration::from_millis(2))),
            "2.00 ms"
        );
        assert_eq!(format_duration(None), "—");
    }
}
