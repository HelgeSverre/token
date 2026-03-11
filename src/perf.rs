//! Performance monitoring module
//!
//! Contains `PerfStats` for tracking frame timing and render breakdown.
//! In release builds, all timing methods compile to no-ops for zero overhead.

#[cfg(debug_assertions)]
use std::array::from_fn;
#[cfg(debug_assertions)]
use std::collections::VecDeque;
#[cfg(debug_assertions)]
use std::time::{Duration, Instant};

#[cfg(debug_assertions)]
use crate::overlay::{
    render_overlay_background, render_overlay_border, OverlayAnchor, OverlayConfig,
};
#[cfg(debug_assertions)]
use crate::theme::Theme;
#[cfg(debug_assertions)]
use crate::view::{Frame, TextPainter};

#[cfg(debug_assertions)]
pub const PERF_HISTORY_SIZE: usize = 60;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfStage {
    BuildPlan = 0,
    Clear,
    CursorFastPath,
    TabBar,
    TextBackground,
    TextDecorations,
    TextGlyphs,
    TextCursors,
    Gutter,
    Scrollbars,
    Csv,
    Image,
    BinaryPlaceholder,
    PreviewPane,
    Splitters,
    Sidebar,
    RightDock,
    BottomDock,
    StatusBar,
    Modal,
    DropOverlay,
    PerfOverlay,
    DebugOverlay,
    SurfaceAcquire,
    BufferCopy,
    SurfacePresent,
    WebviewSync,
    WebviewVisibility,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy)]
struct PerfStageSpec {
    label: &'static str,
    short_label: &'static str,
    color: u32,
}

#[cfg(feature = "profile-tracing")]
impl PerfStage {
    pub const fn tracing_label(&self) -> &'static str {
        match self {
            Self::BuildPlan => "build_plan",
            Self::Clear => "clear",
            Self::CursorFastPath => "cursor_fast_path",
            Self::TabBar => "tab_bar",
            Self::TextBackground => "text_background",
            Self::TextDecorations => "text_decorations",
            Self::TextGlyphs => "text_glyphs",
            Self::TextCursors => "text_cursors",
            Self::Gutter => "gutter",
            Self::Scrollbars => "scrollbars",
            Self::Csv => "csv",
            Self::Image => "image",
            Self::BinaryPlaceholder => "binary_placeholder",
            Self::PreviewPane => "preview_pane",
            Self::Splitters => "splitters",
            Self::Sidebar => "sidebar",
            Self::RightDock => "right_dock",
            Self::BottomDock => "bottom_dock",
            Self::StatusBar => "status_bar",
            Self::Modal => "modal",
            Self::DropOverlay => "drop_overlay",
            Self::PerfOverlay => "perf_overlay",
            Self::DebugOverlay => "debug_overlay",
            Self::SurfaceAcquire => "surface_acquire",
            Self::BufferCopy => "buffer_copy",
            Self::SurfacePresent => "surface_present",
            Self::WebviewSync => "webview_sync",
            Self::WebviewVisibility => "webview_visibility",
        }
    }
}

impl PerfStage {
    pub const ALL: [Self; 28] = [
        Self::BuildPlan,
        Self::Clear,
        Self::CursorFastPath,
        Self::TabBar,
        Self::TextBackground,
        Self::TextDecorations,
        Self::TextGlyphs,
        Self::TextCursors,
        Self::Gutter,
        Self::Scrollbars,
        Self::Csv,
        Self::Image,
        Self::BinaryPlaceholder,
        Self::PreviewPane,
        Self::Splitters,
        Self::Sidebar,
        Self::RightDock,
        Self::BottomDock,
        Self::StatusBar,
        Self::Modal,
        Self::DropOverlay,
        Self::PerfOverlay,
        Self::DebugOverlay,
        Self::SurfaceAcquire,
        Self::BufferCopy,
        Self::SurfacePresent,
        Self::WebviewSync,
        Self::WebviewVisibility,
    ];

    pub const COUNT: usize = Self::ALL.len();

