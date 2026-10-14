//! Performance monitoring module
//!
//! Contains `PerfStats` for tracking frame timing and render breakdown.
//! In release builds, all timing methods compile to no-ops for zero overhead.

use serde::{Deserialize, Serialize};
#[cfg(debug_assertions)]
use std::collections::VecDeque;
#[cfg(debug_assertions)]
use std::time::{Duration, Instant};

#[cfg(debug_assertions)]
pub const PERF_HISTORY_SIZE: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationMode {
    Hidden,
    DockOpen,
    FloatingOpen,
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone)]
pub struct CompletedFrameSample {
    pub frame_time: Duration,
    pub stages: [Duration; PerfStage::COUNT],
    pub untracked: Option<Duration>,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub cache_entries: usize,
    pub observation_mode: ObservationMode,
}

#[cfg(debug_assertions)]
#[derive(Clone, Copy)]
pub struct CompletedPerfView<'a> {
    samples: &'a VecDeque<CompletedFrameSample>,
    total_cache_hits: usize,
    total_cache_misses: usize,
}

#[cfg(debug_assertions)]
impl CompletedPerfView<'_> {
    pub fn samples(&self) -> impl ExactSizeIterator<Item = &CompletedFrameSample> {
        self.samples.iter()
    }

    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    pub fn latest(&self) -> Option<&CompletedFrameSample> {
        self.samples.back()
    }

    pub fn frame_mean(&self) -> Option<Duration> {
        mean_duration(self.samples.iter().map(|sample| sample.frame_time)).map(|(mean, _)| mean)
    }

    pub fn frame_median(&self) -> Option<Duration> {
        percentile(self.samples.iter().map(|sample| sample.frame_time), 50)
    }

    pub fn frame_p95(&self) -> Option<Duration> {
        percentile(self.samples.iter().map(|sample| sample.frame_time), 95)
    }

    pub fn stage_mean(&self, stage: PerfStage) -> Option<Duration> {
        mean_duration(
            self.samples
                .iter()
                .map(|sample| sample.stages[stage.index()]),
        )
        .map(|(mean, _)| mean)
    }

    /// The mean and denominator of only composable untracked samples.
    /// Overlapping stage totals remain gaps, not zero-duration samples.
    pub fn untracked_mean(&self) -> Option<(Duration, usize)> {
        mean_duration(self.samples.iter().filter_map(|sample| sample.untracked))
    }

    pub fn lifetime_cache_hit_rate(&self) -> Option<f64> {
        let lookups = self.total_cache_hits + self.total_cache_misses;
        (lookups > 0).then(|| self.total_cache_hits as f64 / lookups as f64 * 100.0)
    }

    pub fn lifetime_cache_counts(&self) -> (usize, usize) {
        (self.total_cache_hits, self.total_cache_misses)
    }
}

#[cfg(debug_assertions)]
fn mean_duration(values: impl IntoIterator<Item = Duration>) -> Option<(Duration, usize)> {
    let (total, count) = values
        .into_iter()
        .fold((0_u128, 0_usize), |(sum, n), value| {
            (sum + value.as_nanos(), n + 1)
        });
    (count > 0).then(|| {
        let nanos = total / count as u128;
        (
            Duration::new(
                (nanos / 1_000_000_000) as u64,
                (nanos % 1_000_000_000) as u32,
            ),
            count,
        )
    })
}

