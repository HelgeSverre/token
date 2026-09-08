//! `runtime::app`'s test module, split out of `app.rs` so the
//! production half stays navigable (see the module's own docs there —
//! this file is included via `\#[path\]\) and shares its private items
//! through `use super::\*`.

use super::*;
use crate::automation::OpenPath;
use token::cli::{StartupConfig, StartupMode};
use token::messages::DocumentMsg;
use token::outline::{OutlineData, OutlineKind, OutlineNode, OutlineRange};

#[test]
fn file_identity_runtime_boundary_keeps_snapshot_and_refreshes_a_changed_path() {
    let dir = tempfile::tempdir().unwrap();
    let original = dir.path().join("original.rs");
    let renamed = dir.path().join("renamed.rs");
    std::fs::write(&original, "buffer").unwrap();
    let mut document = token::model::Document::from_file(original.clone()).unwrap();
    let previous_uri = document_uri(&mut document).unwrap();
    std::fs::rename(&original, &renamed).unwrap();
    assert_eq!(document_uri(&mut document).unwrap(), previous_uri);
    document.file_path = Some(renamed.clone());
    let current_uri = document_uri(&mut document).unwrap();
    assert_ne!(current_uri, previous_uri);
    assert_eq!(
        token::lsp::uri_to_path(&current_uri).unwrap(),
        std::fs::canonicalize(renamed).unwrap()
    );
    assert!(!document.matches_file_path(&original));
    assert_eq!(document.buffer.to_string(), "buffer");
}
fn empty_startup_config() -> StartupConfig {
    StartupConfig {
        mode: StartupMode::Empty,
        initial_position: None,
        wait_mode: false,
    }
}

#[test]
fn path_completion_runtime_reads_directory_accepts_and_undoes() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join("assets")).unwrap();
    std::fs::write(dir.path().join("assets/logo.svg"), "fixture").unwrap();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.enabled = false;
    app.model.document_mut().file_path = Some(dir.path().join("readme.txt"));
    app.model.document_mut().buffer = "./as".into();
    app.model.editor_mut().cursors[0] = token::model::Cursor::at(0, 4);
    app.model.editor_mut().clear_selection();
    app.process_automation_msg(Msg::Completion(token::messages::CompletionMsg::TriggerMenu));
    assert!(app.path_worker.is_some());
    assert!(
        app.file_io_tx.is_none(),
        "speculative reads cannot delay ordered saves"
    );
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| app
        .model
        .ui
        .cursor_overlay
        .is_some()));
    let menu = app.model.ui.completion_menu.as_ref().unwrap();
    assert_eq!(menu.selected_item(0).unwrap().label, "assets/");
    app.process_automation_msg(Msg::Completion(
        token::messages::CompletionMsg::AcceptMenuItem,
    ));
    assert_eq!(app.model.document().buffer.to_string(), "./assets/");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model
            .ui
            .completion_menu
            .as_ref()
            .is_some_and(|menu| menu.items.iter().any(|item| item.label == "logo.svg"))
    }));
    app.process_automation_msg(Msg::Completion(
        token::messages::CompletionMsg::AcceptMenuItem,
    ));
    assert_eq!(app.model.document().buffer.to_string(), "./assets/logo.svg");
    app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
    assert_eq!(app.model.document().buffer.to_string(), "./assets/");
    app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
    assert_eq!(app.model.document().buffer.to_string(), "./as");
}

#[test]
fn file_io_runtime_dispatch_keeps_saved_snapshot_and_target_after_focus_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("saved.txt");
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.format_on_save = false;
    app.model.config.lsp.enabled = false;
    app.model.document_mut().file_path = Some(path.clone());
    app.model.document_mut().buffer = "snapshot".into();
    let original = app.model.document().id.unwrap();
    let cmd = update(&mut app.model, Msg::App(AppMsg::SaveFile)).unwrap();
    app.process_cmd(cmd);
    update(&mut app.model, Msg::Document(DocumentMsg::InsertChar('X')));
    update(&mut app.model, Msg::Layout(LayoutMsg::NewTab));
    update(&mut app.model, Msg::Document(DocumentMsg::InsertChar('B')));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_saving));
    assert_eq!(std::fs::read_to_string(path).unwrap(), "snapshot");
    assert!(app.model.editor_area.documents[&original].is_modified);
    assert_eq!(app.model.document().buffer.to_string(), "B");
    assert!(app.model.document().is_modified);
}

#[test]
fn shortcut_hints_and_runtime_dispatch_share_user_overrides_and_context() {
    use token::keymap::{
        default_bindings, merge_bindings, parse_keymap_yaml, KeyCode, Keystroke, Modifiers,
    };
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let overrides = parse_keymap_yaml("bindings:\n  - key: cmd+c\n    command: Unbound\n  - key: ctrl+k ctrl+c\n    command: Copy\n    when: [has_selection]\n").unwrap();
    app.model.ui.keymap = Keymap::with_bindings(merge_bindings(default_bindings(), overrides));
    app.model.document_mut().buffer = "selected".into();
    let context = app.get_key_context();
    assert!(app
        .model
        .ui
        .keymap
        .display_for(Command::Copy, &context)
        .is_none());
    update(
        &mut app.model,
        Msg::Editor(token::messages::EditorMsg::SelectAll),
    );
    let context = app.get_key_context();
    assert!(context.has_selection);
    let hint = app
        .model
        .ui
        .keymap
        .display_for(Command::Copy, &context)
        .unwrap();
    let first = Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL);
    let second = Keystroke::new(KeyCode::Char('c'), Modifiers::CTRL);
    assert_eq!(
        hint,
        format!("{} {}", first.display_string(), second.display_string())
    );
    assert_eq!(
        app.resolve_keymap_action([first], false),
        KeyAction::AwaitMore
    );
    assert_eq!(
        app.resolve_keymap_action([second], false),
        KeyAction::Execute(Command::Copy)
    );
    assert_eq!(
        app.model.ui.keymap.lookup_with_context(
            &Keystroke::new(KeyCode::Char('c'), Modifiers::cmd()),
            Some(&context)
        ),
        None
    );
}

#[test]
fn shortcut_resolution_retains_global_commands_and_focus_gates() {
    use token::keymap::{KeyCode, Keybinding, Keystroke, Modifiers};
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let save = Keystroke::new(KeyCode::Char('s'), Modifiers::CTRL);
    let copy = Keystroke::new(KeyCode::Char('c'), Modifiers::CTRL);
    let prefix = Keystroke::new(KeyCode::Char('k'), Modifiers::CTRL);
    let editor_prefix = Keystroke::new(KeyCode::Char('b'), Modifiers::CTRL);
    let settings = Keystroke::new(KeyCode::Char(','), Modifiers::CTRL);
    app.model.ui.keymap = Keymap::with_bindings(vec![
        Keybinding::new(save, Command::SaveFile),
        Keybinding::new(copy, Command::Copy),
        Keybinding::new(settings, Command::OpenSettings),
        Keybinding::chord(vec![prefix, save], Command::SaveFile),
        Keybinding::chord(vec![prefix, copy], Command::Copy),
        Keybinding::chord(vec![editor_prefix, copy], Command::Copy),
    ]);
    assert_eq!(
        app.resolve_keymap_action([prefix], false),
        KeyAction::AwaitMore
    );
    assert_eq!(
        app.resolve_keymap_action([save], false),
        KeyAction::Execute(Command::SaveFile)
    );
    app.model
        .ui
        .open_modal(token::model::ModalState::CommandPalette(Default::default()));
    assert_eq!(
        app.resolve_keymap_action([save], false),
        KeyAction::Execute(Command::SaveFile)
    );
    assert_eq!(app.resolve_keymap_action([copy], false), KeyAction::NoMatch);
    assert_eq!(
        app.resolve_keymap_action([prefix], false),
        KeyAction::AwaitMore
    );
    assert_eq!(
        app.resolve_keymap_action([save], false),
        KeyAction::Execute(Command::SaveFile)
    );
    assert_eq!(
        app.resolve_keymap_action([prefix], false),
        KeyAction::AwaitMore
    );
    assert_eq!(app.resolve_keymap_action([copy], false), KeyAction::NoMatch);
    assert!(!app.model.ui.keymap.has_pending_chord());
    assert_eq!(
        app.resolve_keymap_action([editor_prefix], false),
        KeyAction::NoMatch
    );
    assert!(!app.model.ui.keymap.has_pending_chord());
    app.model.ui.close_modal();
    app.model.ui.focus_dock(token::panel::DockPosition::Left);
    assert_eq!(
        app.resolve_keymap_action([save], false),
        KeyAction::Execute(Command::SaveFile)
    );
    assert_eq!(app.resolve_keymap_action([copy], false), KeyAction::NoMatch);

    // Settings is a global action, and global chords remain usable in docks.
    for dock in [
        token::panel::DockPosition::Left,
        token::panel::DockPosition::Bottom,
    ] {
        app.model
            .dock_layout
            .bottom
            .activate(token::panel::PanelId::TERMINAL);
        app.model.ui.focus_dock(dock);
        assert_eq!(
            app.resolve_keymap_action([settings], false),
            KeyAction::Execute(Command::OpenSettings)
        );
        assert_eq!(
            app.resolve_keymap_action([prefix], false),
            KeyAction::AwaitMore
        );
        assert_eq!(
            app.resolve_keymap_action([save], false),
            KeyAction::Execute(Command::SaveFile)
        );
        assert_eq!(
            app.resolve_keymap_action([editor_prefix], false),
            KeyAction::NoMatch
        );
        assert!(!app.model.ui.keymap.has_pending_chord());
    }

    // An unavailable conditional binding must not shadow a global command
    // on the same key, or terminate a longer eligible global chord.
    use token::keymap::Condition;
    app.model.ui.keymap = Keymap::with_bindings(vec![
        Keybinding::new(settings, Command::Copy).when_single(Condition::ModalActive),
        Keybinding::new(settings, Command::OpenSettings),
        Keybinding::chord(vec![prefix, copy], Command::Copy),
        Keybinding::chord(vec![prefix, copy, save], Command::SaveFile),
    ]);
    app.model
        .ui
        .open_modal(token::model::ModalState::CommandPalette(Default::default()));
    assert_eq!(
        app.resolve_keymap_action([settings], false),
        KeyAction::Execute(Command::OpenSettings)
    );
    assert_eq!(
        app.resolve_keymap_action([prefix], false),
        KeyAction::AwaitMore
    );
    assert_eq!(
        app.resolve_keymap_action([copy], false),
        KeyAction::AwaitMore
    );
    assert_eq!(
        app.resolve_keymap_action([save], false),
        KeyAction::Execute(Command::SaveFile)
    );
}

fn focus_outline_with_symbols(app: &mut App) {
    app.model
        .dock_layout
        .right
        .activate(token::panel::PanelId::OUTLINE);
    app.model.ui.focus_dock(token::panel::DockPosition::Right);
    let revision = app.model.document().revision;
    app.model.document_mut().outline = Some(OutlineData {
        revision,
        roots: [1, 2]
            .into_iter()
            .map(|line| OutlineNode {
                kind: OutlineKind::Function,
                name: format!("symbol_{line}"),
                range: OutlineRange {
                    start_line: line,
                    start_col: 0,
                    end_line: line,
                    end_col: 1,
                },
                children: Vec::new(),
            })
            .collect(),
    });
}

fn dispatch_legacy_key(app: &mut App, key: Key) -> Option<Cmd> {
    handle_key(
        &mut app.model,
        key,
        PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified),
        KeyModifiers::default(),
        false,
    )
}

#[test]
fn focused_outline_bypasses_editor_keymap_and_captures_navigation() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .document_mut()
        .buffer
        .insert(0, "zero\none\ntwo\n");
    focus_outline_with_symbols(&mut app);
    let editor_cursor = app.model.editor().primary_cursor();
    let editor_position = (editor_cursor.line, editor_cursor.column);
    let document_text = app.model.document().buffer.to_string();

    assert!(should_skip_non_global_keymap(&app.model, false, false));

    dispatch_legacy_key(&mut app, Key::Named(NamedKey::ArrowDown));
    assert_eq!(app.model.outline_panel.selected_index, Some(0));
    dispatch_legacy_key(&mut app, Key::Named(NamedKey::ArrowDown));
    assert_eq!(app.model.outline_panel.selected_index, Some(1));
    let editor_cursor = app.model.editor().primary_cursor();
    assert_eq!((editor_cursor.line, editor_cursor.column), editor_position);

    dispatch_legacy_key(&mut app, Key::Character("x".into()));
    assert_eq!(app.model.document().buffer.to_string(), document_text);

    dispatch_legacy_key(&mut app, Key::Named(NamedKey::Enter));
    assert_eq!(app.model.editor().primary_cursor().line, 2);
    assert!(matches!(
        app.model.ui.focus,
        token::model::FocusTarget::Editor
    ));

    app.model.ui.focus_dock(token::panel::DockPosition::Right);
    dispatch_legacy_key(&mut app, Key::Named(NamedKey::Escape));
    assert!(matches!(
        app.model.ui.focus,
        token::model::FocusTarget::Editor
    ));
}

#[test]
fn multiple_startup_files_open_as_distinct_tabs() {
    let directory = tempfile::tempdir().expect("temporary directory should be created");
    let first = directory.path().join("first.rs");
    let second = directory.path().join("second.py");
    std::fs::write(&first, "fn main() {}\n").expect("first fixture should be written");
    std::fs::write(&second, "print('hello')\n").expect("second fixture should be written");
    let config = StartupConfig {
        mode: StartupMode::MultipleFiles(vec![first.clone(), second.clone()]),
        initial_position: None,
        wait_mode: false,
    };
    let preparation = AppPreparation::start(800, 600, config.clone())
        .expect("application preparation thread should start");

    let app = App::new(800, 600, config, None, None, Some(preparation));
    let open_paths: std::collections::HashSet<_> = app
        .model
        .editor_area
        .documents
        .values()
        .filter_map(|document| document.file_path.as_ref())
        .collect();
    let tab_count: usize = app
        .model
        .editor_area
        .groups
        .values()
        .map(|group| group.tabs.len())
        .sum();

    assert_eq!(tab_count, 2);
    assert!(open_paths.contains(&first));
    assert!(open_paths.contains(&second));
    assert_eq!(app.model.document().file_path.as_ref(), Some(&first));
}

#[test]
fn startup_position_is_clamped_to_the_first_successful_document() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first.txt");
    let second = dir.path().join("second.txt");
    std::fs::write(&first, "first\ncafé").unwrap();
    std::fs::write(&second, "second\nlonger line").unwrap();
    let prepared = prepare_app(
        800,
        600,
        StartupConfig {
            mode: StartupMode::MultipleFiles(vec![dir.path().into(), first.clone(), second]),
            initial_position: Some((usize::MAX, usize::MAX)),
            wait_mode: false,
        },
    );
    assert_eq!(prepared.model.document().file_path.as_ref(), Some(&first));
    assert_eq!(prepared.model.editor().active_cursor().line, 1);
    assert_eq!(prepared.model.editor().active_cursor().column, 4);
    assert_eq!(
        prepared.model.editor().selections[0],
        token::model::Selection::new(token::model::Position::new(1, 4))
    );
}

#[test]
fn startup_workspace_files_record_the_workspace_and_keep_an_empty_workspace_usable() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let path = dir.path().join("main.txt");
    std::fs::write(&path, "ready").unwrap();
    for files in [vec![path.clone()], vec![]] {
        let prepared = prepare_app(
            800,
            600,
            StartupConfig {
                mode: StartupMode::Workspace {
                    root: dir.path().into(),
                    initial_files: files.clone(),
                },
                initial_position: None,
                wait_mode: false,
            },
        );
        assert_eq!(
            prepared.model.workspace_root().map(|path| path.as_path()),
            Some(root.as_path())
        );
        assert_eq!(prepared.model.editor_area.documents.len(), 1);
        if files.is_empty() {
            assert!(prepared.model.document().file_path.is_none());
        } else {
            let entry = prepared
                .model
                .recent_files
                .entries
                .iter()
                .find(|entry| entry.path == path.canonicalize().unwrap())
                .unwrap();
            assert_eq!(entry.workspace.as_deref(), Some(root.as_path()));
        }
    }
}

#[test]
fn spawn_terminal_command_adds_session_to_model() {
    use std::time::{Duration, Instant};

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .dock_layout
        .bottom
        .activate(token::panel::PanelId::TERMINAL);

    app.process_cmd(Cmd::SpawnTerminal {
        session_id: 42,
        rows: 12,
        cols: 34,
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let session = loop {
        app.process_terminal_spawn_results();
        if let Some(session) = app.model.terminal.sessions.iter().find(|s| s.id == 42) {
            break session;
        }
        if Instant::now() >= deadline {
            panic!("spawned terminal session should be stored in the model");
        }
        std::thread::sleep(Duration::from_millis(10));
    };

    let content = token::layout::chrome::chrome(&app.model)
        .rect(token::layout::UiKey::PanelContent(
            token::panel::PanelId::Terminal,
        ))
        .unwrap();
    let size = token::panels::terminal::grid_size_for_rect(
        content,
        app.model.char_width,
        app.model.line_height,
    );
    assert_eq!(
        session.size,
        (usize::from(size.rows), usize::from(size.cols))
    );
    assert_eq!(app.model.terminal.active, 0);

    session.pty.write(b"exit\n".to_vec());
}

#[test]
fn terminal_spawn_result_is_discarded_when_request_is_cancelled() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let (spawn_tx, spawn_rx) = mpsc::channel();
    let (pty, _pty_rx) = token::terminal::PtyHandle::new_for_test();

    spawn_tx
        .send(Ok(token::terminal::TerminalSpawnResult {
            session_id: 99,
            rows: 24,
            cols: 80,
            pty,
        }))
        .expect("test spawn result should send");
    app.terminal_spawn_rx = Some((99, spawn_rx));
    app.model.terminal.mark_spawn_pending(99);
    app.model.terminal.clear_spawn_pending(99);

    let needs_redraw = app.process_terminal_spawn_results();

    assert!(!needs_redraw);
    assert!(app.model.terminal.sessions.is_empty());
    assert!(!app.model.terminal.is_spawn_pending(99));
}

#[test]
fn pending_terminal_spawn_is_kept_after_the_panel_moves_docks() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .dock_layout
        .bottom
        .panel_ids
        .retain(|&panel| panel != token::panel::PanelId::TERMINAL);
    app.model.dock_layout.bottom.active_index = Some(0);
    app.model
        .dock_layout
        .right
        .register_panel(token::panel::PanelId::TERMINAL);
    app.model
        .dock_layout
        .right
        .activate(token::panel::PanelId::TERMINAL);
    app.model.terminal.mark_spawn_pending(99);

    assert!(app.should_keep_terminal_spawn_result(99));
    app.model.dock_layout.right.close();
    assert!(
        app.should_keep_terminal_spawn_result(99),
        "hiding a panel must not cancel a requested terminal"
    );
}

