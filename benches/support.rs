//! Shared helpers for benchmarks

use token::config::EditorConfig;
use token::model::document::Document;
use token::model::editor::EditorState;
use token::model::editor_area::EditorArea;
use token::model::ui::UiState;
use token::model::AppModel;
use token::theme::Theme;

/// Create an AppModel with the specified number of lines
#[allow(dead_code)]
pub fn make_model(lines: usize) -> AppModel {
    let window_width = 1920u32;
    let window_height = 1080u32;
    let line_height = 20;
    let char_width = 10.0f32;

    let text = "The quick brown fox jumps over the lazy dog.\n".repeat(lines);
    let document = Document::with_text(&text);

    let status_bar_height = line_height;
    let visible_lines = (window_height as usize).saturating_sub(status_bar_height) / line_height;
    let visible_columns = ((window_width as f32 - 60.0) / char_width).floor() as usize;

    let editor = EditorState::with_viewport(visible_lines, visible_columns);
    let editor_area = EditorArea::single_document(document, editor);

    AppModel {
        editor_area,
        ui: UiState::new(),
        theme: Theme::default(),
        config: EditorConfig::default(),
        window_size: (window_width, window_height),
        line_height,
        status_bar_height,
        char_width,
        metrics: token::model::ScaledMetrics::default(),
        workspace: None,
        dock_layout: token::panel::DockLayout::default(),
        terminal: token::terminal::TerminalState::default(),
        outline_panel: token::model::OutlinePanelState::default(),
        problems_panel: token::model::ProblemsPanelState::default(),
        usages_panel: Default::default(),
        recent_files: token::recent_files::RecentFiles::default(),
        command_history: token::command_history::CommandHistory::default(),
        #[cfg(debug_assertions)]
        debug_overlay: None,
        lsp: token::model::LspUiState::default(),
        jump_history: Vec::new(),
        forward_history: Vec::new(),
    }
}

/// Headless production editor-area renderer. Includes layout and CPU drawing,
/// excludes window scheduling, surface presentation and runtime effects.
#[allow(dead_code)]
pub struct BenchRenderer {
    width: usize,
    height: usize,
    line_height: usize,
    font: fontdue::Font,
    glyph_cache: token::view::GlyphCache,
    buffer: Vec<u32>,
}

#[allow(dead_code)]
impl BenchRenderer {
    pub fn new(width: usize, height: usize, line_height: usize) -> Self {
        Self {
            width,
            height,
            line_height,
            font: fontdue::Font::from_bytes(
                include_bytes!("../assets/JetBrainsMono.ttf") as &[u8],
                fontdue::FontSettings::default(),
            )
            .expect("bundled benchmark font"),
            glyph_cache: Default::default(),
            buffer: vec![0; width * height],
        }
    }

    pub fn render_frame(&mut self, model: &mut AppModel) {
        let font_size = 14.0;
        let ascent = self.font.horizontal_line_metrics(font_size).unwrap().ascent;
        model.line_height = self.line_height;
        model.char_width = self.font.metrics('M', font_size).advance_width;
        model.resize(self.width as u32, self.height as u32);
        let mut frame = token::view::Frame::new(&mut self.buffer, self.width, self.height);
        frame.clear(model.theme.editor.background.to_argb_u32());
        let mut painter = token::view::TextPainter::new(
            &self.font,
            &mut self.glyph_cache,
            font_size,
            ascent,
            model.char_width,
            self.line_height,
        );
        let mut perf = token::perf::PerfStats::default();
        for (&id, group) in &model.editor_area.groups {
            token::view::Renderer::render_editor_group(
                &mut frame,
                &mut painter,
                model,
                id,
                group.rect,
                id == model.editor_area.focused_group_id,
                &mut perf,
            );
        }
        divan::black_box(&self.buffer);
    }
}
