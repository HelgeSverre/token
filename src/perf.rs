//! Performance monitoring module
//!
//! Contains PerfStats struct for tracking frame timing and render breakdown.
//! In release builds, all timing methods compile to no-ops for zero overhead.

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

#[cfg(debug_assertions)]
pub struct PerfStats {
    pub frame_start: Option<Instant>,
    pub last_frame_time: Duration,
    pub frame_times: VecDeque<Duration>,

    pub clear_time: Duration,
    pub line_highlight_time: Duration,
    pub gutter_time: Duration,
    pub text_time: Duration,
    pub cursor_time: Duration,
    pub status_bar_time: Duration,
    pub present_time: Duration,

    pub clear_history: VecDeque<Duration>,
    pub highlight_history: VecDeque<Duration>,
    pub gutter_history: VecDeque<Duration>,
    pub text_history: VecDeque<Duration>,
    pub cursor_history: VecDeque<Duration>,
    pub status_history: VecDeque<Duration>,
    pub present_history: VecDeque<Duration>,

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
            clear_time: Duration::ZERO,
            line_highlight_time: Duration::ZERO,
            gutter_time: Duration::ZERO,
            text_time: Duration::ZERO,
            cursor_time: Duration::ZERO,
            status_bar_time: Duration::ZERO,
            present_time: Duration::ZERO,
            clear_history: VecDeque::new(),
            highlight_history: VecDeque::new(),
            gutter_history: VecDeque::new(),
            text_history: VecDeque::new(),
            cursor_history: VecDeque::new(),
            status_history: VecDeque::new(),
            present_history: VecDeque::new(),
            frame_cache_hits: 0,
            frame_cache_misses: 0,
            total_cache_hits: 0,
            total_cache_misses: 0,
            show_overlay: false,
        }
    }
}

/// Timer guard that records elapsed time when dropped (debug only)
#[cfg(debug_assertions)]
pub struct TimerGuard<'a> {
    start: Instant,
    target: &'a mut Duration,
}

#[cfg(not(debug_assertions))]
pub struct TimerGuard;

#[cfg(debug_assertions)]
impl<'a> TimerGuard<'a> {
    fn new(target: &'a mut Duration) -> Self {
        Self {
            start: Instant::now(),
            target,
        }
    }
}

#[cfg(debug_assertions)]
impl Drop for TimerGuard<'_> {
    fn drop(&mut self) {
        *self.target = self.start.elapsed();
    }
}

#[cfg(debug_assertions)]
impl PerfStats {
    #[inline(always)]
    pub fn reset_frame_stats(&mut self) {
        self.frame_cache_hits = 0;
        self.frame_cache_misses = 0;
    }

    /// Accumulate cache statistics from a text painter
    #[inline(always)]
    #[allow(dead_code)]
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
            if self.frame_times.len() > 60 {
                self.frame_times.pop_front();
            }
        }
    }

    #[inline(always)]
    pub fn record_render_history(&mut self) {
        fn push_history(history: &mut VecDeque<Duration>, value: Duration) {
            history.push_back(value);
            if history.len() > PERF_HISTORY_SIZE {
                history.pop_front();
            }
        }

        push_history(&mut self.clear_history, self.clear_time);
        push_history(&mut self.highlight_history, self.line_highlight_time);
        push_history(&mut self.gutter_history, self.gutter_time);
        push_history(&mut self.text_history, self.text_time);
        push_history(&mut self.cursor_history, self.cursor_time);
        push_history(&mut self.status_history, self.status_bar_time);
        push_history(&mut self.present_history, self.present_time);
    }

    #[inline(always)]
    pub fn time_clear(&mut self) -> TimerGuard<'_> {
        TimerGuard::new(&mut self.clear_time)
    }

    #[inline(always)]
    pub fn time_text(&mut self) -> TimerGuard<'_> {
        TimerGuard::new(&mut self.text_time)
    }

    #[inline(always)]
    pub fn time_status_bar(&mut self) -> TimerGuard<'_> {
        TimerGuard::new(&mut self.status_bar_time)
    }

    #[inline(always)]
    pub fn time_present(&mut self) -> TimerGuard<'_> {
        TimerGuard::new(&mut self.present_time)
    }

    #[inline(always)]
    pub fn should_show_overlay(&self) -> bool {
        self.show_overlay
    }

    pub fn avg_frame_time(&self) -> Duration {
        if self.frame_times.is_empty() {
            return Duration::ZERO;
        }
        let total: Duration = self.frame_times.iter().sum();
        total / self.frame_times.len() as u32
    }

    pub fn fps(&self) -> f64 {
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
}