#[test]
fn closing_terminal_tab_preserves_other_session_and_does_not_reuse_identity() {
    use token::terminal::{PtyHandle, TabAction, TerminalSession};
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .dock_layout
        .bottom
        .activate(token::panel::PanelId::Terminal);
    for id in 0..2 {
        app.model.terminal.mark_spawn_pending(id);
        app.model.terminal.clear_spawn_pending(id);
        let (pty, _) = PtyHandle::new_for_test();
        let mut session = TerminalSession::new(id, 4, 20, pty, app.msg_tx.clone());
        session.apply_bytes(b"one\r\ntwo\r\nthree\r\nfour\r\nfive\r\n");
        app.model.terminal.sessions.push(session);
    }
    app.model.terminal.sessions[0].scroll_offset = 1;
    app.model.terminal.active = 1;
    app.process_cmd(Cmd::CloseTerminal { session_id: 1 });
    assert_eq!(app.model.terminal.sessions.len(), 1);
    assert_eq!(app.model.terminal.active_session().unwrap().id, 0);
    // The pane resize may consume available history, but never resets the
    // surviving process identity or replaces its grid with a fresh session.
    assert!(!app.model.terminal.active_session().unwrap().exited);
    app.process_cmd(Cmd::CloseTerminal { session_id: 0 });
    assert!(app.model.terminal.sessions.is_empty());
    assert!(!app.model.dock_layout.bottom.is_open);
    assert!(matches!(
        update(
            &mut app.model,
            Msg::Terminal(token::messages::TerminalMsg::Tab(TabAction::New))
        ),
        Some(Cmd::SpawnTerminal { session_id: 2, .. })
    ));
}

#[test]
fn ignored_terminal_spawn_command_clears_its_pending_marker() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let (_spawn_tx, spawn_rx) = mpsc::channel();
    app.terminal_spawn_rx = Some((6, spawn_rx));
    app.model.terminal.mark_spawn_pending(6);
    app.model.terminal.mark_spawn_pending(7);

    app.process_cmd(Cmd::SpawnTerminal {
        session_id: 7,
        rows: 24,
        cols: 80,
    });

    assert!(!app.model.terminal.is_spawn_pending(7));
    assert!(app.model.terminal.is_spawn_pending(6));
}

#[test]
fn duplicate_terminal_spawn_command_preserves_in_flight_pending_marker() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let (_spawn_tx, spawn_rx) = mpsc::channel();
    app.terminal_spawn_rx = Some((7, spawn_rx));
    app.model.terminal.mark_spawn_pending(7);

    app.process_cmd(Cmd::SpawnTerminal {
        session_id: 7,
        rows: 24,
        cols: 80,
    });

    assert!(app.model.terminal.is_spawn_pending(7));
}

/// Push a request through `automation_tx` -> `automation_rx` and drain
/// it with `process_automation_requests`, the same path a real socket
/// client (MCP tool, CLI) drives — exercises `AutomationRequest`
/// end-to-end instead of calling `update()` directly.
fn send_automation_request(app: &mut App, request: AutomationRequest) -> AutomationResponse {
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    app.automation_tx
        .send(AutomationEnvelope {
            request,
            response_tx,
        })
        .expect("automation channel should still be open");
    app.process_automation_requests();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.process_async_messages();
        app.poll_document_waiters();
        if let Ok(response) = response_rx.try_recv() {
            return response;
        }
        assert!(Instant::now() < deadline, "automation response timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Like `send_automation_request`, but for requests that may defer their
/// answer (`OpenPaths { wait: true }`): returns the receiver instead.
fn send_deferred_automation_request(
    app: &mut App,
    request: AutomationRequest,
) -> mpsc::Receiver<AutomationResponse> {
    let (response_tx, response_rx) = mpsc::sync_channel(1);
    app.automation_tx
        .send(AutomationEnvelope {
            request,
            response_tx,
        })
        .expect("automation channel should still be open");
    app.process_automation_requests();
    assert!(pump_until(app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    response_rx
}

fn open_path(path: &std::path::Path, line: Option<usize>, column: Option<usize>) -> OpenPath {
    OpenPath {
        path: path.to_path_buf(),
        line,
        column,
    }
}

fn focused_tab_id(app: &App) -> token::model::TabId {
    app.model
        .editor_area
        .focused_group()
        .and_then(|group| group.active_tab())
        .map(|tab| tab.id)
        .expect("a focused tab")
}

fn open_paths_fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let a = dir.path().join("a.rs");
    let b = dir.path().join("b.rs");
    std::fs::write(&a, "line one\nline two\nline three\n").unwrap();
    std::fs::write(&b, "other\n").unwrap();
    (dir, a, b)
}

#[test]
fn focus_gain_bumps_focused_at_and_state_reports_it() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.open_workspace(dir.path().to_path_buf());
    let before = app.focused_at;
    std::thread::sleep(std::time::Duration::from_millis(2));
    app.handle_event(&winit::event::WindowEvent::Focused(true));
    assert!(
        app.focused_at > before,
        "gaining focus records a newer time"
    );
    let after_gain = app.focused_at;
    app.handle_event(&winit::event::WindowEvent::Focused(false));
    assert_eq!(app.focused_at, after_gain, "losing focus keeps the time");

    let response = send_automation_request(&mut app, AutomationRequest::State);
    let state = response.state.expect("state");
    assert_eq!(state.instance_id, std::process::id());
    assert_eq!(
        state.workspace_root.as_deref(),
        app.model.workspace_root().map(|p| p.as_path())
    );
    let expected_ms = after_gain
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert_eq!(state.focused_at_ms, expected_ms);
}

#[test]
fn soft_wrap_automation_reports_visual_rows_and_preserves_document() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let text = "a long line that wraps into several visual rows ".repeat(30);
    app.model.document_mut().buffer = ropey::Rope::from_str(&text);
    let response = send_automation_request(
        &mut app,
        AutomationRequest::ExecuteAction {
            name: "ToggleSoftWrap".into(),
        },
    );
    assert!(response.ok, "{response:?}");
    let state = send_automation_request(&mut app, AutomationRequest::State)
        .state
        .unwrap();
    assert!(state.soft_wrap);
    assert!(state.visual_row_count > state.line_count);
    assert_eq!(state.viewport_left_column, 0);
    assert_eq!(app.model.document().buffer.to_string(), text);
    let response = send_automation_request(
        &mut app,
        AutomationRequest::ExecuteAction {
            name: "MoveCursorDown".into(),
        },
    );
    assert!(response.ok);
    assert_eq!(app.model.editor().cursors[0].line, 0);
    assert!(app.model.editor().cursors[0].column > 0);
}

/// A fake llama-server answering every `/infill` with `content`.
fn fake_infill_server(content: &'static str) -> String {
    fake_inline_server("/infill", serde_json::json!({ "content": content }))
}

fn fake_inline_server(path: &'static str, reply: serde_json::Value) -> String {
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let reply = reply.to_string();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let mut stream = stream;
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            let mut first_line = true;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).is_err() || line.is_empty() {
                    return;
                }
                if first_line {
                    assert!(line.starts_with(&format!("POST {path} HTTP/1.1")), "{line}");
                    first_line = false;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap_or(0);
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            let _ = reader.read_exact(&mut body);
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{reply}",
                    reply.len()
                )
                .as_bytes(),
            );
        }
    });
    url
}

/// Phase 2 acceptance gate against a fake backend: typing schedules a
/// request, the deadline fires it through the worker thread, the reply
/// becomes ghost text, typing through it consumes it, Tab accepts the
/// rest as one undo step, and the automation snapshot reports it all.
#[test]
fn recency_inline_suggestion_round_trips_through_the_worker_and_accepts() {
    let url = fake_infill_server("1 + 2;\n    let y = x;");
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline = token::config::InlineConfig {
        enabled: true,
        statistics: false,
        provider: "local".into(),
        debounce_ms: 0,
        max_line_suffix: 8,
    };
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            timeout_ms: 2000,
            ..token::config::ProviderConfig::default()
        },
    );
    app.model.document_mut().buffer = ropey::Rope::from_str("fn main() {\n    let x = \n}\n");
    assert!(app.model.document().id.is_some(), "documents carry ids");
    app.process_automation_msg(Msg::Editor(EditorMsg::SetCursorPosition {
        line: 1,
        column: 12,
    }));

    app.model
        .config
        .completion
        .providers
        .get_mut("local")
        .unwrap()
        .context = token::completion::recency::ContextStrategy::RecencyRing {
        max_chunks: 8,
        chunk_lines: 64,
    };
    let now = Instant::now();
    app.inline_context.observe(&app.model, now);
    let idle = app.inline_context.deadline().unwrap();
    assert!(
        app.next_wake(now) <= idle,
        "idle work participates in the existing wake scheduler"
    );
    app.inline_context.observe(&app.model, idle);

    // Typing arms the debounce; the (zero) deadline sends the request.
    app.process_automation_msg(Msg::Document(DocumentMsg::InsertChar(' ')));
    assert!(app.inline_deadline.is_some(), "debounce armed");
    app.check_inline_deadlines();
    assert!(app.inline_deadline.is_none());
    assert!(app.model.ui.inline_in_flight);
    {
        let latest = app.inline_worker.latest();
        let request = &latest.as_ref().unwrap().request;
        assert_eq!(request.extra_context.len(), 1);
        assert!(request.extra_context[0].text.contains("fn main()"));
        assert!(
            !request.prefix.contains("Path:"),
            "context never rewrites the cursor prefix"
        );
    }
    assert!(!app
        .model
        .ui
        .status_bar
        .get_segment(token::model::status_bar::SegmentId::InlineSuggestion)
        .unwrap()
        .content
        .is_empty());
    assert!(
        send_automation_request(&mut app, AutomationRequest::State)
            .state
            .unwrap()
            .inline_in_flight
    );

    assert!(
        pump_until(&mut app, Duration::from_secs(5), |app| {
            token::update::inline::visible(&app.model).is_some()
        }),
        "the worker's reply never became ghost text"
    );
    let response = send_automation_request(&mut app, AutomationRequest::State);
    assert!(!response.state.as_ref().unwrap().inline_in_flight);
    assert!(app
        .model
        .ui
        .status_bar
        .get_segment(token::model::status_bar::SegmentId::InlineSuggestion)
        .unwrap()
        .content
        .is_empty());
    assert_eq!(
        response.state.unwrap().inline_suggestion.as_deref(),
        Some("1 + 2;\n    let y = x;")
    );

    // Type through the first char, then accept the rest.
    app.process_automation_msg(Msg::Document(DocumentMsg::InsertChar('1')));
    assert_eq!(
        token::update::inline::visible(&app.model)
            .unwrap()
            .remaining(),
        " + 2;\n    let y = x;"
    );
    app.process_automation_msg(Msg::Completion(CompletionMsg::AcceptInline(
        token::completion::inline::AcceptGranularity::Full,
    )));
    assert_eq!(
        app.model.document().buffer.to_string(),
        "fn main() {\n    let x =  1 + 2;\n    let y = x;\n}\n"
    );
    assert!(token::update::inline::visible(&app.model).is_none());
    app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
    assert_eq!(
        app.model.document().buffer.to_string(),
        "fn main() {\n    let x =  1\n}\n",
        "accept is one undo step"
    );
}

/// Retrieval preparation feeds the same provider worker as recency context.
#[test]
fn workspace_retrieval_reaches_the_inline_provider_through_background_preparation() {
    let directory = tempfile::tempdir().unwrap();
    std::fs::write(
        directory.path().join("helpers.rs"),
        "fn parse_widget() -> i32 { 42 }\n",
    )
    .unwrap();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .open_workspace(directory.path().canonicalize().unwrap());
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.inline.provider = "local".into();
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url: fake_infill_server("42"),
            context: token::completion::recency::ContextStrategy::WorkspaceRetrieval {
                max_chunks: 8,
                chunk_lines: 64,
            },
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = "// parse_widget\n".into();
    app.process_automation_msg(Msg::Editor(EditorMsg::SetCursorPosition {
        line: 1,
        column: 0,
    }));
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(app.inline_retrieval.is_some());
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        token::update::inline::visible(&app.model).is_some()
    }));
    let latest = app.inline_worker.latest();
    let request = &latest.as_ref().unwrap().request;
    assert_eq!(request.extra_context.len(), 1);
    assert_eq!(request.extra_context[0].filename, "helpers.rs");
    assert_eq!(request.prefix, "// parse_widget\n");
}

/// Real worker arrival followed by partial/full acceptance through named actions.
#[test]
fn inline_partial_accept_is_available_through_automation() {
    let url = fake_infill_server("héllo_world\nnext();");
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            timeout_ms: 2000,
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = ropey::Rope::from_str("\n");
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        token::update::inline::visible(&app.model).is_some()
    }));
    for (action, text, remaining) in [
        ("AcceptInlineWord", "héllo\n", Some("_world\nnext();")),
        ("AcceptInlineLine", "héllo_world\n\n", Some("next();")),
        ("AcceptInlineSuggestion", "héllo_world\nnext();\n", None),
    ] {
        let response = send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: action.into(),
            },
        );
        assert!(response.ok, "{response:?}");
        assert_eq!(app.model.document().buffer.to_string(), text);
        let snapshot = send_automation_request(&mut app, AutomationRequest::State)
            .state
            .unwrap();
        assert_eq!(snapshot.inline_suggestion.as_deref(), remaining);
        if remaining.is_some() {
            assert!(
                app.inline_deadline.is_none(),
                "partial acceptance keeps the existing suggestion"
            );
        }
    }
}

#[test]
fn ollama_native_and_raw_fim_round_trips_and_accepts() {
    for prompt_format in [
        token::completion::prompt::PromptFormat::Native,
        token::completion::prompt::PromptFormat::DeepSeek,
    ] {
        let url = fake_inline_server(
            "/api/generate",
            serde_json::json!({ "response": "hello_world();<｜fim▁end｜>leaked", "done": true }),
        );
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model.config.completion.inline.enabled = true;
        app.model.config.completion.inline.statistics = false;
        app.model.config.completion.providers.insert(
            "local".into(),
            token::config::ProviderConfig {
                url,
                transport: token::config::TransportKind::Ollama,
                prompt_format,
                model: Some("code-model".into()),
                ..Default::default()
            },
        );
        app.model.document_mut().buffer = "\n".into();
        app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
            explicit: true,
        }));
        app.check_inline_deadlines();
        assert!(pump_until(&mut app, Duration::from_secs(3), |app| {
            token::update::inline::visible(&app.model).is_some()
        }));
        app.process_automation_msg(Msg::Completion(CompletionMsg::AcceptInline(
            token::completion::inline::AcceptGranularity::Full,
        )));
        assert_eq!(app.model.document().buffer.to_string(), "hello_world();\n");
        app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
        assert_eq!(app.model.document().buffer.to_string(), "\n");
    }
}

#[test]
fn inline_dismissal_and_window_focus_loss_clear_runtime_debounces() {
    for window_focus_loss in [false, true] {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model.config.completion.inline.enabled = true;
        app.model.config.completion.inline.statistics = false;
        app.model
            .config
            .completion
            .providers
            .insert("local".into(), Default::default());
        app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
            explicit: true,
        }));
        assert!(app.inline_deadline.is_some());
        if window_focus_loss {
            if let Some(cmd) = app.handle_event(&WindowEvent::Focused(false)) {
                app.process_cmd(cmd);
            }
        } else {
            app.process_automation_msg(Msg::Completion(CompletionMsg::DismissInline));
        }
        assert!(app.inline_deadline.is_none());
        assert!(app.model.ui.inline_session.is_none());
        app.check_inline_deadlines();
        assert!(!app.model.ui.inline_in_flight);
    }
}

#[test]
fn openai_compatible_inline_worker_supports_partial_accept_and_undo() {
    inline_transport_partial_accept_and_undo(token::config::TransportKind::OpenAiCompat);
}

#[test]
fn tabby_inline_worker_supports_partial_accept_and_undo() {
    inline_transport_partial_accept_and_undo(token::config::TransportKind::Tabby);
}

#[test]
fn tabby_empty_result_does_not_count_as_backend_failure() {
    let url = fake_inline_server(
        "/v1/completions",
        serde_json::json!({ "id": "empty", "choices": [] }),
    );
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            transport: token::config::TransportKind::Tabby,
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = "\n".into();
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(app.model.ui.inline_in_flight);
    assert!(pump_until(&mut app, Duration::from_secs(3), |app| !app
        .model
        .ui
        .inline_in_flight));
    assert_eq!(app.model.ui.inline_failures, 0);
    assert!(token::update::inline::visible(&app.model).is_none());
    assert!(app.model.ui.inline_session.is_none());
    assert_eq!(app.model.document().buffer.to_string(), "\n");
}

fn inline_transport_partial_accept_and_undo(transport: token::config::TransportKind) {
    let url = fake_inline_server(
        "/v1/completions",
        serde_json::json!({ "choices": [{ "text": "héllo_world();<|fim_suffix|>discarded" }] }),
    );
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            transport,
            model: (transport == token::config::TransportKind::OpenAiCompat)
                .then(|| "fixture-fim-model".into()),
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = "\n".into();
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(pump_until(&mut app, Duration::from_secs(3), |app| {
        token::update::inline::visible(&app.model).is_some()
    }));
    for (action, expected, remainder) in [
        ("AcceptInlineWord", "héllo\n", Some("_world();")),
        ("AcceptInlineSuggestion", "héllo_world();\n", None),
    ] {
        let response = send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: action.into(),
            },
        );
        assert!(response.ok);
        assert_eq!(app.model.document().buffer.to_string(), expected);
        let state = send_automation_request(&mut app, AutomationRequest::State)
            .state
            .unwrap();
        assert_eq!(state.inline_suggestion.as_deref(), remainder);
    }
    for expected in ["héllo\n", "\n"] {
        app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
        assert_eq!(app.model.document().buffer.to_string(), expected);
    }
}

/// Run the environment lookup in an isolated test process. The parent never
/// mutates its global environment or reads the user's API credentials.
#[test]
fn mistral_inline_worker_resolves_environment_reference_and_accepts() {
    const CHILD: &str = "TOKEN_TEST_MISTRAL_WORKER_CHILD";
    const KEY: &str = "TOKEN_TEST_MISTRAL_WORKER_KEY";
    if std::env::var(CHILD).as_deref() != Ok("1") {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "runtime::app::tests::mistral_inline_worker_resolves_environment_reference_and_accepts", "--nocapture"])
            .env(CHILD, "1")
            .env(KEY, "synthetic-fixture-only")
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("1 passed"),
            "child test must actually run"
        );
        return;
    }
    let url = fake_inline_server(
        "/v1/fim/completions",
        serde_json::json!({ "choices": [{ "message": { "content": "mistral_fixture();" } }] }),
    );
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            transport: token::config::TransportKind::MistralFim,
            model: Some("fixture-fim-model".into()),
            api_key_env: Some(KEY.into()),
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = "\n".into();
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(pump_until(&mut app, Duration::from_secs(3), |app| {
        token::update::inline::visible(&app.model).is_some()
    }));
    assert!(
        send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: "AcceptInlineSuggestion".into()
            }
        )
        .ok
    );
    assert_eq!(
        app.model.document().buffer.to_string(),
        "mistral_fixture();\n"
    );
    let config = serde_yaml::to_string(&app.model.config).unwrap();
    assert!(config.contains(KEY));
    assert!(!config.contains("synthetic-fixture-only"));
    app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
    assert_eq!(app.model.document().buffer.to_string(), "\n");
}

