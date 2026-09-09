//! Fold projection must agree with a deliberately simple visible-row iterator.
mod common;
use std::sync::Arc;
use token::{
    folding::{FoldAction, FoldStamp},
    messages::{DocumentMsg, EditorMsg, Msg},
    model::{AppModel, Cursor, Document, EditorState, Position, Selection},
    syntax::{LanguageId, ParserState},
    update::update,
    util::text::TabStops,
};

fn detect(document: &mut Document, syntax: bool) {
    let source = document.buffer.to_string();
    let mut parser = ParserState::new();
    let id = document.id.unwrap_or(token::model::DocumentId(0));
    if syntax {
        parser.parse_and_highlight(&source, document.language, id, document.revision);
    }
    let tree = parser.syntax_tree_snapshot(id, document.revision);
    document.folds = Some(Arc::new(token::syntax::folding::detect(
        &source,
        FoldStamp {
            revision: document.revision,
            language: document.language,
            policy_generation: document.text_policy_generation,
        },
        document.text_settings.tabs,
        tree.as_ref(),
    )));
}

fn fixture() -> &'static str {
    "outer\n\tfirst body with long words 🙂\n\tinner\n\t\tdeep\n\t\tdeeper\n\tlast\n\nsecond\n  body\nend\n"
}
fn model() -> AppModel {
    let mut model = common::test_model(fixture(), 0, 0);
    detect(model.document_mut(), false);
    model
}
fn fold(model: &mut AppModel, action: FoldAction, header: Option<usize>) {
    update(
        model,
        Msg::Editor(EditorMsg::Fold {
            editor_id: None,
            header,
            action,
        }),
    );
}

#[test]
fn folding_indentation_handles_nested_siblings_blank_lines_tabs_and_eof() {
    let mut document = Document::with_text(fixture());
    detect(&mut document, false);
    let regions = &document.folds.as_ref().unwrap().regions;
    assert_eq!(
        regions
            .iter()
            .map(|r| (r.header, r.end))
            .collect::<Vec<_>>(),
        [(0, 6), (2, 5), (7, 9)]
    );
    for source in [
        "header\n  body",
        "header\r  body\r\r",
        "header\r\n \r\n  body\r\n",
    ] {
        let mut document = Document::with_text(source);
        detect(&mut document, false);
        let regions = &document.folds.as_ref().unwrap().regions;
        assert_eq!(regions.len(), 1, "{source:?}");
        assert_eq!(regions[0].header, 0);
    }
}

