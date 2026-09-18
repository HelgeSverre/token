//! In-memory editor specimens. Build a real single-group `AppModel` from a
//! small Rust sample and paint it through `Renderer::render_editor_group`;
//! never launch parsers asynchronously, LSPs, or open files.
use super::{Frame, Renderer, TextPainter};
use crate::folding::{FoldAction, FoldStamp};
use crate::model::gallery::EditorPreview;
use crate::model::{
    AppModel, Cursor, Document, EditorState, GhostProjection, Position, Rect, Selection,
};
use crate::syntax::{LanguageId, ParserState};
use crate::theme::Theme;
use std::path::PathBuf;
use std::sync::Arc;

const SAMPLE: &str = r#"use std::collections::HashMap;

pub struct Inventory {
    items: HashMap<String, u32>,
    capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self { items: HashMap::new(), capacity }
    }

    pub fn add(&mut self, name: &str, count: u32) -> bool {
        if self.items.len() < self.capacity {
            let entry = self.items.entry(name.to_owned()).or_insert(0);
            *entry += count;
            true
        } else {
            false
        }
    }
}

fn main() {
    let mut inventory = Inventory::new(8);
    let added = inventory.add("widget", 3);
    println!("added: {added}");
}
"#;

/// Sample line numbers (0-based) the variants anchor to.
const LINE_NEW_BODY: usize = 9;
const LINE_FN_ADD: usize = 12;
const LINE_IF: usize = 13;
const LINE_ENTRY: usize = 14;
const LINE_INCREMENT: usize = 15;
const LINE_ELSE_BODY: usize = 18;
const LINE_MAIN: usize = 23;
const LINE_INVENTORY_NEW: usize = 24;
const LINE_ADDED: usize = 25;

/// Isolated model whose focused group fills `size`, with syntax highlights and
/// fold candidates computed synchronously as the screenshot binary does.
pub(super) fn build_model(
    theme: &Theme,
    painter: &TextPainter,
    size: (usize, usize),
    scale: f64,
    kind: EditorPreview,
) -> AppModel {
    let path = PathBuf::from("/workspace/token/src/inventory.rs");
    let mut document = Document::with_text(SAMPLE);
    document.language = LanguageId::from_path(&path);
    document.file_path = Some(path);
    let mut model = AppModel::with_document(size.0 as u32, size.1 as u32, scale, document);
    model.theme = theme.clone();
    model.line_height = painter.line_height();
    model.char_width = painter.char_width();
    model.recompute_tab_bar_height_from_line_height();

    let mut parser = ParserState::new();
    for (doc_id, doc) in &mut model.editor_area.documents {
        let source = doc.buffer.to_string();
        doc.syntax_highlights =
            Some(parser.parse_and_highlight(&source, doc.language, *doc_id, doc.revision));
        doc.syntax_tree = parser.syntax_tree_snapshot(*doc_id, doc.revision);
        doc.folds = Some(Arc::new(crate::syntax::folding::detect(
            &source,
            FoldStamp {
                revision: doc.revision,
                language: doc.language,
                policy_generation: 0,
            },
            doc.text_settings.tabs,
            doc.syntax_tree.as_ref(),
        )));
    }

    let group_id = model.editor_area.focused_group_id;
    if let Some(group) = model.editor_area.groups.get_mut(&group_id) {
        group.rect = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
    }
    apply_variant(&mut model, kind);
    model.resync_viewports();
    model
}

/// Split borrow of the focused pane and its document, for pane-local state
/// (folds, ghost text) that reads the document while mutating the editor.
fn editor_and_document_mut(model: &mut AppModel) -> (&mut EditorState, &Document) {
    let editor_id = model.editor().id.expect("focused editor has an id");
    let document_id = model
        .editor()
        .document_id
        .expect("focused editor has a document");
    let area = &mut model.editor_area;
    let document = &area.documents[&document_id];
    let editor = area
        .editors
        .get_mut(&editor_id)
        .expect("focused editor exists");
    (editor, document)
}