#[test]
fn inline_alternatives_cycle_through_automation_without_editing_or_requesting() {
    let url = fake_inline_server(
        "/v1/completions",
        serde_json::json!({ "choices": [
        {"text": "<|fim_middle|>discarded"}, {"text": "héllo_one();"},
        {"text": "héllo_one();"}, {"text": "héllo_two();"}, {"text": "different();"}
    ] }),
    );
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            transport: token::config::TransportKind::OpenAiCompat,
            model: Some("fixture".into()),
            n: 5,
            ..Default::default()
        },
    );
    app.model.document_mut().buffer = "\n".into();
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(pump_until(&mut app, Duration::from_secs(3), |app| {
        token::update::inline::visible(&app.model).is_some()
    }));
    let initial = send_automation_request(&mut app, AutomationRequest::State)
        .state
        .unwrap();
    assert_eq!(initial.inline_choice, Some((1, 3)));
    assert_eq!(initial.inline_suggestion.as_deref(), Some("héllo_one();"));
    let revision = app.model.document().revision;
    let undo_len = app.model.document().undo_stack.len();
    let request_id = app.model.ui.inline_next_request_id;
    for (action, expected, choice) in [
        ("PrevInlineSuggestion", "different();", (3, 3)),
        ("NextInlineSuggestion", "héllo_one();", (1, 3)),
        ("NextInlineSuggestion", "héllo_two();", (2, 3)),
    ] {
        assert!(
            send_automation_request(
                &mut app,
                AutomationRequest::ExecuteAction {
                    name: action.into()
                }
            )
            .ok
        );
        let state = send_automation_request(&mut app, AutomationRequest::State)
            .state
            .unwrap();
        assert_eq!(state.inline_suggestion.as_deref(), Some(expected));
        assert_eq!(state.inline_choice, Some(choice));
        assert_eq!(app.model.document().buffer.to_string(), "\n");
        assert_eq!(app.model.document().revision, revision);
        assert_eq!(app.model.document().undo_stack.len(), undo_len);
        assert_eq!(app.model.ui.inline_next_request_id, request_id);
        assert!(!state.inline_in_flight && app.inline_deadline.is_none());
    }
    assert!(
        send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: "AcceptInlineWord".into()
            }
        )
        .ok
    );
    assert!(
        send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: "NextInlineSuggestion".into()
            }
        )
        .ok
    );
    let state = send_automation_request(&mut app, AutomationRequest::State)
        .state
        .unwrap();
    assert_eq!(state.inline_suggestion.as_deref(), Some("_one();"));
    assert_eq!(state.inline_choice, Some((1, 2)));
    assert!(
        send_automation_request(
            &mut app,
            AutomationRequest::ExecuteAction {
                name: "AcceptInlineSuggestion".into()
            }
        )
        .ok
    );
    assert_eq!(app.model.document().buffer.to_string(), "héllo_one();\n");
    for expected in ["héllo\n", "\n"] {
        app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
        assert_eq!(app.model.document().buffer.to_string(), expected);
    }
}

/// A dead backend is a status transient, never a modal.
#[test]
fn inline_suggestion_backend_failure_is_a_transient() {
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.completion.inline.enabled = true;
    app.model.config.completion.inline.statistics = false;
    app.model.config.completion.providers.insert(
        "local".into(),
        token::config::ProviderConfig {
            url,
            timeout_ms: 300,
            ..token::config::ProviderConfig::default()
        },
    );
    app.process_automation_msg(Msg::Completion(CompletionMsg::TriggerInline {
        explicit: true,
    }));
    app.check_inline_deadlines();
    assert!(app.model.ui.inline_in_flight);
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        !app.model.ui.inline_in_flight
    }));
    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|m| m.text.contains("Inline suggestion failed")));
    assert!(!app.model.ui.has_modal());
}

#[test]
fn open_paths_opens_tab_and_responds_immediately() {
    let (_dir, a, _) = open_paths_fixture();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let tabs_before = app.model.editor_area.focused_group().unwrap().tabs.len();

    let response = send_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(&a, None, None)],
            wait: false,
        },
    );

    assert!(response.ok, "{}", response.message);
    assert_eq!(
        app.model.editor_area.focused_group().unwrap().tabs.len(),
        tabs_before + 1
    );
    assert!(app.model.editor_area.find_open_file(&a).is_some());
}

#[test]
fn open_paths_places_cursor_at_one_indexed_position() {
    let (_dir, a, _) = open_paths_fixture();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);

    send_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(&a, Some(3), Some(2))],
            wait: false,
        },
    );

    let cursor = &app.model.editor().cursors[0];
    assert_eq!((cursor.line, cursor.column), (2, 1));
}

#[test]
fn open_paths_wait_defers_until_the_tab_closes() {
    let (_dir, a, _) = open_paths_fixture();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);

    let response_rx = send_deferred_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(&a, None, None)],
            wait: true,
        },
    );
    app.poll_document_waiters();
    assert!(
        response_rx.try_recv().is_err(),
        "must not answer while the tab is open"
    );

    let tab_id = focused_tab_id(&app);
    app.process_automation_msg(Msg::Layout(LayoutMsg::CloseTab(tab_id)));
    app.poll_document_waiters();

    let response = response_rx
        .try_recv()
        .expect("closing the tab answers the wait");
    assert!(response.ok);
    assert!(app.document_waiters.is_empty());
}

#[test]
fn open_paths_wait_on_two_files_answers_after_both_close() {
    let (_dir, a, b) = open_paths_fixture();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);

    let response_rx = send_deferred_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(&a, None, None), open_path(&b, None, None)],
            wait: true,
        },
    );

    let (b_id, _, _) = app.model.editor_area.find_open_file(&b).unwrap();
    let b_tab = app
        .model
        .editor_area
        .focused_group()
        .unwrap()
        .tabs
        .iter()
        .find(|tab| {
            app.model
                .editor_area
                .editors
                .get(&tab.editor_id)
                .and_then(|e| e.document_id)
                == Some(b_id)
        })
        .map(|tab| tab.id)
        .unwrap();
    app.process_automation_msg(Msg::Layout(LayoutMsg::CloseTab(b_tab)));
    app.poll_document_waiters();
    assert!(response_rx.try_recv().is_err(), "a is still open");

    let (a_id, _, _) = app.model.editor_area.find_open_file(&a).unwrap();
    let a_tab = app
        .model
        .editor_area
        .focused_group()
        .unwrap()
        .tabs
        .iter()
        .find(|tab| {
            app.model
                .editor_area
                .editors
                .get(&tab.editor_id)
                .and_then(|e| e.document_id)
                == Some(a_id)
        })
        .map(|tab| tab.id)
        .unwrap();
    app.process_automation_msg(Msg::Layout(LayoutMsg::CloseTab(a_tab)));
    app.poll_document_waiters();
    assert!(
        response_rx.try_recv().is_ok(),
        "both closed answers the wait"
    );
}

#[test]
fn open_paths_wait_shared_across_groups_answers_on_last_editor() {
    let (_dir, a, _) = open_paths_fixture();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);

    let response_rx = send_deferred_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(&a, None, None)],
            wait: true,
        },
    );
    // Split: the new group shows the same document through a second editor.
    app.process_automation_msg(Msg::Layout(LayoutMsg::SplitFocused(
        token::model::SplitDirection::Vertical,
    )));
    let (doc_id, _, _) = app.model.editor_area.find_open_file(&a).unwrap();
    assert!(
        app.model.editor_area.editors_for_document(doc_id).len() >= 2,
        "split should share the document"
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::CloseFocusedGroup));
    app.poll_document_waiters();
    assert!(
        response_rx.try_recv().is_err(),
        "the other group still shows the file"
    );

    let tab_id = focused_tab_id(&app);
    app.process_automation_msg(Msg::Layout(LayoutMsg::CloseTab(tab_id)));
    app.poll_document_waiters();
    assert!(response_rx.try_recv().is_ok());
}

#[test]
fn open_paths_wait_without_files_answers_only_on_exit() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let response_rx = send_deferred_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![],
            wait: true,
        },
    );
    app.poll_document_waiters();
    assert!(response_rx.try_recv().is_err());

    app.answer_exit_waiters();
    let response = response_rx.try_recv().expect("exit answers every waiter");
    assert!(response.ok);
}

#[test]
fn open_paths_directory_is_not_awaited_on_documents() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let response_rx = send_deferred_automation_request(
        &mut app,
        AutomationRequest::OpenPaths {
            paths: vec![open_path(dir.path(), None, None)],
            wait: true,
        },
    );
    app.poll_document_waiters();
    assert!(response_rx.try_recv().is_err());
    assert!(app.document_waiters.iter().all(|waiter| waiter.exit_only));
}

#[test]
fn automation_flow_triggers_menu_and_reports_completion_snapshot() {
    // autocomplete.md Phase 1 Gate: type -> menu opens -> filter,
    // driven through the actual `AutomationRequest` socket path
    // (`EditorSnapshot.completion` exists for exactly this).
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model
        .editor_area
        .documents
        .values_mut()
        .next()
        .unwrap()
        .buffer
        .insert(0, "value_one\nval");

    let response = send_automation_request(
        &mut app,
        AutomationRequest::SetCursor { line: 1, column: 3 },
    );
    assert!(response.ok);
    let state = response.state.expect("state should be present");
    assert!(
        state.completion.is_none(),
        "cursor move alone shouldn't open the popup"
    );

    let response = send_automation_request(
        &mut app,
        AutomationRequest::ExecuteAction {
            name: "TriggerCompletionMenu".to_string(),
        },
    );
    assert!(response.ok, "{}", response.message);

    let response = send_automation_request(&mut app, AutomationRequest::State);
    let completion = response
        .state
        .expect("state should be present")
        .completion
        .expect("completion popup should be open after the trigger action");
    assert!(completion.items.contains(&"value_one".to_string()));
}

#[test]
fn automation_execute_action_show_context_menu_actually_opens_it() {
    // Regression: `Command::ShowContextMenu::to_msgs()` returns `vec![]`
    // (it needs a live clipboard read, resolved in `dispatch_command`,
    // not `update()`), so routing it through the generic `to_msgs()`
    // loop reported "action executed" while doing nothing — the
    // automation flow context-menu.md's Phase 5 asks for was
    // unreachable by command name.
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);

    let response = send_automation_request(
        &mut app,
        AutomationRequest::ExecuteAction {
            name: "ShowContextMenu".to_string(),
        },
    );
    assert!(response.ok, "{}", response.message);

    let response = send_automation_request(&mut app, AutomationRequest::State);
    let context_menu = response
        .state
        .expect("state should be present")
        .context_menu
        .expect("the editor context menu should be open after ShowContextMenu");
    assert_eq!(context_menu.region, "editor");
}

// ---- LspManager lifecycle bookkeeping ----

/// A cheap, always-available real child (`sh` sleeping) standing in for
/// a language server, so `handle_lsp_server_exited`/`restart_lsp_server`
/// can be exercised against a real `ServerHandle` without depending on
/// an actual LSP server binary being installed.
fn spawn_fake_handle(server_id: &LspServerId) -> ServerHandle {
    let (msg_tx, _msg_rx) = mpsc::channel();
    lsp::client::spawn_server(
        "sh",
        &["-c".to_owned(), "sleep 5".to_owned()],
        Path::new("/tmp"),
        server_id.clone(),
        msg_tx,
        None,
        serde_json::Value::Null,
        serde_json::Value::Null,
    )
    .expect("spawning `sh` for a fake handle should succeed")
}

#[test]
fn exited_message_with_stale_generation_is_ignored() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-a");
    let mut handle = spawn_fake_handle(&server_id);
    let real_generation = handle.generation;
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    // A generation that doesn't match the live handle (e.g. an EOF
    // from a process already killed and replaced) must not touch the
    // current handle or bump restart_attempts.
    app.handle_lsp_server_exited(&server_id, real_generation.wrapping_add(1));

    assert!(app
        .lsp
        .servers
        .contains_key(&(server_id.clone(), root.clone())));
    assert!(!app
        .lsp
        .restart_attempts
        .contains_key(&(server_id.clone(), root.clone())));

    // Clean up the still-live fake child.
    handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn exited_message_with_matching_generation_removes_handle_and_bumps_attempts() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-b");
    let handle = spawn_fake_handle(&server_id);
    let generation = handle.generation;
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.handle_lsp_server_exited(&server_id, generation);

    assert!(!app
        .lsp
        .servers
        .contains_key(&(server_id.clone(), root.clone())));
    assert_eq!(
        app.lsp.restart_attempts.get(&(server_id, root)).copied(),
        Some(1)
    );
}

/// The backoff-window duplicate-spawn race: a crash removes the dead
/// handle and arms a backoff deadline; a file-open during that window
/// (`ensure_lsp_server`, which only sees `is_running` false since the
/// handle isn't installed yet) spawns a replacement directly, then
/// `check_lsp_restart_deadlines` fires for the same `(server_id,
/// root)` and must not spawn a *second* process on top of it —
/// `servers.insert` would silently overwrite the first handle,
/// orphaning a live process with nothing left to `kill()`/`wait()` it.
#[test]
fn restart_deadline_does_not_duplicate_spawn_a_root_already_running() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-backoff-race");

    // Simulate the concurrent respawn that happened during the
    // backoff window (a file-open's `ensure_lsp_server` beat the
    // deadline sweep to it).
    let handle = spawn_fake_handle(&server_id);
    let generation = handle.generation;
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    // A backoff deadline for the same root, already due — as if
    // `handle_lsp_server_exited` armed it before the race above
    // resolved.
    app.lsp
        .restart_deadlines
        .insert((server_id.clone(), root.clone()), Instant::now());

    app.check_lsp_restart_deadlines();

    // The deadline fired (consumed), but the *funnel guard* in
    // `spawn_lsp_server_at` must have short-circuited before touching
    // `servers` — same generation means the original handle survived
    // untouched, not overwritten by a second spawn.
    assert!(app.lsp.restart_deadlines.is_empty());
    let surviving = app
        .lsp
        .servers
        .get(&(server_id.clone(), root.clone()))
        .expect("the concurrently spawned handle must still be present");
    assert_eq!(
        surviving.generation, generation,
        "a duplicate spawn must not replace the already-running handle"
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn restart_attempts_are_scoped_per_root_not_per_server_id() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root_a = PathBuf::from("/tmp/proj-c");
    let root_b = PathBuf::from("/tmp/proj-d");

    let handle_a = spawn_fake_handle(&server_id);
    let gen_a = handle_a.generation;
    app.lsp
        .servers
        .insert((server_id.clone(), root_a.clone()), handle_a);
    app.handle_lsp_server_exited(&server_id, gen_a);

    // root_b never crashed; its count must stay untouched by root_a's.
    assert_eq!(
        app.lsp
            .restart_attempts
            .get(&(server_id.clone(), root_a.clone()))
            .copied(),
        Some(1)
    );
    assert!(!app.lsp.restart_attempts.contains_key(&(server_id, root_b)));
}

#[test]
fn exceeding_max_restart_attempts_reports_failed_and_retains_the_root() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-e");

    // Drive it past the cap: each iteration simulates "it respawned,
    // then crashed again" by reinserting a fresh fake handle before
    // reporting the exit — the real respawn attempt inside
    // `handle_lsp_server_exited` targets the actual `rust-analyzer`
    // binary and is allowed to fail (Missing) without affecting the
    // counters under test.
    for _ in 0..=MAX_RESTART_ATTEMPTS {
        let handle = spawn_fake_handle(&server_id);
        let generation = handle.generation;
        app.lsp
            .servers
            .insert((server_id.clone(), root.clone()), handle);
        app.handle_lsp_server_exited(&server_id, generation);
    }

    assert_eq!(
        app.lsp
            .restart_attempts
            .get(&(server_id.clone(), root.clone()))
            .copied(),
        Some(MAX_RESTART_ATTEMPTS + 1)
    );
    assert_eq!(
        app.lsp.failed_roots.get(&server_id).map(Vec::as_slice),
        Some([root.clone()].as_slice())
    );
    assert!(!app.lsp.servers.contains_key(&(server_id, root)));
}

#[test]
fn manual_restart_falls_back_to_failed_roots_when_no_handle_is_running() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-f");
    app.lsp
        .failed_roots
        .insert(server_id.clone(), vec![root.clone()]);

    // No live handle for `server_id` anywhere — `roots_for` is empty,
    // so `restart_lsp_server` must fall back to `failed_roots` instead
    // of silently doing nothing.
    app.restart_lsp_server(&server_id);

    assert!(!app.lsp.failed_roots.contains_key(&server_id));
}

/// `restart_lsp_server` must clear pending definition/hover
/// bookkeeping for the roots it restarts, exactly like
/// `handle_lsp_server_exited` does — otherwise a stale entry can
/// collide with a request id the *new* process allocates (fresh
/// `PendingRequests` restart at 1) and its deadline sweep abandons a
/// live request that happens to reuse the old id.
#[test]
fn manual_restart_clears_stale_pending_requests_for_the_restarted_root() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-restart-pending");
    let doc_id = app.model.document().id.unwrap();

    let handle = spawn_fake_handle(&server_id);
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    let key = (server_id.clone(), root.clone(), 2i64);
    app.lsp.definition.insert(
        key.clone(),
        doc_id,
        PendingDefinition {
            document_id: doc_id,
            revision: 0,
            origin: test_origin(&app),
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );
    app.lsp.definition.arm_deadline(key);
    let hover_key = (server_id.clone(), root.clone(), 3i64);
    app.lsp.hover.insert(
        hover_key.clone(),
        doc_id,
        PendingHover {
            document_id: doc_id,
            revision: 0,
            cursor: token::model::editor::Position::new(0, 0),
        },
    );
    app.lsp.hover.arm_deadline(hover_key);

    app.restart_lsp_server(&server_id);

    assert!(app.lsp.definition.requests.is_empty());
    assert!(app.lsp.definition.by_doc.is_empty());
    assert!(app.lsp.definition.deadlines.is_empty());
    assert!(app.lsp.hover.requests.is_empty());
    assert!(app.lsp.hover.by_doc.is_empty());
    assert!(app.lsp.hover.deadlines.is_empty());
}

/// Clearing diagnostics on server exit/restart must damage the editor
/// area, not just the status bar — otherwise painted squiggles/gutter
/// marks survive on screen until an unrelated event happens to
/// repaint the editor (`Renderer::render` skips the editor entirely
/// for status-bar-only damage).
#[test]
fn clearing_diagnostics_for_a_root_damages_the_editor_area() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-clear-damage");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-clear-damage/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);
    app.model.document_mut().diagnostics = vec![lsp_types::Diagnostic {
        range: lsp_types::Range::default(),
        severity: Some(lsp_types::DiagnosticSeverity::ERROR),
        message: "boom".to_owned(),
        ..Default::default()
    }];
    app.pending_damage = Damage::None;

    app.clear_diagnostics_for_roots(&server_id, &[root]);

    assert!(app.model.document().diagnostics.is_empty());
    assert!(
        app.pending_damage.includes_editor(),
        "clearing diagnostics must merge editor damage, got {:?}",
        app.pending_damage
    );
}

/// `clear_diagnostics_for_roots` must sweep `model.lsp.diagnostics`
/// (the Problems panel's render mirror) in exact parity with the
/// runtime's own store — including entries for files that were never
/// opened, which the editor-side `doc.diagnostics` clear above never
/// touches.
#[test]
fn clearing_diagnostics_for_a_root_sweeps_the_model_mirror_including_unopened_files() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-clear-mirror");
    let unopened_uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-clear-mirror/unopened.rs"));
    let unopened_path = lsp::uri_to_path(&unopened_uri).unwrap();
    app.model.lsp.diagnostics.insert(
        unopened_path.clone(),
        vec![lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(lsp_types::DiagnosticSeverity::WARNING),
            message: "unopened boom".to_owned(),
            ..Default::default()
        }],
    );
    app.lsp
        .diagnostics
        .insert(unopened_uri, vec![lsp_types::Diagnostic::default()]);

    app.clear_diagnostics_for_roots(&server_id, &[root]);

    assert!(
        !app.model.lsp.diagnostics.contains_key(&unopened_path),
        "the mirror must drop unopened-file entries too, not just open documents"
    );
}