#[test]
fn folding_projection_matches_reference_with_wrap_tabs_nested_choices_and_hidden_positions() {
    for tabs in [2, 3, 4, 8] {
        for width in [2, 7, 80] {
            for wrapped in [false, true] {
                let mut document = Document::with_text(fixture());
                document.text_settings.tabs = TabStops::new(tabs);
                detect(&mut document, false);
                let candidates = document.folds.as_ref().unwrap().regions.clone();
                for mask in 0..1 << candidates.len() {
                    let mut editor = EditorState::with_viewport(200, width);
                    editor.soft_wrap = wrapped;
                    for (index, region) in candidates.iter().enumerate() {
                        if mask & (1 << index) != 0 {
                            editor.fold(&document, FoldAction::Collapse, Some(region.header));
                        }
                    }
                    editor.ensure_wrap_cache(&document);
                    let reference: Vec<_> = (0..document.line_count())
                        .filter(|&line| !editor.folds.collapsed().iter().any(|r| r.hides(line)))
                        .flat_map(|line| {
                            let rows = if wrapped {
                                editor.wrap_cache.visual_line_count(line)
                            } else {
                                1
                            };
                            std::iter::repeat_n(line, rows)
                        })
                        .collect();
                    let map = editor.viewport_map(&document);
                    assert_eq!(
                        map.row_count(),
                        reference.len(),
                        "mask {mask}, wrap {wrapped}, width {width}, tabs {tabs}"
                    );
                    for (row, &line) in reference.iter().enumerate() {
                        assert_eq!(map.doc_line_for_visible_row(row), Some(line));
                        assert_eq!(map.position_at_display_column(&document, row, 0).line, line);
                    }
                    for line in 0..document.line_count() {
                        if editor.folds.collapsed().iter().any(|r| r.hides(line)) {
                            assert!(map.hidden_header(line).is_some());
                            assert!(map.visible_row_for_doc_line(line).is_none());
                            continue;
                        }
                        for column in 0..=document.line_length(line) {
                            let (row, display_column) =
                                map.display_position(&document, line, column);
                            assert_eq!(reference[row], line);
                            assert_eq!(
                                map.position_at_display_column(&document, row, display_column),
                                Position::new(line, column)
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn folding_navigation_skips_hidden_bodies_and_explicit_targets_reveal_them() {
    let mut model = model();
    fold(&mut model, FoldAction::Collapse, Some(0));
    update(
        &mut model,
        Msg::Editor(EditorMsg::MoveCursor(token::messages::Direction::Down)),
    );
    assert_eq!(model.editor().active_cursor().line, 6);
    assert!(model.editor().folds.is_collapsed(0));
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 3, column: 2 }),
    );
    assert!(model.editor().folds.collapsed().is_empty());
    fold(&mut model, FoldAction::Collapse, Some(0));
    assert_eq!(model.editor().active_cursor().line, 0);
    assert!(!model.document().is_modified);
    assert!(model.document().undo_stack.is_empty());
}

#[test]
fn folding_edits_preserve_unaffected_anchors_and_expand_touched_regions_in_all_panes() {
    let mut model = model();
    fold(&mut model, FoldAction::Collapse, Some(7));
    update(
        &mut model,
        Msg::Document(DocumentMsg::InsertText("inserted\n".into())),
    );
    assert!(model.editor().folds.is_collapsed(8));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert!(model.editor().folds.is_collapsed(7));
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 8, column: 2 }),
    );
    assert!(model.editor().folds.collapsed().is_empty());
}

#[test]
fn folding_selection_blocks_collapse_and_nested_choices_survive_expansion() {
    let mut model = model();
    model.editor_mut().cursors[0] = Cursor::at(4, 2);
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(3, 1), Position::new(4, 2));
    fold(&mut model, FoldAction::CollapseAll, None);
    assert!(!model.editor().folds.is_collapsed(0));
    assert!(!model.editor().folds.is_collapsed(2));
    assert!(model.editor().folds.is_collapsed(7));
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 0, column: 0 }),
    );
    fold(&mut model, FoldAction::Collapse, Some(2));
    fold(&mut model, FoldAction::Collapse, Some(0));
    fold(&mut model, FoldAction::Expand, Some(0));
    assert!(model.editor().folds.is_collapsed(2));
}

#[test]
fn folding_syntax_profiles_cover_initial_languages_and_keep_following_text_visible() {
    for (language, source) in [
        (LanguageId::Rust, "fn main() {\n    hello();\n}\n"),
        (LanguageId::JavaScript, "function main() {\n  hello();\n}\n"),
        (
            LanguageId::TypeScript,
            "interface Item {\n name: string;\n}\n",
        ),
        (
            LanguageId::Jsx,
            "const x = <div>\n  <p>hello</p>\n</div>;\n",
        ),
        (
            LanguageId::Tsx,
            "const x = <div>\n  <p>hello</p>\n</div>;\n",
        ),
        (LanguageId::Python, "def main():\n    hello()\n"),
        (LanguageId::Json, "{\n  \"x\": [1,2]\n}\n"),
        (LanguageId::Yaml, "key:\n  item: value\n"),
        (LanguageId::Html, "<div>\n  <p>hello</p>\n</div>\n"),
        (LanguageId::Css, "p {\n color: red;\n}\n"),
        (
            LanguageId::Markdown,
            "# Heading\n\nBody\n\n## Child\n\nContent\n",
        ),
    ] {
        let mut document = Document::with_text(source);
        document.language = language;
        detect(&mut document, true);
        assert!(
            !document.folds.as_ref().unwrap().regions.is_empty(),
            "{language:?}"
        );
        assert!(
            document
                .folds
                .as_ref()
                .unwrap()
                .regions
                .iter()
                .all(|r| r.kind != "indentation"),
            "{language:?}"
        );
    }
    let mut document = Document::with_text("const x = {\n value: 1\n}; other();\n");
    document.language = LanguageId::JavaScript;
    detect(&mut document, true);
    assert!(document
        .folds
        .as_ref()
        .unwrap()
        .regions
        .iter()
        .all(|r| !r.hides(2)));
}

