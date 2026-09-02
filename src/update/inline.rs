//! Inline (ghost-text) suggestion handlers — autocomplete.md Phase 2.
//!
//! Triggering, prefix consumption, revision-guarded arrival, accept as
//! one undo step, and the failure/backoff policy. The HTTP work happens on
//! the runtime's completion worker; this module only speaks `Cmd`s.

use std::time::Duration;

use crate::commands::Cmd;
use crate::completion::inline::{
    build_request, line_tail_is_short, InlineEndpoint, InlineSuggestionState, RequestSnapshot,
    MAX_CONSECUTIVE_FAILURES,
};
use crate::config::TransportKind;
use crate::model::editor_area::DocumentId;
use crate::model::{AppModel, Cursor, EditOperation, Selection, TransientMessage};

use super::lsp::schedule_lsp_did_change;
use super::syntax::schedule_syntax_parse;

/// The suggestion the user can see right now, if the document and cursor
/// still match it.
pub fn visible(model: &AppModel) -> Option<&InlineSuggestionState> {
    let state = model.ui.inline_suggestion.as_ref()?;
    let cursor = model.editor().cursors[0];
    state
        .applies_to(model.document(), (cursor.line, cursor.column))
        .then_some(state)
}

/// The configured llama.cpp endpoint, when inline suggestions are on.
fn endpoint(model: &AppModel) -> Option<InlineEndpoint> {
    let completion = &model.config.completion;
    if !(completion.enabled && completion.inline.enabled) {
        return None;
    }
    let provider = completion.providers.get(&completion.inline.provider)?;
    match provider.transport {
        TransportKind::LlamaCpp => Some(InlineEndpoint {
            url: provider.url.clone(),
            max_tokens: provider.max_tokens,
            timeout_ms: provider.timeout_ms,
        }),
    }
}

/// Ask the runtime to fire a request after the debounce (or now, when
/// `explicit`). Auto-triggers respect the gates; an explicit trigger only
/// needs a configured backend.
fn schedule(model: &mut AppModel, explicit: bool) -> Option<Cmd> {
    endpoint(model)?;
    let document_id = model.document().id?;
    if !explicit {
        let ui = &model.ui;
        if ui.completion_menu.is_some() || ui.inline_failures >= MAX_CONSECUTIVE_FAILURES {
            return None;
        }
        if !model.editor().is_plain_text_mode() {
            return None;
        }
        let cursor = model.editor().cursors[0];
        let line = model.document().get_line_cow(cursor.line)?;
        let rest: String = line.chars().skip(cursor.column).collect();
        if !line_tail_is_short(&rest, model.config.completion.inline.max_line_suffix) {
            return None;
        }
    } else {
        model.ui.inline_failures = 0;
    }
    Some(Cmd::ScheduleInlineRequest {
        document_id,
        revision: model.document().revision,
        delay_ms: if explicit {
            0
        } else {
            model.config.completion.inline.debounce_ms
        },
        explicit,
    })
}

/// Every buffer edit lands here (after the edit): consume the typed char
/// when it matches the ghost text, un-consume on backspace, otherwise
/// drop the suggestion; then consider a fresh request.
pub(crate) fn after_document_edit(
    model: &mut AppModel,
    typed_char: Option<char>,
    backspaced: bool,
) -> Option<Cmd> {
    let mut redraw_line = None;
    if let Some(mut state) = model.ui.inline_suggestion.take() {
        let cursor = model.editor().cursors[0];
        let cursor = (cursor.line, cursor.column);
        let kept = match typed_char {
            Some(ch) if state.remaining().starts_with(ch) => {
                state.consumed += 1;
                true
            }
            None if backspaced && state.consumed > 0 => {
                state.consumed -= 1;
                true
            }
            _ => false,
        };
        if kept {
            state.valid_revision = model.document().revision;
            if state.applies_to(model.document(), cursor) && !state.remaining().is_empty() {
                redraw_line = Some(Cmd::redraw_cursor_lines(vec![cursor.0]));
                model.ui.inline_suggestion = Some(state);
                return redraw_line;
            }
        }
        redraw_line = Some(Cmd::redraw_cursor_lines(vec![cursor.0]));
    }
    // Only insertions ask for a suggestion; deleting text rarely wants one.
    let request = typed_char.and_then(|_| schedule(model, false));
    match (redraw_line, request) {
        (Some(a), Some(b)) => Some(Cmd::Batch(vec![a, b])),
        (a, b) => a.or(b),
    }
}