/// `Cmd::LspClearDiagnostics` (the language-change clearing path) must
/// sweep `model.lsp.diagnostics` under the same key `DiagnosticsPublished`
/// inserted it with — `uri_to_path(published_uri)`, not the document's
/// raw `file_path` — or a canonicalized publish (e.g. macOS's
/// `/tmp` -> `/private/tmp`) leaves the stale row behind.
#[test]
fn lsp_clear_diagnostics_sweeps_the_mirror_via_the_published_uri() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}").unwrap();

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    app.model.document_mut().file_path = Some(file_path.clone());

    // Mirror the exact population path: insert under the canonicalized
    // path a real publish would decode to, which may differ from the
    // raw `file_path` used to open the document.
    let published_uri = lsp::path_to_uri(&file_path);
    let mirror_path = lsp::uri_to_path(&published_uri).unwrap();
    app.model.lsp.diagnostics.insert(
        mirror_path.clone(),
        vec![lsp_types::Diagnostic {
            range: lsp_types::Range::default(),
            severity: Some(lsp_types::DiagnosticSeverity::ERROR),
            message: "boom".to_owned(),
            ..Default::default()
        }],
    );
    app.lsp
        .diagnostics
        .insert(published_uri, vec![lsp_types::Diagnostic::default()]);

    app.process_cmd(Cmd::LspClearDiagnostics {
        document_id: doc_id,
    });

    assert!(
        !app.model.lsp.diagnostics.contains_key(&mirror_path),
        "the mirror row must be removed even when the published path canonicalizes \
         differently from the document's raw file_path"
    );
}

/// Same clearing path, but with the Problems panel open: the panel has
/// no dedicated damage area, so a clear must merge `Damage::Full` or
/// the stale row stays painted (mirrors
/// `clearing_diagnostics_for_a_root_damages_the_editor_area`).
#[test]
fn lsp_clear_diagnostics_damages_the_problems_panel_when_open() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let file_path = PathBuf::from("/tmp/proj-clear-lang/main.rs");
    app.model.document_mut().file_path = Some(file_path.clone());
    app.model
        .dock_layout
        .bottom
        .activate(token::panel::PanelId::PROBLEMS);
    app.model.dock_layout.bottom.is_open = true;
    app.pending_damage = Damage::None;

    app.process_cmd(Cmd::LspClearDiagnostics {
        document_id: doc_id,
    });

    assert!(
        matches!(app.pending_damage, Damage::Full),
        "clearing diagnostics with the Problems panel open must request a full repaint, got {:?}",
        app.pending_damage
    );
}

/// Quitting while a server's handshake never completed (still
/// starting, or `initialize` answered with an error — both leave
/// `capabilities_snapshot()` `None` forever) must not pay the full
/// shutdown-ack + exit-wait budget: `shutdown`/`exit` would sit queued
/// behind a handshake gate that can never open. `graceful_lsp_teardown`
/// must recognize this and kill directly instead.
#[test]
fn quit_with_an_unhandshaked_server_does_not_pay_the_graceful_shutdown_timeout() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-quit-unhandshaked");

    // A handle whose capabilities were never set, mirroring both
    // "still starting" and "initialize answered with an error" (the
    // reader never opens the gate or sets capabilities in either
    // case).
    let handle = spawn_fake_handle(&server_id);
    assert!(handle.capabilities_snapshot().is_none());
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    let started = Instant::now();
    app.process_cmd(Cmd::Quit);
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(1),
        "quit must not block on shutdown/exit acks a stuck handshake can never send, took {elapsed:?}"
    );
    assert!(app.lsp.servers.is_empty());
}

#[test]
fn detached_roots_are_capped_and_further_roots_are_not_spawned() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let dir = tempfile::tempdir().expect("temp dir should be created");
    // No workspace is set, so every root below is "detached".
    assert!(app.model.workspace.is_none());

    let mut roots = Vec::new();
    for i in 0..MAX_DETACHED_ROOTS + 1 {
        let root_dir = dir.path().join(format!("proj{i}"));
        // The spawn sets this as the child's cwd (`Command::current_dir`)
        // — it must exist or the spawn fails and (after the missing-
        // server fix) never claims a detached-root slot at all, which
        // would collapse this test into the one below it.
        std::fs::create_dir_all(&root_dir).expect("root dir should be created");
        let file = root_dir.join("main.rs");
        app.ensure_lsp_server(token::syntax::LanguageId::Rust, &file);
        roots.push(root_dir);
    }

    assert_eq!(app.lsp.detached_roots.len(), MAX_DETACHED_ROOTS);
    // The last root (over the cap) was never admitted.
    assert!(!app.lsp.detached_roots.contains(roots.last().unwrap()));
    for root in &roots[..MAX_DETACHED_ROOTS] {
        assert!(app.lsp.detached_roots.contains(root));
    }
}

/// A server binary that isn't on `PATH` must be memoized as `Missing`
/// per `(server_id, root)` — repeated `ensure_lsp_server` calls for
/// the same root (every matching file-open funnels through it) must
/// neither re-flash the transient nor re-attempt the spawn, and must
/// never consume a detached-root slot at all (a failed spawn has no
/// server running there to justify spending one of the limited
/// slots).
#[test]
fn missing_server_is_memoized_and_not_retried_on_repeated_opens() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some("/definitely/not/a/real/binary-xyz".to_owned()),
            args: None,
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let root_dir = dir.path().join("proj");
    std::fs::create_dir_all(&root_dir).expect("root dir should be created");
    let file = root_dir.join("main.rs");
    let server_id = LspServerId::from("rust-analyzer");

    app.ensure_lsp_server(token::syntax::LanguageId::Rust, &file);
    assert_eq!(
        app.model.lsp.servers.get(&server_id),
        Some(&ServerState::Missing)
    );
    assert!(app
        .lsp
        .missing_servers
        .contains(&(server_id.clone(), root_dir.clone())));
    assert!(
        app.lsp.detached_roots.is_empty(),
        "a failed spawn must not consume a detached-root slot"
    );

    // Repeated opens of the same missing-server file (e.g. reopening
    // the tab, or opening a sibling file under the same root) must be
    // a silent no-op: no new transient, no new attempt.
    app.model.ui.transient_message = None;
    for _ in 0..3 {
        app.ensure_lsp_server(token::syntax::LanguageId::Rust, &file);
    }
    assert!(
        app.model.ui.transient_message.is_none(),
        "a memoized-missing server must not re-flash a transient on repeated opens"
    );
    assert!(
        app.lsp.detached_roots.is_empty(),
        "repeated opens of a memoized-missing server must never consume a detached-root slot"
    );
}

/// `Cmd::LspRestartServer` clears the missing-server memo so a later
/// `ensure_lsp_server` (the next matching file-open) can retry — e.g.
/// after the user installs the binary the editor previously couldn't
/// find.
#[test]
fn restart_command_clears_the_missing_server_memo() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-missing-restart");
    app.lsp
        .missing_servers
        .insert((server_id.clone(), root.clone()));

    app.process_cmd(Cmd::LspRestartServer {
        server_id: server_id.clone(),
    });

    assert!(!app.lsp.missing_servers.contains(&(server_id.clone(), root)));
}

// ---- Phase 3: go-to-definition request plumbing ----

fn install_open_document(
    app: &mut App,
    document_id: token::model::editor_area::DocumentId,
    server_id: &LspServerId,
    root: &Path,
    uri: lsp_types::Uri,
) {
    app.lsp.open_documents.insert(
        document_id,
        OpenDocState {
            server_id: server_id.clone(),
            root: root.to_path_buf(),
            uri,
            synced_revision: 0,
        },
    );
}

fn test_origin(app: &App) -> JumpEntry {
    JumpEntry {
        group_id: app.model.editor_area.focused_group_id,
        document_id: app.model.document().id.unwrap(),
        path: PathBuf::from("/tmp/origin.rs"),
        line: 0,
        col: 0,
    }
}

#[test]
fn definition_request_with_no_synced_document_reports_not_supported() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let origin = test_origin(&app);

    app.request_lsp_definition(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        revision,
        origin,
    );

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("not supported")));
}

#[test]
fn definition_request_before_handshake_completes_reports_still_indexing() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let origin = test_origin(&app);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-indexing");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-def-indexing/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);
    // Deliberately no handle in `app.lsp.servers` — the handshake
    // hasn't produced one yet, indistinguishable from "still
    // indexing" from the user's point of view.

    app.request_lsp_definition(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        revision,
        origin,
    );

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("indexing")));
}

#[test]
fn definition_request_reports_not_supported_when_capabilities_lack_definition_provider() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let origin = test_origin(&app);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-nosupport");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-def-nosupport/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);

    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities::default());
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_definition(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        revision,
        origin,
    );

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("not supported")));

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

/// A server that advertises no `textDocumentSync` at all gets no sync
/// traffic (design doc: "absent: no sync messages at all") — checked
/// via `open_documents`, since `lsp_open_document_on` is the one place
/// that populates it and every other sync send is a no-op without an
/// entry there.
#[test]
fn open_document_is_not_registered_for_sync_when_the_server_advertises_no_sync() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let file_path = PathBuf::from("/tmp/proj-nosync/main.rs");
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-nosync");

    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities::default());
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.lsp_open_document_on(doc_id, file_path, server_id.clone(), root.clone(), "rust");

    assert!(
        !app.lsp.open_documents.contains_key(&doc_id),
        "a server with no textDocumentSync must never be told about an open document"
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn a_newer_definition_request_supersedes_and_cancels_the_previous_one() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-supersede");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-def-supersede/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);

    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
        definition_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    });
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_definition(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        revision,
        test_origin(&app),
    );
    let (first_server, first_root, first_id) =
        app.lsp.definition.by_doc.get(&doc_id).cloned().unwrap();

    app.request_lsp_definition(
        doc_id,
        lsp_types::Position {
            line: 1,
            character: 0,
        },
        revision,
        test_origin(&app),
    );
    let (_, _, second_id) = app.lsp.definition.by_doc.get(&doc_id).cloned().unwrap();

    assert_ne!(first_id, second_id, "a new request id must be allocated");
    // The superseded entry stays pending (abandoned, not dropped —
    // the server still owns the id and will reply) until its
    // response actually arrives.
    assert_eq!(app.lsp.definition.requests.len(), 2);
    let handle = app.lsp.servers.get(&(first_server, first_root)).unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(first_id)
            .unwrap()
            .abandoned,
        "the superseded request must be marked abandoned"
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn an_abandoned_definition_response_is_consumed_and_discarded() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-abandoned");
    let origin = test_origin(&app);
    app.lsp.definition.insert(
        (server_id.clone(), root.clone(), 7),
        doc_id,
        PendingDefinition {
            document_id: doc_id,
            revision,
            origin,
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );

    app.msg_tx
        .send(Msg::Lsp(LspMsg::DefinitionResponseFromServer {
            server_id,
            root,
            request_id: 7,
            locations: vec![],
            abandoned: true,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app.lsp.definition.requests.is_empty());
    assert!(app.lsp.definition.by_doc.is_empty());
    assert!(
        app.model.jump_history.is_empty(),
        "a discarded (cancelled) response must never push jump history or navigate"
    );
}

// ---- completion (lsp-integration.md Phase 5) ----

/// A `textDocument/completion` reply translates into
/// `CompletionResolved` carrying converted menu items — the deferred
/// accept's context (`can_resolve`) rides along from the responding
/// handle's capability snapshot.
#[test]
fn a_completion_response_translates_into_completion_resolved() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-completion");
    // A live (fake) handle is required: the interception pass consults
    // its capability snapshot for `resolveProvider`.
    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(
        serde_json::from_value(serde_json::json!({
            "completionProvider": { "resolveProvider": true }
        }))
        .unwrap(),
    );
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.lsp.completion.insert(
        (server_id.clone(), root.clone(), 9),
        doc_id,
        PendingCompletion {
            document_id: doc_id,
            revision,
        },
    );

    app.msg_tx
        .send(Msg::Lsp(LspMsg::CompletionResponseFromServer {
            server_id,
            root,
            request_id: 9,
            items: vec![lsp_types::CompletionItem {
                label: "vacuum".to_owned(),
                ..Default::default()
            }],
            is_incomplete: false,
            abandoned: false,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app.lsp.completion.requests.is_empty());
    assert!(
        app.model.ui.completion_menu.is_none(),
        "the runtime only translates; update() merges into an open menu"
    );
}

#[test]
fn an_abandoned_completion_response_is_consumed_and_discarded() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-completion-abandoned");
    app.lsp.completion.insert(
        (server_id.clone(), root.clone(), 3),
        doc_id,
        PendingCompletion {
            document_id: doc_id,
            revision,
        },
    );

    app.msg_tx
        .send(Msg::Lsp(LspMsg::CompletionResponseFromServer {
            server_id,
            root,
            request_id: 3,
            items: vec![],
            is_incomplete: false,
            abandoned: true,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app.lsp.completion.requests.is_empty());
    assert!(app.lsp.completion.by_doc.is_empty());
}

#[test]
fn a_member_response_opens_a_hidden_session_with_structured_method_details() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.document_mut().buffer = ropey::Rope::from("cc::Build::new().");
    app.model.document_mut().language = token::syntax::LanguageId::Rust;
    app.model.document_mut().file_path = Some("/tmp/proj-completion/build.rs".into());
    app.model.editor_mut().cursors[0] = token::model::Cursor::at(0, 16);
    app.model.editor_mut().clear_selection();
    token::update::update(
        &mut app.model,
        Msg::Completion(token::messages::CompletionMsg::TriggerMenu),
    );
    assert!(app.model.ui.completion_menu.is_some());
    assert!(app.model.ui.cursor_overlay.is_none());
    assert!(send_automation_request(&mut app, AutomationRequest::State)
        .state
        .unwrap()
        .completion
        .is_none());

    let doc = app.model.document();
    let (document_id, revision) = (doc.id.unwrap(), doc.revision);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-completion");
    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(
        serde_json::from_value(serde_json::json!({
            "completionProvider": {}
        }))
        .unwrap(),
    );
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);
    app.lsp.completion.insert(
        (server_id.clone(), root.clone(), 91),
        document_id,
        PendingCompletion {
            document_id,
            revision,
        },
    );
    let item = serde_json::from_value(serde_json::json!({
        "label": "compile", "kind": 2, "preselect": true,
        "labelDetails": { "detail": "(output: &str)", "description": "()" }
    }))
    .unwrap();
    app.msg_tx
        .send(Msg::Lsp(LspMsg::CompletionResponseFromServer {
            server_id,
            root,
            request_id: 91,
            items: vec![item],
            is_incomplete: false,
            abandoned: false,
        }))
        .unwrap();
    app.process_async_messages();
    assert!(app.model.ui.has_visible_completion());
    let menu = app.model.ui.completion_menu.as_ref().unwrap();
    assert_eq!(menu.items.len(), 1);
    let item = menu.selected_item(0).unwrap();
    assert_eq!(item.label, "compile");
    assert_eq!(item.detail.as_deref(), Some("(output: &str) ()"));
    assert!(item.preselect);
}

/// The per-document completion debounce fires exactly one request's
/// worth of bookkeeping after its deadline, and re-arming the same
/// document replaces the pending entry instead of accumulating.
#[test]
fn completion_debounce_rearm_replaces_and_fires_once() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let position = lsp_types::Position::new(1, 2);

    app.process_cmd(Cmd::LspScheduleCompletion {
        document_id: doc_id,
        position,
        revision: 1,
        trigger_character: Some(".".to_owned()),
    });
    app.process_cmd(Cmd::LspScheduleCompletion {
        document_id: doc_id,
        position,
        revision: 2,
        trigger_character: None,
    });

    assert_eq!(
        app.lsp.completion_debounces.len(),
        1,
        "re-arming must replace, not accumulate"
    );
    assert_eq!(
        app.lsp.completion_debounces[&doc_id].revision, 2,
        "the newest schedule wins"
    );

    // Force the deadline due, then fire.
    for scheduled in app.lsp.completion_debounces.values_mut() {
        scheduled.deadline = std::time::Instant::now() - Duration::from_secs(1);
    }
    // No server has this document open: firing gates out silently but
    // still drains the debounce entry.
    app.check_lsp_completion_debounces();
    assert!(app.lsp.completion_debounces.is_empty());
    assert!(app.lsp.completion.requests.is_empty());
}

#[test]
fn cancel_completion_drops_the_debounce_and_supersedes_the_request() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-cancel");

    app.process_cmd(Cmd::LspScheduleCompletion {
        document_id: doc_id,
        position: lsp_types::Position::new(0, 0),
        revision: 1,
        trigger_character: None,
    });
    app.lsp.completion.insert(
        (server_id.clone(), root.clone(), 5),
        doc_id,
        PendingCompletion {
            document_id: doc_id,
            revision: 1,
        },
    );

    app.process_cmd(Cmd::LspCancelCompletion {
        document_id: doc_id,
    });

    assert!(app.lsp.completion_debounces.is_empty());
    assert!(
        !app.lsp.completion.by_doc.contains_key(&doc_id),
        "the in-flight request must be superseded so its late reply is dropped"
    );
}

/// A resolve timeout emits an empty `CompletionItemResolved` so a
/// blocked accept applies instead of hanging until Escape.
#[test]
fn a_resolve_past_its_deadline_unblocks_the_accept_with_no_extras() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-resolve-timeout");
    app.lsp.resolve.insert(
        (server_id.clone(), root.clone(), 4),
        doc_id,
        PendingResolve {
            document_id: doc_id,
            revision: 1,
            selected: 0,
            purpose: ResolvePurpose::Accept,
        },
    );
    app.lsp.resolve.arm_deadline((server_id, root, 4));

    // Force every deadline due.
    for deadline in app.lsp.resolve.deadlines.values_mut() {
        *deadline = std::time::Instant::now() - Duration::from_secs(1);
    }
    app.check_lsp_resolve_deadlines();

    assert!(app.lsp.resolve.requests.is_empty());
    assert!(app.lsp.resolve.by_doc.is_empty());
}

