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

/// Explicitly deliver text-file replies for these update-only fixtures.
fn finish_file_opens(model: &mut token::AppModel, cmd: Cmd) -> Cmd {
    match cmd {
        Cmd::PrepareFileOpen(request) => {
            let path = request.source.path().unwrap().to_path_buf();
            let document = token::model::Document::from_file(path.clone()).unwrap();
            let result = Ok(Box::new(token::model::PreparedFile::Loaded {
                document: Box::new(document),
                view_mode: ViewMode::Text,
                tab_content: TabContent::Text,
            }));
            update(
                model,
                Msg::Layout(token::messages::LayoutMsg::FilePrepared { request, result }),
            )
            .unwrap()
        }
        Cmd::Batch(commands) => {
            let mut effects = Vec::new();
            for cmd in commands {
                match finish_file_opens(model, cmd) {
                    Cmd::Batch(commands) => effects.extend(commands),
                    command => effects.push(command),
                }
            }
            Cmd::Batch(effects)
        }
        command => command,
    }
}

#[test]
fn config_resource_actions_defer_discovery_and_share_keyboard_palette_routing() {
    use token::commands::{CommandId, ConfigResource};
    let mut model = test_model("unsaved", 0, 0);
    for (action, resource) in [
        (CommandId::OpenConfigDirectory, ConfigResource::Directory),
        (CommandId::OpenKeybindings, ConfigResource::Keybindings),
        (CommandId::OpenLogFile, ConfigResource::Log),
    ] {
        let cmd = token::update::execute_command(&mut model, action).unwrap();
        assert!(matches!(&cmd, Cmd::PrepareFileOpen(request)
            if request.source == token::model::FileOpenSource::Configuration(resource)));
        assert!(cmd.needs_redraw(), "preparation updates the loading status");
        assert_eq!(model.document().buffer.to_string(), "unsaved");
        assert_eq!(model.editor_area.documents.len(), 1);
    }
    let messages = token::keymap::Command::OpenLogFile.to_msgs();
    assert!(matches!(
        messages.as_slice(),
        [Msg::App(AppMsg::OpenConfigResource(ConfigResource::Log))]
    ));
    for message in messages {
        assert!(matches!(
            update(&mut model, message),
            Some(Cmd::PrepareFileOpen(request))
                if request.source == token::model::FileOpenSource::Configuration(ConfigResource::Log)
        ));
    }
}

#[test]
fn config_resource_files_open_new_tabs_and_reuse_unsaved_buffers() {
    use token::commands::ConfigResource;
    use token::messages::DocumentMsg;
    let dir = tempfile::tempdir().unwrap();
    for (resource, name) in [
        (ConfigResource::Keybindings, "keymap.yaml"),
        (ConfigResource::Log, "token.log.2026-09-06"),
    ] {
        let path = dir.path().join(name);
        std::fs::write(&path, "on disk").unwrap();
        let mut model = test_model("original", 0, 8);
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
        let original = model.document().clone();
        let original_id = original.id.unwrap();
        let open = |model: &mut token::AppModel| {
            let Cmd::PrepareFileOpen(request) =
                update(model, Msg::App(AppMsg::OpenConfigResource(resource))).unwrap()
            else {
                panic!("configuration open must be deferred")
            };
            update(
                model,
                Msg::Layout(token::messages::LayoutMsg::FilePrepared {
                    request,
                    result: Ok(Box::new(token::model::PreparedFile::Loaded {
                        document: Box::new(
                            token::model::Document::from_file(path.clone()).unwrap(),
                        ),
                        view_mode: ViewMode::Text,
                        tab_content: TabContent::Text,
                    })),
                }),
            )
        };
        let cmd = open(&mut model).unwrap();
        assert!(cmd.needs_redraw());
        finish_file_opens(&mut model, cmd);
        assert_eq!(model.editor_area.documents.len(), 2);
        let kept = &model.editor_area.documents[&original_id];
        assert_eq!(kept.buffer, original.buffer);
        assert!(kept.is_modified);
        assert_eq!(kept.undo_stack.len(), original.undo_stack.len());
        assert_eq!(model.document().buffer.to_string(), "on disk");
        assert_eq!(model.document().file_path.as_ref(), Some(&path));
        assert!(model.editor().is_plain_text_mode());
        let resource_id = model.document().id;
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        let edited = model.document().buffer.clone();
        open(&mut model);
        assert_eq!(model.editor_area.documents.len(), 2);
        assert_eq!(model.document().id, resource_id);
        assert_eq!(
            model.document().buffer,
            edited,
            "reopening must not reload over unsaved edits"
        );
        assert!(model.document().is_modified);
    }
}