#[cfg(not(debug_assertions))]
impl PerfStats {
    #[inline(always)]
    pub fn reset_frame_stats(&mut self) {}

    #[inline(always)]
    pub fn start_frame(&mut self) {}

    #[inline(always)]
    pub fn record_frame_time(&mut self) {}

    #[inline(always)]
    pub fn record_render_history(&mut self) {}

    #[inline(always)]
    pub fn time_clear(&mut self) -> TimerGuard {
        TimerGuard
    }

    #[inline(always)]
    pub fn time_text(&mut self) -> TimerGuard {
        TimerGuard
    }

    #[inline(always)]
    pub fn time_status_bar(&mut self) -> TimerGuard {
        TimerGuard
    }

    #[inline(always)]
    pub fn time_present(&mut self) -> TimerGuard {
        TimerGuard
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

    // Scale overlay dimensions based on line_height to handle HiDPI correctly.
    // At 1x scale (line_height ~20), we want ~380x480. At 2x, we want ~760x960.
    // Use line_height as the scaling reference since it's already DPI-aware.
    let scale = (line_height as f32 / 20.0).max(1.0);
    let overlay_width = (420.0 * scale).round() as usize;
    let overlay_height = (500.0 * scale).round() as usize;

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

    // Scale padding for HiDPI
    let padding_x = (8.0 * scale).round() as usize;
    let padding_y = (4.0 * scale).round() as usize;
    let text_x = bounds.x + padding_x;
    let mut text_y = bounds.y + padding_y;

    painter.draw(frame, text_x, text_y, "Performance", text_color);
    text_y += line_height;

    let frame_ms = perf.last_frame_time.as_secs_f64() * 1000.0;
    let fps = perf.fps();
    let budget_pct = (frame_ms / 16.67 * 100.0).min(999.0);
    let frame_color = if budget_pct < 80.0 {
        highlight_color
    } else if budget_pct < 100.0 {
        warning_color
    } else {
        error_color
    };

    let frame_text = format!("Frame: {:.1}ms ({:.0} fps)", frame_ms, fps);
    painter.draw(frame, text_x, text_y, &frame_text, frame_color);
    text_y += line_height;

    let phases: [(&str, Duration, u32); 7] = [
        ("Clear", perf.clear_time, 0xFF7AA2F7),
        ("Highl", perf.line_highlight_time, 0xFF9ECE6A),
        ("Text", perf.text_time, 0xFFE0AF68),
        ("Cursor", perf.cursor_time, 0xFFBB9AF7),
        ("Gutter", perf.gutter_time, 0xFF7DCFFF),
        ("Status", perf.status_bar_time, 0xFFF7768E),
        ("Present", perf.present_time, 0xFFFF9E64),
    ];

    let total_render: Duration = phases.iter().map(|(_, d, _)| *d).sum();
    let total_render_us = total_render.as_micros().max(1) as f32;
    let frame_us = perf.last_frame_time.as_micros().max(1) as f32;

    // Scale stacked bar dimensions for HiDPI
    let bar_total_width = (bounds.width - 16).min((300.0 * scale).round() as usize);
    let bar_height = ((line_height as f32 * 0.7).round() as usize).max(10);
    let bar_y = text_y + 2;

    let unaccounted_color = 0xFF404040_u32;
    frame.fill_rect_px(
        text_x,
        bar_y,
        bar_total_width,
        bar_height,
        unaccounted_color,
    );

    let mut bar_x = text_x;
    let bar_end_x = text_x + bar_total_width;
    for (_name, duration, color) in &phases {
        let phase_us = duration.as_micros() as f32;
        let segment_width = ((phase_us / frame_us) * bar_total_width as f32) as usize;
        if segment_width > 0 && bar_x < bar_end_x {
            let remaining = bar_end_x.saturating_sub(bar_x);
            let actual_width = segment_width.min(remaining);
            frame.fill_rect_px(bar_x, bar_y, actual_width, bar_height, *color);
            bar_x += segment_width;
        }
    }
    text_y += bar_height + 6;

    let mut sorted_phases: Vec<_> = phases.iter().collect();
    sorted_phases.sort_by(|a, b| b.1.cmp(&a.1));

    let legend: String = sorted_phases
        .iter()
        .take(3)
        .map(|(name, dur, _)| {
            let pct = (dur.as_micros() as f32 / total_render_us * 100.0) as u32;
            format!("{} {}%", name, pct)
        })
        .collect::<Vec<_>>()
        .join(" │ ");

    painter.draw(frame, text_x, text_y, &legend, text_color);
    text_y += line_height + 4;

    let avg_ms = perf.avg_frame_time().as_secs_f64() * 1000.0;
    let avg_text = format!("Avg: {:.1}ms", avg_ms);
    painter.draw(frame, text_x, text_y, &avg_text, text_color);
    text_y += line_height + 4;

    let cache_size = painter.glyph_cache_size();
    let hit_rate = perf.cache_hit_rate();
    let cache_text = format!("Cache: {} glyphs", cache_size);
    painter.draw(frame, text_x, text_y, &cache_text, text_color);
    text_y += line_height;

    let hit_color = if hit_rate > 99.0 {
        highlight_color
    } else if hit_rate > 90.0 {
        warning_color
    } else {
        error_color
    };
    let hits_text = format!("Hits: {} ({:.1}%)", perf.total_cache_hits, hit_rate);
    painter.draw(frame, text_x, text_y, &hits_text, hit_color);
    text_y += line_height;

    let miss_text = format!("Miss: {}", perf.total_cache_misses);
    painter.draw(frame, text_x, text_y, &miss_text, text_color);
    text_y += line_height + 4;

    painter.draw(frame, text_x, text_y, "Render breakdown:", text_color);
    text_y += line_height;

    // Scale chart dimensions with line_height for HiDPI support.
    // Label column needs ~10 chars worth of space ("Highlight:" is longest).
    // Approximate char width as ~0.6 * line_height for monospace fonts.
    let char_width_approx = (line_height as f32 * 0.6).round() as usize;
    let label_width = char_width_approx * 10;
    let chart_width = (160.0 * scale).round() as usize;
    let chart_height = line_height;
    let chart_x = text_x + label_width;
    let value_width = char_width_approx * 10; // "99999 µs" + padding
    let chart_bg = theme.overlay.background.with_alpha(200).to_argb_u32();

    // Ensure chart fits within overlay bounds
    let max_chart_width = bounds.width.saturating_sub(label_width + value_width + 24);
    let chart_width = chart_width.min(max_chart_width);

    let breakdown_with_history: [(&str, Duration, &VecDeque<Duration>, u32); 7] = [
        ("Clear", perf.clear_time, &perf.clear_history, 0xFF7AA2F7),
        (
            "Highlight",
            perf.line_highlight_time,
            &perf.highlight_history,
            0xFF9ECE6A,
        ),
        ("Text", perf.text_time, &perf.text_history, 0xFFE0AF68),
        ("Cursor", perf.cursor_time, &perf.cursor_history, 0xFFBB9AF7),
        ("Gutter", perf.gutter_time, &perf.gutter_history, 0xFF7DCFFF),
        (
            "Status",
            perf.status_bar_time,
            &perf.status_history,
            0xFFF7768E,
        ),
        (
            "Present",
            perf.present_time,
            &perf.present_history,
            0xFFFF9E64,
        ),
    ];

    for (name, duration, history, color) in breakdown_with_history {
        let us = duration.as_micros();
        let breakdown_text = format!("{:>7}:", name);
        painter.draw(frame, text_x, text_y, &breakdown_text, text_color);

        frame.draw_sparkline(
            chart_x,
            text_y + 2,
            chart_width,
            chart_height,
            history,
            color,
            chart_bg,
        );

        let value_text = format!("{} µs", us);
        let value_x = chart_x + chart_width + 6;
        painter.draw(frame, value_x, text_y, &value_text, color);

        text_y += chart_height + 4;
    }
}