#[test]
fn commit_character_runtime_reply_timeout_and_missing_server_preserve_the_transaction() {
    use token::completion::lsp::items_to_menu_items;
    for outcome in ["reply", "timeout", "missing"] {
        let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
        app.model.document_mut().language = token::syntax::LanguageId::Rust;
        app.model.document_mut().file_path = Some("/tmp/proj-commit/lib.rs".into());
        for ch in "va".chars() {
            app.process_automation_msg(Msg::Document(DocumentMsg::InsertChar(ch)));
        }
        let menu = app.model.ui.completion_menu.clone().unwrap();
        let server = LspServerId::from("rust-analyzer");
        let root = PathBuf::from("/tmp/proj-commit");
        let options = lsp_types::CompletionOptions {
            resolve_provider: Some(true),
            all_commit_characters: Some(vec!["(".into()]),
            ..Default::default()
        };
        if outcome != "missing" {
            let handle = spawn_fake_handle(&server);
            *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
                completion_provider: Some(options.clone()),
                ..Default::default()
            });
            app.lsp
                .servers
                .insert((server.clone(), root.clone()), handle);
        }
        let import = |text: &str| lsp_types::TextEdit {
            range: lsp_types::Range::new(
                lsp_types::Position::new(0, 0),
                lsp_types::Position::new(0, 0),
            ),
            new_text: text.into(),
        };
        let items = items_to_menu_items(
            vec![lsp_types::CompletionItem {
                label: "vacuum".into(),
                additional_text_edits: Some(vec![import("// upfront\n")]),
                ..Default::default()
            }],
            &server,
            &root,
            Some(&options),
        );
        app.process_automation_msg(Msg::Lsp(LspMsg::CompletionResolved {
            document_id: menu.document_id,
            revision: menu.revision,
            items,
            is_incomplete: false,
        }));
        let history = app.model.document().undo_stack.len();
        app.process_automation_msg(Msg::Document(DocumentMsg::InsertChar('(')));
        if outcome != "missing" {
            assert_eq!(app.model.document().buffer.to_string(), "va(");
        }
        match outcome {
            "reply" => {
                let (server_id, root, request_id) =
                    app.lsp.resolve.requests.keys().next().cloned().unwrap();
                app.msg_tx
                    .send(Msg::Lsp(LspMsg::ResolveResponseFromServer {
                        server_id,
                        root,
                        request_id,
                        abandoned: false,
                        item: Some(Box::new(lsp_types::CompletionItem {
                            label: "vacuum".into(),
                            additional_text_edits: Some(vec![import("// resolved\n")]),
                            ..Default::default()
                        })),
                    }))
                    .unwrap();
                app.process_async_messages();
            }
            "timeout" => {
                for deadline in app.lsp.resolve.deadlines.values_mut() {
                    *deadline = Instant::now() - Duration::from_secs(1);
                }
                app.check_lsp_resolve_deadlines();
            }
            "missing" => {}
            _ => unreachable!(),
        }
        let expected = if outcome == "reply" {
            "// resolved\nvacuum("
        } else {
            "// upfront\nvacuum("
        };
        assert_eq!(app.model.document().buffer.to_string(), expected);
        assert_eq!(app.model.document().undo_stack.len(), history + 1);
        assert_eq!(
            app.syntax_deadlines[&menu.document_id].1,
            app.model.document().revision
        );
        assert!(app.lsp.resolve.requests.is_empty());
        app.process_automation_msg(Msg::Document(DocumentMsg::Undo));
        assert_eq!(app.model.document().buffer.to_string(), "va");
        app.process_automation_msg(Msg::Document(DocumentMsg::Redo));
        assert_eq!(app.model.document().buffer.to_string(), expected);
    }
}

/// A docs-purpose resolve that times out is dropped silently: no
/// `CompletionItemResolved` reaches the menu (the item stays unresolved
/// so the next selection change may retry), and the slot is cleaned up.
#[test]
fn a_docs_resolve_past_its_deadline_is_dropped_silently() {
    use token::completion::menu::{
        CompletionMenuState, LspInsert, MenuInsert, MenuItem, MenuItemKind, MenuSourceId,
    };

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-docs-timeout");
    app.model.ui.completion_menu = Some(CompletionMenuState {
        document_id: doc_id,
        revision,
        query_start: token::model::Cursor::at(0, 0),
        query: String::new(),
        items: vec![MenuItem {
            label: "foo".to_owned(),
            filter_text: "foo".to_owned(),
            insert: MenuInsert::Lsp(Box::new(LspInsert {
                text: "foo".to_owned(),
                server_id: server_id.clone(),
                root: root.clone(),
                raw: std::sync::Arc::new(lsp_types::CompletionItem {
                    label: "foo".into(),
                    ..Default::default()
                }),
                can_resolve: true,
                resolved: false,
                text_edit: None,
                additional_text_edits: Vec::new(),
                commit_characters: std::sync::Arc::from([]),
                documentation: None,
                caret_offset: None,
            })),
            kind: MenuItemKind::Function,
            source: MenuSourceId::Lsp,
            detail: None,
            sort_text: None,
            preselect: false,
        }],
        filtered: vec![(0, 0, Vec::new())],
        is_incomplete: false,
        pending_resolve: None,
        context: Default::default(),
        selection_changed: false,
    });
    app.model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
        token::model::CursorOverlayKind::Completion,
    ));
    app.lsp.resolve.insert(
        (server_id.clone(), root.clone(), 4),
        doc_id,
        PendingResolve {
            document_id: doc_id,
            revision,
            selected: 0,
            purpose: ResolvePurpose::Docs,
        },
    );
    app.lsp.resolve.arm_deadline((server_id, root, 4));
    for deadline in app.lsp.resolve.deadlines.values_mut() {
        *deadline = std::time::Instant::now() - Duration::from_secs(1);
    }

    app.check_lsp_resolve_deadlines();

    assert!(app.lsp.resolve.requests.is_empty());
    let menu = app.model.ui.completion_menu.as_ref().unwrap();
    let MenuInsert::Lsp(data) = &menu.items[0].insert else {
        panic!("expected LSP item");
    };
    assert!(
        !data.resolved,
        "a silent docs timeout must not mark the item resolved"
    );
}

#[test]
fn cancel_completion_drops_the_resolve_debounce() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    app.process_cmd(Cmd::LspScheduleResolve {
        document_id: doc_id,
        revision: 1,
        server_id: LspServerId::from("rust-analyzer"),
        root: PathBuf::from("/tmp/proj-cancel-docs"),
        raw_item: std::sync::Arc::new(lsp_types::CompletionItem {
            label: "foo".into(),
            ..Default::default()
        }),
        selected: 0,
    });
    assert_eq!(app.lsp.resolve_debounces.len(), 1);

    app.process_cmd(Cmd::LspCancelCompletion {
        document_id: doc_id,
    });
    assert!(app.lsp.resolve_debounces.is_empty());
}

/// An empty `textDocument/definition` reply while the server is still
/// `Starting`/`Indexing` must report "still indexing…", never "no
/// definition found" — the design doc's "while not Ready, empty
/// feature results display 'still indexing…', never 'not found'"
/// (lines 101/212). Once the mirror reports `Ready`, the same empty
/// reply is a genuine `NoResult`.
#[test]
fn an_empty_definition_reply_reports_still_indexing_before_ready() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-empty-indexing");
    let origin = test_origin(&app);
    app.lsp.definition.insert(
        (server_id.clone(), root.clone(), 9),
        doc_id,
        PendingDefinition {
            document_id: doc_id,
            revision,
            origin: origin.clone(),
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );
    app.model
        .lsp
        .servers
        .insert(server_id.clone(), ServerState::Indexing);

    app.msg_tx
        .send(Msg::Lsp(LspMsg::DefinitionResponseFromServer {
            server_id: server_id.clone(),
            root: root.clone(),
            request_id: 9,
            locations: vec![],
            abandoned: false,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("indexing")));

    // The same empty reply once the server is `Ready` is a genuine
    // "no definition found".
    app.lsp.definition.insert(
        (server_id.clone(), root.clone(), 10),
        doc_id,
        PendingDefinition {
            document_id: doc_id,
            revision,
            origin,
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );
    app.model
        .lsp
        .servers
        .insert(server_id.clone(), ServerState::Ready);
    app.msg_tx
        .send(Msg::Lsp(LspMsg::DefinitionResponseFromServer {
            server_id,
            root,
            request_id: 10,
            locations: vec![],
            abandoned: false,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("No definition found")));
}

/// Mirrors `an_empty_definition_reply_reports_still_indexing_before_ready`
/// for hover: a null hover reply while the server is still
/// `Starting`/`Indexing` must report "still indexing…", not "no hover
/// information".
#[test]
fn a_null_hover_reply_reports_still_indexing_before_ready() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-empty-indexing");
    let cursor = test_cursor(&app);
    app.lsp.hover.insert(
        (server_id.clone(), root.clone(), 9),
        doc_id,
        PendingHover {
            document_id: doc_id,
            revision,
            cursor,
        },
    );
    app.model
        .lsp
        .servers
        .insert(server_id.clone(), ServerState::Indexing);

    app.msg_tx
        .send(Msg::Lsp(LspMsg::HoverResponseFromServer {
            server_id,
            root,
            request_id: 9,
            content: None,
            abandoned: false,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("indexing")));
    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "still-indexing must not open the hover card"
    );
}

#[test]
fn a_definition_request_past_its_deadline_is_abandoned_and_reports_no_result() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-timeout");
    let origin = test_origin(&app);
    let handle = spawn_fake_handle(&server_id);
    // Register a real pending id on the handle (`begin_request`) so
    // `check_lsp_definition_deadlines`' `abandon` call has something
    // to mark — mirrors how `request_lsp_definition` allocates it.
    let request_id = handle.begin_request("textDocument/definition", serde_json::json!({}));
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    let key = (server_id.clone(), root.clone(), request_id);
    app.lsp.definition.insert(
        key.clone(),
        doc_id,
        PendingDefinition {
            document_id: doc_id,
            revision,
            origin,
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );
    // Already past due, rather than sleeping 30s in a test.
    app.lsp
        .definition
        .deadlines
        .insert(key.clone(), Instant::now() - Duration::from_secs(1));

    app.check_lsp_definition_deadlines();

    assert!(app.lsp.definition.requests.is_empty());
    assert!(app.lsp.definition.by_doc.is_empty());
    assert!(app.lsp.definition.deadlines.is_empty());
    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("No definition found")));
    // The abandoned id must still be tracked as such on the handle
    // (advisory `$/cancelRequest`) so a late reply is discarded.
    let handle = app
        .lsp
        .servers
        .get(&(server_id.clone(), root.clone()))
        .unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(request_id)
            .unwrap()
            .abandoned
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn a_deadline_for_an_already_superseded_request_is_dropped_silently() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-def-timeout-superseded");
    let origin = test_origin(&app);

    let stale_key = (server_id.clone(), root.clone(), 1);
    app.lsp.definition.requests.insert(
        stale_key.clone(),
        PendingDefinition {
            document_id: doc_id,
            revision,
            origin: origin.clone(),
            server_id: server_id.clone(),
            root: root.clone(),
        },
    );
    // A newer request now owns `by_doc` for this document — `stale_key`
    // is superseded but its deadline is still ticking.
    let current_key = (server_id.clone(), root.clone(), 2);
    app.lsp
        .definition
        .by_doc
        .insert(doc_id, current_key.clone());
    app.lsp
        .definition
        .deadlines
        .insert(stale_key.clone(), Instant::now() - Duration::from_secs(1));
    let status_before = app.model.ui.transient_message.clone();

    app.check_lsp_definition_deadlines();

    // The stale entry's deadline firing must clean up its `requests`
    // bookkeeping too — leaving it behind would leak for the rest of
    // the session (nothing else ever removes a superseded entry once
    // its `by_doc` half is gone).
    assert!(
        !app.lsp.definition.requests.contains_key(&stale_key),
        "a superseded request's stale deadline must remove its bookkeeping, not leak it"
    );
    assert_eq!(
        app.lsp.definition.by_doc.get(&doc_id),
        Some(&current_key),
        "the newer request must still own the doc's outcome"
    );
    assert_eq!(
        app.model.ui.transient_message.map(|t| t.text),
        status_before.map(|t| t.text),
        "an already-superseded request's timeout must not flash a status"
    );
}

// ---- Phase 4: hover request plumbing ----

fn test_cursor(app: &App) -> token::model::editor::Position {
    app.model.editor().active_cursor().to_position()
}

#[test]
fn hover_request_with_no_synced_document_reports_not_supported() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let cursor = test_cursor(&app);

    app.request_lsp_hover(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        cursor,
        revision,
    );

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("not supported")));
}

#[test]
fn hover_request_reports_not_supported_when_capabilities_lack_hover_provider() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let cursor = test_cursor(&app);
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-nosupport");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-hover-nosupport/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);

    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities::default());
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_hover(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        cursor,
        revision,
    );

    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("not supported")));

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn a_newer_hover_request_supersedes_and_cancels_the_previous_one() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-supersede");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-hover-supersede/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);

    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
        hover_provider: Some(lsp_types::HoverProviderCapability::Simple(true)),
        ..Default::default()
    });
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_hover(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        test_cursor(&app),
        revision,
    );
    let (first_server, first_root, first_id) = app.lsp.hover.by_doc.get(&doc_id).cloned().unwrap();

    app.request_lsp_hover(
        doc_id,
        lsp_types::Position {
            line: 1,
            character: 0,
        },
        test_cursor(&app),
        revision,
    );
    let (_, _, second_id) = app.lsp.hover.by_doc.get(&doc_id).cloned().unwrap();

    assert_ne!(first_id, second_id, "a new request id must be allocated");
    assert_eq!(app.lsp.hover.requests.len(), 2);
    let handle = app.lsp.servers.get(&(first_server, first_root)).unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(first_id)
            .unwrap()
            .abandoned,
        "the superseded request must be marked abandoned"
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

#[test]
fn an_abandoned_hover_response_is_consumed_and_discarded() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-abandoned");
    let cursor = test_cursor(&app);
    app.lsp.hover.insert(
        (server_id.clone(), root.clone(), 7),
        doc_id,
        PendingHover {
            document_id: doc_id,
            revision,
            cursor,
        },
    );

    app.msg_tx
        .send(Msg::Lsp(LspMsg::HoverResponseFromServer {
            server_id,
            root,
            request_id: 7,
            content: Some("should never be seen".into()),
            abandoned: true,
        }))
        .unwrap();
    app.process_async_messages();

    assert!(app.lsp.hover.requests.is_empty());
    assert!(app.lsp.hover.by_doc.is_empty());
    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "a discarded (cancelled) response must never open the hover card"
    );
}

#[test]
fn a_hover_request_past_its_deadline_is_abandoned_with_no_content() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-timeout");
    let cursor = test_cursor(&app);
    let handle = spawn_fake_handle(&server_id);
    let request_id = handle.begin_request("textDocument/hover", serde_json::json!({}));
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    let key = (server_id.clone(), root.clone(), request_id);
    app.lsp.hover.insert(
        key.clone(),
        doc_id,
        PendingHover {
            document_id: doc_id,
            revision,
            cursor,
        },
    );
    app.lsp
        .hover
        .deadlines
        .insert(key.clone(), Instant::now() - Duration::from_secs(1));

    app.check_lsp_hover_deadlines();

    assert!(app.lsp.hover.requests.is_empty());
    assert!(app.lsp.hover.by_doc.is_empty());
    assert!(app.lsp.hover.deadlines.is_empty());
    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "no content and no diagnostics -> nothing to show"
    );
    let handle = app
        .lsp
        .servers
        .get(&(server_id.clone(), root.clone()))
        .unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(request_id)
            .unwrap()
            .abandoned
    );

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

/// Mirrors `a_deadline_for_an_already_superseded_request_is_dropped_silently`
/// for hover: a superseded entry's stale deadline must remove its
/// `hover_requests` bookkeeping, not just skip over it and leak.
#[test]
fn a_hover_deadline_for_an_already_superseded_request_removes_its_bookkeeping() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-hover-timeout-superseded");
    let cursor = test_cursor(&app);

    let stale_key = (server_id.clone(), root.clone(), 1);
    app.lsp.hover.requests.insert(
        stale_key.clone(),
        PendingHover {
            document_id: doc_id,
            revision,
            cursor,
        },
    );
    let current_key = (server_id.clone(), root.clone(), 2);
    app.lsp.hover.by_doc.insert(doc_id, current_key.clone());
    app.lsp
        .hover
        .deadlines
        .insert(stale_key.clone(), Instant::now() - Duration::from_secs(1));

    app.check_lsp_hover_deadlines();

    assert!(
        !app.lsp.hover.requests.contains_key(&stale_key),
        "a superseded hover request's stale deadline must remove its bookkeeping, not leak it"
    );
    assert_eq!(
        app.lsp.hover.by_doc.get(&doc_id),
        Some(&current_key),
        "the newer request must still own the doc's outcome"
    );
}

// ========================================================================
// Mouse-dwell hover (`hover_dwell` state machine)
// ========================================================================

#[test]
fn hover_dwell_arms_on_the_first_move() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    assert!(app.hover_dwell.is_none());

    app.update_hover_dwell(None, 10.0, 20.0);

    let (x, y, _) = app.hover_dwell.expect("first move arms the dwell timer");
    assert_eq!((x, y), (10.0, 20.0));
}

#[test]
fn a_small_move_does_not_reset_the_dwell_position() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.update_hover_dwell(None, 10.0, 10.0);
    let armed_at = app.hover_dwell.unwrap().2;

    // 1px move, under HOVER_DWELL_MOVE_THRESHOLD_PX — jitter, not a
    // real move.
    app.update_hover_dwell(Some((10.0, 10.0)), 11.0, 10.0);

    let (x, y, started) = app.hover_dwell.expect("still armed");
    assert_eq!((x, y), (10.0, 10.0), "position must not move for jitter");
    assert_eq!(started, armed_at, "timer must not restart for jitter");
}

#[test]
fn a_significant_move_restarts_the_dwell_at_the_new_position() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.update_hover_dwell(None, 10.0, 10.0);

    app.update_hover_dwell(Some((10.0, 10.0)), 200.0, 10.0);

    let (x, y, _) = app.hover_dwell.expect("still armed at the new position");
    assert_eq!((x, y), (200.0, 10.0));
}

#[test]
fn dwell_does_not_arm_while_a_modal_is_open() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.open_modal(token::model::ModalState::GotoLine(
        token::model::GotoLineState::default(),
    ));

    app.update_hover_dwell(None, 10.0, 10.0);

    assert!(app.hover_dwell.is_none());
}

#[test]
fn dwell_does_not_arm_while_a_cursor_overlay_is_open() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
        token::model::CursorOverlayKind::DebugCompletion,
    ));

    app.update_hover_dwell(None, 10.0, 10.0);

    assert!(app.hover_dwell.is_none());
}

#[test]
fn moving_outside_the_hover_card_panel_dismisses_it() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
        token::model::CursorOverlayKind::Hover,
    ));
    app.model.ui.hover_card = Some(token::model::HoverCardState {
        content: Some("fn main()".into()),
        ..Default::default()
    });
    // Simulates `update_cursor_icon` having hit-tested the new point
    // outside the card's panel.
    app.model.ui.hover = token::model::HoverRegion::EditorText;

    app.update_hover_dwell(Some((10.0, 10.0)), 200.0, 200.0);

    assert!(app.model.ui.cursor_overlay.is_none());
    assert!(app.model.ui.hover_card.is_none());
}

#[test]
fn moving_within_the_hover_card_panel_does_not_dismiss_it() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.cursor_overlay = Some(token::model::CursorOverlayState::new(
        token::model::CursorOverlayKind::Hover,
    ));
    app.model.ui.hover_card = Some(token::model::HoverCardState {
        content: Some("fn main()".into()),
        ..Default::default()
    });
    // Simulates `update_cursor_icon` having hit-tested the new point
    // as still inside the card's own (scrollable/clickable) panel.
    app.model.ui.hover = token::model::HoverRegion::CursorOverlay;

    app.update_hover_dwell(Some((10.0, 10.0)), 200.0, 200.0);

    assert!(
        app.model.ui.cursor_overlay.is_some(),
        "moving within the card must not dismiss it"
    );
    assert!(app.model.ui.hover_card.is_some());
}

#[test]
fn check_hover_dwell_is_a_noop_when_disabled_in_config() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.hover_on_mouse = false;
    app.model.ui.hover = token::model::HoverRegion::EditorText;
    app.hover_dwell = Some((10.0, 10.0, Instant::now() - Duration::from_secs(1)));

    assert!(!app.check_hover_dwell());
    assert!(
        app.hover_dwell.is_some(),
        "a disabled feature must leave the armed dwell alone (no surprise clear)"
    );
}

#[test]
fn check_hover_dwell_does_not_fire_before_the_delay_elapses() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.hover = token::model::HoverRegion::EditorText;
    app.hover_dwell = Some((10.0, 10.0, Instant::now()));

    assert!(!app.check_hover_dwell());
}

#[test]
fn next_wake_ignores_an_expired_dwell_deadline() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.hover = token::model::HoverRegion::Sidebar;
    app.hover_dwell = Some((10.0, 10.0, Instant::now() - Duration::from_secs(1)));

    let now = Instant::now();
    assert!(
        app.next_wake(now) > now,
        "a past dwell deadline must not schedule an immediate (spinning) wake-up"
    );
}

