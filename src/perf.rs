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
}

#[cfg(not(debug_assertions))]
#[derive(Default)]
pub struct PerfStats;

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
        }
    }
}

/// Timer guard that records elapsed time when dropped (debug only).
#[cfg(debug_assertions)]
pub struct TimerGuard<'a> {
    start: Instant,
    perf: &'a mut PerfStats,
    stage: PerfStage,
}

#[cfg(not(debug_assertions))]
pub struct TimerGuard;

#[cfg(debug_assertions)]
impl<'a> TimerGuard<'a> {
    fn new(perf: &'a mut PerfStats, stage: PerfStage) -> Self {
        Self {
            start: Instant::now(),
            perf,
            stage,
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
    pub fn start_frame(&mut self) {}

    #[inline(always)]
    pub fn record_frame_time(&mut self) {}

    #[inline(always)]
    pub fn record_render_history(&mut self) {}

    #[inline(always)]
    pub fn time_stage(&mut self, _stage: PerfStage) -> TimerGuard {
        TimerGuard
    }

    #[inline(always)]
    pub fn measure_stage<R>(&mut self, _stage: PerfStage, f: impl FnOnce() -> R) -> R {
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

    let breakdown_rows = active_stages.len() + usize::from(show_untracked);
    let overlay_width = (500.0 * scale).round() as usize;
    let overlay_height = ((line_height * 10) + (breakdown_rows * (line_height + 4)) + 36)
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

    let padding_x = (8.0 * scale).round() as usize;
    let padding_y = (4.0 * scale).round() as usize;
    let text_x = bounds.x + padding_x;
    let mut text_y = bounds.y + padding_y;

    painter.draw(frame, text_x, text_y, "Performance", text_color);
    text_y += line_height;

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
        text_x,
        text_y,
        &format!("Frame: {:.1}ms", frame_ms),
        frame_color,
    );
    text_y += line_height;

    painter.draw(
        frame,
        text_x,
        text_y,
        &format!("Throughput: {:.1} renders/s", throughput),
        text_color,
    );
    text_y += line_height;

    let tracked_ms = perf.tracked_time().as_secs_f64() * 1000.0;
    let untracked_ms = perf.untracked_time().as_secs_f64() * 1000.0;
    painter.draw(
        frame,
        text_x,
        text_y,
        &format!(
            "Tracked: {:.1}ms | Untracked: {:.1}ms",
            tracked_ms, untracked_ms
        ),
        text_color,
    );
    text_y += line_height;

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
    painter.draw(frame, text_x, text_y, &legend, text_color);
    text_y += line_height + 4;

    let frame_us = perf.last_frame_time.as_micros().max(1) as f32;
    let bar_total_width = (bounds.width - 16).min((340.0 * scale).round() as usize);
    let bar_height = ((line_height as f32 * 0.7).round() as usize).max(10);
    let bar_y = text_y + 2;

    frame.fill_rect_px(text_x, bar_y, bar_total_width, bar_height, untracked_color);

    let mut bar_x = text_x;
    let bar_end_x = text_x + bar_total_width;
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
    text_y += bar_height + 6;

    let avg_ms = perf.avg_frame_time().as_secs_f64() * 1000.0;
    painter.draw(
        frame,
        text_x,
        text_y,
        &format!("Avg render: {:.1}ms", avg_ms),
        text_color,
    );
    text_y += line_height + 4;

    let cache_size = painter.glyph_cache_size();
    let hit_rate = perf.cache_hit_rate();
    painter.draw(
        frame,
        text_x,
        text_y,
        &format!("Cache: {} glyphs", cache_size),
        text_color,
    );
    text_y += line_height;

    let hit_color = if hit_rate > 99.0 {
        highlight_color
    } else if hit_rate > 90.0 {
        warning_color
    } else {
        error_color
    };
    painter.draw(
        frame,
        text_x,
        text_y,
        &format!("Hits: {} ({:.1}%)", perf.total_cache_hits, hit_rate),
        hit_color,
    );
    text_y += line_height;

    painter.draw(
        frame,
        text_x,
        text_y,
        &format!("Miss: {}", perf.total_cache_misses),
        text_color,
    );
    text_y += line_height + 4;

    painter.draw(frame, text_x, text_y, "Render breakdown:", text_color);
    text_y += line_height;

    let mut max_label_chars = active_stages
        .iter()
        .map(|stage| stage.spec().label.chars().count())
        .max()
        .unwrap_or(0);
    if show_untracked {
        max_label_chars = max_label_chars.max("Untracked".chars().count());
    }

    let char_width_approx = (line_height as f32 * 0.6).round() as usize;
    let label_width = char_width_approx * max_label_chars.max(10);
    let chart_width = (160.0 * scale).round() as usize;
    let chart_height = line_height;
    let chart_x = text_x + label_width + 6;
    let value_width = char_width_approx * 10;
    let chart_bg = theme.overlay.background.with_alpha(200).to_argb_u32();
    let max_chart_width = bounds.width.saturating_sub(label_width + value_width + 30);
    let chart_width = chart_width.min(max_chart_width);

    for stage in &active_stages {
        let spec = stage.spec();
        let duration = perf.stage_time(*stage);
        painter.draw(frame, text_x, text_y, spec.label, text_color);
        frame.draw_sparkline(
            chart_x,
            text_y + 2,
            chart_width,
            chart_height,
            perf.stage_history(*stage),
            spec.color,
            chart_bg,
        );
        painter.draw(
            frame,
            chart_x + chart_width + 6,
            text_y,
            &format!("{} µs", duration.as_micros()),
            spec.color,
        );
        text_y += chart_height + 4;
    }

    if show_untracked {
        let duration = perf.untracked_time();
        painter.draw(frame, text_x, text_y, "Untracked", text_color);
        frame.draw_sparkline(
            chart_x,
            text_y + 2,
            chart_width,
            chart_height,
            perf.untracked_history(),
            untracked_color,
            chart_bg,
        );
        painter.draw(
            frame,
            chart_x + chart_width + 6,
            text_y,
            &format!("{} µs", duration.as_micros()),
            untracked_color,
        );
    }
}