    #[inline(always)]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline(always)]
    #[cfg(debug_assertions)]
    const fn spec(self) -> PerfStageSpec {
        match self {
            Self::BuildPlan => PerfStageSpec {
                label: "Build Plan",
                short_label: "Plan",
                color: 0xFF73DACA,
            },
            Self::Clear => PerfStageSpec {
                label: "Clear",
                short_label: "Clear",
                color: 0xFF7AA2F7,
            },
            Self::CursorFastPath => PerfStageSpec {
                label: "Cursor Fast Path",
                short_label: "Cursor",
                color: 0xFFBB9AF7,
            },
            Self::TabBar => PerfStageSpec {
                label: "Tab Bar",
                short_label: "Tabs",
                color: 0xFF9ECE6A,
            },
            Self::TextBackground => PerfStageSpec {
                label: "Text Background",
                short_label: "Text BG",
                color: 0xFF7AA2F7,
            },
            Self::TextDecorations => PerfStageSpec {
                label: "Text Decorations",
                short_label: "Decor",
                color: 0xFFBB9AF7,
            },
            Self::TextGlyphs => PerfStageSpec {
                label: "Text Glyphs",
                short_label: "Glyphs",
                color: 0xFFE0AF68,
            },
            Self::TextCursors => PerfStageSpec {
                label: "Text Cursors",
                short_label: "Cursor",
                color: 0xFFCBA6F7,
            },
            Self::Gutter => PerfStageSpec {
                label: "Gutter",
                short_label: "Gutter",
                color: 0xFF7DCFFF,
            },
            Self::Scrollbars => PerfStageSpec {
                label: "Scrollbars",
                short_label: "Scroll",
                color: 0xFF89DDFF,
            },
            Self::Csv => PerfStageSpec {
                label: "CSV",
                short_label: "CSV",
                color: 0xFF9ECE6A,
            },
            Self::Image => PerfStageSpec {
                label: "Image",
                short_label: "Image",
                color: 0xFFF7768E,
            },
            Self::BinaryPlaceholder => PerfStageSpec {
                label: "Binary",
                short_label: "Binary",
                color: 0xFFFF9E64,
            },
            Self::PreviewPane => PerfStageSpec {
                label: "Preview Pane",
                short_label: "Preview",
                color: 0xFF2AC3DE,
            },
            Self::Splitters => PerfStageSpec {
                label: "Splitters",
                short_label: "Split",
                color: 0xFFC0CAF5,
            },
            Self::Sidebar => PerfStageSpec {
                label: "Sidebar",
                short_label: "Sidebar",
                color: 0xFF73DACA,
            },
            Self::RightDock => PerfStageSpec {
                label: "Right Dock",
                short_label: "R Dock",
                color: 0xFF7DCFFF,
            },
            Self::BottomDock => PerfStageSpec {
                label: "Bottom Dock",
                short_label: "B Dock",
                color: 0xFF9ECE6A,
            },
            Self::StatusBar => PerfStageSpec {
                label: "Status Bar",
                short_label: "Status",
                color: 0xFFF7768E,
            },
            Self::Modal => PerfStageSpec {
                label: "Modal",
                short_label: "Modal",
                color: 0xFFBB9AF7,
            },
            Self::DropOverlay => PerfStageSpec {
                label: "Drop Overlay",
                short_label: "Drop",
                color: 0xFFE0AF68,
            },
            Self::PerfOverlay => PerfStageSpec {
                label: "Perf Overlay",
                short_label: "Perf",
                color: 0xFFCBA6F7,
            },
            Self::DebugOverlay => PerfStageSpec {
                label: "Debug Overlay",
                short_label: "Debug",
                color: 0xFFF5C2E7,
            },
            Self::SurfaceAcquire => PerfStageSpec {
                label: "Acquire Surface",
                short_label: "Acquire",
                color: 0xFF94E2D5,
            },
            Self::BufferCopy => PerfStageSpec {
                label: "Copy Buffer",
                short_label: "Copy",
                color: 0xFFFAB387,
            },
            Self::SurfacePresent => PerfStageSpec {
                label: "Present Surface",
                short_label: "Present",
                color: 0xFFFF9E64,
            },
            Self::WebviewSync => PerfStageSpec {
                label: "Webview Sync",
                short_label: "WV Sync",
                color: 0xFF89B4FA,
            },
            Self::WebviewVisibility => PerfStageSpec {
                label: "Webview Visibility",
                short_label: "WV Vis",
                color: 0xFFB4BEFE,
            },
        }
    }
}

#[cfg(debug_assertions)]
pub struct PerfStats {
    pub frame_start: Option<Instant>,
    pub last_frame_time: Duration,
    pub frame_times: VecDeque<Duration>,
    stage_times: [Duration; PerfStage::COUNT],
    stage_histories: [VecDeque<Duration>; PerfStage::COUNT],
    untracked_history: VecDeque<Duration>,
    pub frame_cache_hits: usize,
    pub frame_cache_misses: usize,
    pub total_cache_hits: usize,
    pub total_cache_misses: usize,
    pub show_overlay: bool,
    #[cfg(feature = "profile-tracing")]
    frame_span: Option<tracing::span::EnteredSpan>,
}

