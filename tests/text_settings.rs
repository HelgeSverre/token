//! Per-file indentation and tab geometry share one document policy.
mod common;

use token::{
    messages::{DocumentMsg, Msg},
    model::{Document, EditorState, IndentStyle, LineEnding, TextPreferences},
    update::update,
    util::text::TabStops,
};

#[test]
fn text_settings_tabs_align_wrap_projection_and_cursor_mapping() {
    for width in [2, 3, 4, 8] {
        let mut document = Document::with_text("é\tab\tcd\n\tz");
        document.file_text_preferences = TextPreferences {
            tab_width: Some(width),
            ..Default::default()
        };
        document.resolve_text_settings(Default::default());
        let mut editor = EditorState::new();
        editor.soft_wrap = true;
        editor.viewport.visible_columns = 6;
        editor.ensure_wrap_cache(&document);
        let map = editor.viewport_map(&document);
        for line in 0..document.line_count() {
            for column in 0..=document.line_length(line) {
                let (row, visual) = map.display_position(&document, line, column);
                let back = map.position_at_display_column(&document, row, visual);
                assert_eq!(
                    (back.line, back.column),
                    (line, column),
                    "tab width {width}"
                );
            }
        }
        assert_eq!(
            document.text_settings.tabs.expand("é\t"),
            format!("é{}", " ".repeat(width - 1))
        );
    }
}

#[test]
fn text_settings_width_change_reflows_without_editing_or_losing_anchor() {
    let mut document = Document::with_text("\taaaa\n\tbbbb\n\tcccc\n\tdddd");
    let mut editor = EditorState::new();
    editor.soft_wrap = true;
    editor.viewport.visible_columns = 6;
    editor.viewport.visible_lines = 1;
    editor.ensure_wrap_cache(&document);
    editor.viewport.top_line = editor.wrap_cache.logical_to_visual(2, 0).0;
    let old_rows = editor.wrap_cache.total_visual_lines();
    document.file_text_preferences.tab_width = Some(2);
    document.resolve_text_settings(Default::default());
    editor.ensure_wrap_cache(&document);
    assert!(editor.wrap_cache.total_visual_lines() < old_rows);
    assert_eq!(
        editor.viewport_map(&document).doc_line_for_visible_row(0),
        Some(2)
    );
    assert_eq!(document.revision, 0);
    assert!(!document.is_modified);
}

#[test]
fn text_settings_typed_tabs_use_indent_stops_and_clipboard_tabs_stay_literal() {
    let mut model = common::test_model("éx", 0, 2);
    model.config.text.indent_style = Some(IndentStyle::Space);
    model.config.text.indent_size = Some(3);
    model.config.text.tab_width = Some(8);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('\t')));
    assert_eq!(model.document().buffer.to_string(), "éx ");
    update(
        &mut model,
        Msg::App(token::messages::AppMsg::PasteFromClipboard("\t".into())),
    );
    assert_eq!(model.document().buffer.to_string(), "éx \t");
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "éx");
}

#[test]
fn text_settings_indent_step_is_distinct_from_tab_width() {
    let mut model = common::test_model("\tx", 0, 1);
    model.config.text.indent_size = Some(2);
    model.config.text.tab_width = Some(8);
    update(&mut model, Msg::Document(DocumentMsg::IndentLines));
    assert_eq!(model.document().buffer.to_string(), "\t  x");
    update(&mut model, Msg::Document(DocumentMsg::UnindentLines));
    assert_eq!(
        TabStops::new(8).visual_width(model.document().buffer.chars().take_while(|ch| *ch != 'x')),
        8
    );
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "\tx");
}

#[test]
fn text_settings_line_ending_detection_counts_crlf_once_and_breaks_ties_by_order() {
    for (text, ending) in [
        ("", LineEnding::Lf),
        ("a\rb\rc\n", LineEnding::Cr),
        ("a\r\nb\nc\r\n", LineEnding::Crlf),
        ("a\r\nb\n", LineEnding::Crlf),
        ("a\nb\r\n", LineEnding::Lf),
    ] {
        assert_eq!(LineEnding::detect(text), ending);
        let mut model = common::test_model(text, 0, 0);
        update(&mut model, Msg::Document(DocumentMsg::InsertNewline));
        assert!(model
            .document()
            .buffer
            .to_string()
            .starts_with(ending.as_str()));
    }
}
