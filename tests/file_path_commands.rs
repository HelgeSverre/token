//! Tests for file path commands: Reveal in Finder, Copy Absolute Path, Copy Relative Path

mod common;

use std::path::PathBuf;

use common::test_model;
use token::command_history::CommandHistory;
use token::commands::{Cmd, CommandId};
use token::model::{AppModel, CommandPaletteState, SegmentId};
use token::update::{execute_command, resolve_palette_rows};

fn status_message(model: &AppModel) -> &str {
    model
        .ui
        .status_bar
        .get_segment(SegmentId::StatusMessage)
        .map_or("", |segment| segment.content.display_text())
}

// ============================================================================
// Command Palette Registration Tests
// ============================================================================

fn palette_matches(query: &str) -> Vec<CommandId> {
    let mut state = CommandPaletteState::default();
    state.set_input(query);
    resolve_palette_rows(&mut state, &CommandHistory::default());
    state.matches.iter().map(|m| m.def.id).collect()
}

#[test]
fn test_reveal_in_finder_appears_in_palette() {
    let results = palette_matches("Reveal");
    assert!(
        results.contains(&CommandId::RevealInFinder),
        "RevealInFinder should appear when searching 'Reveal'"
    );
}

#[test]
fn test_copy_absolute_path_appears_in_palette() {
    let results = palette_matches("Copy Absolute");
    assert!(
        results.contains(&CommandId::CopyAbsolutePath),
        "CopyAbsolutePath should appear when searching 'Copy Absolute'"
    );
}

#[test]
fn test_copy_relative_path_appears_in_palette() {
    let results = palette_matches("Copy Relative");
    assert!(
        results.contains(&CommandId::CopyRelativePath),
        "CopyRelativePath should appear when searching 'Copy Relative'"
    );
}

// ============================================================================
// Reveal in Finder Tests
// ============================================================================

#[test]
fn test_reveal_in_finder_with_file_path() {
    let mut model = test_model("hello\n", 0, 0);
    model.document_mut().file_path = Some(PathBuf::from("/tmp/test.txt"));

    let cmd = execute_command(&mut model, CommandId::RevealInFinder);
    assert!(cmd.is_some());

    // Should produce a batch containing RevealFileInFinder
    match cmd.unwrap() {
        Cmd::Batch(cmds) => {
            assert!(cmds
                .iter()
                .any(|c| matches!(c, Cmd::RevealFileInFinder { .. })));
        }
        _ => panic!("Expected Cmd::Batch"),
    }
}

#[test]
fn test_reveal_in_finder_without_file_path() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.document().file_path.is_none());

    let cmd = execute_command(&mut model, CommandId::RevealInFinder);
    assert!(cmd.is_some());

    // Should set a status message about unsaved file
    assert!(status_message(&model).contains("unsaved"));
    // Should NOT produce a RevealFileInFinder command
    assert!(!matches!(cmd.unwrap(), Cmd::Batch(_)));
}

// ============================================================================
// Copy Absolute Path Tests
// ============================================================================

#[test]
fn test_copy_absolute_path_with_file() {
    let mut model = test_model("hello\n", 0, 0);
    model.document_mut().file_path = Some(PathBuf::from("/tmp/test.txt"));

    let cmd = execute_command(&mut model, CommandId::CopyAbsolutePath);
    assert!(cmd.is_some());

    // Should set a status message confirming the copy
    assert!(
        status_message(&model).contains("/tmp/test.txt")
            || status_message(&model).contains("clipboard"),
        "Status should mention path or clipboard, got: {}",
        status_message(&model)
    );
}

#[test]
fn test_copy_absolute_path_without_file() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.document().file_path.is_none());

    let cmd = execute_command(&mut model, CommandId::CopyAbsolutePath);
    assert!(cmd.is_some());

    assert!(
        status_message(&model).contains("unsaved"),
        "Status should mention unsaved, got: {}",
        status_message(&model)
    );
}

// ============================================================================
// Copy Relative Path Tests
// ============================================================================

#[test]
fn test_copy_relative_path_without_file() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.document().file_path.is_none());

    let cmd = execute_command(&mut model, CommandId::CopyRelativePath);
    assert!(cmd.is_some());

    assert!(
        status_message(&model).contains("unsaved"),
        "Status should mention unsaved, got: {}",
        status_message(&model)
    );
}

#[test]
fn test_copy_relative_path_without_workspace_falls_back_to_absolute() {
    let mut model = test_model("hello\n", 0, 0);
    model.document_mut().file_path = Some(PathBuf::from("/tmp/project/src/main.rs"));
    assert!(model.workspace_root().is_none());

    let cmd = execute_command(&mut model, CommandId::CopyRelativePath);
    assert!(cmd.is_some());

    // Without workspace, should fall back to absolute path in status
    assert!(
        status_message(&model).contains("/tmp/project/src/main.rs")
            || status_message(&model).contains("clipboard"),
        "Status should contain absolute path or clipboard error, got: {}",
        status_message(&model)
    );
}

// ============================================================================
// Command returns correct Cmd type
// ============================================================================

#[test]
fn test_reveal_returns_reveal_cmd_with_correct_path() {
    let mut model = test_model("hello\n", 0, 0);
    let path = PathBuf::from("/Users/test/project/file.rs");
    model.document_mut().file_path = Some(path.clone());

    let cmd = execute_command(&mut model, CommandId::RevealInFinder).unwrap();
    match cmd {
        Cmd::Batch(cmds) => {
            let has_reveal = cmds.iter().any(|c| match c {
                Cmd::RevealFileInFinder { path: p } => *p == path,
                _ => false,
            });
            assert!(
                has_reveal,
                "Batch should contain RevealFileInFinder with the correct path"
            );
        }
        _ => panic!("Expected Cmd::Batch"),
    }
}

#[test]
fn test_all_path_commands_return_redraw_status_bar() {
    let commands = [
        CommandId::RevealInFinder,
        CommandId::CopyAbsolutePath,
        CommandId::CopyRelativePath,
    ];

    for cmd_id in &commands {
        let mut model = test_model("hello\n", 0, 0);
        // Test without file path - should still return a cmd (status bar redraw)
        let result = execute_command(&mut model, *cmd_id);
        assert!(
            result.is_some(),
            "{:?} should always return Some(Cmd)",
            cmd_id
        );
    }
}