#[test]
fn config_resource_preparation_errors_and_directories_preserve_focused_buffer() {
    use token::commands::ConfigResource;
    let mut model = test_model("keep me", 0, 0);
    for resource in [
        ConfigResource::Directory,
        ConfigResource::Keybindings,
        ConfigResource::Log,
    ] {
        let Cmd::PrepareFileOpen(request) =
            update(&mut model, Msg::App(AppMsg::OpenConfigResource(resource))).unwrap()
        else {
            panic!("configuration request")
        };
        let cmd = update(
            &mut model,
            Msg::Layout(token::messages::LayoutMsg::FilePrepared {
                request,
                result: Err("permission denied".to_owned()),
            }),
        )
        .unwrap();
        assert!(cmd.needs_redraw());
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("permission denied"));
        assert_eq!(model.editor_area.documents.len(), 1);
        assert_eq!(model.document().buffer.to_string(), "keep me");
    }
    let path = std::path::PathBuf::from("/config/directory");
    let Cmd::PrepareFileOpen(request) = update(
        &mut model,
        Msg::App(AppMsg::OpenConfigResource(ConfigResource::Directory)),
    )
    .unwrap() else {
        panic!("configuration request")
    };
    let Cmd::Batch(commands) = update(
        &mut model,
        Msg::Layout(token::messages::LayoutMsg::FilePrepared {
            request,
            result: Ok(Box::new(token::model::PreparedFile::Directory {
                path: path.clone(),
            })),
        }),
    )
    .unwrap() else {
        panic!("directory reveal effects")
    };
    assert!(commands
        .iter()
        .any(|cmd| matches!(cmd, Cmd::OpenInExplorer { path: found } if found == &path)));
    assert_eq!(model.document().buffer.to_string(), "keep me");
}

// ============================================================================
// FileLoaded must not leave a stale non-text view_mode/tab_content behind
// ============================================================================

#[test]
fn file_loaded_resets_view_mode_for_non_text_tab() {
    let mut model = test_model("hello\n", 0, 0);

    // An explicit LoadFile replaces the focused buffer, even if that tab
    // previously showed an image. Configuration-opening actions no longer
    // use this replacement path; they open/reuse ordinary tabs.
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
    let Cmd::LoadFile { target, .. } =
        update(&mut model, Msg::App(AppMsg::LoadFile(path.clone()))).unwrap()
    else {
        panic!("load request")
    };
    update(
        &mut model,
        Msg::App(AppMsg::FileLoaded {
            identity: None,
            target,
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
    let group_id = model.editor_area.focused_group_id;

    let cmd = update(
        &mut model,
        Msg::App(AppMsg::OpenFileDialogResult {
            group_id,
            paths: vec![file_a.clone(), file_b.clone()],
        }),
    );

    let cmd = cmd.expect("OpenFileDialogResult should return a command");
    let cmd = finish_file_opens(&mut model, cmd);
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
    assert!(matches!(cmd, Some(Cmd::ReloadConfiguration)));
    let cmd = update(
        &mut model,
        Msg::App(AppMsg::ConfigurationLoaded {
            config: Box::default(),
            theme: Box::default(),
            result: token::config::ReloadResult::Loaded,
        }),
    );

    let damage = cmd
        .as_ref()
        .expect("ReloadConfiguration must return a command")
        .damage();
    assert!(
        damage.is_full(),
        "ReloadConfiguration must produce full-redraw damage so theme changes repaint immediately, got: {:?}",
        cmd
    );
}
