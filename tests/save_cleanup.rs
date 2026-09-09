//! Saved bytes, visible text and undo must describe the same cleanup transaction.
mod common;
use token::{
    commands::Cmd,
    messages::{AppMsg, DocumentMsg, Msg},
    model::{LineEnding, TextPreferences},
    update::update,
};

fn write(command: Cmd) -> Cmd {
    match command {
        Cmd::SaveFile { .. } => command,
        Cmd::Batch(commands) => commands
            .into_iter()
            .find_map(|cmd| {
                fn contains(cmd: &Cmd) -> bool {
                    match cmd {
                        Cmd::SaveFile { .. } => true,
                        Cmd::Batch(v) => v.iter().any(contains),
                        _ => false,
                    }
                }
                contains(&cmd).then(|| write(cmd))
            })
            .expect("write effect"),
        other => panic!("expected write, got {other:?}"),
    }
}

#[test]
fn save_cleanup_is_one_undoable_transaction_and_failed_write_preserves_it() {
    let mut model = common::test_model("héllo  \r\nworld\t", 1, 5);
    model.document_mut().file_path = Some("/fixture/file.txt".into());
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    let before = model.document().buffer.to_string();
    let history = model.document().undo_stack.len();
    model.config.text = TextPreferences {
        end_of_line: Some(LineEnding::Cr),
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(true),
        ..Default::default()
    };
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = write(update(&mut model, Msg::App(AppMsg::SaveFile)).unwrap())
    else {
        unreachable!()
    };
    assert_eq!(content.to_string(), "héllo\rworld!\r");
    assert_eq!(model.document().buffer, content);
    assert_eq!(model.document().undo_stack.len(), history + 1);
    update(
        &mut model,
        Msg::App(AppMsg::SaveCompleted {
            target,
            path,
            content,
            identity: None,
            result: Err("read only".into()),
        }),
    );
    assert!(model.document().is_modified);
    assert!(model.document().save_error.is_some());
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), before);
    assert!(model.document().is_modified);
}

#[test]
fn save_cleanup_success_undo_and_noop_save_keep_saved_snapshot_and_history_honest() {
    let mut model = common::test_model("a \r\nb \r\n", 1, 1);
    model.document_mut().file_path = Some("/fixture/file.txt".into());
    model.config.text = TextPreferences {
        end_of_line: Some(LineEnding::Lf),
        trim_trailing_whitespace: Some(true),
        insert_final_newline: Some(false),
        ..Default::default()
    };
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = write(update(&mut model, Msg::App(AppMsg::SaveFile)).unwrap())
    else {
        unreachable!()
    };
    assert_eq!(content.to_string(), "a\nb");
    update(
        &mut model,
        Msg::App(AppMsg::SaveCompleted {
            target,
            path,
            content,
            identity: None,
            result: Ok(()),
        }),
    );
    assert!(!model.document().is_modified);
    let history = model.document().undo_stack.len();
    write(update(&mut model, Msg::App(AppMsg::SaveFile)).unwrap());
    assert_eq!(model.document().undo_stack.len(), history);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "a \r\nb \r\n");
    assert!(model.document().is_modified);
}

#[test]
fn save_cleanup_cr_only_display_lsp_coordinates_enter_and_delete_agree() {
    let mut model = common::test_model("a🦀\rb\rc", 0, 2);
    assert_eq!(model.document().line_count(), 3);
    assert_eq!(model.document().line_length(0), 2);
    assert_eq!(model.document().get_line_cow(0).as_deref(), Some("a🦀"));
    let position = token::model::Position::new(1, 1);
    let lsp = token::lsp::position_to_lsp(model.document(), position);
    assert_eq!(token::lsp::lsp_to_position(model.document(), lsp), position);
    update(&mut model, Msg::Document(DocumentMsg::InsertNewline));
    assert_eq!(model.document().buffer.to_string(), "a🦀\r\rb\rc");
    assert_eq!(model.editor().primary_cursor().line, 1);
    update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
    assert_eq!(model.document().buffer.to_string(), "a🦀\rb\rc");
}

#[test]
fn save_cleanup_line_iterator_preserves_mixed_endings() {
    let text = "a\r\nb\rc\nd";
    let lines: Vec<_> = token::util::text::lines_with_endings(text).collect();
    assert_eq!(lines, ["a\r\n", "b\r", "c\n", "d"]);
    assert_eq!(lines.concat(), text);
    assert_eq!(
        token::util::text::lines_with_endings("\r\r").collect::<Vec<_>>(),
        ["\r", "\r"]
    );
    assert_eq!(token::util::text::lines_with_endings("").count(), 0);
}
