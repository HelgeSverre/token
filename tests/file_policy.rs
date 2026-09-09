//! Policy I/O replies must follow their document and current resolution generation.
mod common;
use std::path::{Path, PathBuf};
use token::{
    commands::Cmd,
    editorconfig::{parse_layer, resolve_layers, PolicyRequest, ResolvedFilePolicy},
    messages::{AppMsg, DocumentMsg, Msg},
    model::{AppModel, LineEnding},
    update::update,
};

fn policy(path: &Path, text: &str) -> ResolvedFilePolicy {
    let config = path.parent().unwrap().join(".editorconfig");
    let mut result = resolve_layers(path.into(), vec![parse_layer(&config, text, path).unwrap()]);
    result.dependencies.push(config);
    result
}
fn model() -> AppModel {
    let mut model = common::test_model("text  ", 0, 4);
    let path = PathBuf::from("/project/source.txt");
    model.document_mut().file_path = Some(path.clone());
    let resolved = policy(&path, "root=true\n[*]\nindent_size=2\n");
    model.document_mut().file_text_preferences = resolved.preferences;
    model.document_mut().file_policy.enabled = true;
    model.document_mut().file_policy.install(path, resolved);
    model
}
fn find(command: &Cmd, predicate: fn(&Cmd) -> bool) -> Option<Cmd> {
    if predicate(command) {
        Some(command.clone())
    } else if let Cmd::Batch(commands) = command {
        commands.iter().find_map(|c| find(c, predicate))
    } else {
        None
    }
}
fn request(command: Cmd) -> PolicyRequest {
    let Cmd::ResolveFilePolicy(request) =
        find(&command, |c| matches!(c, Cmd::ResolveFilePolicy(_))).expect("policy request")
    else {
        unreachable!()
    };
    request
}
fn changed(model: &mut AppModel) -> PolicyRequest {
    request(
        update(
            model,
            Msg::App(AppMsg::FilesChanged(vec!["/project/.editorconfig".into()])),
        )
        .unwrap(),
    )
}
fn reply(model: &mut AppModel, request: PolicyRequest, text: &str) -> Option<Cmd> {
    let result = Ok(policy(&request.path, text));
    update(
        model,
        Msg::App(AppMsg::FilePolicyResolved { request, result }),
    )
}
fn save_as(model: &mut AppModel) -> PolicyRequest {
    let commands = update(model, Msg::App(AppMsg::SaveFileAs)).unwrap();
    let Cmd::ShowSaveFileDialog { target, .. } =
        find(&commands, |c| matches!(c, Cmd::ShowSaveFileDialog { .. })).unwrap()
    else {
        panic!("dialog")
    };
    request(
        update(
            model,
            Msg::App(AppMsg::SaveFileAsDialogResult {
                target,
                path: Some("/destination/new.txt".into()),
            }),
        )
        .unwrap(),
    )
}

#[test]
fn editorconfig_policy_replies_reject_old_generation_and_changed_source_identity() {
    let mut model = model();
    let old = changed(&mut model);
    let current = changed(&mut model);
    assert!(reply(&mut model, old, "[*]\nindent_size=8").is_none());
    reply(&mut model, current, "[*]\nindent_size=3");
    assert_eq!(model.document().text_settings.indent_size, 3);
    let old = changed(&mut model);
    model
        .document_mut()
        .set_file_identity(Some(token::util::FileIdentity::from_resolved(
            "/project/source.txt".into(),
            Path::new("/moved/source.txt"),
        )));
    let commands = reply(&mut model, old, "[*]\nindent_size=8").unwrap();
    let new = request(commands);
    assert_eq!(new.path, Path::new("/moved/source.txt"));
    assert_eq!(model.document().text_settings.indent_size, 3);
}