#[cfg(debug_assertions)]
fn percentile(values: impl Iterator<Item = Duration>, percentile: usize) -> Option<Duration> {
    let mut values: Vec<_> = values.collect();
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let rank = (percentile * values.len()).div_ceil(100);
    values.get(rank.saturating_sub(1)).copied()
}

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerfStage {
    BuildPlan = 0,
    Layout,
    Clear,
    CursorFastPath,
    TabBar,
    FindBar,
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
    DebugOverlay,
    SurfaceAcquire,
    BufferCopy,
    SurfacePresent,
    WebviewSync,
    WebviewVisibility,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerfSnapshot {
    pub frame_count: usize,
    pub average_frame_ms: f64,
    pub last_frame_ms: f64,
    pub stages: Vec<PerfStageSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfStageSnapshot {
    pub name: String,
    pub average_ms: f64,
    pub last_ms: f64,
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
            Self::Layout => "layout",
            Self::Clear => "clear",
            Self::CursorFastPath => "cursor_fast_path",
            Self::TabBar => "tab_bar",
            Self::FindBar => "find_bar",
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
    pub const ALL: [Self; 29] = [
        Self::BuildPlan,
        Self::Layout,
        Self::Clear,
        Self::CursorFastPath,
        Self::TabBar,
        Self::FindBar,
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

    #[cfg(debug_assertions)]
    pub const fn label(self) -> &'static str {
        self.spec().label
    }

    #[cfg(debug_assertions)]
    pub const fn short_label(self) -> &'static str {
        self.spec().short_label
    }

    #[cfg(debug_assertions)]
    pub const fn color(self) -> u32 {
        self.spec().color
    }

    #[inline(always)]
    #[cfg(debug_assertions)]
    const fn spec(self) -> PerfStageSpec {
        match self {
            Self::FindBar => PerfStageSpec {
                label: "Find Bar",
                short_label: "Find",
                color: 0xFF73DACA,
            },
            Self::BuildPlan => PerfStageSpec {
                label: "Build Plan",
                short_label: "Plan",
                color: 0xFF73DACA,
            },
            Self::Layout => PerfStageSpec {
                label: "Layout",
                short_label: "Layout",
                color: 0xFFB4F9F8,
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
    stage_times: [Duration; PerfStage::COUNT],
    pub frame_cache_hits: usize,
    pub frame_cache_misses: usize,
    pub total_cache_hits: usize,
    pub total_cache_misses: usize,
    completed_frames: VecDeque<CompletedFrameSample>,
    cache_entries: usize,
    observation_mode: ObservationMode,
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
            stage_times: [Duration::ZERO; PerfStage::COUNT],
            frame_cache_hits: 0,
            frame_cache_misses: 0,
            total_cache_hits: 0,
            total_cache_misses: 0,
            completed_frames: VecDeque::new(),
            cache_entries: 0,
            observation_mode: ObservationMode::Hidden,
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

    pub fn set_observation_mode(&mut self, mode: ObservationMode) {
        self.observation_mode = mode;
    }

    pub fn set_cache_entries(&mut self, entries: usize) {
        self.cache_entries = entries;
    }

    #[inline(always)]
    pub fn record_frame_time(&mut self) {
        if let Some(start) = self.frame_start.take() {
            self.last_frame_time = start.elapsed();
            let tracked: Duration = self.stage_times.iter().copied().sum();
            let sample = CompletedFrameSample {
                frame_time: self.last_frame_time,
                stages: self.stage_times,
                untracked: (tracked <= self.last_frame_time)
                    .then(|| self.last_frame_time.saturating_sub(tracked)),
                cache_hits: self.frame_cache_hits,
                cache_misses: self.frame_cache_misses,
                cache_entries: self.cache_entries,
                observation_mode: self.observation_mode,
            };
            self.completed_frames.push_back(sample);
            if self.completed_frames.len() > PERF_HISTORY_SIZE {
                self.completed_frames.pop_front();
            }
        }
        #[cfg(feature = "profile-tracing")]
        {
            self.frame_span.take();
        }
    }

    #[inline(always)]
    pub fn record_render_history(&mut self) {
        // Completed samples are published atomically by `record_frame_time`.
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
    pub fn stage_time(&self, stage: PerfStage) -> Duration {
        self.stage_times[stage.index()]
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
    pub fn avg_frame_time(&self) -> Duration {
        self.completed_view().frame_mean().unwrap_or_default()
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
                        .completed_frames
                        .iter()
                        .any(|sample| sample.stages[stage.index()] > Duration::ZERO)
            })
            .collect()
    }

    pub fn snapshot(&self) -> PerfSnapshot {
        let stages = PerfStage::ALL
            .into_iter()
            .map(|stage| {
                let average = self.completed_view().stage_mean(stage).unwrap_or_default();
                PerfStageSnapshot {
                    name: stage.spec().label.to_owned(),
                    average_ms: average.as_secs_f64() * 1000.0,
                    last_ms: self.stage_time(stage).as_secs_f64() * 1000.0,
                }
            })
            .collect();
        PerfSnapshot {
            frame_count: self.completed_frames.len(),
            average_frame_ms: self.avg_frame_time().as_secs_f64() * 1000.0,
            last_frame_ms: self.last_frame_time.as_secs_f64() * 1000.0,
            stages,
        }
    }

    pub fn completed_view(&self) -> CompletedPerfView<'_> {
        CompletedPerfView {
            samples: &self.completed_frames,
            total_cache_hits: self.total_cache_hits,
            total_cache_misses: self.total_cache_misses,
        }
    }

    #[cfg(any(test, feature = "ui-gallery"))]
    pub fn push_fixture_sample(
        &mut self,
        frame_time: Duration,
        stages: &[(PerfStage, Duration)],
        cache_entries: usize,
        mode: ObservationMode,
    ) {
        let mut stage_values = [Duration::ZERO; PerfStage::COUNT];
        for &(stage, elapsed) in stages {
            stage_values[stage.index()] = elapsed;
        }
        let tracked: Duration = stage_values.iter().copied().sum();
        self.completed_frames.push_back(CompletedFrameSample {
            frame_time,
            stages: stage_values,
            untracked: (tracked <= frame_time).then(|| frame_time - tracked),
            cache_hits: 21,
            cache_misses: 3,
            cache_entries,
            observation_mode: mode,
        });
        self.total_cache_hits += 21;
        self.total_cache_misses += 3;
        if self.completed_frames.len() > PERF_HISTORY_SIZE {
            self.completed_frames.pop_front();
        }
    }

    pub fn clear_history(&mut self) {
        self.completed_frames.clear();
    }
}

#[cfg(not(debug_assertions))]
impl PerfStats {
    #[inline(always)]
    pub fn reset_frame_stats(&mut self) {}

    #[inline(always)]
    pub fn add_cache_stats(&mut self, _hits: usize, _misses: usize) {}

    #[inline(always)]
    pub fn set_observation_mode(&mut self, _mode: ObservationMode) {}

    #[inline(always)]
    pub fn set_cache_entries(&mut self, _entries: usize) {}

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
        let _span = tracing::info_span!("render_stage", stage = _stage.tracing_label()).entered();
        f()
    }

    #[inline(always)]
    pub fn record_stage_elapsed(&mut self, _stage: PerfStage, _elapsed: std::time::Duration) {}

    pub fn snapshot(&self) -> PerfSnapshot {
        PerfSnapshot::default()
    }

    pub fn clear_history(&mut self) {}
}

#[cfg(all(test, debug_assertions))]
mod tests {
    use super::*;

    fn sample(ms: u64, mode: ObservationMode) -> CompletedFrameSample {
        CompletedFrameSample {
            frame_time: Duration::from_millis(ms),
            stages: [Duration::ZERO; PerfStage::COUNT],
            untracked: Some(Duration::from_millis(ms)),
            cache_hits: 0,
            cache_misses: 0,
            cache_entries: 0,
            observation_mode: mode,
        }
    }

    #[test]
    fn completed_view_uses_actual_sample_count_and_nearest_rank_percentiles() {
        let samples = [1, 2, 3, 4, 20]
            .into_iter()
            .map(|ms| sample(ms, ObservationMode::Hidden))
            .collect();
        let view = CompletedPerfView {
            samples: &samples,
            total_cache_hits: 0,
            total_cache_misses: 0,
        };
        assert_eq!(view.sample_count(), 5);
        assert_eq!(view.frame_mean(), Some(Duration::from_millis(6)));
        assert_eq!(view.frame_median(), Some(Duration::from_millis(3)));
        assert_eq!(view.frame_p95(), Some(Duration::from_millis(20)));
        assert_eq!(view.lifetime_cache_hit_rate(), None);
    }

    #[test]
    fn completed_sample_latches_observation_mode_and_all_stage_values() {
        let mut stats = PerfStats::default();
        stats.set_observation_mode(ObservationMode::FloatingOpen);
        stats.start_frame();
        stats.record_stage_elapsed(PerfStage::Layout, Duration::from_micros(25));
        stats.record_frame_time();
        let view = stats.completed_view();
        let latest = view.latest().unwrap();
        assert_eq!(latest.observation_mode, ObservationMode::FloatingOpen);
        assert_eq!(
            latest.stages[PerfStage::Layout.index()],
            Duration::from_micros(25)
        );
    }

    #[test]
    fn untracked_mean_skips_overlap_gaps_but_stage_mean_uses_every_frame() {
        let mut stats = PerfStats::default();
        for (layout, glyphs) in [(2, 0), (6, 7), (4, 0)] {
            stats.push_fixture_sample(
                Duration::from_millis(10),
                &[
                    (PerfStage::Layout, Duration::from_millis(layout)),
                    (PerfStage::TextGlyphs, Duration::from_millis(glyphs)),
                ],
                0,
                ObservationMode::DockOpen,
            );
        }
        let view = stats.completed_view();
        assert_eq!(
            view.stage_mean(PerfStage::Layout),
            Some(Duration::from_millis(4))
        );
        assert_eq!(view.untracked_mean(), Some((Duration::from_millis(7), 2)));
        assert_eq!(
            view.latest().and_then(|sample| sample.untracked),
            Some(Duration::from_millis(6))
        );

        let mut only_overlap = PerfStats::default();
        only_overlap.push_fixture_sample(
            Duration::from_millis(10),
            &[
                (PerfStage::Layout, Duration::from_millis(6)),
                (PerfStage::TextGlyphs, Duration::from_millis(7)),
            ],
            0,
            ObservationMode::DockOpen,
        );
        assert_eq!(only_overlap.completed_view().untracked_mean(), None);
    }

    #[test]
    fn mean_duration_handles_large_values_without_duration_sum_overflow() {
        let almost_max = Duration::new(u64::MAX, 999_999_998);
        assert_eq!(
            mean_duration([almost_max, almost_max]),
            Some((almost_max, 2))
        );
        assert_eq!(mean_duration([]), None);
    }
}