/// A manual trigger, or the runtime replaying one after the debounce.
pub(crate) fn trigger(model: &mut AppModel, explicit: bool) -> Option<Cmd> {
    schedule(model, explicit)
}

/// The debounce elapsed: if nothing changed meanwhile, snapshot the
/// document and hand the request to the worker.
pub(crate) fn deadline_fired(
    model: &mut AppModel,
    document_id: DocumentId,
    revision: u64,
    explicit: bool,
) -> Option<Cmd> {
    let document = model.document();
    if document.id != Some(document_id) || document.revision != revision {
        return None;
    }
    if !explicit && model.ui.completion_menu.is_some() {
        return None;
    }
    let endpoint = endpoint(model)?;
    let cursor = model.editor().cursors[0];
    let language = Some(format!("{:?}", model.document().language).to_lowercase());
    model.ui.inline_next_request_id += 1;
    let request = build_request(
        model.document(),
        (cursor.line, cursor.column),
        model.ui.inline_next_request_id,
        endpoint,
        language,
        explicit,
    )?;
    model.ui.inline_in_flight = true;
    Some(Cmd::RunInlineRequest(Box::new(request)))
}

/// The worker answered. Stale replies (document, revision, or cursor
/// moved on; a menu opened meanwhile) are dropped silently.
pub(crate) fn ready(model: &mut AppModel, snapshot: RequestSnapshot, text: String) -> Option<Cmd> {
    model.ui.inline_in_flight = false;
    model.ui.inline_failures = 0;
    let state = InlineSuggestionState {
        valid_revision: snapshot.revision,
        snapshot,
        text,
        consumed: 0,
    };
    let cursor = model.editor().cursors[0];
    if model.ui.completion_menu.is_some()
        || !state.applies_to(model.document(), (cursor.line, cursor.column))
        || state.remaining().is_empty()
    {
        return None;
    }
    model.ui.inline_suggestion = Some(state);
    Some(Cmd::redraw_cursor_lines(vec![cursor.line]))
}

/// The worker failed. Never modal: a status transient on the first
/// failure and when auto-trigger pauses; explicit triggers always report.
pub(crate) fn failed(
    model: &mut AppModel,
    snapshot: RequestSnapshot,
    error: String,
) -> Option<Cmd> {
    model.ui.inline_in_flight = false;
    let _ = snapshot;
    model.ui.inline_failures = model.ui.inline_failures.saturating_add(1);
    let message = if model.ui.inline_failures >= MAX_CONSECUTIVE_FAILURES {
        format!(
            "Inline suggestions paused after repeated errors ({error}); trigger manually to retry"
        )
    } else {
        format!("Inline suggestion failed: {error}")
    };
    model.ui.transient_message = Some(TransientMessage::new(message, Duration::from_secs(3)));
    Some(Cmd::redraw_editor())
}

/// Tab: insert the remaining ghost text at the cursor as one undo step,
/// then ask for the next suggestion (the chained-accept flow).
pub(crate) fn accept(model: &mut AppModel) -> Option<Cmd> {
    let remaining = visible(model)?.remaining().to_owned();
    model.ui.inline_suggestion = None;
    let cursor_before = model.editor().cursors[0];
    let position = model
        .document()
        .cursor_to_offset(cursor_before.line, cursor_before.column);
    model.document_mut().buffer.insert(position, &remaining);
    let (line, column) = model
        .document()
        .offset_to_cursor(position + remaining.chars().count());
    let cursor_after = Cursor::at(line, column);
    let document_id = model.document().id;
    model.document_mut().push_edit(EditOperation::Insert {
        position,
        text: remaining,
        cursor_before,
        cursor_after,
    });
    model.document_mut().is_modified = true;
    {
        let editor = model.editor_mut();
        editor.cursors = vec![cursor_after];
        editor.selections = vec![Selection::new(cursor_after.to_position())];
        editor.occurrence_state = None;
        editor.clear_selection_history();
    }
    model.reset_cursor_blink();
    model.ensure_cursor_visible();

    let mut cmds = vec![Cmd::redraw_editor()];
    if let Some(document_id) = document_id {
        cmds.extend(schedule_syntax_parse(model, document_id));
        cmds.extend(schedule_lsp_did_change(model, document_id));
    }
    cmds.extend(schedule(model, false));
    Some(Cmd::Batch(cmds))
}

