//! The Find bar reserves editor space without capturing the entire editor.
mod common;

use token::messages::{DocumentMsg, LayoutMsg, ModalMsg, Msg, UiMsg};
use token::model::{AppModel, FindReplaceField, FocusTarget, ModalId, Position, Rect, Selection};
use token::update::update;
use token::view::find_bar::{Control, FindBarLayout};
use token::view::geometry::GroupLayout;
use token::view::hit_test::{hit_test_groups, HitTarget, Point};

fn open(model: &mut AppModel, replace: bool) {
    update(model, Msg::Ui(UiMsg::OpenFind { replace }));
}

#[test]
fn find_bar_keeps_search_and_editor_usable_across_focus_and_modals() {
    let mut model = common::test_model("foo bar foo", 0, 0);
    open(&mut model, false);
    assert!(!model.ui.has_modal());
    assert_eq!(model.ui.focus, FocusTarget::FindBar);
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("foo".into()))),
    );
    assert_eq!(model.document().buffer.to_string(), "foo bar foo");
    model.ui.focus_editor();
    update(
        &mut model,
        Msg::Document(DocumentMsg::InsertText("foo ".into())),
    );
    assert_eq!(
        model
            .ui
            .find_bar
            .as_ref()
            .unwrap()
            .matches(model.document())
            .len(),
        3
    );

    update(&mut model, Msg::Ui(UiMsg::ToggleModal(ModalId::GotoLine)));
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("1".into()))),
    );
    assert_eq!(model.ui.find_bar.as_ref().unwrap().query(), "foo");
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
    assert!(model.ui.find_bar.is_some());

    open(&mut model, true);
    assert!(model.ui.find_bar.as_ref().unwrap().replace_mode);
    update(
        &mut model,
        Msg::Ui(UiMsg::FocusFindField(FindReplaceField::Replace)),
    );
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("baz".into()))),
    );
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::ReplaceAll)));
    assert_eq!(model.document().buffer.to_string(), "baz baz bar baz");
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "foo foo bar foo");
    update(&mut model, Msg::Ui(UiMsg::CloseFind));
    assert!(model.ui.find_bar.is_none());
    open(&mut model, false);
    assert_eq!(model.ui.find_bar.as_ref().unwrap().query(), "foo");
}

#[test]
fn find_bar_scopes_cannot_leak_into_another_document() {
    let mut model = common::test_model("foo foo", 0, 0);
    open(&mut model, false);
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("foo".into()))),
    );
    model.editor_mut().selections[0] =
        Selection::from_anchor_head(Position::new(0, 0), Position::new(0, 3));
    model.editor_mut().cursors[0].column = 3;
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::ToggleFindReplaceSelectionOnly)),
    );
    assert_eq!(model.ui.find_bar.as_ref().unwrap().scope, Some((0, 3)));
    update(&mut model, Msg::Layout(LayoutMsg::NewTab));
    let state = model.ui.find_bar.as_ref().unwrap();
    assert!(!state.selection_only);
    assert_eq!(state.scope, None);
    assert_eq!(state.query(), "foo");
}