fn apply_variant(model: &mut AppModel, kind: EditorPreview) {
    match kind {
        EditorPreview::Selection => {
            let editor = model.editor_mut();
            editor.cursors = vec![Cursor::at(LINE_ADDED, 25), Cursor::at(LINE_ADDED, 42)];
            editor.selections = vec![
                Selection::from_anchor_head(
                    Position::new(LINE_ADDED, 16),
                    Position::new(LINE_ADDED, 25),
                ),
                Selection::new(Position::new(LINE_ADDED, 42)),
            ];
            editor.matched_brackets =
                Some((Position::new(LINE_ADDED, 29), Position::new(LINE_ADDED, 41)));
            editor.viewport.top_line = LINE_INCREMENT + 2;
        }
        EditorPreview::IndentGuides => {
            model.config.indent_guides = true;
            let editor = model.editor_mut();
            editor.cursors = vec![Cursor::at(LINE_INCREMENT, 12)];
            editor.selections = vec![Selection::new(Position::new(LINE_INCREMENT, 12))];
            editor.viewport.top_line = 7;
        }
        EditorPreview::Folding => {
            let (editor, document) = editor_and_document_mut(model);
            editor.cursors = vec![Cursor::at(LINE_MAIN, 0)];
            editor.selections = vec![Selection::new(Position::new(LINE_MAIN, 0))];
            let collapsed = editor.fold(document, FoldAction::Collapse, Some(LINE_FN_ADD));
            debug_assert!(collapsed, "sample fn header must be a fold candidate");
            editor.viewport.top_line = 7;
        }
        EditorPreview::Diagnostics => {
            use lsp_types::{
                Diagnostic, DiagnosticSeverity, DiagnosticTag, Position as LspPos, Range,
            };
            let diagnostic = |line: usize, start: u32, end: u32, severity, message: &str| {
                let mut diagnostic = Diagnostic::new_simple(
                    Range::new(
                        LspPos::new(line as u32, start),
                        LspPos::new(line as u32, end),
                    ),
                    message.to_owned(),
                );
                diagnostic.severity = Some(severity);
                diagnostic.source = Some("rust-analyzer".into());
                diagnostic
            };
            let mut unused = diagnostic(
                LINE_ENTRY,
                16,
                21,
                DiagnosticSeverity::HINT,
                "unused variable: `entry`",
            );
            unused.tags = Some(vec![DiagnosticTag::UNNECESSARY]);
            model.document_mut().diagnostics = vec![
                diagnostic(
                    LINE_NEW_BODY,
                    15,
                    34,
                    DiagnosticSeverity::ERROR,
                    "mismatched types",
                ),
                diagnostic(
                    LINE_ELSE_BODY,
                    12,
                    17,
                    DiagnosticSeverity::WARNING,
                    "unreachable expression",
                ),
                unused,
            ];
            let editor = model.editor_mut();
            editor.cursors = vec![Cursor::at(LINE_IF, 8)];
            editor.selections = vec![Selection::new(Position::new(LINE_IF, 8))];
            editor.viewport.top_line = 7;
        }
        EditorPreview::Inlays => {
            model.config.lsp.inlay_hints = true;
            {
                let document = model.document_mut();
                document.lsp_features.revision = document.revision;
                document
                    .lsp_features
                    .hints
                    .insert(LINE_ENTRY, vec![": &mut u32".to_owned()]);
                document
                    .lsp_features
                    .hints
                    .insert(LINE_INVENTORY_NEW, vec![": Inventory".to_owned()]);
            }
            let (editor, document) = editor_and_document_mut(model);
            let anchor = Position::new(LINE_ADDED, document.line_length(LINE_ADDED));
            let ghost = GhostProjection::new(
                document,
                anchor,
                "\n    let removed = inventory.add(\"gadget\", 1);",
                None,
            );
            editor.set_ghost_text(document, ghost.map(Arc::new));
            editor.cursors = vec![Cursor::from_position(anchor)];
            editor.selections = vec![Selection::new(anchor)];
            editor.viewport.top_line = LINE_INCREMENT + 2;
        }
        EditorPreview::Find => {
            let mut find = crate::model::FindReplaceState::default();
            find.set_query("inventory");
            model.ui.open_find(find);
            let editor = model.editor_mut();
            editor.cursors = vec![Cursor::at(LINE_INVENTORY_NEW, 12)];
            editor.selections = vec![Selection::new(Position::new(LINE_INVENTORY_NEW, 12))];
            editor.viewport.top_line = LINE_MAIN - 2;
        }
    }
}

pub(super) fn render(
    frame: &mut Frame,
    painter: &mut TextPainter,
    theme: &Theme,
    rect: Rect,
    scale: f64,
    kind: EditorPreview,
) {
    let (width, height) = (rect.width.ceil() as usize, rect.height.ceil() as usize);
    let model = build_model(theme, painter, (width, height), scale, kind);
    let mut pixels = vec![theme.editor.background.to_argb_u32(); width * height];
    {
        let mut tile = Frame::new(&mut pixels, width, height);
        let group_id = model.editor_area.focused_group_id;
        let group_rect = model.editor_area.groups[&group_id].rect;
        Renderer::render_editor_group(
            &mut tile,
            painter,
            &model,
            group_id,
            group_rect,
            true,
            &mut crate::perf::PerfStats::default(),
        );
    }
    for y in 0..height.min(rect.height as usize) {
        for x in 0..width.min(rect.width as usize) {
            frame.set_pixel(
                rect.x as usize + x,
                rect.y as usize + y,
                pixels[y * width + x],
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::GlyphCache;

    fn painter_and_model(kind: EditorPreview) -> AppModel {
        let font = fontdue::Font::from_bytes(
            include_bytes!("../../assets/JetBrainsMono.ttf") as &[u8],
            fontdue::FontSettings::default(),
        )
        .unwrap();
        let mut cache = GlyphCache::default();
        let ascent = font.horizontal_line_metrics(14.0).unwrap().ascent;
        let char_width = font.rasterize('M', 14.0).0.advance_width;
        let painter = TextPainter::new(&font, &mut cache, 14.0, ascent, char_width, 20);
        build_model(&Theme::default_dark(), &painter, (580, 240), 1.0, kind)
    }

    #[test]
    fn editor_specimens_carry_their_intended_state() {
        let model = painter_and_model(EditorPreview::Selection);
        assert_eq!(model.editor().cursors.len(), 2);
        assert!(model.editor().matched_brackets.is_some());
        assert!(!model.editor().selections[0].is_empty());

        let model = painter_and_model(EditorPreview::IndentGuides);
        assert!(model.config.indent_guides);
        assert!(model.editor().is_plain_text_mode());

        let model = painter_and_model(EditorPreview::Folding);
        assert!(model.editor().folds.is_collapsed(LINE_FN_ADD));

        let model = painter_and_model(EditorPreview::Diagnostics);
        assert_eq!(model.document().diagnostics.len(), 3);

        let model = painter_and_model(EditorPreview::Inlays);
        assert_eq!(model.document().lsp_features.hints.len(), 2);
        assert!(model.config.lsp.inlay_hints);
        assert!(model.editor().ghost_text.0.is_some());

        let model = painter_and_model(EditorPreview::Find);
        let results = model
            .ui
            .find_bar
            .as_ref()
            .and_then(|find| find.display_results(model.document()));
        assert!(results.is_some_and(|results| results.matches.len() >= 3));
    }
}
