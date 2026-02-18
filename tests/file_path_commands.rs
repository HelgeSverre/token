//! Tests for file path commands: Reveal in Finder, Copy Absolute Path, Copy Relative Path

mod common;

use std::path::PathBuf;

use common::test_model;
use token::commands::{filter_commands, Cmd, CommandId};
use token::update::execute_command;

// ============================================================================
// Command Palette Registration Tests
// ============================================================================

#[test]
fn test_reveal_in_finder_appears_in_palette() {
    let results = filter_commands("Reveal");
    assert!(
        results
            .iter()
            .any(|cmd| cmd.id == CommandId::RevealInFinder),
        "RevealInFinder should appear when searching 'Reveal'"
    );
}

#[test]
fn test_copy_absolute_path_appears_in_palette() {
    let results = filter_commands("Copy Absolute");
    assert!(
        results
            .iter()
            .any(|cmd| cmd.id == CommandId::CopyAbsolutePath),
        "CopyAbsolutePath should appear when searching 'Copy Absolute'"
    );
}

#[test]
fn test_copy_relative_path_appears_in_palette() {
    let results = filter_commands("Copy Relative");
    assert!(
        results
            .iter()
            .any(|cmd| cmd.id == CommandId::CopyRelativePath),
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
    assert!(model.ui.status_message.contains("unsaved"));
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
        model.ui.status_message.contains("/tmp/test.txt")
            || model.ui.status_message.contains("clipboard"),
        "Status should mention path or clipboard, got: {}",
        model.ui.status_message
    );
}

#[test]
fn test_copy_absolute_path_without_file() {
    let mut model = test_model("hello\n", 0, 0);
    assert!(model.document().file_path.is_none());

    let cmd = execute_command(&mut model, CommandId::CopyAbsolutePath);
    assert!(cmd.is_some());

    assert!(
        model.ui.status_message.contains("unsaved"),
        "Status should mention unsaved, got: {}",
        model.ui.status_message
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
        model.ui.status_message.contains("unsaved"),
        "Status should mention unsaved, got: {}",
        model.ui.status_message
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
        model.ui.status_message.contains("/tmp/project/src/main.rs")
            || model.ui.status_message.contains("clipboard"),
        "Status should contain absolute path or clipboard error, got: {}",
        model.ui.status_message
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
