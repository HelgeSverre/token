//! Modal handler tests
//!
//! Tests for modal system: command palette, goto line, find/replace, theme picker

mod common;

use common::test_model;

use token::messages::{ModalMsg, Msg, UiMsg};
use token::model::{
    CommandPaletteState, FindReplaceState, GotoLineState, ModalId, ModalState, ThemePickerState,
};
use token::update::update;

// Helper to create a CommandPaletteState with initial text
fn command_palette_with_input(text: &str, selected_index: usize) -> CommandPaletteState {
    let mut state = CommandPaletteState::default();
    state.set_input(text);
    state.selected_index = selected_index;
    state
}

// Helper to create a GotoLineState with initial text
fn goto_line_with_input(text: &str) -> GotoLineState {
    let mut state = GotoLineState::default();
    state.set_input(text);
    state
}

// Helper to create a FindReplaceState with initial text
fn find_replace_with_query(query: &str) -> FindReplaceState {
    let mut state = FindReplaceState::default();
    state.set_query(query);
    state
}

// ========================================================================
// Modal Open/Close Tests
// ========================================================================

#[test]
fn test_toggle_modal_opens_command_palette() {
    let mut model = test_model("hello\n", 0, 0);

    assert!(model.ui.active_modal.is_none());

    update(
        &mut model,
        Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
    );

    assert!(model.ui.active_modal.is_some());
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::CommandPalette
    );
}

#[test]
fn test_toggle_modal_closes_same_modal() {
    let mut model = test_model("hello\n", 0, 0);

    update(
        &mut model,
        Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
    );
    assert!(model.ui.active_modal.is_some());

    update(
        &mut model,
        Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
    );
    assert!(model.ui.active_modal.is_none());
}

#[test]
fn test_toggle_modal_switches_to_different_modal() {
    let mut model = test_model("hello\n", 0, 0);

    update(
        &mut model,
        Msg::Ui(UiMsg::ToggleModal(ModalId::CommandPalette)),
    );
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::CommandPalette
    );

    update(&mut model, Msg::Ui(UiMsg::ToggleModal(ModalId::GotoLine)));
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::GotoLine
    );
}

#[test]
fn test_modal_close_message() {
    let mut model = test_model("hello\n", 0, 0);

    model
        .ui
        .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));
    assert!(model.ui.active_modal.is_some());

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
    assert!(model.ui.active_modal.is_none());
}

// ========================================================================
// Command Palette Input Tests
// ========================================================================

#[test]
fn test_command_palette_insert_char() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('s'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('a'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('v'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('e'))));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "save");
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_delete_backward() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(command_palette_with_input(
            "save", 0,
        )));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteBackward)));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "sav");
        assert_eq!(state.selected_index, 0); // Reset on delete
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_delete_word_backward() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(command_palette_with_input(
            "switch theme",
            5,
        )));

    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::DeleteWordBackward)),
    );

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "switch ");
        assert_eq!(state.selected_index, 0); // Reset on delete
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_select_next() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, 2);
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_select_previous() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(command_palette_with_input(
            "", 5,
        )));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectPrevious)));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, 4);
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_select_previous_at_zero() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectPrevious)));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, 0); // Stays at 0
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_command_palette_input_resets_selection() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(command_palette_with_input(
            "", 5,
        )));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('a'))));

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, 0); // Reset when input changes
        assert_eq!(state.input(), "a");
    } else {
        panic!("Expected command palette modal");
    }
}

// ========================================================================
// Goto Line Input Tests
// ========================================================================

#[test]
fn test_goto_line_accepts_digits() {
    let mut model = test_model("line1\nline2\nline3\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(GotoLineState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('1'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('2'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('3'))));

    if let Some(ModalState::GotoLine(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "123");
    } else {
        panic!("Expected goto line modal");
    }
}

#[test]
fn test_goto_line_accepts_colon() {
    let mut model = test_model("line1\nline2\nline3\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(GotoLineState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('1'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('0'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar(':'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('5'))));

    if let Some(ModalState::GotoLine(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "10:5");
    } else {
        panic!("Expected goto line modal");
    }
}

#[test]
fn test_goto_line_rejects_letters() {
    let mut model = test_model("line1\nline2\nline3\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(GotoLineState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('1'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('a'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('2'))));

    if let Some(ModalState::GotoLine(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "12"); // 'a' was rejected
    } else {
        panic!("Expected goto line modal");
    }
}

#[test]
fn test_goto_line_confirm_jumps_to_line() {
    let mut model = test_model("line1\nline2\nline3\nline4\nline5\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(goto_line_with_input("3")));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));

    assert!(model.ui.active_modal.is_none()); // Modal closed
    assert_eq!(model.editor().primary_cursor().line, 2); // 0-indexed, so line 3 = index 2
}

#[test]
fn test_goto_line_confirm_with_column() {
    let mut model = test_model("hello world\nfoo bar\nbaz qux\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(goto_line_with_input("2:5")));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));

    assert!(model.ui.active_modal.is_none());
    assert_eq!(model.editor().primary_cursor().line, 1); // Line 2 = index 1
    assert_eq!(model.editor().primary_cursor().column, 4); // Column 5 = index 4
}

#[test]
fn test_goto_line_clamps_beyond_document() {
    let mut model = test_model("line1\nline2", 0, 0); // No trailing newline = 2 lines
    model
        .ui
        .open_modal(ModalState::GotoLine(goto_line_with_input("999")));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));

    assert!(model.ui.active_modal.is_none());
    // Should clamp to last line (index 1 for 2-line document)
    assert_eq!(model.editor().primary_cursor().line, 1);
}

#[test]
fn test_goto_line_empty_input_goes_to_line_1() {
    let mut model = test_model("line1\nline2\nline3\n", 2, 3);
    model
        .ui
        .open_modal(ModalState::GotoLine(GotoLineState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Confirm)));

    assert!(model.ui.active_modal.is_none());
    assert_eq!(model.editor().primary_cursor().line, 0); // Line 1 = index 0
}

// ========================================================================
// Find/Replace Input Tests
// ========================================================================

#[test]
fn test_find_replace_insert_char() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::FindReplace(FindReplaceState::default()));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('h'))));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('i'))));

    if let Some(ModalState::FindReplace(state)) = &model.ui.active_modal {
        assert_eq!(state.query(), "hi");
    } else {
        panic!("Expected find/replace modal");
    }
}