#[cfg(not(debug_assertions))]
pub struct PerfStats {
    #[cfg(feature = "profile-tracing")]
    frame_span: Option<tracing::span::EnteredSpan>,
}

#[cfg(not(debug_assertions))]
impl Default for PerfStats {
    fn default() -> Self {
        Self {
            #[cfg(feature = "profile-tracing")]
            frame_span: None,
        }
    }
}

#[cfg(debug_assertions)]
impl Default for PerfStats {
    fn default() -> Self {
        Self {
            frame_start: None,
            last_frame_time: Duration::ZERO,
            frame_times: VecDeque::new(),
            stage_times: [Duration::ZERO; PerfStage::COUNT],
            stage_histories: from_fn(|_| VecDeque::new()),
            untracked_history: VecDeque::new(),
            frame_cache_hits: 0,
            frame_cache_misses: 0,
            total_cache_hits: 0,
            total_cache_misses: 0,
            show_overlay: false,
            #[cfg(feature = "profile-tracing")]
            frame_span: None,
        }
    }
}

/// Timer guard that records elapsed time when dropped (debug only).
#[cfg(debug_assertions)]
pub struct TimerGuard<'a> {
    start: Instant,
    perf: &'a mut PerfStats,
    stage: PerfStage,
    #[cfg(feature = "profile-tracing")]
    _span: tracing::span::EnteredSpan,
}

#[cfg(not(debug_assertions))]
pub struct TimerGuard {
    #[cfg(feature = "profile-tracing")]
    _span: tracing::span::EnteredSpan,
}

#[cfg(debug_assertions)]
impl<'a> TimerGuard<'a> {
    fn new(perf: &'a mut PerfStats, stage: PerfStage) -> Self {
        Self {
            start: Instant::now(),
            perf,
            stage,
            #[cfg(feature = "profile-tracing")]
            _span: tracing::info_span!("render_stage", stage = stage.tracing_label()).entered(),
        }
    }
}

#[cfg(debug_assertions)]
impl Drop for TimerGuard<'_> {
    fn drop(&mut self) {
        self.perf
            .record_stage_elapsed(self.stage, self.start.elapsed());
    }
}

#[cfg(debug_assertions)]
impl PerfStats {
    #[inline(always)]
    pub fn reset_frame_stats(&mut self) {
        self.stage_times.fill(Duration::ZERO);
        self.frame_cache_hits = 0;
        self.frame_cache_misses = 0;
    }

    /// Accumulate cache statistics from a text painter.
    #[inline(always)]
    pub fn add_cache_stats(&mut self, hits: usize, misses: usize) {
        self.frame_cache_hits += hits;
        self.frame_cache_misses += misses;
        self.total_cache_hits += hits;
        self.total_cache_misses += misses;
    }

    #[inline(always)]
    pub fn start_frame(&mut self) {
        self.frame_start = Some(Instant::now());
        #[cfg(feature = "profile-tracing")]
        {
            self.frame_span = Some(tracing::info_span!("frame").entered());
        }
    }

    #[inline(always)]
    pub fn record_frame_time(&mut self) {
        if let Some(start) = self.frame_start.take() {
            self.last_frame_time = start.elapsed();
            self.frame_times.push_back(self.last_frame_time);
            if self.frame_times.len() > PERF_HISTORY_SIZE {
                self.frame_times.pop_front();
            }
        }
        #[cfg(feature = "profile-tracing")]
        {
            self.frame_span.take();
        }
    }

    #[inline(always)]
    pub fn record_render_history(&mut self) {
        for stage in PerfStage::ALL {
            let history = &mut self.stage_histories[stage.index()];
            history.push_back(self.stage_times[stage.index()]);
            if history.len() > PERF_HISTORY_SIZE {
                history.pop_front();
            }
        }

        self.untracked_history.push_back(self.untracked_time());
        if self.untracked_history.len() > PERF_HISTORY_SIZE {
            self.untracked_history.pop_front();
        }
    }

