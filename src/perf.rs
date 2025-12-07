//! Performance monitoring module (debug builds only)
//!
//! Contains PerfStats struct for tracking frame timing and render breakdown,
//! plus the render_perf_overlay function for displaying performance metrics.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use fontdue::Font;
use token::overlay::{
    render_overlay_background, render_overlay_border, OverlayAnchor, OverlayConfig,
};
use token::theme::Theme;

use crate::view::{draw_sparkline, draw_text, GlyphCache};

pub const PERF_HISTORY_SIZE: usize = 60;

#[derive(Default)]
#[allow(dead_code)]
pub struct PerfStats {
    // Frame timing
    pub frame_start: Option<Instant>,
    pub last_frame_time: Duration,
    pub frame_times: VecDeque<Duration>,

    // Render breakdown (current frame)
    pub clear_time: Duration,
    pub line_highlight_time: Duration,
    pub gutter_time: Duration,
    pub text_time: Duration,
    pub cursor_time: Duration,
    pub status_bar_time: Duration,
    pub present_time: Duration,

    // Render breakdown history (for sparklines)
    pub clear_history: VecDeque<Duration>,
    pub highlight_history: VecDeque<Duration>,
    pub gutter_history: VecDeque<Duration>,
    pub text_history: VecDeque<Duration>,
    pub cursor_history: VecDeque<Duration>,
    pub status_history: VecDeque<Duration>,
    pub present_history: VecDeque<Duration>,

    // Cache stats (reset per frame)
    pub frame_cache_hits: usize,
    pub frame_cache_misses: usize,

    // Cumulative cache stats
    pub total_cache_hits: usize,
    pub total_cache_misses: usize,

    // Display toggle
    pub show_overlay: bool,
}

#[allow(dead_code)]
impl PerfStats {
    pub fn reset_frame_stats(&mut self) {
        self.frame_cache_hits = 0;
        self.frame_cache_misses = 0;
    }

    pub fn record_frame_time(&mut self) {
        if let Some(start) = self.frame_start.take() {
            self.last_frame_time = start.elapsed();
            self.frame_times.push_back(self.last_frame_time);
            if self.frame_times.len() > 60 {
                self.frame_times.pop_front();
            }
        }
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
}

pub fn render_perf_overlay(
    buffer: &mut [u32],
    font: &Font,
    glyph_cache: &mut GlyphCache,
    perf: &PerfStats,
    theme: &Theme,
    width: u32,
    height: u32,
    font_size: f32,
    line_height: usize,
    ascent: f32,
) {
    let width_usize = width as usize;
    let height_usize = height as usize;

    // Configure and render overlay background using theme colors
    let config = OverlayConfig::new(OverlayAnchor::TopRight, 380, 480)
        .with_margin(10)
        .with_background(theme.overlay.background.to_argb_u32());

    let bounds = config.compute_bounds(width_usize, height_usize);
    render_overlay_background(
        buffer,
        &bounds,
        config.background,
        width_usize,
        height_usize,
    );

    // Render border if theme specifies one
    if let Some(border_color) = &theme.overlay.border {
        render_overlay_border(
            buffer,
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

    let text_x = bounds.x + 8;
    let mut text_y = bounds.y + 4;

    // Title
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        "Performance",
        text_color,
    );
    text_y += line_height;

    // Frame time
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
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &frame_text,
        frame_color,
    );
    text_y += line_height;

    // Budget bar
    let bar_chars = (budget_pct / 10.0).min(10.0) as usize;
    let bar = format!(
        "[{}{}] {:.0}%",
        "█".repeat(bar_chars),
        "░".repeat(10 - bar_chars),
        budget_pct
    );
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &bar,
        frame_color,
    );
    text_y += line_height + 4;

    // Average frame time
    let avg_ms = perf.avg_frame_time().as_secs_f64() * 1000.0;
    let avg_text = format!("Avg: {:.1}ms", avg_ms);
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &avg_text,
        text_color,
    );
    text_y += line_height + 4;

    // Cache stats
    let cache_size = glyph_cache.len();
    let hit_rate = perf.cache_hit_rate();
    let cache_text = format!("Cache: {} glyphs", cache_size);
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &cache_text,
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
    let hits_text = format!("Hits: {} ({:.1}%)", perf.total_cache_hits, hit_rate);
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &hits_text,
        hit_color,
    );
    text_y += line_height;

    let miss_text = format!("Miss: {}", perf.total_cache_misses);
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        &miss_text,
        text_color,
    );
    text_y += line_height + 4;

    // Render breakdown with sparklines
    draw_text(
        buffer,
        font,
        glyph_cache,
        font_size,
        ascent,
        width,
        height,
        text_x,
        text_y,
        "Render breakdown:",
        text_color,
    );
    text_y += line_height;

    // Chart dimensions
    let chart_width = 180;
    let chart_height = 20;
    let chart_x = text_x + 80;
    let chart_bg = theme.overlay.background.with_alpha(200).to_argb_u32();

    let breakdown_with_history: [(&str, Duration, &VecDeque<Duration>, u32); 7] = [
        ("Clear", perf.clear_time, &perf.clear_history, 0xFF7AA2F7), // Blue
        (
            "Highlight",
            perf.line_highlight_time,
            &perf.highlight_history,
            0xFF9ECE6A,
        ), // Green
        ("Text", perf.text_time, &perf.text_history, 0xFFE0AF68),    // Yellow/Orange
        ("Cursor", perf.cursor_time, &perf.cursor_history, 0xFFBB9AF7), // Purple
        ("Gutter", perf.gutter_time, &perf.gutter_history, 0xFF7DCFFF), // Cyan
        (
            "Status",
            perf.status_bar_time,
            &perf.status_history,
            0xFFF7768E,
        ), // Pink
        (
            "Present",
            perf.present_time,
            &perf.present_history,
            0xFFFF9E64,
        ), // Orange
    ];

    for (name, duration, history, color) in breakdown_with_history {
        let us = duration.as_micros();
        let breakdown_text = format!("{:>7}:", name);
        draw_text(
            buffer,
            font,
            glyph_cache,
            font_size,
            ascent,
            width,
            height,
            text_x,
            text_y,
            &breakdown_text,
            text_color,
        );

        // Draw sparkline next to label
        draw_sparkline(
            buffer,
            width,
            height,
            chart_x,
            text_y + 2,
            chart_width,
            chart_height,
            history,
            color,
            chart_bg,
        );

        // Draw current value at end
        let value_text = format!("{} µs", us);
        let value_x = chart_x + chart_width + 6;
        draw_text(
            buffer,
            font,
            glyph_cache,
            font_size,
            ascent,
            width,
            height,
            value_x,
            text_y,
            &value_text,
            color,
        );

        text_y += chart_height + 4;
    }
}
