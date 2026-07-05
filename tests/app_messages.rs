//! Regression tests for AppMsg handlers in src/update/app.rs:
//! - FileLoaded resetting view_mode/tab_content for non-text tabs
//! - OpenFileDialogResult preserving per-file commands
//! - ReloadConfiguration triggering a full redraw

mod common;

use common::test_model;
use token::commands::Cmd;
use token::messages::{AppMsg, Msg};
use token::model::editor::{TabContent, ViewMode};
use token::update::update;

// ============================================================================
// FileLoaded must not leave a stale non-text view_mode/tab_content behind
// ============================================================================

#[test]
fn file_loaded_resets_view_mode_for_non_text_tab() {
    let mut model = test_model("hello\n", 0, 0);

    // Simulate a focused tab that is currently showing an image, as would
    // happen if the user had an image tab focused when triggering
    // "Open Keybindings" or "Open Log File" (both route through
    // Cmd::OpenFileInEditor -> AppMsg::FileLoaded on the *focused* editor).
    {
        let editor = model.editor_mut();
        editor.view_mode = ViewMode::Image(Box::new(token::image::ImageState::new(
            vec![0u8; 4],
            1,
            1,
            4,
            "PNG".to_string(),
            100,
            100,
        )));
        editor.tab_content =
            TabContent::BinaryPlaceholder(token::model::editor::BinaryPlaceholderState {
                path: std::path::PathBuf::from("/tmp/pic.png"),
                size_bytes: 1234,
            });
    }
    assert!(model.editor().view_mode.is_image());

    let path = std::path::PathBuf::from("/tmp/keymap.yaml");
    let result: Result<String, String> = Ok("keymap: contents".to_string());
    update(
        &mut model,
        Msg::App(AppMsg::FileLoaded {
            path: path.clone(),
            result,
        }),
    );

    assert!(
        matches!(model.editor().view_mode, ViewMode::Text),
        "view_mode should be reset to Text after FileLoaded overwrites the document"
    );
    assert!(
        matches!(model.editor().tab_content, TabContent::Text),
        "tab_content should be reset to Text after FileLoaded overwrites the document"
    );
    assert_eq!(model.document().buffer.to_string(), "keymap: contents");
}

// ============================================================================
// OpenFileDialogResult must not discard per-file commands
// ============================================================================

#[test]
fn open_file_dialog_result_preserves_per_file_commands() {
    use std::fs;
    use tempfile::tempdir;

    let mut model = test_model("hello\n", 0, 0);

    let dir = tempdir().expect("failed to create temp dir");
    let file_a = dir.path().join("a.rs");
    let file_b = dir.path().join("b.rs");
    fs::write(&file_a, "fn a() {}").unwrap();
    fs::write(&file_b, "fn b() {}").unwrap();

    let cmd = update(
        &mut model,
        Msg::App(AppMsg::OpenFileDialogResult {
            paths: vec![file_a.clone(), file_b.clone()],
        }),
    );

    let cmd = cmd.expect("OpenFileDialogResult should return a command");
    match cmd {
        Cmd::Batch(cmds) => {
            // Both files are Rust source (`.rs`), which has syntax highlighting,
            // so opening them should schedule a DebouncedSyntaxParse for each
            // in addition to the final Redraw. Previously these per-file
            // commands were silently discarded.
            let syntax_parses = cmds
                .iter()
                .filter(|c| matches!(c, Cmd::DebouncedSyntaxParse { .. }))
                .count();
            assert!(
                syntax_parses >= 1,
                "expected at least one DebouncedSyntaxParse command to survive in the batch, got: {:?}",
                cmds
            );
            assert!(
                cmds.iter().any(|c| matches!(c, Cmd::Redraw)),
                "expected a final Redraw command in the batch, got: {:?}",
                cmds
            );
        }
        other => panic!(
            "expected Cmd::Batch preserving per-file commands, got: {:?}",
            other
        ),
    }

    // Both files should actually be open as tabs.
    assert!(model.editor_area.find_open_file(&file_a).is_some());
    assert!(model.editor_area.find_open_file(&file_b).is_some());
}

// ============================================================================
// ReloadConfiguration must trigger a full redraw, not just the status bar
// ============================================================================

#[test]
fn reload_configuration_returns_full_redraw() {
    let mut model = test_model("hello\n", 0, 0);

    let cmd = update(&mut model, Msg::App(AppMsg::ReloadConfiguration));

    assert!(
        matches!(cmd, Some(Cmd::Redraw)),
        "ReloadConfiguration should return a full Cmd::Redraw so theme changes repaint immediately, got: {:?}",
        cmd
    );
}