/// Escape (or any other reason to hide the ghost text).
pub(crate) fn dismiss(model: &mut AppModel) -> Option<Cmd> {
    let line = model.editor().cursors[0].line;
    model
        .ui
        .inline_suggestion
        .take()
        .map(|_| Cmd::redraw_cursor_lines(vec![line]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{InlineConfig, ProviderConfig};
    use crate::messages::{CompletionMsg, DocumentMsg, EditorMsg, Msg};
    use crate::update::update;

    fn model_with_inline(text: &str) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0, vec![]);
        model.document_mut().buffer = ropey::Rope::from_str(text);
        model.document_mut().id = Some(DocumentId(7));
        model.config.completion.inline = InlineConfig {
            enabled: true,
            provider: "local".into(),
            debounce_ms: 250,
            max_line_suffix: 8,
        };
        model
            .config
            .completion
            .providers
            .insert("local".into(), ProviderConfig::default());
        model
    }

    fn place(model: &mut AppModel, line: usize, column: usize) {
        let editor = model.editor_mut();
        editor.cursors = vec![Cursor::at(line, column)];
        editor.selections = vec![Selection::new(crate::model::Position::new(line, column))];
    }

    fn snapshot(model: &AppModel) -> RequestSnapshot {
        let cursor = model.editor().cursors[0];
        RequestSnapshot {
            document_id: model.document().id.unwrap(),
            revision: model.document().revision,
            line: cursor.line,
            column: cursor.column,
            request_id: 1,
        }
    }

    fn arrive(model: &mut AppModel, text: &str) {
        let snapshot = snapshot(model);
        update(
            model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot,
                text: text.to_owned(),
            }),
        );
    }

    fn find_schedule(cmd: &Option<Cmd>) -> Option<(u64, bool)> {
        fn walk(cmd: &Cmd) -> Option<(u64, bool)> {
            match cmd {
                Cmd::ScheduleInlineRequest {
                    delay_ms, explicit, ..
                } => Some((*delay_ms, *explicit)),
                Cmd::Batch(cmds) => cmds.iter().find_map(walk),
                _ => None,
            }
        }
        cmd.as_ref().and_then(walk)
    }

    #[test]
    fn typing_at_end_of_line_schedules_a_debounced_request() {
        let mut model = model_with_inline("fn main() {\n    let x = \n}\n");
        place(&mut model, 1, 12);
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('1')));
        assert_eq!(find_schedule(&cmd), Some((250, false)));
    }

    #[test]
    fn a_single_char_insert_text_counts_as_typing() {
        let mut model = model_with_inline("abc\n");
        place(&mut model, 0, 3);
        let cmd = update(
            &mut model,
            Msg::Document(DocumentMsg::InsertText("d".into())),
        );
        assert!(find_schedule(&cmd).is_some());
        let cmd = update(
            &mut model,
            Msg::Document(DocumentMsg::InsertText("paste".into())),
        );
        assert_eq!(find_schedule(&cmd), None, "a paste is not typing");
    }

    #[test]
    fn auto_trigger_respects_the_gates() {
        // Disabled config: nothing.
        let mut model = model_with_inline("abc\n");
        model.config.completion.inline.enabled = false;
        place(&mut model, 0, 3);
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('d')));
        assert_eq!(find_schedule(&cmd), None);
        // Too much text right of the cursor: nothing.
        let mut model = model_with_inline("a_long_tail_here\n");
        place(&mut model, 0, 0);
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
        assert_eq!(find_schedule(&cmd), None);
        // Paused after repeated failures, until an explicit trigger.
        let mut model = model_with_inline("abc\n");
        place(&mut model, 0, 3);
        model.ui.inline_failures = MAX_CONSECUTIVE_FAILURES;
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('d')));
        assert_eq!(find_schedule(&cmd), None);
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::TriggerInline { explicit: true }),
        );
        assert_eq!(find_schedule(&cmd), Some((0, true)));
        assert_eq!(model.ui.inline_failures, 0);
    }

    #[test]
    fn deadline_builds_a_request_only_for_the_current_revision() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        let stale = update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineDeadlineFired {
                document_id: DocumentId(7),
                revision: 99,
                explicit: false,
            }),
        );
        assert!(stale.is_none());
        let revision = model.document().revision;
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineDeadlineFired {
                document_id: DocumentId(7),
                revision,
                explicit: false,
            }),
        );
        let Some(Cmd::RunInlineRequest(request)) = cmd else {
            panic!("expected a worker request, got {cmd:?}");
        };
        assert_eq!(request.prefix, "let a = ");
        assert_eq!(request.suffix, "\n");
        assert_eq!(request.endpoint.url, ProviderConfig::default().url);
        assert!(model.ui.inline_in_flight);
    }

    #[test]
    fn a_reply_is_applied_only_when_document_revision_and_cursor_still_match() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        let mut stale = snapshot(&model);
        stale.revision += 1;
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot: stale,
                text: "1;".into(),
            }),
        );
        assert!(model.ui.inline_suggestion.is_none());
        let mut moved = snapshot(&model);
        moved.column = 3;
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot: moved,
                text: "1;".into(),
            }),
        );
        assert!(model.ui.inline_suggestion.is_none());
        arrive(&mut model, "1;");
        assert_eq!(visible(&model).unwrap().remaining(), "1;");
        assert!(!model.ui.inline_in_flight);
    }

    #[test]
    fn typing_through_consumes_and_a_divergent_char_clears() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        arrive(&mut model, "1 + 2;");
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('1')));
        assert_eq!(visible(&model).unwrap().remaining(), " + 2;");
        update(&mut model, Msg::Document(DocumentMsg::InsertChar(' ')));
        assert_eq!(visible(&model).unwrap().remaining(), "+ 2;");
        // Backspace un-consumes.
        update(&mut model, Msg::Document(DocumentMsg::DeleteBackward));
        assert_eq!(visible(&model).unwrap().remaining(), " + 2;");
        // A different char drops it.
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('x')));
        assert!(model.ui.inline_suggestion.is_none());
    }

    #[test]
    fn moving_the_cursor_hides_the_suggestion_and_escape_clears_it() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        arrive(&mut model, "1;");
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(crate::messages::Direction::Left)),
        );
        assert!(visible(&model).is_none(), "cursor moved away");
        assert!(
            model.ui.inline_suggestion.is_some(),
            "state lingers until dismissed"
        );
        update(&mut model, Msg::Completion(CompletionMsg::DismissInline));
        assert!(model.ui.inline_suggestion.is_none());
    }

    #[test]
    fn accept_inserts_the_remainder_as_one_undo_step_and_chains_a_request() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        arrive(&mut model, "1 + 2;\nlet b = 3;");
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('1')));
        let cmd = update(&mut model, Msg::Completion(CompletionMsg::AcceptInline));
        assert_eq!(
            model.document().buffer.to_string(),
            "let a = 1 + 2;\nlet b = 3;\n"
        );
        let cursor = model.editor().cursors[0];
        assert_eq!((cursor.line, cursor.column), (1, 10));
        assert!(model.ui.inline_suggestion.is_none());
        assert!(find_schedule(&cmd).is_some(), "chained follow-up request");
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "let a = 1\n");
        assert!(
            update(&mut model, Msg::Completion(CompletionMsg::AcceptInline)).is_none(),
            "nothing to accept"
        );
    }

    #[test]
    fn a_reply_while_the_menu_is_open_is_dropped() {
        let mut model = model_with_inline("value_one\n\n");
        place(&mut model, 1, 0);
        for ch in "val".chars() {
            update(&mut model, Msg::Document(DocumentMsg::InsertChar(ch)));
        }
        assert!(model.ui.completion_menu.is_some());
        arrive(&mut model, "ue_two");
        assert!(model.ui.inline_suggestion.is_none());
    }

    #[test]
    fn failures_report_once_then_pause_auto_trigger() {
        let mut model = model_with_inline("abc\n");
        place(&mut model, 0, 3);
        for _ in 0..MAX_CONSECUTIVE_FAILURES {
            let snapshot = snapshot(&model);
            update(
                &mut model,
                Msg::Completion(CompletionMsg::InlineFailed {
                    snapshot,
                    error: "connection refused".into(),
                }),
            );
        }
        assert!(model
            .ui
            .transient_message
            .as_ref()
            .is_some_and(|m| m.text.contains("paused")));
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('d')));
        assert_eq!(find_schedule(&cmd), None);
    }
}