#[test]
fn settings_blink_off_uses_a_positive_maintenance_interval() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.cursor_blink_ms = 0;
    let now = Instant::now();
    app.last_tick = now;
    assert_eq!(app.cursor_tick_interval(), Duration::from_millis(250));
    assert!(app.next_wake(now) > now);
}

#[test]
fn check_hover_dwell_does_not_fire_outside_editor_text() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.ui.hover = token::model::HoverRegion::Sidebar;
    app.hover_dwell = Some((10.0, 10.0, Instant::now() - Duration::from_secs(1)));

    assert!(!app.check_hover_dwell());
}

/// End-to-end "hover resolved and rendered" (design doc's Testing
/// Strategy fake-server scenario): markdown content is stripped to
/// plaintext and lands on `ui.hover_card`, the card opens
/// (`CursorOverlayKind::Hover`).
#[test]
fn hover_resolved_opens_the_card_with_plaintext_content() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "hoverProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/hover", "respond": {
                "contents": { "kind": "markdown", "value": "**fn** main() -> ()" },
            }},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));

    app.process_automation_msg(Msg::Lsp(LspMsg::ShowHover));

    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.ui.cursor_overlay.is_some()
    }));

    assert_eq!(
        app.model.ui.cursor_overlay.map(|o| o.kind),
        Some(token::model::CursorOverlayKind::Hover)
    );
    assert_eq!(
        app.model
            .ui
            .hover_card
            .as_ref()
            .and_then(|s| s.content.as_ref().map(|t| t.text.as_str())),
        Some("fn main() -> ()"),
        "markdown emphasis must be stripped to plaintext"
    );

    app.process_cmd(Cmd::Quit);
}

/// A hover response for a revision the document has since moved past
/// (an edit landed between request and reply) must never open the
/// card — the design doc's revision guard, exercised end-to-end.
#[test]
fn a_stale_hover_response_after_a_revision_bump_is_dropped() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "hoverProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/hover", "respond": {
                "contents": { "kind": "plaintext", "value": "stale hover" },
            }},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));

    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let cursor = test_cursor(&app);
    // Issue the request directly (bypassing the flush the real
    // `ShowHover` -> `request_lsp_hover` path would run) so the edit
    // below is guaranteed to land after the request is already
    // in flight, deterministically reproducing the race.
    app.request_lsp_hover(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        cursor,
        revision,
    );
    app.model.document_mut().buffer.insert(0, "x");
    app.model.document_mut().revision += 1;

    // Drain every async message the scenario produces — the response
    // arrives, fails the revision guard, and must never open a card.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !app.lsp.hover.requests.is_empty() {
        app.process_async_messages();
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "a stale (revision-bumped) hover response must be dropped, not rendered"
    );

    app.process_cmd(Cmd::Quit);
}

/// End-to-end "references resolved and rendered" (Show Usages fake-
/// server scenario): a `textDocument/references` reply with more than
/// one location opens the popup (`CursorOverlayKind::References`)
/// with rows built from the real response, not a hand-set model.
#[test]
fn references_resolved_opens_the_popup_with_two_locations() {
    references_resolved_populates_destination(false);
}

#[test]
fn usages_panel_real_server_response_populates_persistent_results() {
    references_resolved_populates_destination(true);
}

fn references_resolved_populates_destination(panel: bool) {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {\n    foo();\n    foo();\n}\n")
        .expect("write fixture file");
    let file_uri = lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "referencesProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/references", "respond": [
                {
                    "uri": file_uri.as_str(),
                    "range": {
                        "start": { "line": 1, "character": 4 },
                        "end": { "line": 1, "character": 7 },
                    },
                },
                {
                    "uri": file_uri.as_str(),
                    "range": {
                        "start": { "line": 2, "character": 4 },
                        "end": { "line": 2, "character": 7 },
                    },
                },
            ]},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));

    app.process_automation_msg(Msg::Lsp(if panel {
        LspMsg::FindUsagesInPanel
    } else {
        LspMsg::FindReferences
    }));

    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        if panel {
            !app.model.usages_panel.is_loading()
        } else {
            app.model.ui.cursor_overlay.is_some()
        }
    }));

    let items = if panel {
        assert!(app.model.ui.cursor_overlay.is_none());
        assert_eq!(
            app.model.ui.focus,
            token::model::FocusTarget::Dock(token::panel::DockPosition::Bottom)
        );
        &app.model.usages_panel.items
    } else {
        assert_eq!(
            app.model.ui.cursor_overlay.map(|o| o.kind),
            Some(token::model::CursorOverlayKind::References)
        );
        app.model
            .ui
            .reference_list
            .as_ref()
            .expect("popup rows stored")
    };
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].position.line, 1);
    assert_eq!(items[1].position.line, 2);
    // Previews are read from the (now-open) document's own buffer.
    assert_eq!(items[0].preview, "foo();");

    app.process_cmd(Cmd::Quit);
}

/// A references response for a revision the document has since moved
/// past must never open the popup — the revision guard, exercised
/// end-to-end (mirrors `a_stale_hover_response_after_a_revision_bump_is_dropped`).
#[test]
fn stale_references_response_after_a_revision_bump_is_dropped() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let file_uri = lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "referencesProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/references", "respond": [
                {
                    "uri": file_uri.as_str(),
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 3 },
                    },
                },
                {
                    "uri": file_uri.as_str(),
                    "range": {
                        "start": { "line": 0, "character": 5 },
                        "end": { "line": 0, "character": 8 },
                    },
                },
            ]},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));

    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let cursor = test_cursor(&app);
    app.request_lsp_references(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        cursor,
        revision,
        token::model::usages::ReferencesTarget::Popup,
    );
    app.model.document_mut().buffer.insert(0, "x");
    app.model.document_mut().revision += 1;

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !app.lsp.references.requests.is_empty() {
        app.process_async_messages();
        std::thread::sleep(Duration::from_millis(20));
    }

    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "a stale (revision-bumped) references response must be dropped, not rendered"
    );

    app.process_cmd(Cmd::Quit);
}

/// End-to-end "multi-def popup" (go-to-definition upgrade): a
/// `textDocument/definition` reply with more than one location opens
/// the same `CursorOverlayKind::References` popup Show Usages uses,
/// instead of jumping to the first location.
#[test]
fn goto_definition_with_multiple_locations_opens_the_popup() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let a_uri = lsp::path_to_uri(&dir.path().join("a.rs"));
    let b_uri = lsp::path_to_uri(&dir.path().join("b.rs"));

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "definitionProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/definition", "respond": [
                {
                    "uri": a_uri.as_str(),
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 0, "character": 3 },
                    },
                },
                {
                    "uri": b_uri.as_str(),
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 1, "character": 3 },
                    },
                },
            ]},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));

    app.process_automation_msg(Msg::Lsp(LspMsg::GotoDefinition));

    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.ui.cursor_overlay.is_some()
    }));

    assert_eq!(
        app.model.ui.cursor_overlay.map(|o| o.kind),
        Some(token::model::CursorOverlayKind::References)
    );
    assert_eq!(app.model.ui.reference_list.as_ref().map(Vec::len), Some(2));
    // No jump happened — the origin document must still be focused.
    assert_eq!(
        app.model.document().file_path.as_deref(),
        Some(file_path.as_path())
    );

    app.process_cmd(Cmd::Quit);
}

/// Hover on a line with a diagnostic whose `relatedInformation` points
/// elsewhere ("first borrow occurs here") surfaces that related message
/// alongside the primary diagnostic — driven end-to-end through a fake
/// server (`publishDiagnostics` + `ShowHover`) and asserted against the
/// real render path (`view::modal::with_cursor_overlay_spec`), not a
/// hand-set model plus a duplicated automation-only projection.
#[test]
fn hover_on_a_diagnostic_line_includes_related_information() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "let x = y;\n").expect("write fixture file");
    let file_uri = lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "hoverProvider": true }
            }},
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": file_uri.as_str(),
                "diagnostics": [{
                    "range": {
                        "start": { "line": 0, "character": 8 },
                        "end": { "line": 0, "character": 9 },
                    },
                    "severity": 1,
                    "message": "cannot find value `y`",
                    "relatedInformation": [{
                        "location": {
                            "uri": lsp::path_to_uri(&dir.path().join("other.rs")).as_str(),
                            "range": {
                                "start": { "line": 11, "character": 0 },
                                "end": { "line": 11, "character": 1 },
                            },
                        },
                        "message": "first borrow occurs here",
                    }],
                }],
            }},
            // No hover content from the server — the diagnostic alone
            // is reason enough to open the card.
            { "op": "expect_request", "method": "textDocument/hover", "respond": serde_json::Value::Null },
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready)
    }));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        !app.model.document().diagnostics.is_empty()
    }));

    app.model.editor_mut().cursors[0] = token::model::editor::Cursor::at(0, 8);
    app.model.editor_mut().clear_selection();
    app.process_automation_msg(Msg::Lsp(LspMsg::ShowHover));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.ui.cursor_overlay.is_some()
    }));

    let (banner, banner_spans, text) =
        token::view::modal::with_cursor_overlay_spec(&app.model, |spec| match &spec.body {
            token::view::overlay_surface::Body::Zones(zones) => (
                zones.banner.map(|(_, message, _)| message.to_owned()),
                zones.banner_spans.to_vec(),
                zones.text.map(str::to_owned),
            ),
            _ => panic!("hover card must render a Zones body"),
        })
        .expect("hover overlay must be open");

    // The backticked identifier becomes a code chip: the banner text drops
    // the backticks and carries a `Code` span over `y`.
    assert_eq!(banner.as_deref(), Some("cannot find value y"));
    assert_eq!(
        banner_spans,
        vec![token::model::Span {
            range: 18..19,
            style: token::model::SpanStyle::Code,
        }]
    );
    assert!(
        text.as_deref()
            .is_some_and(|t| t.contains("first borrow occurs here")),
        "relatedInformation must reach the rendered card: {text:?}"
    );

    app.process_cmd(Cmd::Quit);
}

/// End-to-end Problems panel flow driven through a fake server: a
/// publish lands rows in the mirror, `CommandId::ToggleProblems` (the
/// palette's own confirm path — "invoke by command name") opens the
/// dock and the automation snapshot reports them, and keyboard nav
/// (Down, Enter) jumps to the selected diagnostic's file and cursor.
#[test]
fn problems_panel_end_to_end_via_fake_server_publish_and_command() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\nlet x = y;\n").expect("write fixture file");
    let file_uri = lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": file_uri.as_str(),
                "diagnostics": [{
                    "range": {
                        "start": { "line": 1, "character": 8 },
                        "end": { "line": 1, "character": 9 },
                    },
                    "severity": 1,
                    "message": "cannot find value `y`",
                }],
            }},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    // The publish targets a file that isn't open yet -- the mirror
    // must still populate (design doc: retained for unopened files).
    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        !app.model.lsp.diagnostics.is_empty()
    }));

    // The panel is current-file scoped: focus the diagnosed file
    // (canonicalized, so the tab and the mirror key agree byte-wise).
    let canon_path = std::fs::canonicalize(&file_path).unwrap();
    app.process_automation_msg(Msg::Layout(token::messages::LayoutMsg::OpenFileInNewTab(
        canon_path,
    )));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));

    // "Invoke by command name": the same `execute_command` path the
    // command palette's confirm handler runs.
    let cmd =
        token::update::execute_command(&mut app.model, token::commands::CommandId::ToggleProblems);
    if let Some(cmd) = cmd {
        app.process_cmd(cmd);
    }

    let snapshot = crate::automation::EditorSnapshot::from_model(&app.model);
    let problems = snapshot
        .problems
        .expect("panel must be open after the toggle");
    assert_eq!(problems.errors, 1);
    assert_eq!(
        problems.rows.len(),
        2,
        "a File row plus its one Diagnostic row"
    );
    assert_eq!(problems.rows[0].kind, "file");
    assert_eq!(problems.rows[1].kind, "diagnostic");

    app.process_automation_msg(Msg::Problems(token::messages::ProblemsMsg::SelectNext)); // File row
    app.process_automation_msg(Msg::Problems(token::messages::ProblemsMsg::SelectNext)); // Diagnostic row
    assert_eq!(app.model.problems_panel.selected_index, Some(1));

    app.process_automation_msg(Msg::Problems(token::messages::ProblemsMsg::OpenSelected));

    // macOS's /tmp -> /private/tmp symlink means the opened tab's path
    // and the fixture's raw `tempdir()` path canonicalize the same but
    // aren't byte-identical.
    assert_eq!(
        app.model
            .document()
            .file_path
            .as_ref()
            .map(|p| std::fs::canonicalize(p).unwrap()),
        Some(std::fs::canonicalize(&file_path).unwrap())
    );
    assert_eq!(app.model.editor().active_cursor().line, 1);
    assert_eq!(app.model.editor().active_cursor().column, 8);

    app.process_cmd(Cmd::Quit);
}

/// Server exit must sweep the mirror, and with it the Problems panel:
/// a stale row must never survive a crash — this is the model-side
/// consequence `clear_diagnostics_for_roots` exists for, driven here
/// through the real crash path instead of calling the sweep directly.
#[test]
fn server_exit_empties_the_open_problems_panel() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    // Canonicalize the root so `clear_diagnostics_for_roots`'s
    // `path.starts_with(root)` prefix check agrees with the URI's
    // (also-canonicalized) path -- macOS's /tmp -> /private/tmp
    // symlink otherwise makes them disagree.
    let dir_path = dir.path().canonicalize().unwrap();
    let file_path = dir_path.join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let file_uri = lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": file_uri.as_str(),
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 2 } },
                    "severity": 1,
                    "message": "boom",
                }],
            }},
            // Give the client a window to observe the published row
            // before the crash, so this test can assert the panel
            // really held it (not just that it ends up empty).
            { "op": "sleep_ms", "ms": 300 },
            { "op": "exit", "code": 1 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        !app.model.lsp.diagnostics.is_empty()
    }));

    // The panel is current-file scoped: focus the diagnosed file.
    app.process_automation_msg(Msg::Layout(token::messages::LayoutMsg::OpenFileInNewTab(
        file_path.clone(),
    )));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    app.model
        .dock_layout
        .bottom
        .activate(token::panel::PanelId::PROBLEMS);
    app.model.problems_panel.selected_index = Some(1);
    assert!(
        crate::automation::EditorSnapshot::from_model(&app.model)
            .problems
            .is_some_and(|p| !p.rows.is_empty()),
        "panel must show the published row before the crash"
    );

    // The scenario's `exit` op crashes the fake server; the manager's
    // crash-handling path is `clear_diagnostics_for_roots`, same as a
    // manual restart or `ToggleLsp` off.
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model.lsp.diagnostics.is_empty()
    }));

    let problems = crate::automation::EditorSnapshot::from_model(&app.model)
        .problems
        .expect("panel stays open, just empty");
    assert!(problems.rows.is_empty(), "rows: {:?}", problems.rows);
    assert_eq!(
        problems.selected, None,
        "a stale selection must be clamped away, not just the rows"
    );

    app.process_cmd(Cmd::Quit);
}

// ---- Phase 1 gate: fake-lsp-server integration ----
//
// Drives the real spawn/handshake/didOpen/didChange/shutdown code
// paths against a real child process (`fake-lsp-server`, built as a
// sibling `[[bin]]` — docs/feature/lsp-integration.md's "Integration:
// scriptable fake server"), instead of asserting on `App`'s own
// bookkeeping alone. The fake server's `record_until_exit` op writes
// one line per message it actually received, in receipt order, to a
// transcript file this test reads back.

/// Locates the `fake-lsp-server` binary built alongside the test
/// binary. `env!("CARGO_BIN_EXE_<name>")` only works from files under
/// `tests/`; this is a unit test inside the `token` binary crate, so
/// the path is derived from the test binary's own location instead
/// (`cargo test` still builds every `[[bin]]` target first).
fn fake_lsp_server_path() -> PathBuf {
    let mut path = std::env::current_exe().expect("current test exe");
    path.pop(); // drop the test binary's own filename
    if path.ends_with("deps") {
        path.pop();
    }
    path.push(if cfg!(windows) {
        "fake-lsp-server.exe"
    } else {
        "fake-lsp-server"
    });
    assert!(
        path.is_file(),
        "fake-lsp-server not found at {} — `cargo test` should have built it as a [[bin]]",
        path.display()
    );
    path
}

/// Points `rust-analyzer` at the fake server for this test's config,
/// running the single-step `record_until_exit` scenario that writes
/// every received message to `transcript_path`.
fn configure_fake_rust_analyzer(app: &mut App, dir: &Path, transcript_path: &Path) {
    let scenario_path = dir.join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([{
            "op": "record_until_exit",
            "file": transcript_path.to_string_lossy(),
        }])
        .to_string(),
    )
    .expect("write scenario file");

    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );
}

fn read_transcript_lines(transcript_path: &Path) -> Vec<String> {
    std::fs::read_to_string(transcript_path)
        .map(|s| s.lines().map(str::to_owned).collect())
        .unwrap_or_default()
}

/// Blocks (bounded) until the transcript file has at least
/// `min_lines` lines — the fake server writes asynchronously from a
/// separate process, so assertions can't run the instant a `Cmd` is
/// processed.
fn wait_for_transcript_lines(transcript_path: &Path, min_lines: usize) -> Vec<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let lines = read_transcript_lines(transcript_path);
        if lines.len() >= min_lines || Instant::now() >= deadline {
            return lines;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn edit_heavy_session_stays_in_sync_with_fake_lsp_server() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let transcript_path = dir.path().join("transcript.log");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    configure_fake_rust_analyzer(&mut app, dir.path(), &transcript_path);

    // Open the document as a real editor session would.
    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    app.process_cmd(Cmd::LspDidOpen {
        document_id: doc_id,
        file_path: file_path.clone(),
        language: LanguageId::Rust,
    });

    // initialize (+ the handshake's `initialized`) + didOpen must
    // all land before anything else.
    let lines = wait_for_transcript_lines(&transcript_path, 3);
    assert!(
        lines[0].starts_with("request:initialize"),
        "expected initialize first, got {lines:?}"
    );
    assert!(
        lines[1].starts_with("notify:initialized"),
        "expected initialized second, got {lines:?}"
    );
    assert!(
        lines[2].starts_with("notify:textDocument/didOpen"),
        "expected didOpen third, got {lines:?}"
    );

    // Edit-heavy burst: schedule several debounced didChange calls in
    // quick succession (well under the 30ms debounce each time), the
    // way `schedule_lsp_did_change` does after every keystroke.
    for revision in 1..=5u64 {
        if let Some(doc) = app.model.editor_area.documents.get_mut(&doc_id) {
            doc.revision = revision;
        }
        app.process_cmd(Cmd::LspScheduleDidChange {
            document_id: doc_id,
            revision,
        });
    }
    assert!(
        app.lsp_change_deadlines.is_pending(doc_id),
        "a debounce should still be pending mid-burst"
    );

    // A request issued mid-debounce (the flush-before-request
    // invariant — Phase 1 wires this generically via
    // `flush_lsp_did_change`; Phase 3+ feature requests call through
    // it before their own request frame).
    app.flush_lsp_did_change(doc_id);
    assert!(
        !app.lsp_change_deadlines.is_pending(doc_id),
        "flush must fire the pending didChange immediately, not wait for its deadline"
    );

    let lines = wait_for_transcript_lines(&transcript_path, 4);
    assert!(
        lines[3].starts_with("notify:textDocument/didChange"),
        "expected the flushed didChange fourth, got {lines:?}"
    );
    assert!(
        lines[3].contains("version=Some(Number(5))"),
        "flush must send the latest revision, got {lines:?}"
    );

    // Save: didSave (with text, since the fake server's initialize
    // response advertised `save: { includeText: true }`).
    app.process_cmd(Cmd::LspDidSave {
        saved_text: "the saved snapshot, not the current buffer".into(),
        document_id: doc_id,
    });
    let lines = wait_for_transcript_lines(&transcript_path, 5);
    assert!(
        lines[4].starts_with("notify:textDocument/didSave"),
        "expected didSave fifth, got {lines:?}"
    );
    assert!(lines[4].contains("the saved snapshot, not the current buffer"));

    // Close: didClose, and the document is forgotten by the manager.
    app.process_cmd(Cmd::LspDidClose {
        document_id: doc_id,
    });
    assert!(!app.lsp.open_documents.contains_key(&doc_id));
    let lines = wait_for_transcript_lines(&transcript_path, 6);
    assert!(
        lines[5].starts_with("notify:textDocument/didClose"),
        "expected didClose sixth, got {lines:?}"
    );

    // Quit teardown: shutdown -> (fake server acks) -> exit -> process
    // exit, all within the 2s budget, no hang.
    app.process_cmd(Cmd::Quit);
    assert!(app.lsp.servers.is_empty());
}