fn restore_source(source: &str) -> AppModel {
    let mut model = common::test_model(source, 0, 0);
    model.document_mut().file_path = Some("/project/folds.txt".into());
    model
}
fn complete_candidates(model: &mut AppModel) {
    detect(model.document_mut(), false);
    let folds = model.document_mut().folds.take();
    let document = model.document();
    let message = token::messages::SyntaxMsg::ParseCompleted {
        document_id: document.id.unwrap(),
        revision: document.revision,
        highlights: token::syntax::SyntaxHighlights::new(document.language, document.revision),
        syntax_tree: None,
        outline: None,
        folds,
        timing: Box::default(),
        replace_line_ranges: None,
    };
    update(model, Msg::Syntax(message));
}

#[test]
fn folding_session_restores_nested_metadata_and_unique_relocated_regions() {
    let mut original = restore_source(fixture());
    detect(original.document_mut(), false);
    fold(&mut original, FoldAction::Collapse, Some(2));
    fold(&mut original, FoldAction::Collapse, Some(0));
    let session = token::session::Session::capture(&original, std::path::Path::new("/project"));
    let json = serde_json::to_string(&session).unwrap();
    assert!(!json.contains("first body"));
    assert!(!json.contains("deeper"));
    assert!(json.contains("fingerprint"));
    let session: token::session::Session = serde_json::from_str(&json).unwrap();
    for (source, offset) in [
        (fixture().to_owned(), 0),
        (format!("added\n{}", fixture()), 1),
    ] {
        let mut restored = restore_source(&source);
        session.install(&mut restored).unwrap();
        assert!(restored.editor().folds.collapsed().is_empty());
        complete_candidates(&mut restored);
        assert!(restored.editor().folds.is_collapsed(offset));
        assert!(restored.editor().folds.is_collapsed(offset + 2));
    }
    // Existing version-one files without the optional metadata stay readable.
    let mut old: serde_json::Value = serde_json::from_str(&json).unwrap();
    old["layout"]["tabs"][0]
        .as_object_mut()
        .unwrap()
        .remove("folds");
    let old: token::session::Session = serde_json::from_value(old).unwrap();
    old.validate().unwrap();
}

#[test]
fn folding_session_navigation_cancels_late_restore_and_dirty_text_retains_saved_anchors() {
    let mut original = restore_source(fixture());
    detect(original.document_mut(), false);
    fold(&mut original, FoldAction::Collapse, Some(7));
    let before = token::session::Session::capture(&original, std::path::Path::new("/project"));
    update(
        &mut original,
        Msg::Document(DocumentMsg::InsertText("SECRET unsaved\n".into())),
    );
    let dirty = token::session::Session::capture(&original, std::path::Path::new("/project"));
    let before = serde_json::to_value(before).unwrap();
    let dirty_json = serde_json::to_value(&dirty).unwrap();
    assert_eq!(
        before["layout"]["tabs"][0]["folds"],
        dirty_json["layout"]["tabs"][0]["folds"]
    );
    assert!(!dirty_json.to_string().contains("SECRET"));
    let mut restored = restore_source(fixture());
    dirty.install(&mut restored).unwrap();
    update(
        &mut restored,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 8, column: 2 }),
    );
    complete_candidates(&mut restored);
    assert!(restored.editor().folds.collapsed().is_empty());
    assert_eq!(restored.editor().active_cursor().line, 8);
}

#[test]
fn folding_splits_copy_collapse_choices_then_remain_independent() {
    let mut model = model();
    fold(&mut model, FoldAction::CollapseAll, None);
    let first = model.editor().id.unwrap();
    update(
        &mut model,
        Msg::Layout(token::messages::LayoutMsg::SplitFocused(
            token::model::SplitDirection::Horizontal,
        )),
    );
    let second = model.editor().id.unwrap();
    assert_ne!(first, second);
    assert_eq!(
        model.editor_area.editors[&first].folds.collapsed(),
        model.editor().folds.collapsed()
    );
    fold(&mut model, FoldAction::ExpandAll, None);
    assert!(!model.editor_area.editors[&first]
        .folds
        .collapsed()
        .is_empty());
    assert!(model.editor().folds.collapsed().is_empty());
}