#[test]
fn editorconfig_manual_save_waits_for_policy_reload_then_cleans_current_text() {
    let mut model = model();
    let pending = changed(&mut model);
    let commands = update(&mut model, Msg::App(AppMsg::SaveFile)).unwrap();
    assert!(find(&commands, |c| matches!(c, Cmd::SaveFile { .. })).is_none());
    let commands = reply(
        &mut model,
        pending,
        "[*]\ntrim_trailing_whitespace=true\ninsert_final_newline=true\nend_of_line=cr\n",
    )
    .unwrap();
    let Cmd::SaveFile { content, .. } =
        find(&commands, |c| matches!(c, Cmd::SaveFile { .. })).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(content.to_string(), "text\r");
}

#[test]
fn editorconfig_save_as_policy_is_provisional_and_failure_keeps_cleanup_undoable() {
    let mut model = model();
    let request = save_as(&mut model);
    assert_eq!(model.document().text_settings.indent_size, 2);
    let commands=reply(&mut model,request,"[*]\nindent_size=8\nend_of_line=crlf\ntrim_trailing_whitespace=true\ninsert_final_newline=true\n").unwrap();
    assert_eq!(model.document().text_settings.indent_size, 2);
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = find(&commands, |c| matches!(c, Cmd::SaveFile { .. })).unwrap()
    else {
        unreachable!()
    };
    assert_eq!(content.to_string(), "text\r\n");
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
    assert_eq!(
        model.document().file_path.as_deref(),
        Some(Path::new("/project/source.txt"))
    );
    assert_eq!(model.document().text_settings.indent_size, 2);
    update(&mut model, Msg::Document(DocumentMsg::Undo));
    assert_eq!(model.document().buffer.to_string(), "text  ");
}

#[test]
fn editorconfig_save_as_success_installs_destination_and_disable_cancels_pending_resolution() {
    let mut model = model();
    let request = save_as(&mut model);
    let commands = reply(
        &mut model,
        request,
        "[*]\nindent_size=8\nend_of_line=crlf\n",
    )
    .unwrap();
    let Cmd::SaveFile {
        target,
        path,
        content,
    } = find(&commands, |c| matches!(c, Cmd::SaveFile { .. })).unwrap()
    else {
        unreachable!()
    };
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
    assert_eq!(model.document().text_settings.indent_size, 8);
    assert_eq!(model.document().line_ending(), LineEnding::Crlf);
    let request = save_as(&mut model);
    model.config.editorconfig = false;
    let command = update(&mut model, Msg::Document(DocumentMsg::Copy)).unwrap();
    assert!(find(&command, |c| matches!(c, Cmd::SaveFile { .. })).is_some());
    reply(&mut model, request, "[*]\nindent_size=2");
    assert_eq!(model.document().text_settings.indent_size, 4);
}

#[test]
fn editorconfig_reload_does_not_resume_an_automatic_save_for_an_older_revision() {
    let mut model = model();
    model.config.auto_save.mode = token::config::AutoSaveMode::AfterDelay;
    model.config.auto_save.format_on_save = true;
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('!')));
    let trigger = token::messages::AutoSaveRequest {
        document_id: model.document().id.unwrap(),
        revision: model.document().revision,
        path: model.document().file_path.clone().unwrap(),
        policy: model.config.auto_save.clone(),
        reason: token::model::SaveReason::Idle,
    };
    let commands = update(&mut model, Msg::App(AppMsg::AutoSave(trigger))).unwrap();
    assert!(find(&commands, |c| matches!(c, Cmd::LspRequestFormatting { .. })).is_some());
    let pending = changed(&mut model);
    update(&mut model, Msg::Document(DocumentMsg::InsertChar('?')));
    // A settings reload can disable the formatter while policy I/O is pending.
    model.config.lsp.enabled = false;
    let commands = reply(&mut model, pending, "[*]\ntrim_trailing_whitespace=true\n").unwrap();
    assert!(find(&commands, |c| matches!(c, Cmd::SaveFile { .. })).is_none());
    assert!(!model.document().save_busy());
    assert!(model.document().is_modified);
    assert_eq!(model.document().buffer.to_string(), "text!?  ");
}
