//! Large-file display searches are snapshot-bound effects, never render-time scans.
use crate::common;

use std::sync::Arc;
use token::messages::{DocumentMsg, ModalMsg, Msg, UiMsg};
use token::model::ui::{FindSearchRequest, FindStatus};
use token::model::{AppModel, FindReplaceState, ModalState};
use token::update::update;
use token::Cmd;

fn request(cmd: &Cmd) -> Option<Arc<FindSearchRequest>> {
    match cmd {
        Cmd::RunFindSearch(request) => Some(Arc::clone(request)),
        Cmd::Batch(cmds) => cmds.iter().find_map(request),
        _ => None,
    }
}

fn fixture() -> (AppModel, Arc<FindSearchRequest>) {
    let mut model = common::test_model(&"foo ".repeat(70_000), 0, 0);
    let mut state = FindReplaceState::default();
    state.set_query("foo");
    state.case_sensitive = true;
    model.ui.open_modal(ModalState::FindReplace(state));
    let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor)).unwrap();
    let request = request(&cmd).expect("large-file search effect");
    (model, request)
}

fn state(model: &AppModel) -> &FindReplaceState {
    match model.ui.active_modal.as_ref().unwrap() {
        ModalState::FindReplace(state) => state,
        _ => panic!("expected Find modal"),
    }
}

fn complete(model: &mut AppModel, request: Arc<FindSearchRequest>) {
    let results = request.compute();
    update(
        model,
        Msg::Ui(UiMsg::FindSearchCompleted {
            request,
            result: Ok(results),
        }),
    );
}

#[test]
fn find_async_schedules_once_and_publishes_current_count() {
    let (mut model, first) = fixture();
    assert_eq!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Searching)
    );
    let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(cmd.as_ref().and_then(request).is_none());
    complete(&mut model, first);
    assert_eq!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Count {
            total: 70_000,
            current: None
        })
    );
}

#[test]
fn find_async_rejects_old_query_edit_and_same_revision_buffer_replies() {
    for change in 0..3 {
        let (mut model, old) = fixture();
        match change {
            0 => {
                if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
                    state.set_query("bar");
                }
            }
            1 => {
                update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
            }
            2 => {
                model.document_mut().buffer = "bar ".repeat(70_000).into();
            }
            _ => unreachable!(),
        }
        let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
        // The document edit's own postlude may already have scheduled the new job.
        if change != 1 {
            assert!(cmd.as_ref().and_then(request).is_some());
        }
        complete(&mut model, old);
        assert_eq!(
            state(&model).status(model.document(), &model.editor().selections[0]),
            Some(FindStatus::Searching)
        );
    }
}

#[test]
fn find_async_close_reopen_gives_the_new_session_its_own_request() {
    let (mut model, old) = fixture();
    // Find actions remember the query; a bare Close has never persisted it.
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::FindNext)));
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::Close)));
    complete(&mut model, Arc::clone(&old));
    assert!(model.ui.active_modal.is_none());
    let cmd = update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::OpenFindReplace))).unwrap();
    let fresh = request(&cmd).unwrap();
    assert!(!Arc::ptr_eq(&old, &fresh));
    complete(&mut model, old);
    assert_eq!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Searching)
    );
    complete(&mut model, fresh);
    assert!(matches!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Count { total: 70_000, .. })
    ));
}

#[test]
fn find_async_rejects_mismatched_result_ownership_and_reports_worker_failure() {
    let (mut model, pending) = fixture();
    let (_, other) = fixture();
    update(
        &mut model,
        Msg::Ui(UiMsg::FindSearchCompleted {
            request: Arc::clone(&pending),
            result: Ok(other.compute()),
        }),
    );
    assert_eq!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Searching)
    );
    update(
        &mut model,
        Msg::Ui(UiMsg::FindSearchCompleted {
            request: pending,
            result: Err("worker unavailable".into()),
        }),
    );
    assert!(matches!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Unavailable(_))
    ));
    let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(
        cmd.as_ref().and_then(request).is_none(),
        "do not retry a failed worker on every blink"
    );
}

#[test]
fn find_async_explicit_replace_uses_fresh_matches_while_display_is_pending() {
    let (mut model, stale) = fixture();
    if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
        state.set_query("absent");
        state.set_replacement("oops");
    }
    update(&mut model, Msg::Ui(UiMsg::Modal(ModalMsg::ReplaceAll)));
    assert!(model.document().undo_stack.is_empty());
    complete(&mut model, stale);
    assert_eq!(model.document().buffer.to_string(), "foo ".repeat(70_000));
}

#[test]
fn find_async_options_and_selection_scope_invalidate_pending_results() {
    for change in 0..4 {
        let (mut model, old) = fixture();
        if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
            match change {
                0 => state.case_sensitive = false,
                1 => state.whole_word = true,
                2 => state.use_regex = true,
                3 => {
                    state.selection_only = true;
                    state.scope = Some((0, 3));
                }
                _ => unreachable!(),
            }
        }
        let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor)).unwrap();
        let fresh = request(&cmd).unwrap();
        complete(&mut model, old);
        assert_eq!(
            state(&model).status(model.document(), &model.editor().selections[0]),
            Some(FindStatus::Searching)
        );
        complete(&mut model, fresh);
        let total = if change == 3 { 1 } else { 70_000 };
        assert_eq!(
            state(&model).status(model.document(), &model.editor().selections[0]),
            Some(FindStatus::Count {
                total,
                current: None
            })
        );
    }
}

#[test]
fn find_async_regex_errors_are_distinct_from_pending_and_small_files_stay_immediate() {
    let (mut model, _) = fixture();
    if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
        state.use_regex = true;
        state.set_query("[");
    }
    let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor)).unwrap();
    complete(&mut model, request(&cmd).unwrap());
    assert!(matches!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Error(_))
    ));

    model.document_mut().buffer = "foo foo".into();
    if let Some(ModalState::FindReplace(state)) = &mut model.ui.active_modal {
        state.use_regex = false;
        state.set_query("foo");
    }
    let cmd = update(&mut model, Msg::Ui(UiMsg::BlinkCursor));
    assert!(cmd.as_ref().and_then(request).is_none());
    assert_eq!(
        state(&model).status(model.document(), &model.editor().selections[0]),
        Some(FindStatus::Count {
            total: 2,
            current: None
        })
    );
}