#[test]
fn find_bar_geometry_agrees_with_viewport_and_hits_at_different_sizes() {
    for scale in [1.0, 1.25, 2.0] {
        for width in [220.0, 400.0, 1000.0] {
            let mut model = AppModel::with_document(
                (width * scale) as u32,
                (800.0 * scale) as u32,
                scale,
                token::model::Document::with_text(&"alpha beta gamma\n".repeat(80)),
            );
            model.editor_area.compute_layout(Rect::new(
                17.5,
                23.5,
                (width * scale) as f32 + 0.5,
                (800.0 * scale) as f32 + 0.5,
            ));
            model.resync_viewports();
            let full_height = model.editor().viewport.pixels.y.extent;
            open(&mut model, true);
            let bar = FindBarLayout::new(&model).unwrap();
            let group = model.editor_area.focused_group().unwrap();
            let content = GroupLayout::new(group, &model, model.char_width);
            assert_eq!(content.content_rect.y, bar.rect.y + bar.rect.height);
            assert_eq!(
                model.editor().viewport.pixels.y.extent,
                content.content_h() as f64
            );
            assert_eq!(
                model.editor().viewport.pixels.x.extent,
                content.text_width() as f64
            );
            assert_eq!(
                model.editor().viewport.visible_lines,
                content.visible_lines(model.line_height)
            );
            assert_eq!(
                model.editor().viewport.visible_columns,
                content.visible_columns(
                    model.char_width,
                    model.editor().soft_wrap,
                    model.metrics.scrollbar_width,
                )
            );
            let x = content.text_start_x as f64 + model.char_width as f64 * 3.0;
            let y = content.content_y() as f64 + model.line_height as f64 * 0.25;
            assert_eq!(
                token::view::geometry::pixel_to_cursor(
                    x,
                    y,
                    model.char_width,
                    model.line_height as f64,
                    &model
                ),
                (0, 3)
            );
            assert_eq!(
                token::view::geometry::pixel_to_line_and_visual_column(
                    x,
                    y,
                    model.char_width,
                    model.line_height as f64,
                    &model
                ),
                (0, 3)
            );
            assert_eq!(
                model.editor().viewport.pixels.y.extent,
                full_height - bar.rect.height as f64
            );
            for (control, rect) in &bar.controls {
                assert!(rect.x as f32 >= bar.rect.x);
                assert!(
                    (rect.x + rect.w) as f32 <= bar.rect.x + bar.rect.width,
                    "{control:?} overflows at {width}"
                );
                let point = Point {
                    x: (rect.x + rect.w / 2) as f64,
                    y: (rect.y + rect.h / 2) as f64,
                };
                assert!(
                    matches!(hit_test_groups(&model, point, model.char_width), Some(HitTarget::FindBar { control: Some(hit) }) if hit == *control)
                );
            }
            let input = bar.field(FindReplaceField::Query).unwrap();
            assert!(input.w > 0);
            assert!(matches!(
                bar.hit((input.x + 1) as f64, (input.y + 1) as f64),
                Some(Control::Field(FindReplaceField::Query))
            ));
            update(&mut model, Msg::Ui(UiMsg::CloseFind));
            assert_eq!(model.editor().viewport.pixels.y.extent, full_height);
        }
    }
}

#[test]
fn find_bar_pointer_selection_edits_only_the_field() {
    let mut model = common::test_model("unchanged", 0, 0);
    open(&mut model, true);
    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("first second".into()))),
    );
    for (column, clicks, extend) in [(0, 1, false), (5, 0, true)] {
        update(
            &mut model,
            Msg::Ui(UiMsg::FindFieldPointer {
                field: FindReplaceField::Query,
                column,
                extend,
                clicks,
            }),
        );
    }
    assert_eq!(
        model
            .ui
            .find_bar
            .as_ref()
            .unwrap()
            .query_editable
            .selected_text(),
        "first"
    );
    update(&mut model, Msg::Ui(UiMsg::EndFindSelection));
    assert_eq!(model.ui.find_selection_drag, None);
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('x'))));
    assert_eq!(model.ui.find_bar.as_ref().unwrap().query(), "x second");
    assert_eq!(model.document().buffer.to_string(), "unchanged");
}

#[test]
fn find_bar_follows_active_text_pane_and_hides_on_special_tabs() {
    let mut model = AppModel::new(1000, 800, 1.0);
    model
        .editor_area
        .compute_layout(Rect::new(0.0, 0.0, 1000.0, 780.0));
    let original = model.editor_area.focused_group_id;
    open(&mut model, false);
    update(
        &mut model,
        Msg::Layout(LayoutMsg::SplitFocused(
            token::model::editor_area::SplitDirection::Vertical,
        )),
    );
    let other = &model.editor_area.groups[&original];
    let other_layout = GroupLayout::new(other, &model, model.char_width);
    assert_eq!(
        other_layout.content_rect.y,
        other.rect.y + model.metrics.tab_bar_height as f32
    );
    assert_eq!(
        FindBarLayout::new(&model).unwrap().rect.x,
        model.editor_area.focused_group().unwrap().rect.x
    );

    model.editor_mut().tab_content =
        token::model::TabContent::BinaryPlaceholder(token::model::BinaryPlaceholderState {
            path: "fixture.bin".into(),
            size_bytes: 0,
        });
    update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(FindBarLayout::new(&model).is_none());
    assert_eq!(model.ui.focus, FocusTarget::Editor);
    open(&mut model, true);
    assert_eq!(model.ui.focus, FocusTarget::Editor);
    model.editor_mut().tab_content = token::model::TabContent::Text;
    update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(FindBarLayout::new(&model).is_some());
}