#[test]
fn test_find_replace_delete_backward() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::FindReplace(find_replace_with_query("search")));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteBackward)));

    if let Some(ModalState::FindReplace(state)) = &model.ui.active_modal {
        assert_eq!(state.query(), "searc");
    } else {
        panic!("Expected find/replace modal");
    }
}

// ========================================================================
// Theme Picker Tests
// ========================================================================

#[test]
fn test_theme_picker_select_next() {
    let mut model = test_model("hello\n", 0, 0);
    model.ui.open_modal(ModalState::ThemePicker(
        ThemePickerState::new("default-dark".to_string()),
    ));

    let initial_index = if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
        state.selected_index
    } else {
        panic!("Expected theme picker modal");
    };

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));

    if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, initial_index + 1);
    } else {
        panic!("Expected theme picker modal");
    }
}

#[test]
fn test_theme_picker_select_previous() {
    let mut model = test_model("hello\n", 0, 0);
    let mut picker_state = ThemePickerState::new("default-dark".to_string());
    picker_state.selected_index = 2;
    model.ui.open_modal(ModalState::ThemePicker(picker_state));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectPrevious)));

    if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, 1);
    } else {
        panic!("Expected theme picker modal");
    }
}

#[test]
fn test_theme_picker_select_next_respects_bounds() {
    let mut model = test_model("hello\n", 0, 0);
    let mut picker_state = ThemePickerState::new("default-dark".to_string());
    let max_index = picker_state.themes.len().saturating_sub(1);
    picker_state.selected_index = max_index;
    model.ui.open_modal(ModalState::ThemePicker(picker_state));

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));

    if let Some(ModalState::ThemePicker(state)) = &model.ui.active_modal {
        assert_eq!(state.selected_index, max_index); // Stays at max
    } else {
        panic!("Expected theme picker modal");
    }
}

// ========================================================================
// Modal Input Without Active Modal
// ========================================================================

#[test]
fn test_insert_char_without_modal_returns_none() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.ui.active_modal.is_none());

    let result = update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::InsertChar('a'))));
    assert!(result.is_none());
}

#[test]
fn test_delete_backward_without_modal_returns_none() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.ui.active_modal.is_none());

    let result = update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::DeleteBackward)));
    assert!(result.is_none());
}

#[test]
fn test_select_next_without_modal_returns_none() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.ui.active_modal.is_none());

    let result = update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::SelectNext)));
    assert!(result.is_none());
}

// ========================================================================
// SetInput Tests
// ========================================================================

#[test]
fn test_set_input_command_palette() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::CommandPalette(CommandPaletteState::default()));

    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("new text".to_string()))),
    );

    if let Some(ModalState::CommandPalette(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "new text");
    } else {
        panic!("Expected command palette modal");
    }
}

#[test]
fn test_set_input_goto_line() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::GotoLine(GotoLineState::default()));

    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("42".to_string()))),
    );

    if let Some(ModalState::GotoLine(state)) = &model.ui.active_modal {
        assert_eq!(state.input(), "42");
    } else {
        panic!("Expected goto line modal");
    }
}

#[test]
fn test_set_input_find_replace() {
    let mut model = test_model("hello\n", 0, 0);
    model
        .ui
        .open_modal(ModalState::FindReplace(FindReplaceState::default()));

    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::SetInput("search term".to_string()))),
    );

    if let Some(ModalState::FindReplace(state)) = &model.ui.active_modal {
        assert_eq!(state.query(), "search term");
    } else {
        panic!("Expected find/replace modal");
    }
}

// ========================================================================
// Open Modal Messages
// ========================================================================

#[test]
fn test_open_command_palette_message() {
    let mut model = test_model("hello\n", 0, 0);

    update(
        &mut model,
        Msg::Ui(UiMsg::Modal(ModalMsg::OpenCommandPalette)),
    );

    assert!(model.ui.active_modal.is_some());
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::CommandPalette
    );
}

#[test]
fn test_open_goto_line_message() {
    let mut model = test_model("hello\n", 0, 0);

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::OpenGotoLine)));

    assert!(model.ui.active_modal.is_some());
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::GotoLine
    );
}

#[test]
fn test_open_find_replace_message() {
    let mut model = test_model("hello\n", 0, 0);

    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::OpenFindReplace)));

    assert!(model.ui.active_modal.is_some());
    assert_eq!(
        model.ui.active_modal.as_ref().unwrap().id(),
        ModalId::FindReplace
    );
}