/// A server-initiated `workspace/applyEdit` round trip through the real
/// wire: the reader forwards it as `LspMsg::ApplyEditRequested`,
/// `update()` applies the edit, and `Cmd::LspRespondToServer` answers
/// `applied: true` back over stdin — asserted from the fake server's own
/// transcript of what it received.
#[test]
fn server_initiated_apply_edit_is_applied_and_acknowledged() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let transcript_path = dir.path().join("transcript.log");
    let scenario_path = dir.path().join("scenario.json");
    let uri = token::lsp::path_to_uri(&file_path);
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "request", "id": 77, "method": "workspace/applyEdit", "params": {
                "edit": { "changes": { uri.as_str(): [
                    { "range": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 7 } },
                      "newText": "start" }
                ] } }
            } },
            { "op": "record_until_exit", "file": transcript_path.to_string_lossy() },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );
    let doc_id = app.model.document().id.expect("document id");
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);
    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let reply = loop {
        app.process_async_messages();
        let lines = read_transcript_lines(&transcript_path);
        if let Some(line) = lines
            .iter()
            .find(|l| l.starts_with("response:Some(Number(77))"))
        {
            break line.clone();
        }
        assert!(
            Instant::now() < deadline,
            "no applyEdit reply; transcript: {lines:?}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(reply.contains("\"applied\":true"), "got {reply}");
    assert_eq!(app.model.document().buffer.to_string(), "fn start() {}\n");

    app.process_cmd(Cmd::Quit);
}

/// The debounce/max-wait timer path itself — not the flush helper —
/// actually reaches the wire: schedules through the real
/// `Cmd::LspScheduleDidChange` -> `record_edit` wiring
/// (`DID_CHANGE_DEBOUNCE_MS`/`DID_CHANGE_MAX_WAIT_MS`), lets the
/// deadline elapse for real, then drives it through
/// `check_lsp_did_change_deadlines` (`about_to_wait`'s real per-tick
/// call, zero test callers before this) instead of `flush_lsp_did_change`.
#[test]
fn did_change_deadline_check_sends_the_debounced_change_over_the_wire() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let transcript_path = dir.path().join("transcript.log");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    configure_fake_rust_analyzer(&mut app, dir.path(), &transcript_path);

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    app.process_cmd(Cmd::LspDidOpen {
        document_id: doc_id,
        file_path: file_path.clone(),
        language: LanguageId::Rust,
    });
    wait_for_transcript_lines(&transcript_path, 3);

    if let Some(doc) = app.model.editor_area.documents.get_mut(&doc_id) {
        doc.revision = 7;
    }
    app.process_cmd(Cmd::LspScheduleDidChange {
        document_id: doc_id,
        revision: 7,
    });
    assert!(app.lsp_change_deadlines.is_pending(doc_id));

    // Let the real 30ms debounce elapse, then let the real per-tick
    // check (not the flush shortcut) fire it — repeatedly, since a
    // single call right at the boundary can race the clock.
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.lsp_change_deadlines.is_pending(doc_id) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
        app.check_lsp_did_change_deadlines(&HashMap::new());
    }
    assert!(
        !app.lsp_change_deadlines.is_pending(doc_id),
        "the real deadline check must fire the debounce on its own, with no flush call"
    );

    let lines = wait_for_transcript_lines(&transcript_path, 4);
    assert!(
        lines[3].starts_with("notify:textDocument/didChange"),
        "expected the deadline-fired didChange fourth, got {lines:?}"
    );
    assert!(
        lines[3].contains("version=Some(Number(7))"),
        "expected the scheduled revision on the wire, got {lines:?}"
    );

    app.process_cmd(Cmd::Quit);
}

/// `Cmd::LspScheduleDidChange` for a document no server has open
/// (no server for the language, `didOpen` never sent, plaintext/
/// untitled buffer) must be a no-op — otherwise every keystroke in a
/// zero-server session arms a 30ms deadline that only ever no-ops
/// when it fires.
#[test]
fn schedule_did_change_is_a_no_op_for_a_document_no_server_has_open() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    assert!(!app.lsp.open_documents.contains_key(&doc_id));

    app.process_cmd(Cmd::LspScheduleDidChange {
        document_id: doc_id,
        revision: 1,
    });

    assert!(
        !app.lsp_change_deadlines.is_pending(doc_id),
        "no server has this document open — nothing should arm a debounce deadline"
    );
}

/// The max-wait cap actually caps traffic sent to the wire under
/// continuous edits (not just the pure `DidChangeDeadlines` map), and
/// successive `didChange` versions the wire actually receives
/// strictly increase across a burst — the on-wire counterpart of
/// `lsp::sync::tests::revisions_only_increase_across_a_burst`, which
/// only proves it for the bookkeeping map.
#[test]
fn did_change_max_wait_cap_sends_strictly_increasing_versions_under_continuous_edits() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let transcript_path = dir.path().join("transcript.log");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    configure_fake_rust_analyzer(&mut app, dir.path(), &transcript_path);

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    app.process_cmd(Cmd::LspDidOpen {
        document_id: doc_id,
        file_path: file_path.clone(),
        language: LanguageId::Rust,
    });
    wait_for_transcript_lines(&transcript_path, 3);

    // Two bursts back to back, each re-editing every 20ms (well under
    // the 30ms plain debounce, so only the 300ms max-wait cap can ever
    // fire it) for longer than the cap — the plain debounce alone
    // would never fire this, only `about_to_wait`'s per-tick check
    // honoring `DID_CHANGE_MAX_WAIT_MS`.
    let mut revision = 8u64;
    for _burst in 0..2 {
        let burst_deadline = Instant::now() + Duration::from_millis(360);
        while Instant::now() < burst_deadline {
            if let Some(doc) = app.model.editor_area.documents.get_mut(&doc_id) {
                doc.revision = revision;
            }
            app.process_cmd(Cmd::LspScheduleDidChange {
                document_id: doc_id,
                revision,
            });
            revision += 1;
            app.check_lsp_did_change_deadlines(&HashMap::new());
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    // Drain whatever's left pending from the tail of the second burst.
    let flush_deadline = Instant::now() + Duration::from_secs(1);
    while app.lsp_change_deadlines.is_pending(doc_id) && Instant::now() < flush_deadline {
        std::thread::sleep(Duration::from_millis(10));
        app.check_lsp_did_change_deadlines(&HashMap::new());
    }

    let lines = read_transcript_lines(&transcript_path);
    let did_change_versions: Vec<i64> = lines
        .iter()
        .filter(|l| l.starts_with("notify:textDocument/didChange"))
        .filter_map(|l| {
            let marker = "version=Some(Number(";
            let start = l.find(marker)? + marker.len();
            let rest = &l[start..];
            let end = rest.find(')')?;
            rest[..end].parse::<i64>().ok()
        })
        .collect();
    assert!(
        did_change_versions.len() >= 2,
        "the max-wait cap must fire more than once across two 360ms bursts, got {lines:?}"
    );
    for pair in did_change_versions.windows(2) {
        assert!(
            pair[1] > pair[0],
            "on-wire didChange versions must strictly increase, got {did_change_versions:?}"
        );
    }

    app.process_cmd(Cmd::Quit);
}

/// End-to-end "go to definition into an unopened file" (design doc's
/// Testing Strategy fake-server scenario): the fake server responds
/// to `textDocument/definition` with a location in a file that was
/// never opened; `GotoDefinition` must open it in a new tab, reusing
/// none, and place the cursor at the resolved position.
#[test]
fn goto_definition_into_an_unopened_file_opens_it_and_places_the_cursor() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let target_path = dir.path().join("target.rs");
    std::fs::write(&target_path, "one\ntwo\nthree\n").expect("write target fixture file");
    let target_uri = token::lsp::path_to_uri(&target_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "definitionProvider": true }
            }},
            { "op": "expect_request", "method": "textDocument/definition", "respond": [{
                "uri": target_uri.as_str(),
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 1, "character": 3 },
                },
            }]},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    // Open main.rs through the real worker and complete its deferred effects.
    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    assert_eq!(
        app.model.document().file_path.as_deref(),
        Some(file_path.as_path())
    );

    let server_id = LspServerId::from("rust-analyzer");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.process_async_messages();
        if app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready) {
            break;
        }
        assert!(Instant::now() < deadline, "server never reached Ready");
        std::thread::sleep(Duration::from_millis(20));
    }

    app.process_automation_msg(Msg::Lsp(LspMsg::GotoDefinition));

    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.process_async_messages();
        // The definition opened the server's canonical URI spelling. Unknown
        // alternate spellings need worker resolution, not a querying syscall.
        if app
            .model
            .editor_area
            .find_open_file(&token::lsp::uri_to_path(&target_uri).unwrap())
            .is_some()
        {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "definition response never opened the target file"
        );
        std::thread::sleep(Duration::from_millis(20));
    }

    // Compare canonicalized: macOS's /tmp -> /private/tmp symlink
    // means the URI round trip resolves to a different (but
    // equivalent) path string than the raw `tempdir()` path.
    assert_eq!(
        app.model
            .document()
            .file_path
            .as_deref()
            .map(|p| std::fs::canonicalize(p).unwrap()),
        Some(std::fs::canonicalize(&target_path).unwrap())
    );
    assert_eq!(app.model.editor().cursors[0].line, 1);
    assert_eq!(app.model.editor().cursors[0].column, 0);
    // The jump away from main.rs must be recorded for `NavigateBack`.
    assert_eq!(app.model.jump_history.len(), 1);
    assert_eq!(app.model.jump_history[0].path, file_path);

    app.process_cmd(Cmd::Quit);
}

/// The flush-before-request invariant as `request_lsp_definition` and
/// `request_lsp_hover` themselves apply it — not via a direct
/// `flush_lsp_did_change` call — asserted on the fake server's
/// receipt-order transcript (design doc: "any `textDocument/*`
/// request first flushes the document's pending `didChange`"). A
/// pending debounced edit is left in place (never manually flushed)
/// right before each request message; if the flush call were ever
/// dropped from either request function (as happened to the old
/// `flush_before_request` helper), the request frame would land on
/// the wire *before* the edit and this test would catch it.
#[test]
fn goto_definition_and_hover_flush_a_pending_did_change_ahead_of_their_request() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let transcript_path = dir.path().join("transcript.log");

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": {
                    "textDocumentSync": { "openClose": true, "change": 1 },
                    "definitionProvider": true,
                    "hoverProvider": true,
                }
            }},
            { "op": "record_until_exit", "file": transcript_path.to_string_lossy() },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_automation_msg(Msg::Layout(LayoutMsg::OpenFileInNewTab(file_path.clone())));
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| !app
        .model
        .ui
        .is_loading));
    let server_id = LspServerId::from("rust-analyzer");
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.process_async_messages();
        if app.model.lsp.servers.get(&server_id) == Some(&ServerState::Ready) {
            break;
        }
        assert!(Instant::now() < deadline, "server never reached Ready");
        std::thread::sleep(Duration::from_millis(20));
    }
    let doc_id = app.model.document().id.unwrap();
    wait_for_transcript_lines(&transcript_path, 3); // initialize, initialized, didOpen

    // Leave a debounced didChange pending — never flushed by hand —
    // then immediately request a definition. Only
    // `request_lsp_definition`'s own flush can put the didChange on
    // the wire ahead of the request.
    if let Some(doc) = app.model.editor_area.documents.get_mut(&doc_id) {
        doc.revision = 9;
    }
    app.process_cmd(Cmd::LspScheduleDidChange {
        document_id: doc_id,
        revision: 9,
    });
    assert!(app.lsp_change_deadlines.is_pending(doc_id));
    app.process_automation_msg(Msg::Lsp(LspMsg::GotoDefinition));
    assert!(
        !app.lsp_change_deadlines.is_pending(doc_id),
        "request_lsp_definition must flush the pending didChange itself"
    );

    let lines = wait_for_transcript_lines(&transcript_path, 5);
    let change_idx = lines
        .iter()
        .position(|l| l.starts_with("notify:textDocument/didChange"))
        .expect("didChange must reach the wire");
    let definition_idx = lines
        .iter()
        .position(|l| l.starts_with("request:textDocument/definition"))
        .expect("definition request must reach the wire");
    assert!(
        change_idx < definition_idx,
        "the flushed didChange must land ahead of the definition request, got {lines:?}"
    );
    assert!(
        lines[change_idx].contains("version=Some(Number(9))"),
        "expected the pending revision on the wire, got {lines:?}"
    );

    // Same invariant for hover, on a fresh pending edit.
    if let Some(doc) = app.model.editor_area.documents.get_mut(&doc_id) {
        doc.revision = 10;
    }
    app.process_cmd(Cmd::LspScheduleDidChange {
        document_id: doc_id,
        revision: 10,
    });
    assert!(app.lsp_change_deadlines.is_pending(doc_id));
    app.process_automation_msg(Msg::Lsp(LspMsg::ShowHover));
    assert!(
        !app.lsp_change_deadlines.is_pending(doc_id),
        "request_lsp_hover must flush the pending didChange itself"
    );

    let lines = wait_for_transcript_lines(&transcript_path, 7);
    let second_change_idx = lines
        .iter()
        .rposition(|l| l.starts_with("notify:textDocument/didChange"))
        .expect("second didChange must reach the wire");
    let hover_idx = lines
        .iter()
        .position(|l| l.starts_with("request:textDocument/hover"))
        .expect("hover request must reach the wire");
    assert!(
        second_change_idx < hover_idx,
        "the flushed didChange must land ahead of the hover request, got {lines:?}"
    );
    assert!(
        lines[second_change_idx].contains("version=Some(Number(10))"),
        "expected the pending revision on the wire, got {lines:?}"
    );

    app.process_cmd(Cmd::Quit);
}

/// A `publishDiagnostics` for a file with no open document is
/// retained in `LspManager`'s authoritative store (never dropped for
/// lack of a projection target) and applied to the document's
/// `diagnostics` projection the moment it's opened — the design
/// doc's "retains publishes for unopened files" rule.
#[test]
fn diagnostics_publish_for_an_unopened_file_is_retained_and_applied_on_open() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let uri = token::lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            // `textDocumentSync` present (a diagnostics-only server
            // still needs it for `didOpen`; a bare `{}` here would
            // mean "no sync messages at all", suppressing `didOpen`
            // and thus the projection pull this test asserts on).
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "textDocumentSync": { "openClose": true, "change": 1 } }
            }},
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": uri.as_str(),
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 2 } },
                    "severity": 1,
                    "message": "retained before open",
                }],
            }},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });

    // The publish arrives before any document is open — it must land
    // in the store without a target document to project onto.
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| app
        .lsp
        .diagnostics
        .contains_key(&uri)));

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);
    assert!(app
        .model
        .editor_area
        .documents
        .get(&doc_id)
        .unwrap()
        .diagnostics
        .is_empty());

    app.process_cmd(Cmd::LspDidOpen {
        document_id: doc_id,
        file_path,
        language: LanguageId::Rust,
    });

    let projected = &app
        .model
        .editor_area
        .documents
        .get(&doc_id)
        .unwrap()
        .diagnostics;
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].message, "retained before open");

    app.process_cmd(Cmd::Quit);
}

/// An out-of-order (older-`version`) `publishDiagnostics` for a URI
/// must not clobber a newer one already applied — the design doc's
/// "version used only to discard out-of-order publishes" rule.
#[test]
fn stale_version_diagnostics_publish_is_dropped() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");
    let uri = token::lsp::path_to_uri(&file_path);

    let scenario_path = dir.path().join("scenario.json");
    std::fs::write(
        &scenario_path,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": { "capabilities": {} } },
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": uri.as_str(),
                "version": 2,
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 2 } },
                    "severity": 1,
                    "message": "newer",
                }],
            }},
            { "op": "notify", "method": "textDocument/publishDiagnostics", "params": {
                "uri": uri.as_str(),
                "version": 1,
                "diagnostics": [{
                    "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 2 } },
                    "severity": 1,
                    "message": "stale",
                }],
            }},
            { "op": "sleep_ms", "ms": 60000 },
        ])
        .to_string(),
    )
    .expect("write scenario file");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_path.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });

    // The document only needs to be present in the model for
    // `update_lsp` to find and project onto it — no `didOpen`
    // required for this test (projection doesn't gate on it).
    assert!(pump_until(&mut app, Duration::from_secs(5), |app| {
        app.model
            .editor_area
            .documents
            .get(&doc_id)
            .is_some_and(|d| d.diagnostics.iter().any(|d| d.message == "newer"))
    }));
    // Give the (already-sent) stale publish a moment to be drained
    // too, so a regression that applies it wouldn't race the assert.
    std::thread::sleep(Duration::from_millis(200));
    app.process_async_messages();

    let projected = &app
        .model
        .editor_area
        .documents
        .get(&doc_id)
        .unwrap()
        .diagnostics;
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].message, "newer");
    assert_eq!(app.lsp.diagnostics_versions.get(&uri).copied(), Some(2));

    app.process_cmd(Cmd::Quit);
}