    #[inline(always)]
    pub fn time_stage(&mut self, stage: PerfStage) -> TimerGuard<'_> {
        TimerGuard::new(self, stage)
    }

    #[inline(always)]
    pub fn measure_stage<R>(&mut self, stage: PerfStage, f: impl FnOnce() -> R) -> R {
        #[cfg(feature = "profile-tracing")]
        let _span = tracing::info_span!("render_stage", stage = stage.tracing_label()).entered();

        let start = Instant::now();
        let result = f();
        self.record_stage_elapsed(stage, start.elapsed());
        result
    }

    #[inline(always)]
    pub fn record_stage_elapsed(&mut self, stage: PerfStage, elapsed: Duration) {
        self.stage_times[stage.index()] += elapsed;
    }

    #[inline(always)]
    pub fn should_show_overlay(&self) -> bool {
        self.show_overlay
    }

    #[inline(always)]
    pub fn stage_time(&self, stage: PerfStage) -> Duration {
        self.stage_times[stage.index()]
    }

    #[inline(always)]
    pub fn stage_history(&self, stage: PerfStage) -> &VecDeque<Duration> {
        &self.stage_histories[stage.index()]
    }

    #[inline(always)]
    pub fn tracked_time(&self) -> Duration {
        self.stage_times.iter().copied().sum()
    }

    #[inline(always)]
    pub fn untracked_time(&self) -> Duration {
        self.last_frame_time.saturating_sub(self.tracked_time())
    }

    #[inline(always)]
    pub fn untracked_history(&self) -> &VecDeque<Duration> {
        &self.untracked_history
    }

    pub fn avg_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().copied().sum();
        total / self.frame_times.len() as u32
    }

    pub fn render_throughput_per_sec(&self) -> f64 {
        let avg = self.avg_frame_time();
        if avg.as_secs_f64() > 0.0 {
            1.0 / avg.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn cache_hit_rate(&self) -> f64 {
        let total = self.total_cache_hits + self.total_cache_misses;
        if total > 0 {
            self.total_cache_hits as f64 / total as f64 * 100.0
        } else {
            0.0
        }
    }

    pub fn visible_stages(&self) -> Vec<PerfStage> {
        PerfStage::ALL
            .into_iter()
            .filter(|stage| {
                self.stage_time(*stage) > Duration::ZERO
                    || self
                        .stage_history(*stage)
                        .iter()
                        .any(|value| *value > Duration::ZERO)
            })
            .collect()
    }
}

#[cfg(not(debug_assertions))]
impl PerfStats {
    #[inline(always)]
    pub fn reset_frame_stats(&mut self) {}

    #[inline(always)]
    pub fn add_cache_stats(&mut self, _hits: usize, _misses: usize) {}

    #[inline(always)]
    pub fn start_frame(&mut self) {
        #[cfg(feature = "profile-tracing")]
        {
            self.frame_span = Some(tracing::info_span!("frame").entered());
        }
    }

    #[inline(always)]
    pub fn record_frame_time(&mut self) {
        #[cfg(feature = "profile-tracing")]
        {
            self.frame_span.take();
        }
    }

    #[inline(always)]
    pub fn record_render_history(&mut self) {}

    #[inline(always)]
    pub fn time_stage(&mut self, _stage: PerfStage) -> TimerGuard {
        #[cfg(feature = "profile-tracing")]
        return TimerGuard {
            _span: tracing::info_span!("render_stage", stage = _stage.tracing_label()).entered(),
        };
        #[cfg(not(feature = "profile-tracing"))]
        TimerGuard {}
    }

    #[inline(always)]
    pub fn measure_stage<R>(&mut self, _stage: PerfStage, f: impl FnOnce() -> R) -> R {
        #[cfg(feature = "profile-tracing")]
        let _span =
            tracing::info_span!("render_stage", stage = _stage.tracing_label()).entered();
        f()
    }

    #[inline(always)]
    pub fn record_stage_elapsed(&mut self, _stage: PerfStage, _elapsed: std::time::Duration) {}

    #[inline(always)]
    pub fn should_show_overlay(&self) -> bool {
        false
    }
}

#[cfg(debug_assertions)]
pub fn render_perf_overlay(
    frame: &mut Frame,
    painter: &mut TextPainter,
    perf: &PerfStats,
    theme: &Theme,
) {
    let width_usize = frame.width();
    let height_usize = frame.height();
    let line_height = painter.line_height();
    let scale = (line_height as f32 / 20.0).max(1.0);

    let active_stages = perf.visible_stages();
    let show_untracked = perf.untracked_time() > Duration::ZERO
        || perf
            .untracked_history()
            .iter()
            .any(|value| *value > Duration::ZERO);

    let padding_x = (10.0 * scale).round() as usize;
    let padding_y = (8.0 * scale).round() as usize;
    let row_gap = (4.0 * scale).round() as usize;
    let section_gap = (6.0 * scale).round() as usize;
    let row_height = line_height + row_gap;
    let summary_rows = 4;
    let legend_rows = 1;
    let stacked_bar_rows = 1;
    let cache_rows = 4;
    let breakdown_header_rows = 1;
    let breakdown_rows = active_stages.len() + usize::from(show_untracked);
    let overlay_width = (500.0 * scale).round() as usize;
    let overlay_rows = summary_rows
        + legend_rows
        + stacked_bar_rows
        + cache_rows
        + breakdown_header_rows
        + breakdown_rows;
    let overlay_height = (padding_y * 2 + overlay_rows * row_height + section_gap * 3)
        .max((360.0 * scale).round() as usize);

    let config = OverlayConfig::new(OverlayAnchor::TopRight, overlay_width, overlay_height)
        .with_margin((10.0 * scale).round() as usize)
        .with_background(theme.overlay.background.to_argb_u32());

    let bounds = config.compute_bounds(width_usize, height_usize);
    render_overlay_background(
        frame.buffer_mut(),
        &bounds,
        config.background,
        width_usize,
        height_usize,
    );

    if let Some(border_color) = &theme.overlay.border {
        render_overlay_border(
            frame.buffer_mut(),
            &bounds,
            border_color.to_argb_u32(),
            width_usize,
            height_usize,
        );
    }

    let text_color = theme.overlay.foreground.to_argb_u32();
    let highlight_color = theme.overlay.highlight.to_argb_u32();
    let warning_color = theme.overlay.warning.to_argb_u32();
    let error_color = theme.overlay.error.to_argb_u32();
    let untracked_color = 0xFF4B5563_u32;

    let inner_left = bounds.x + padding_x;
    let inner_right = bounds
        .x
        .saturating_add(bounds.width)
        .saturating_sub(padding_x);
    let inner_width = inner_right.saturating_sub(inner_left);
    let row_text_y = |row_top: usize| row_top + row_height.saturating_sub(line_height) / 2;
    let row_box_y =
        |row_top: usize, box_height: usize| row_top + row_height.saturating_sub(box_height) / 2;
    let mut row_top = bounds.y + padding_y;

    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        "Performance",
        text_color,
    );
    row_top += row_height;

    let frame_ms = perf.last_frame_time.as_secs_f64() * 1000.0;
    let throughput = perf.render_throughput_per_sec();
    let budget_pct = (frame_ms / 16.67 * 100.0).min(999.0);
    let frame_color = if budget_pct < 80.0 {
        highlight_color
    } else if budget_pct < 100.0 {
        warning_color
    } else {
        error_color
    };

    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Frame: {:.1}ms", frame_ms),
        frame_color,
    );
    row_top += row_height;

    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Throughput: {:.1} renders/s", throughput),
        text_color,
    );
    row_top += row_height;

    let tracked_ms = perf.tracked_time().as_secs_f64() * 1000.0;
    let untracked_ms = perf.untracked_time().as_secs_f64() * 1000.0;
    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!(
            "Tracked: {:.1}ms | Untracked: {:.1}ms",
            tracked_ms, untracked_ms
        ),
        text_color,
    );
    row_top += row_height;

    let mut phase_entries: Vec<(&'static str, Duration, u32)> = active_stages
        .iter()
        .map(|stage| {
            let spec = stage.spec();
            (spec.short_label, perf.stage_time(*stage), spec.color)
        })
        .collect();
    if show_untracked {
        phase_entries.push(("Untracked", perf.untracked_time(), untracked_color));
    }
    phase_entries.sort_by(|a, b| b.1.cmp(&a.1));

    let legend = phase_entries
        .iter()
        .take(4)
        .map(|(name, duration, _)| {
            let pct = if perf.last_frame_time.is_zero() {
                0
            } else {
                (duration.as_secs_f64() / perf.last_frame_time.as_secs_f64() * 100.0) as u32
            };
            format!("{} {}%", name, pct)
        })
        .collect::<Vec<_>>()
        .join(" │ ");
    painter.draw(frame, inner_left, row_text_y(row_top), &legend, text_color);
    row_top += row_height;
    row_top += section_gap;

    let frame_us = perf.last_frame_time.as_micros().max(1) as f32;
    let bar_height = ((line_height as f32 * 0.7).round() as usize).max(10);
    let bar_total_width = inner_width.min((340.0 * scale).round() as usize);
    let bar_y = row_box_y(row_top, bar_height);

    frame.fill_rect_px(
        inner_left,
        bar_y,
        bar_total_width,
        bar_height,
        untracked_color,
    );

    let mut bar_x = inner_left;
    let bar_end_x = inner_left + bar_total_width;
    for stage in &active_stages {
        let spec = stage.spec();
        let duration = perf.stage_time(*stage);
        let segment_width =
            ((duration.as_micros() as f32 / frame_us) * bar_total_width as f32).round() as usize;
        if segment_width > 0 && bar_x < bar_end_x {
            let remaining = bar_end_x.saturating_sub(bar_x);
            let actual_width = segment_width.min(remaining);
            frame.fill_rect_px(bar_x, bar_y, actual_width, bar_height, spec.color);
            bar_x += actual_width;
        }
    }
    row_top += row_height;
    row_top += section_gap;

    let avg_ms = perf.avg_frame_time().as_secs_f64() * 1000.0;
    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Avg render: {:.1}ms", avg_ms),
        text_color,
    );
    row_top += row_height;

    let cache_size = painter.glyph_cache_size();
    let hit_rate = perf.cache_hit_rate();
    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Cache: {} glyphs", cache_size),
        text_color,
    );
    row_top += row_height;

    let hit_color = if hit_rate > 99.0 {
        highlight_color
    } else if hit_rate > 90.0 {
        warning_color
    } else {
        error_color
    };
    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Hits: {} ({:.1}%)", perf.total_cache_hits, hit_rate),
        hit_color,
    );
    row_top += row_height;

    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        &format!("Miss: {}", perf.total_cache_misses),
        text_color,
    );
    row_top += row_height;
    row_top += section_gap;

    painter.draw(
        frame,
        inner_left,
        row_text_y(row_top),
        "Render breakdown:",
        text_color,
    );
    row_top += row_height;

    let mut max_label_width = active_stages
        .iter()
        .map(|stage| painter.measure_width(stage.spec().label).ceil() as usize)
        .max()
        .unwrap_or(0);
    if show_untracked {
        max_label_width = max_label_width.max(painter.measure_width("Untracked").ceil() as usize);
    }

    let mut max_value_width = active_stages
        .iter()
        .map(|stage| {
            painter
                .measure_width(&format!("{} µs", perf.stage_time(*stage).as_micros()))
                .ceil() as usize
        })
        .max()
        .unwrap_or(0);
    if show_untracked {
        max_value_width = max_value_width.max(
            painter
                .measure_width(&format!("{} µs", perf.untracked_time().as_micros()))
                .ceil() as usize,
        );
    }

    let chart_gap = (8.0 * scale).round() as usize;
    let value_gap = (8.0 * scale).round() as usize;
    let label_width = max_label_width.max((80.0 * scale).round() as usize);
    let value_width = max_value_width.max((84.0 * scale).round() as usize);
    let chart_x = inner_left + label_width + chart_gap;
    let value_x = inner_right.saturating_sub(value_width);
    let chart_bg = theme.overlay.background.with_alpha(200).to_argb_u32();
    let chart_height = ((line_height as f32 * 0.75).round() as usize).max(10);
    let chart_width = value_x.saturating_sub(value_gap).saturating_sub(chart_x);

    for stage in &active_stages {
        let spec = stage.spec();
        let duration = perf.stage_time(*stage);
        let text_y = row_text_y(row_top);
        let chart_y = row_box_y(row_top, chart_height);
        painter.draw(frame, inner_left, text_y, spec.label, text_color);
        if chart_width > 0 {
            frame.draw_sparkline(
                chart_x,
                chart_y,
                chart_width,
                chart_height,
                perf.stage_history(*stage),
                spec.color,
                chart_bg,
            );
        }
        painter.draw(
            frame,
            value_x,
            text_y,
            &format!("{} µs", duration.as_micros()),
            spec.color,
        );
        row_top += row_height;
    }

    if show_untracked {
        let duration = perf.untracked_time();
        let text_y = row_text_y(row_top);
        let chart_y = row_box_y(row_top, chart_height);
        painter.draw(frame, inner_left, text_y, "Untracked", text_color);
        if chart_width > 0 {
            frame.draw_sparkline(
                chart_x,
                chart_y,
                chart_width,
                chart_height,
                perf.untracked_history(),
                untracked_color,
                chart_bg,
            );
        }
        painter.draw(
            frame,
            value_x,
            text_y,
            &format!("{} µs", duration.as_micros()),
            untracked_color,
        );
    }
}