#[test]
fn folding_gutter_chevron_and_badge_hit_the_header_without_hitting_hidden_text() {
    use token::view::{
        geometry::{fold_badge_rect, GroupLayout, LaneId},
        hit_test::{hit_test_groups, HitTarget, Point},
    };
    let mut model = model();
    model.resize(800, 600);
    fold(&mut model, FoldAction::Collapse, Some(0));
    let group = model.editor_area.focused_group().unwrap();
    let layout = GroupLayout::new(group, &model, model.char_width);
    assert!(layout.gutter.fold_w > 0);
    let points = [
        Point::new(
            (layout.gutter_right_x - layout.gutter.fold_w as usize / 2) as f64,
            layout.content_y() as f64 + model.line_height as f64 / 2.0,
        ),
        {
            let badge = fold_badge_rect(
                model.editor(),
                model.document(),
                &layout,
                0,
                model.char_width,
                model.line_height,
            )
            .unwrap();
            Point::new(
                (badge.x + badge.width / 2.0) as f64,
                (badge.y + badge.height / 2.0) as f64,
            )
        },
    ];
    for point in points {
        assert!(matches!(
            hit_test_groups(&model, point, model.char_width),
            Some(HitTarget::EditorGutter {
                line: 0,
                lane: Some(LaneId::Fold),
                ..
            })
        ));
    }
    let map = model.editor().viewport_map(model.document());
    assert_eq!(map.doc_line_for_visible_row(1), Some(6));
}

#[test]
fn folding_insert_at_end_boundary_keeps_new_visible_text_outside_the_fold() {
    let mut model = model();
    fold(&mut model, FoldAction::Collapse, Some(0));
    update(
        &mut model,
        Msg::Editor(EditorMsg::SetCursorPosition { line: 6, column: 0 }),
    );
    update(
        &mut model,
        Msg::Document(DocumentMsg::InsertText("visible\n".into())),
    );
    assert!(model.editor().folds.is_collapsed(0));
    assert_eq!(model.editor().folds.collapsed()[0].end, 6);
    let map = model.editor().viewport_map(model.document());
    assert!(map.visible_row_for_doc_line(6).is_some());
    assert_eq!(model.document().get_line(6).unwrap(), "visible\n");
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert!(model.editor().folds.is_collapsed(0));
    assert_eq!(model.editor().folds.collapsed()[0].end, 6);
}

#[test]
fn folding_changed_files_expand_ambiguous_and_deleted_saved_regions() {
    let source = "start\n  body\nend\n";
    let mut original = restore_source(source);
    detect(original.document_mut(), false);
    fold(&mut original, FoldAction::Collapse, Some(0));
    let session = token::session::Session::capture(&original, std::path::Path::new("/project"));
    for changed in [format!("{source}{source}"), "end\n".into()] {
        let mut restored = restore_source(&changed);
        session.install(&mut restored).unwrap();
        complete_candidates(&mut restored);
        assert!(restored.editor().folds.collapsed().is_empty());
    }
}

#[test]
fn folding_gutter_hit_tracks_the_docked_find_bar_content_inset() {
    let mut model = model();
    model.resize(800, 600);
    update(
        &mut model,
        Msg::Ui(token::messages::UiMsg::OpenFind { replace: true }),
    );
    let group = model.editor_area.focused_group().unwrap();
    let layout = token::view::geometry::GroupLayout::new(group, &model, model.char_width);
    assert!(layout.find_bar_rect.height > 0.0);
    let point = token::view::hit_test::Point::new(
        (layout.gutter_right_x - layout.gutter.fold_w as usize / 2) as f64,
        layout.content_y() as f64 + model.line_height as f64 / 2.0,
    );
    assert!(matches!(
        token::view::hit_test::hit_test_groups(&model, point, model.char_width),
        Some(token::view::hit_test::HitTarget::EditorGutter {
            line: 0,
            lane: Some(token::view::geometry::LaneId::Fold),
            ..
        })
    ));
}

#[test]
fn folding_cancels_hover_intent_when_geometry_changes_without_moving_the_caret() {
    let mut model = model();
    let id = model.document().id.unwrap();
    model.ui.hover_request = Some(token::model::hover::HoverRequest {
        anchor: token::model::hover::HoverAnchor::capture(&model).unwrap(),
        position: Position::new(3, 2),
        origin: token::model::hover::HoverOrigin::Mouse,
    });
    let command = update(
        &mut model,
        Msg::Editor(EditorMsg::Fold {
            editor_id: None,
            header: Some(0),
            action: FoldAction::Collapse,
        }),
    )
    .unwrap();
    assert!(model.ui.hover_request.is_none());
    assert_eq!(model.editor().active_cursor().line, 0);
    fn cancels(command: &token::commands::Cmd, id: token::model::DocumentId) -> bool {
        match command {
            token::commands::Cmd::LspCancelHover { document_id } => *document_id == id,
            token::commands::Cmd::Batch(commands) => commands.iter().any(|cmd| cancels(cmd, id)),
            _ => false,
        }
    }
    assert!(cancels(&command, id));
}