/// Pumps `process_async_messages` until `cond` holds or `timeout`
/// elapses — worker `Msg`s arrive from a real subprocess
/// asynchronously, same as `wait_for_transcript_lines`.
fn pump_until(app: &mut App, timeout: Duration, cond: impl Fn(&App) -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
        app.process_async_messages();
        if cond(app) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn crash_restart_resyncs_previously_open_documents() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    let file_path = dir.path().join("main.rs");
    std::fs::write(&file_path, "fn main() {}\n").expect("write fixture file");

    // First incarnation: answers `initialize`, then exits (crash).
    let scenario_a = dir.path().join("scenario_a.json");
    std::fs::write(
        &scenario_a,
        serde_json::json!([
            { "op": "expect_request", "method": "initialize", "respond": {
                "capabilities": { "textDocumentSync": { "openClose": true, "change": 1 } }
            }},
            { "op": "exit", "code": 1 },
        ])
        .to_string(),
    )
    .unwrap();

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_a.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    app.model.editor_area.documents.insert(doc_id, doc);

    app.process_cmd(Cmd::LspEnsureServer {
        language: LanguageId::Rust,
        file_path: file_path.clone(),
    });
    app.process_cmd(Cmd::LspDidOpen {
        document_id: doc_id,
        file_path: file_path.clone(),
        language: LanguageId::Rust,
    });
    assert!(
        pump_until(&mut app, Duration::from_secs(5), |app| app
            .lsp
            .open_documents
            .contains_key(&doc_id)),
        "didOpen should have registered the document against the first incarnation"
    );

    // Now point the (still-live) config override at a second scenario
    // that records everything it receives — the crash-restart below
    // will spawn a fresh process with *these* args.
    let transcript_b = dir.path().join("transcript_b.log");
    let scenario_b = dir.path().join("scenario_b.json");
    std::fs::write(
        &scenario_b,
        serde_json::json!([{
            "op": "record_until_exit",
            "file": transcript_b.to_string_lossy(),
        }])
        .to_string(),
    )
    .unwrap();
    app.model.config.lsp.servers.insert(
        "rust-analyzer".to_owned(),
        token::config::LspServerOverride {
            command: Some(fake_lsp_server_path().to_string_lossy().into_owned()),
            args: Some(vec![scenario_b.to_string_lossy().into_owned()]),
            enabled: None,
            initialization_options: None,
            settings: None,
        },
    );

    // Wait for the crash (`ServerExited`) to be processed and a
    // restart scheduled (backoff), then fire it immediately instead
    // of waiting out the real delay.
    assert!(
        pump_until(&mut app, Duration::from_secs(5), |app| !app
            .lsp
            .restart_deadlines
            .is_empty()),
        "a crash should schedule a backoff restart"
    );
    for deadline in app.lsp.restart_deadlines.values_mut() {
        *deadline = Instant::now();
    }
    app.check_lsp_restart_deadlines();

    // The new incarnation reaches Ready and re-`didOpen`s the
    // document (design doc: "after any restart, didOpen is re-sent
    // for every currently-open matching document").
    assert!(
        pump_until(&mut app, Duration::from_secs(5), |_| {
            read_transcript_lines(&transcript_b)
                .iter()
                .any(|l| l.starts_with("notify:textDocument/didOpen"))
        }),
        "expected the restarted server to receive a re-sent didOpen"
    );
    let lines = read_transcript_lines(&transcript_b);
    assert!(lines[0].starts_with("request:initialize"));

    app.process_cmd(Cmd::Quit);
}

/// A document opened through the out-of-root route hint (a
/// definition jump into e.g. a registry crate) is tracked under the
/// *resolving* server/root, not the root its own file path would
/// naturally resolve to (it has no project markers of its own under
/// test, and lives in a directory with no LSP handle at all).
/// `resync_open_documents` must re-`didOpen` it against the *stored*
/// `(server_id, root)` — re-resolving from `file_path`/`language`
/// (the pre-fix behavior) would derive the file's own directory,
/// find no handle there, and silently send nothing.
#[test]
fn resync_uses_the_stored_server_and_root_not_a_re_resolved_one() {
    let dir = tempfile::tempdir().expect("temp dir should be created");
    // The document's own directory: no project markers, no LSP
    // handle ever spawned here — this is what the buggy re-resolve
    // would have picked.
    let registry_dir = dir.path().join("registry_pkg");
    std::fs::create_dir_all(&registry_dir).unwrap();
    let file_path = registry_dir.join("lib.rs");
    std::fs::write(&file_path, "pub fn util() {}\n").unwrap();

    // The root the document was actually opened against, via the
    // route hint — a live handle only exists here.
    let resolving_root = dir.path().join("resolving_root");
    std::fs::create_dir_all(&resolving_root).unwrap();
    let transcript_path = dir.path().join("transcript.log");

    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    configure_fake_rust_analyzer(&mut app, dir.path(), &transcript_path);

    let server_id = LspServerId::from("rust-analyzer");
    let def = lsp::server_def_by_id(&server_id.0).unwrap();
    let resolved = lsp::resolve_server(def, &app.model.config.lsp).unwrap();
    app.spawn_lsp_server_at(&resolved, &resolving_root);

    let doc_id = token::model::editor_area::DocumentId(1);
    let mut doc = token::model::Document::from_file(file_path.clone()).unwrap();
    doc.id = Some(doc_id);
    let revision = doc.revision;
    app.model.editor_area.documents.insert(doc_id, doc);

    // Mirrors what `Cmd::LspDidOpenOnServer` installs for a route-hint
    // open — tracked under `resolving_root`, never the file's own.
    app.lsp.open_documents.insert(
        doc_id,
        OpenDocState {
            server_id: server_id.clone(),
            root: resolving_root.clone(),
            uri: lsp::path_to_uri(&file_path),
            synced_revision: revision,
        },
    );

    app.resync_open_documents(&server_id, &resolving_root);

    let lines = wait_for_transcript_lines(&transcript_path, 3);
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("notify:textDocument/didOpen")),
        "expected a re-sent didOpen against the stored (resolving) root, got {lines:?}"
    );

    app.process_cmd(Cmd::Quit);
}

// ========================================================================
// Context menu (context-menu.md)
// ========================================================================

/// Shift+F10 → `Command::ShowContextMenu`'s special case in
/// `dispatch_command`: opens the editor menu at the caret (no fake LSP
/// server needed — no LSP items are enabled with none configured),
/// visible in the automation snapshot, then Down/Enter navigates and
/// activates a targeted `Messages` action end to end.
#[test]
fn show_context_menu_opens_navigates_and_activates_via_the_real_dispatch_path() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    app.model.document_mut().buffer = ropey::Rope::from("hello world\n");
    app.model.editor_mut().clear_selection();

    let cmd = app.dispatch_command(Command::ShowContextMenu);
    assert!(cmd.is_some(), "opening the menu produces a redraw");
    assert!(app.model.ui.cursor_overlay.is_some());

    let snapshot = crate::automation::EditorSnapshot::from_model(&app.model);
    let menu = snapshot
        .context_menu
        .expect("context menu should be in the automation snapshot");
    assert_eq!(menu.region, "editor");
    let cut = menu
        .rows
        .iter()
        .find(|r| r.label == "Cut")
        .expect("Cut row present");
    assert!(!cut.enabled, "no selection: Cut is disabled");

    // Selection opened on the first enabled row, not "Cut" (disabled).
    let first_enabled_label = menu.rows.iter().find(|r| r.enabled).unwrap().label.clone();
    assert_ne!(first_enabled_label, "Cut");

    // Navigate to "Show Hover" (always enabled) and activate it.
    let show_hover_index = menu
        .rows
        .iter()
        .position(|r| r.label == "Show Hover")
        .expect("Show Hover row present");
    for _ in 0..show_hover_index {
        let modifiers = KeyModifiers::default();
        let result =
            handle_cursor_overlay_key(&mut app.model, &Key::Named(NamedKey::ArrowDown), modifiers);
        assert!(result.is_some());
    }
    assert_eq!(
        app.model.ui.cursor_overlay.unwrap().selected,
        show_hover_index
    );

    let result = handle_cursor_overlay_key(
        &mut app.model,
        &Key::Named(NamedKey::Enter),
        KeyModifiers::default(),
    );
    assert!(result.is_some(), "Enter activates and is consumed");
    assert!(
        app.model.ui.cursor_overlay.is_none(),
        "activation dismisses the menu"
    );
    assert!(app.model.ui.context_menu.is_none());
}

#[test]
fn right_click_on_a_tab_targets_the_clicked_tab_not_the_focused_one() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let group_id = app.model.editor_area.focused_group_id;
    // Open a second tab so there's a non-focused one to right-click.
    update(&mut app.model, Msg::Layout(LayoutMsg::NewTab));
    let tabs: Vec<token::model::TabId> = app
        .model
        .editor_area
        .groups
        .get(&group_id)
        .unwrap()
        .tabs
        .iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(tabs.len(), 2);
    let clicked_tab = tabs[0];
    let focused_tab = app
        .model
        .editor_area
        .groups
        .get(&group_id)
        .unwrap()
        .active_tab()
        .unwrap()
        .id;
    assert_ne!(
        clicked_tab, focused_tab,
        "tab 0 isn't the newly-focused tab"
    );

    update(
        &mut app.model,
        Msg::ContextMenu(token::messages::ContextMenuMsg::Open {
            target: token::context_menu::ContextMenuTarget::Tab {
                group_id,
                tab_id: clicked_tab,
                file_path: None,
            },
            anchor: (0, 0, 0),
        }),
    );

    let close_row_index = app
        .model
        .ui
        .context_menu
        .as_ref()
        .unwrap()
        .items
        .iter()
        .position(|i| i.label == "Close")
        .unwrap();
    update(
        &mut app.model,
        Msg::ContextMenu(token::messages::ContextMenuMsg::ActivateItem {
            index: close_row_index,
        }),
    );

    // The clicked tab closed; the tab that was focused when the menu
    // opened is still present.
    let remaining: Vec<token::model::TabId> = app
        .model
        .editor_area
        .groups
        .get(&group_id)
        .unwrap()
        .tabs
        .iter()
        .map(|t| t.id)
        .collect();
    assert_eq!(remaining, vec![focused_tab]);
}

#[cfg(test)]
mod mouse_wheel_tests {
    use crate::runtime::app::ScrollAccumulator;
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;

    #[test]
    fn horizontal_and_vertical_line_delta_negate_symmetrically() {
        let mut accum = ScrollAccumulator::default();
        let (h, v) = accum.deltas(MouseScrollDelta::LineDelta(1.0, 1.0), 8.0, 16.0);
        // Regression test: horizontal scroll used to pass `x` through
        // unnegated while vertical negated `y`, inverting horizontal scroll
        // direction relative to vertical (and relative to the "positive
        // delta reveals further content" convention both axes share in the
        // model layer). Both axes must now negate the same way.
        assert_eq!(h, -3);
        assert_eq!(v, -3);
    }

    #[test]
    fn horizontal_and_vertical_pixel_delta_negate_symmetrically() {
        let mut accum = ScrollAccumulator::default();
        let (h, v) = accum.deltas(
            MouseScrollDelta::PixelDelta(PhysicalPosition::new(16.0, 32.0)),
            8.0,
            16.0,
        );
        assert_eq!(h, -2);
        assert_eq!(v, -2);
    }

    #[test]
    fn sub_line_pixel_deltas_accumulate_instead_of_truncating_to_zero() {
        let mut accum = ScrollAccumulator::default();
        // Each event moves less than a full line (16px); previously every one
        // truncated to 0 and the motion was lost entirely.
        let mut emitted = 0;
        for _ in 0..4 {
            let (_, v) = accum.deltas(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 6.0)),
                8.0,
                16.0,
            );
            emitted += v;
        }
        // 4 × 6px = 24px ≈ 1.5 lines → one line emitted, remainder carried.
        assert_eq!(emitted, -1);
    }
}

/// A signature help request arms its slot like hover; the deadline sweep
/// abandons it server-side and — unlike hover — says nothing (no status
/// transient, no float).
#[test]
fn a_signature_help_request_arms_the_slot_and_its_sweep_clears_it_silently() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-sig-timeout");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-sig-timeout/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);
    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
        signature_help_provider: Some(lsp_types::SignatureHelpOptions::default()),
        ..Default::default()
    });
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_signature_help(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        test_cursor(&app),
        revision,
        Some("(".to_owned()),
        false,
    );
    let key = app.lsp.signature_help.by_doc.get(&doc_id).cloned().unwrap();
    assert!(app.lsp.signature_help.deadlines.contains_key(&key));

    app.lsp
        .signature_help
        .deadlines
        .insert(key.clone(), Instant::now() - Duration::from_secs(1));
    let status_before = app
        .model
        .ui
        .transient_message
        .as_ref()
        .map(|t| t.text.clone());
    app.check_lsp_signature_help_deadlines();

    assert!(app.lsp.signature_help.requests.is_empty());
    assert!(app.lsp.signature_help.by_doc.is_empty());
    assert!(app.lsp.signature_help.deadlines.is_empty());
    assert_eq!(
        app.model
            .ui
            .transient_message
            .as_ref()
            .map(|t| t.text.clone()),
        status_before,
        "silent sweep: no status transient"
    );
    assert!(app.model.ui.signature_help.is_none());
    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(key.2)
            .unwrap()
            .abandoned
    );
    handle.kill();
}

fn rename_capable_app(
    prepare_provider: Option<bool>,
    root: &str,
) -> (
    App,
    token::model::editor_area::DocumentId,
    LspServerId,
    PathBuf,
) {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from(root);
    let uri = lsp::path_to_uri(&root.join("main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);
    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
        rename_provider: Some(lsp_types::OneOf::Right(lsp_types::RenameOptions {
            prepare_provider,
            work_done_progress_options: Default::default(),
        })),
        ..Default::default()
    });
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);
    (app, doc_id, server_id, root)
}

/// A prepareRename request arms its slot; past its deadline it is
/// abandoned server-side and the status bar says so.
#[test]
fn a_prepare_rename_request_arms_the_slot_and_its_timeout_flashes_a_status() {
    let (mut app, doc_id, server_id, root) =
        rename_capable_app(Some(true), "/tmp/proj-rename-timeout");
    let revision = app.model.document().revision;

    app.request_lsp_prepare_rename(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        test_cursor(&app),
        revision,
        "main".to_owned(),
    );
    let key = app.lsp.prepare_rename.by_doc.get(&doc_id).cloned().unwrap();
    assert!(app.lsp.prepare_rename.deadlines.contains_key(&key));
    assert!(app.model.ui.active_modal.is_none(), "waits for the reply");

    app.lsp
        .prepare_rename
        .deadlines
        .insert(key.clone(), Instant::now() - Duration::from_secs(1));
    app.check_lsp_rename_deadlines();

    assert!(app.lsp.prepare_rename.requests.is_empty());
    assert_eq!(
        app.model
            .ui
            .transient_message
            .as_ref()
            .map(|t| t.text.as_str()),
        Some("Rename: server did not answer")
    );
    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(key.2)
            .unwrap()
            .abandoned
    );
    handle.kill();
}

/// A server with `renameProvider` but no `prepareProvider` skips the
/// round trip: the prompt opens right away with the caret word.
#[test]
fn a_server_without_prepare_rename_opens_the_prompt_directly() {
    let (mut app, doc_id, server_id, root) = rename_capable_app(None, "/tmp/proj-rename-noprepare");
    let revision = app.model.document().revision;

    app.request_lsp_prepare_rename(
        doc_id,
        lsp_types::Position {
            line: 0,
            character: 0,
        },
        test_cursor(&app),
        revision,
        "main".to_owned(),
    );

    assert!(app.lsp.prepare_rename.requests.is_empty());
    let Some(token::model::ModalState::RenameSymbol(state)) = &app.model.ui.active_modal else {
        panic!(
            "expected the rename prompt, got {:?}",
            app.model.ui.active_modal
        );
    };
    assert_eq!(state.input(), "main");
    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

/// A `Range` prepareRename reply has no placeholder text of its own — the
/// interception pass reads it from the document buffer.
#[test]
fn a_range_prepare_rename_reply_reads_the_placeholder_from_the_buffer() {
    let (mut app, doc_id, server_id, root) =
        rename_capable_app(Some(true), "/tmp/proj-rename-range");
    app.process_automation_msg(Msg::Document(token::messages::DocumentMsg::InsertText(
        "fn main() {}".to_owned(),
    )));
    let revision = app.model.document().revision;
    let cursor = test_cursor(&app);
    let request_id = app
        .lsp
        .servers
        .get(&(server_id.clone(), root.clone()))
        .unwrap()
        .begin_request("textDocument/prepareRename", serde_json::json!({}));
    let key = (server_id.clone(), root.clone(), request_id);
    app.lsp.prepare_rename.insert(
        key.clone(),
        doc_id,
        PendingPrepareRename {
            document_id: doc_id,
            revision,
            cursor,
            fallback: String::new(),
        },
    );

    let out =
        app.intercept_rename_replies(vec![Msg::Lsp(LspMsg::PrepareRenameResponseFromServer {
            server_id: server_id.clone(),
            root: root.clone(),
            request_id,
            response: Some(lsp_types::PrepareRenameResponse::Range(
                lsp_types::Range::new(
                    lsp_types::Position::new(0, 3),
                    lsp_types::Position::new(0, 7),
                ),
            )),
            abandoned: false,
        })]);

    assert!(matches!(
        &out[..],
        [Msg::Lsp(LspMsg::PrepareRenameResolved { placeholder: Some(p), .. })] if p == "main"
    ));
    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    handle.kill();
}

/// `Cmd::LspExecuteCommand` sends `workspace/executeCommand` to the server
/// that owns the document, tracked in its pending map like any request.
#[test]
fn execute_command_sends_workspace_execute_command_to_the_documents_server() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let doc_id = app.model.document().id.unwrap();
    let server_id = LspServerId::from("rust-analyzer");
    let root = PathBuf::from("/tmp/proj-exec");
    let uri = lsp::path_to_uri(&PathBuf::from("/tmp/proj-exec/main.rs"));
    install_open_document(&mut app, doc_id, &server_id, &root, uri);
    let handle = spawn_fake_handle(&server_id);
    let probe_id = handle.pending.lock().unwrap().begin("probe");
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.execute_lsp_command(doc_id, "server.doIt".to_owned(), None);

    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    let entry = handle.pending.lock().unwrap().resolve(probe_id + 1);
    assert_eq!(
        entry.map(|e| e.method).as_deref(),
        Some("workspace/executeCommand")
    );
    handle.kill();
}

/// A `format_on_save` formatting request arms the short deadline; the
/// sweep resolves it with no edits so the save still happens.
#[test]
fn a_then_save_formatting_request_past_its_deadline_still_saves() {
    let mut app = App::new(800, 600, empty_startup_config(), None, None, None);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("main.rs");
    std::fs::write(&path, "fn main() {}\n").unwrap();
    app.model.document_mut().file_path = Some(path.clone());
    let doc_id = app.model.document().id.unwrap();
    let revision = app.model.document().revision;
    let server_id = LspServerId::from("rust-analyzer");
    let root = dir.path().to_path_buf();
    install_open_document(&mut app, doc_id, &server_id, &root, lsp::path_to_uri(&path));
    let handle = spawn_fake_handle(&server_id);
    *handle.capabilities.lock().unwrap() = Some(lsp_types::ServerCapabilities {
        document_formatting_provider: Some(lsp_types::OneOf::Left(true)),
        ..Default::default()
    });
    app.lsp
        .servers
        .insert((server_id.clone(), root.clone()), handle);

    app.request_lsp_formatting(
        doc_id,
        revision,
        None,
        lsp_types::FormattingOptions::default(),
        true,
    );
    let key = app.lsp.formatting.by_doc.get(&doc_id).cloned().unwrap();
    assert!(
        app.lsp.formatting.deadlines[&key] <= Instant::now() + FORMAT_ON_SAVE_TIMEOUT,
        "then_save arms the short deadline"
    );
    assert!(!app.model.ui.is_saving, "the save waits for the formatter");

    app.lsp
        .formatting
        .deadlines
        .insert(key.clone(), Instant::now() - Duration::from_secs(1));
    app.check_lsp_formatting_deadlines();

    assert!(app.lsp.formatting.requests.is_empty());
    assert!(
        app.model.ui.is_saving,
        "timeout falls back to an unformatted save"
    );
    assert!(app
        .model
        .ui
        .transient_message
        .as_ref()
        .is_some_and(|t| t.text.contains("saved unformatted")));
    let mut handle = app.lsp.servers.remove(&(server_id, root)).unwrap();
    assert!(
        handle
            .pending
            .lock()
            .unwrap()
            .resolve(key.2)
            .unwrap()
            .abandoned
    );
    handle.kill();
}
