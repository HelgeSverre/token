//! Inline (ghost-text) suggestion handlers — autocomplete.md Phase 2.
//!
//! Triggering, prefix consumption, revision-guarded arrival, accept as
//! one undo step, and the failure/backoff policy. The HTTP work happens on
//! the runtime's completion worker; this module only speaks `Cmd`s.

use std::time::Duration;

use crate::commands::Cmd;
use crate::completion::inline::{
    build_request, line_tail_is_short, AcceptGranularity, InlineSuggestionState, RequestSnapshot,
    MAX_CONSECUTIVE_FAILURES,
};
use crate::completion::provider::{InlineJob, InlineSession};
use crate::completion::statistics::{Observation, Outcome};
use crate::config::ProviderConfig;
use crate::model::{AppModel, FocusTarget, TransientMessage};

/// The suggestion the user can see right now, if the document and cursor
/// still match it.
pub fn visible(model: &AppModel) -> Option<&InlineSuggestionState> {
    if model
        .ui
        .inline_session
        .as_ref()
        .is_some_and(|session| !session_is_current(model, session))
    {
        return None;
    }
    if !eligible(model) {
        return None;
    }
    let state = model.ui.inline_suggestion.as_ref()?;
    let cursor = *model.editor().active_cursor();
    state
        .applies_to(model.document(), (cursor.line, cursor.column))
        .then_some(state)
}

/// Configuration is borrowed until a request session actually needs a snapshot.
fn endpoint(model: &AppModel) -> Option<&ProviderConfig> {
    let completion = &model.config.completion;
    if !(completion.enabled && completion.inline.enabled) {
        return None;
    }
    completion.providers.get(&completion.inline.provider)
}

fn eligible(model: &AppModel) -> bool {
    model.ui.focus == FocusTarget::Editor
        && !model.ui.has_modal()
        && model.ui.context_menu.is_none()
        && model.editor().is_plain_text_mode()
        && !model.ui.has_visible_completion()
        && model.editor().active_selection().is_empty()
        && !model.editor().rectangle_selection.active
}

fn session_is_current(model: &AppModel, session: &InlineSession) -> bool {
    if !eligible(model)
        || model.editor().id != session.editor_id
        || endpoint(model) != Some(&session.provider)
    {
        return false;
    }
    let cursor = model.editor().active_cursor();
    if let Some(state) = &model.ui.inline_suggestion {
        return state.applies_to(model.document(), (cursor.line, cursor.column));
    }
    let snapshot = &session.snapshot;
    model.document().id == Some(snapshot.document_id)
        && model.document().revision == snapshot.revision
        && (cursor.line, cursor.column) == (snapshot.line, snapshot.column)
}

/// A single lifecycle check covers edits, focus/tab/selection changes, config
/// reloads and special-tab transitions, including update handlers' early exits.
pub(crate) fn reconcile(model: &mut AppModel) -> Option<Cmd> {
    // Opting out discards the current observation, including when toggled back
    // on before this response ends. Only newly requested responses are counted.
    if !model.config.completion.inline.statistics {
        if let Some(session) = model.ui.inline_session.as_mut() {
            session.observation = None;
        }
    }
    if model
        .ui
        .inline_session
        .as_ref()
        .is_some_and(|session| !session_is_current(model, session))
    {
        dismiss(model)
    } else {
        None
    }
}

/// Share one immutable projection with every geometry consumer in the focused
/// pane. Reuse it on blink/scroll; only source, candidate, anchor or width changes
/// reflow the affected logical line. Peers viewing the same document stay plain.
pub(crate) fn sync_projection(model: &mut AppModel) -> Option<Cmd> {
    let desired = visible(model).and_then(|state| {
        let editor = model.editor();
        let id = editor.id?;
        let anchor = editor.active_cursor().to_position();
        let width = editor.soft_wrap.then_some(editor.viewport.visible_columns);
        let text = state.remaining();
        let projection = editor
            .ghost_text
            .0
            .as_ref()
            .filter(|g| g.matches(model.document(), anchor, text, width))
            .cloned()
            .or_else(|| {
                crate::model::GhostProjection::new(model.document(), anchor, text, width)
                    .map(std::sync::Arc::new)
            })?;
        Some((id, projection))
    });
    let mut changed = false;
    let area = &mut model.editor_area;
    for (id, editor) in &mut area.editors {
        let next = desired
            .as_ref()
            .filter(|(target, _)| target == id)
            .map(|(_, g)| g.clone());
        let same = match (&editor.ghost_text.0, &next) {
            (Some(old), Some(new)) => std::sync::Arc::ptr_eq(old, new),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(document) = editor.document_id.and_then(|id| area.documents.get(&id)) {
                editor.set_ghost_text(document, next);
                changed = true;
            }
        }
    }
    changed.then(Cmd::redraw_editor)
}

/// Ask the runtime to fire a request after the debounce (or now, when
/// `explicit`). Auto-triggers respect the gates; an explicit trigger only
/// needs a configured backend.
fn schedule(model: &mut AppModel, explicit: bool) -> Option<Cmd> {
    if !eligible(model) {
        return None;
    }
    let provider = endpoint(model)?.clone();
    let document_id = model.document().id?;
    if !explicit {
        let ui = &model.ui;
        if ui.inline_failures >= MAX_CONSECUTIVE_FAILURES {
            return None;
        }
        let cursor = *model.editor().active_cursor();
        let line = model.document().get_line_cow(cursor.line)?;
        let rest: String = line.chars().skip(cursor.column).collect();
        if !line_tail_is_short(&rest, model.config.completion.inline.max_line_suffix) {
            return None;
        }
    } else {
        model.ui.inline_failures = 0;
    }
    let cancel = dismiss(model);
    model.ui.inline_next_request_id += 1;
    let cursor = *model.editor().active_cursor();
    let snapshot = RequestSnapshot {
        document_id,
        revision: model.document().revision,
        line: cursor.line,
        column: cursor.column,
        request_id: model.ui.inline_next_request_id,
    };
    model.ui.inline_session = Some(InlineSession {
        snapshot: snapshot.clone(),
        editor_id: model.editor().id,
        provider,
        observation: model
            .config
            .completion
            .inline
            .statistics
            .then(|| Observation::new(model.config.completion.inline.provider.clone())),
    });
    let schedule = Cmd::ScheduleInlineRequest {
        snapshot,
        delay_ms: if explicit {
            0
        } else {
            model.config.completion.inline.debounce_ms
        },
        explicit,
    };
    Some(match cancel {
        Some(cancel) => Cmd::Batch(vec![cancel, schedule]),
        None => schedule,
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
    let mut usage = None;
    if let Some(mut state) = model.ui.inline_suggestion.take() {
        let cursor = *model.editor().active_cursor();
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
        let outcome =
            if kept && state.applies_to(model.document(), cursor) && state.remaining().is_empty() {
                Outcome::TypedThrough
            } else {
                Outcome::Dismissed
            };
        usage = record_outcome(model, outcome);
        redraw_line = Some(Cmd::redraw_cursor_lines(vec![cursor.0]));
    }
    // Only insertions ask for a suggestion; deleting text rarely wants one.
    let request = typed_char.and_then(|_| schedule(model, false));
    let mut commands: Vec<_> = redraw_line
        .into_iter()
        .chain(usage)
        .chain(request)
        .collect();
    match commands.len() {
        0 => None,
        1 => commands.pop(),
        _ => Some(Cmd::Batch(commands)),
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
    snapshot: RequestSnapshot,
    explicit: bool,
) -> Option<Cmd> {
    let session = model.ui.inline_session.as_ref()?;
    if session.snapshot != snapshot
        || !session_is_current(model, session)
        || model.ui.inline_in_flight
        || model.ui.inline_suggestion.is_some()
    {
        return None;
    }
    let provider = session.provider.clone();
    let cursor = *model.editor().active_cursor();
    let language = Some(format!("{:?}", model.document().language).to_lowercase());
    let request = build_request(
        model.document(),
        (cursor.line, cursor.column),
        snapshot.request_id,
        language,
        explicit,
    )?;
    model.ui.inline_in_flight = true;
    Some(Cmd::PrepareInlineRequest(Box::new(InlineJob {
        request,
        provider,
        context: crate::completion::postprocess::InlineContext::capture(
            model.document(),
            (cursor.line, cursor.column),
        ),
    })))
}

/// Context is speculative too: never transmit a stale request to a provider.
pub(crate) fn context_ready(
    model: &mut AppModel,
    job: Box<InlineJob>,
    root: Option<std::path::PathBuf>,
) -> Option<Cmd> {
    let session = model.ui.inline_session.as_ref()?;
    if session.snapshot != job.request.snapshot || session.provider != job.provider {
        return None;
    }
    if !session_is_current(model, session) || model.workspace_root() != root.as_ref() {
        return dismiss(model);
    }
    model
        .ui
        .inline_in_flight
        .then_some(Cmd::RunInlineRequest(job))
}

/// The worker answered. Stale replies (document, revision, or cursor
/// moved on; a menu opened meanwhile) are dropped silently.
pub(crate) fn ready(
    model: &mut AppModel,
    snapshot: RequestSnapshot,
    texts: Vec<String>,
) -> Option<Cmd> {
    if snapshot.request_id != model.ui.inline_next_request_id {
        return None;
    }
    if model
        .ui
        .inline_session
        .as_ref()
        .is_some_and(|session| !session_is_current(model, session))
    {
        return dismiss(model);
    }
    model.ui.inline_in_flight = false;
    model.ui.inline_failures = 0;
    let Some(state) = InlineSuggestionState::new(snapshot, texts) else {
        model.ui.inline_session = None;
        return None;
    };
    let cursor = *model.editor().active_cursor();
    if model.ui.has_visible_completion()
        || !state.applies_to(model.document(), (cursor.line, cursor.column))
        || state.remaining().is_empty()
    {
        model.ui.inline_session = None;
        return None;
    }
    model.ui.inline_suggestion = Some(state);
    if let Some(observation) = model
        .ui
        .inline_session
        .as_mut()
        .and_then(|session| session.observation.as_mut())
    {
        observation.offered();
    }
    Some(Cmd::redraw_cursor_lines(vec![cursor.line]))
}

/// Cycling uses the same visibility/session guards as acceptance and painting.
pub(crate) fn cycle(model: &mut AppModel, forward: bool) -> Option<Cmd> {
    visible(model)?;
    let state = model.ui.inline_suggestion.as_mut()?;
    state
        .cycle(forward)
        .then(|| Cmd::redraw_cursor_lines(vec![model.editor().active_cursor().line]))
}

/// The worker failed. Never modal: a status transient on the first
/// failure and when auto-trigger pauses; explicit triggers always report.
pub(crate) fn failed(
    model: &mut AppModel,
    snapshot: RequestSnapshot,
    error: String,
) -> Option<Cmd> {
    if snapshot.request_id != model.ui.inline_next_request_id {
        return None;
    }
    if model
        .ui
        .inline_session
        .as_ref()
        .is_some_and(|session| !session_is_current(model, session))
    {
        return dismiss(model);
    }
    model.ui.inline_in_flight = false;
    model.ui.inline_session = None;
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

/// Accept one portion as a single undo batch. Partial acceptance keeps the
/// same suggestion and does not request another completion until it is exhausted.
pub(crate) fn accept(model: &mut AppModel, granularity: AcceptGranularity) -> Option<Cmd> {
    let inserted = visible(model)?.acceptance_prefix(granularity).to_owned();
    if inserted.is_empty() {
        return None;
    }
    let accepted_chars = inserted.chars().count();
    let document_id = model.editor_area.focused_document_id()?;
    let cursor = *model.editor().active_cursor();
    let position = model
        .document()
        .cursor_to_offset(cursor.line, cursor.column);

    let effects = super::text_edits::apply_planned_edits(
        model,
        document_id,
        &[super::text_edits::PlannedEdit {
            start: position,
            deleted: String::new(),
            inserted,
        }],
        super::text_edits::EditCarets::Preserve,
    )?;
    model.editor_mut().occurrence_state = None;
    model.editor_mut().clear_selection_history();
    model.reset_cursor_blink();
    // A pending older reply must not replace the retained remainder or report
    // a failure for a request the user has already superseded by accepting.
    model.ui.inline_next_request_id += 1;
    model.ui.inline_in_flight = false;
    let usage = record_outcome(model, Outcome::Accepted);
    let effects = match usage {
        Some(usage) => Cmd::Batch(vec![effects, usage]),
        None => effects,
    };
    let mut state = model.ui.inline_suggestion.take()?;
    state.consumed += accepted_chars;
    state.valid_revision = model.document().revision;
    if state.remaining().is_empty() {
        model.ui.inline_suggestion = None;
        Some(match schedule(model, false) {
            Some(next) => Cmd::Batch(vec![effects, next]),
            None => effects,
        })
    } else {
        model.ui.inline_suggestion = Some(state);
        Some(effects)
    }
}

/// Escape (or any other reason to hide the ghost text).
pub(crate) fn dismiss(model: &mut AppModel) -> Option<Cmd> {
    let usage = record_outcome(model, Outcome::Dismissed);
    let had_session = model.ui.inline_session.take().is_some();
    let had_suggestion = model.ui.inline_suggestion.take().is_some();
    let had_request = std::mem::take(&mut model.ui.inline_in_flight);
    if !(had_session || had_suggestion || had_request) {
        return None;
    }
    model.ui.inline_next_request_id += 1;
    let mut commands = vec![
        Cmd::CancelInlineRequest,
        Cmd::redraw_editor(),
        Cmd::redraw_status_bar(),
    ];
    commands.extend(usage);
    Some(Cmd::Batch(commands))
}

fn record_outcome(model: &mut AppModel, outcome: Outcome) -> Option<Cmd> {
    if !model.config.completion.inline.statistics {
        return None;
    }
    model
        .ui
        .inline_session
        .as_mut()?
        .observation
        .as_mut()?
        .resolve(outcome)
        .map(Cmd::RecordInlineUsage)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{InlineConfig, ProviderConfig};
    use crate::messages::{CompletionMsg, DocumentMsg, EditorMsg, Msg};
    use crate::model::{Cursor, DocumentId, Selection};
    use crate::update::update;

    fn cancels(cmd: &Option<Cmd>) -> bool {
        fn walk(cmd: &Cmd) -> bool {
            match cmd {
                Cmd::CancelInlineRequest => true,
                Cmd::Batch(cmds) => cmds.iter().any(walk),
                _ => false,
            }
        }
        cmd.as_ref().is_some_and(walk)
    }

    fn arm(model: &mut AppModel) -> RequestSnapshot {
        trigger(model, true);
        model.ui.inline_session.as_ref().unwrap().snapshot.clone()
    }

    #[test]
    fn inline_context_is_revalidated_before_provider_submission() {
        let mut model = model_with_inline("\n");
        let snapshot = arm(&mut model);
        let Some(Cmd::PrepareInlineRequest(job)) = deadline_fired(&mut model, snapshot, true)
        else {
            panic!("preparation required");
        };
        #[cfg(debug_assertions)]
        assert_eq!(
            super::super::msg_type_name(&Msg::Completion(CompletionMsg::InlineContextReady {
                job: job.clone(),
                root: None
            })),
            format!(
                "Completion::InlineContextReady(request={})",
                job.request.snapshot.request_id
            )
        );
        assert!(matches!(
            context_ready(&mut model, job.clone(), None),
            Some(Cmd::RunInlineRequest(_))
        ));
        let mut old = job.clone();
        old.request.snapshot.request_id = old.request.snapshot.request_id.wrapping_sub(1);
        assert!(context_ready(&mut model, old, None).is_none());
        assert!(
            model.ui.inline_in_flight,
            "late work must not clear newer requests"
        );
        assert!(cancels(&context_ready(
            &mut model,
            job,
            Some("changed-workspace".into())
        )));
        assert!(!model.ui.inline_in_flight);
        let snapshot = arm(&mut model);
        let Some(Cmd::PrepareInlineRequest(job)) = deadline_fired(&mut model, snapshot, true)
        else {
            panic!("preparation required");
        };
        model.document_mut().revision += 1;
        assert!(cancels(&context_ready(&mut model, job, None)));
    }

    fn statistics_events(command: Option<Cmd>) -> Vec<crate::completion::statistics::UsageEvent> {
        fn collect(command: Cmd, events: &mut Vec<crate::completion::statistics::UsageEvent>) {
            match command {
                Cmd::RecordInlineUsage(event) => events.push(event),
                Cmd::Batch(commands) => commands
                    .into_iter()
                    .for_each(|command| collect(command, events)),
                _ => {}
            }
        }
        let mut events = Vec::new();
        if let Some(command) = command {
            collect(command, &mut events);
        }
        events
    }

    fn offer(model: &mut AppModel, texts: &[&str]) {
        let snapshot = arm(model);
        let command = update(
            model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot,
                texts: texts.iter().map(|text| (*text).into()).collect(),
            }),
        );
        assert!(statistics_events(command).is_empty());
        assert!(visible(model).is_some());
    }

    fn assert_statistics_outcome(command: Option<Cmd>, outcome: Outcome) {
        assert_eq!(
            statistics_events(command),
            vec![crate::completion::statistics::UsageEvent {
                provider: "local".into(),
                outcome
            }]
        );
    }

    #[test]
    fn statistics_partial_accepts_and_alternative_cycles_count_one_response() {
        let mut model = model_with_inline("\n");
        offer(&mut model, &["hello world", "hello everyone"]);
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::CycleInline { forward: true })
        ))
        .is_empty());
        assert_statistics_outcome(
            update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Word)),
            ),
            Outcome::Accepted,
        );
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Word))
        ))
        .is_empty());
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full))
        ))
        .is_empty());
        assert_eq!(model.document().buffer.to_string(), "hello everyone\n");
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        ))
        .is_empty());
    }

    #[test]
    fn statistics_full_accept_and_dismiss_have_single_terminal_outcomes() {
        let mut model = model_with_inline("\n");
        offer(&mut model, &["hello"]);
        assert_statistics_outcome(
            update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
            ),
            Outcome::Accepted,
        );
        assert!(statistics_events(update(&mut model, Msg::Document(DocumentMsg::Undo))).is_empty());
        offer(&mut model, &["hello"]);
        assert_statistics_outcome(
            update(&mut model, Msg::Completion(CompletionMsg::DismissInline)),
            Outcome::Dismissed,
        );
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        ))
        .is_empty());
        offer(&mut model, &["hello world"]);
        assert_statistics_outcome(
            update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Word)),
            ),
            Outcome::Accepted,
        );
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        ))
        .is_empty());
    }

    #[test]
    fn statistics_typing_through_unicode_and_backspace_is_not_explicit_acceptance() {
        let mut model = model_with_inline("\n");
        offer(&mut model, &["é界"]);
        assert!(statistics_events(update(
            &mut model,
            Msg::Document(DocumentMsg::InsertChar('é'))
        ))
        .is_empty());
        assert!(statistics_events(update(
            &mut model,
            Msg::Document(DocumentMsg::DeleteBackward)
        ))
        .is_empty());
        assert!(statistics_events(update(
            &mut model,
            Msg::Document(DocumentMsg::InsertChar('é'))
        ))
        .is_empty());
        assert_statistics_outcome(
            update(&mut model, Msg::Document(DocumentMsg::InsertChar('界'))),
            Outcome::TypedThrough,
        );
        assert_eq!(model.document().buffer.to_string(), "é界\n");
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        ))
        .is_empty());
    }

    #[test]
    fn statistics_divergent_edits_navigation_and_focus_changes_dismiss_an_offer() {
        use crate::messages::{Direction, LayoutMsg};
        for message in [
            Msg::Document(DocumentMsg::InsertChar('!')),
            Msg::Document(DocumentMsg::InsertText("paste".into())),
            Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
            Msg::Editor(EditorMsg::SelectAll),
            Msg::Layout(LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Horizontal,
            )),
        ] {
            let mut model = model_with_inline("abc\n");
            place(&mut model, 0, 3);
            offer(&mut model, &[" completion"]);
            assert_statistics_outcome(update(&mut model, message), Outcome::Dismissed);
            assert!(statistics_events(dismiss(&mut model)).is_empty());
        }
    }

    #[test]
    fn statistics_pending_empty_failed_and_stale_responses_are_not_offers() {
        let mut model = model_with_inline("\n");
        let stale = arm(&mut model);
        assert!(statistics_events(dismiss(&mut model)).is_empty());
        let current = arm(&mut model);
        assert!(statistics_events(ready(&mut model, stale, vec!["stale".into()])).is_empty());
        assert!(statistics_events(ready(&mut model, current, vec![])).is_empty());
        assert!(statistics_events(dismiss(&mut model)).is_empty());
        let current = arm(&mut model);
        assert!(statistics_events(failed(&mut model, current, "unavailable".into())).is_empty());
        assert!(statistics_events(dismiss(&mut model)).is_empty());
    }

    #[test]
    fn statistics_opt_out_discards_current_observation_and_opt_in_only_counts_new_requests() {
        let mut model = model_with_inline("\n");
        offer(&mut model, &["hello"]);
        model.config.completion.inline.statistics = false;
        reconcile(&mut model);
        model.config.completion.inline.statistics = true;
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full))
        ))
        .is_empty());
        model.config.completion.inline.statistics = false;
        offer(&mut model, &[" world"]);
        model.config.completion.inline.statistics = true;
        assert!(statistics_events(update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        ))
        .is_empty());
        offer(&mut model, &[" world"]);
        assert_statistics_outcome(
            update(&mut model, Msg::Completion(CompletionMsg::DismissInline)),
            Outcome::Dismissed,
        );
    }

    #[test]
    fn statistics_provider_attribution_uses_request_name_not_current_config() {
        let mut model = model_with_inline("\n");
        offer(&mut model, &["hello"]);
        model
            .config
            .completion
            .providers
            .insert("alias".into(), ProviderConfig::default());
        model.config.completion.inline.provider = "alias".into();
        assert_statistics_outcome(
            update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
            ),
            Outcome::Accepted,
        );
    }

    #[test]
    fn statistics_failures_notify_once_until_a_success_rearms_notification() {
        let mut model = model_with_inline("\n");
        let failed = || {
            Msg::Completion(CompletionMsg::InlineStatisticsSaved(Err(
                "private path".into()
            )))
        };
        update(&mut model, failed());
        assert!(model.ui.inline_statistics_failed);
        assert!(!model
            .ui
            .transient_message
            .as_ref()
            .unwrap()
            .text
            .contains("private path"));
        model.ui.transient_message = None;
        update(&mut model, failed());
        assert!(model.ui.transient_message.is_none());
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineStatisticsSaved(Ok(()))),
        );
        assert!(!model.ui.inline_statistics_failed);
        update(&mut model, failed());
        assert!(model.ui.transient_message.is_some());
    }

    #[test]
    fn dismissal_invalidates_debounce_and_queued_success_or_failure() {
        let mut model = model_with_inline("\n");
        let old = arm(&mut model);
        assert!(cancels(&update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        )));
        assert!(deadline_fired(&mut model, old.clone(), true).is_none());
        let current = arm(&mut model);
        assert!(deadline_fired(&mut model, old.clone(), true).is_none());
        assert!(deadline_fired(&mut model, current.clone(), true).is_some());
        assert!(
            deadline_fired(&mut model, current.clone(), true).is_none(),
            "a duplicate deadline must not start two requests"
        );
        assert!(cancels(&update(
            &mut model,
            Msg::Completion(CompletionMsg::DismissInline)
        )));
        assert!(ready(&mut model, current.clone(), vec!["old answer".into()]).is_none());
        assert!(failed(&mut model, current, "old error".into()).is_none());
        assert!(!model.ui.inline_in_flight);
        assert_eq!(model.ui.inline_failures, 0);
        assert!(model.ui.inline_suggestion.is_none());
    }

    #[test]
    fn edits_movement_selection_and_pane_changes_cancel_the_session() {
        use crate::messages::{Direction, LayoutMsg};
        for message in [
            Msg::Editor(EditorMsg::MoveCursor(Direction::Left)),
            Msg::Editor(EditorMsg::SelectAll),
            Msg::Document(DocumentMsg::DeleteBackward),
            Msg::Document(DocumentMsg::InsertText("paste".into())),
            Msg::Layout(LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Horizontal,
            )),
        ] {
            let mut model = model_with_inline("abc\n");
            place(&mut model, 0, 3);
            let snapshot = arm(&mut model);
            deadline_fired(&mut model, snapshot, true);
            assert!(cancels(&update(&mut model, message)));
            assert!(model.ui.inline_session.is_none());
            assert!(!model.ui.inline_in_flight);
        }
    }

    #[test]
    fn new_typing_cancels_old_request_before_arming_a_new_debounce() {
        let mut model = model_with_inline("\n");
        let snapshot = arm(&mut model);
        deadline_fired(&mut model, snapshot.clone(), true);
        let cmd = update(&mut model, Msg::Document(DocumentMsg::InsertChar('a')));
        assert!(cancels(&cmd));
        assert!(find_schedule(&cmd).is_some());
        assert!(!model.ui.inline_in_flight);
        assert_ne!(model.ui.inline_session.as_ref().unwrap().snapshot, snapshot);
        assert!(failed(&mut model, snapshot, "stale".into()).is_none());
    }

    #[test]
    fn configuration_and_focus_changes_cancel_without_counting_failure() {
        for mutate in [
            |model: &mut AppModel| model.config.completion.inline.enabled = false,
            |model: &mut AppModel| {
                model
                    .config
                    .completion
                    .providers
                    .get_mut("local")
                    .unwrap()
                    .timeout_ms += 1
            },
            |model: &mut AppModel| model.ui.focus = FocusTarget::Modal,
        ] {
            let mut model = model_with_inline("\n");
            let snapshot = arm(&mut model);
            deadline_fired(&mut model, snapshot.clone(), true);
            mutate(&mut model);
            let previous_status = model
                .ui
                .transient_message
                .as_ref()
                .map(|message| message.text.clone());
            // A reply can be the next event after a runtime-side config change.
            assert!(cancels(&failed(
                &mut model,
                snapshot,
                "must not show".into()
            )));
            assert_eq!(model.ui.inline_failures, 0);
            assert_eq!(
                model
                    .ui
                    .transient_message
                    .as_ref()
                    .map(|message| message.text.clone()),
                previous_status
            );
            assert!(!model.ui.inline_in_flight);
        }
    }

    #[test]
    fn a_provider_change_hides_existing_ghost_before_acceptance() {
        let mut model = model_with_inline("\n");
        let snapshot = arm(&mut model);
        ready(&mut model, snapshot, vec!["hello_world();".into()]);
        assert!(visible(&model).is_some());
        model.config.completion.inline.enabled = false;
        assert!(visible(&model).is_none());
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
        );
        assert!(cancels(&cmd));
        assert_eq!(model.document().buffer.to_string(), "\n");
    }

    fn model_with_inline(text: &str) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
        model.document_mut().buffer = ropey::Rope::from_str(text);
        model.document_mut().id = Some(DocumentId(7));
        model.config.completion.inline = InlineConfig {
            enabled: true,
            statistics: true,
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
        let cursor = *model.editor().active_cursor();
        RequestSnapshot {
            document_id: model.document().id.unwrap(),
            revision: model.document().revision,
            line: cursor.line,
            column: cursor.column,
            request_id: model.ui.inline_next_request_id,
        }
    }

    fn arrive(model: &mut AppModel, text: &str) {
        let snapshot = snapshot(model);
        update(
            model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot,
                texts: vec![text.to_owned()],
            }),
        );
    }

    #[test]
    fn completion_menu_config_auto_disabled_preserves_inline_acceptance() {
        let mut model = model_with_inline("\n");
        model.config.completion.menu.enabled = false;
        assert!(endpoint(&model).is_some());
        arrive(&mut model, "value");
        assert_eq!(visible(&model).unwrap().remaining(), "value");
        update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
        );
        assert_eq!(model.document().buffer.to_string(), "value\n");
        assert!(model.ui.completion_menu.is_none());
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "\n");
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
    fn explicit_midline_inline_text_is_projected_and_accepted_without_replacing_suffix() {
        let mut model = model_with_inline("a_long_tail_here\n");
        let command = update(
            &mut model,
            Msg::Completion(CompletionMsg::TriggerInline { explicit: true }),
        );
        assert_eq!(find_schedule(&command), Some((0, true)));
        arrive(&mut model, "before\n");
        assert_eq!(visible(&model).unwrap().remaining(), "before\n");
        assert_eq!(model.editor().viewport_map(model.document()).row_count(), 3);
        update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "before\na_long_tail_here\n"
        );
        assert!(model.editor().ghost_text.0.is_none());
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "a_long_tail_here\n");
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
        trigger(&mut model, false);
        let current = model.ui.inline_session.as_ref().unwrap().snapshot.clone();
        let stale = update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineDeadlineFired {
                snapshot: RequestSnapshot {
                    revision: 99,
                    ..current.clone()
                },
                explicit: false,
            }),
        );
        assert!(stale.is_none());
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineDeadlineFired {
                snapshot: current,
                explicit: false,
            }),
        );
        fn worker_request(cmd: &Cmd) -> Option<&crate::completion::inline::InlineRequest> {
            match cmd {
                Cmd::PrepareInlineRequest(job) => Some(&job.request),
                Cmd::Batch(cmds) => cmds.iter().find_map(worker_request),
                _ => None,
            }
        }
        let Some(request) = cmd.as_ref().and_then(worker_request) else {
            panic!("expected a worker request, got {cmd:?}");
        };
        assert_eq!(request.prefix, "let a = ");
        assert_eq!(request.suffix, "\n");
        assert_eq!(
            model.ui.inline_session.as_ref().unwrap().provider,
            ProviderConfig::default()
        );
        assert!(model.ui.inline_in_flight);
        assert!(!model
            .ui
            .status_bar
            .get_segment(crate::model::status_bar::SegmentId::InlineSuggestion)
            .unwrap()
            .content
            .is_empty());
    }

    #[test]
    fn superseded_inline_replies_do_not_clear_progress_or_replace_suggestions() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        let stale = snapshot(&model);
        model.ui.inline_next_request_id += 1;
        model.ui.inline_in_flight = true;
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot: stale.clone(),
                texts: vec!["old".into()],
            }),
        );
        assert!(model.ui.inline_in_flight);
        assert!(model.ui.inline_suggestion.is_none());
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineFailed {
                snapshot: stale,
                error: "old failure".into(),
            }),
        );
        assert!(model.ui.inline_in_flight);
        assert_eq!(model.ui.inline_failures, 0);
        arrive(&mut model, "new");
        assert_eq!(visible(&model).unwrap().remaining(), "new");
        assert!(!model.ui.inline_in_flight);
        assert!(model
            .ui
            .status_bar
            .get_segment(crate::model::status_bar::SegmentId::InlineSuggestion)
            .unwrap()
            .content
            .is_empty());
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
                texts: vec!["1;".into()],
            }),
        );
        assert!(model.ui.inline_suggestion.is_none());
        let mut moved = snapshot(&model);
        moved.column = 3;
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot: moved,
                texts: vec!["1;".into()],
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
    fn moving_the_cursor_clears_the_suggestion_and_escape_is_idempotent() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        arrive(&mut model, "1;");
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(crate::messages::Direction::Left)),
        );
        assert!(visible(&model).is_none(), "cursor moved away");
        assert!(
            model.ui.inline_suggestion.is_none(),
            "navigation must clear state before computing source movement"
        );
        assert!(model.editor().ghost_text.0.is_none());
        update(&mut model, Msg::Completion(CompletionMsg::DismissInline));
        assert!(model.ui.inline_suggestion.is_none());
    }

    #[test]
    fn accept_inserts_the_remainder_as_one_undo_step_and_chains_a_request() {
        let mut model = model_with_inline("let a = \n");
        place(&mut model, 0, 8);
        arrive(&mut model, "1 + 2;\nlet b = 3;");
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('1')));
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
        );
        assert_eq!(
            model.document().buffer.to_string(),
            "let a = 1 + 2;\nlet b = 3;\n"
        );
        let cursor = *model.editor().active_cursor();
        assert_eq!((cursor.line, cursor.column), (1, 10));
        assert!(model.ui.inline_suggestion.is_none());
        assert!(find_schedule(&cmd).is_some(), "chained follow-up request");
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "let a = 1\n");
        assert!(
            update(
                &mut model,
                Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full))
            )
            .is_none(),
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
    fn invisible_completion_request_does_not_suppress_inline_suggestions() {
        let mut model = model_with_inline("builder.");
        model.document_mut().language = crate::syntax::LanguageId::Rust;
        model.document_mut().file_path = Some("/tmp/proj/build.rs".into());
        place(&mut model, 0, 8);
        update(&mut model, Msg::Completion(CompletionMsg::TriggerMenu));
        assert!(model.ui.completion_menu.is_some());
        assert!(!model.ui.has_visible_completion());
        arrive(&mut model, "compile()");
        assert_eq!(visible(&model).unwrap().remaining(), "compile()");
    }

    #[test]
    fn partial_accept_keeps_the_remainder_and_each_step_is_undoable() {
        let mut model = model_with_inline("\n");
        arrive(&mut model, "héllo_world\nnext();");
        let initial_undo = model.document().undo_stack.len();
        let old_reply = snapshot(&model);
        let cmd = accept(&mut model, AcceptGranularity::Word);
        assert_eq!(model.document().buffer.to_string(), "héllo\n");
        assert_eq!(visible(&model).unwrap().remaining(), "_world\nnext();");
        assert_eq!(model.document().undo_stack.len(), initial_undo + 1);
        assert!(
            find_schedule(&cmd).is_none(),
            "retained remainder must not trigger a request"
        );
        ready(&mut model, old_reply, vec!["stale".into()]);
        assert_eq!(visible(&model).unwrap().remaining(), "_world\nnext();");
        let cmd = accept(&mut model, AcceptGranularity::Line);
        assert_eq!(model.document().buffer.to_string(), "héllo_world\n\n");
        assert_eq!(visible(&model).unwrap().remaining(), "next();");
        assert!(find_schedule(&cmd).is_none());
        accept(&mut model, AcceptGranularity::Full);
        assert_eq!(
            model.document().buffer.to_string(),
            "héllo_world\nnext();\n"
        );
        assert!(visible(&model).is_none());
        for expected in ["héllo_world\n\n", "héllo\n", "\n"] {
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), expected);
        }
    }

    #[test]
    fn inline_cycling_changes_only_ghost_state_and_accepts_the_selected_remainder() {
        let mut model = model_with_inline("\n");
        let snap = snapshot(&model);
        ready(
            &mut model,
            snap,
            vec![
                "héllo_one();".into(),
                "other();".into(),
                "héllo_two();".into(),
            ],
        );
        let revision = model.document().revision;
        let undo_count = model.document().undo_stack.len();
        let request_id = model.ui.inline_next_request_id;
        let cmd = update(
            &mut model,
            Msg::Completion(CompletionMsg::CycleInline { forward: false }),
        );
        assert!(cmd.is_some());
        assert!(find_schedule(&cmd).is_none());
        assert_eq!(visible(&model).unwrap().remaining(), "héllo_two();");
        assert_eq!(model.document().revision, revision);
        assert_eq!(model.document().undo_stack.len(), undo_count);
        assert_eq!(model.ui.inline_next_request_id, request_id);
        assert_eq!(model.document().buffer.to_string(), "\n");
        accept(&mut model, AcceptGranularity::Word);
        assert_eq!(model.document().buffer.to_string(), "héllo\n");
        cycle(&mut model, true);
        assert_eq!(visible(&model).unwrap().remaining(), "_one();");
        assert_eq!(visible(&model).unwrap().choice_position(), (1, 2));
        accept(&mut model, AcceptGranularity::Full);
        assert_eq!(model.document().buffer.to_string(), "héllo_one();\n");
        for expected in ["héllo\n", "\n"] {
            update(&mut model, Msg::Document(DocumentMsg::Undo));
            assert_eq!(model.document().buffer.to_string(), expected);
        }
    }

    #[test]
    fn invisible_or_single_inline_choice_cannot_be_cycled() {
        let mut model = model_with_inline("\n");
        assert!(cycle(&mut model, true).is_none());
        arrive(&mut model, "single");
        assert!(cycle(&mut model, true).is_none());
        let snap = snapshot(&model);
        ready(&mut model, snap, vec!["one".into(), "two".into()]);
        model.document_mut().revision += 1;
        assert!(cycle(&mut model, true).is_none());
        assert_eq!(
            model.ui.inline_suggestion.as_ref().unwrap().remaining(),
            "one"
        );
        let mut model = model_with_inline("long_tail_at_cursor\n");
        let snap = snapshot(&model);
        ready(&mut model, snap, vec!["one".into(), "two".into()]);
        assert!(cycle(&mut model, false).is_some());
        assert_eq!(visible(&model).unwrap().remaining(), "two");
        assert_eq!(model.document().buffer.to_string(), "long_tail_at_cursor\n");
    }

    #[test]
    fn partial_accept_targets_active_cursor_and_preserves_peer_selections() {
        use crate::messages::LayoutMsg;
        use crate::model::{Position, SplitDirection};
        let mut model = AppModel::new(800, 600, 1.0);
        model.config.completion.enabled = false;
        model.document_mut().buffer = ropey::Rope::from_str("ab\ncd\n");
        let peer = model.editor().id.unwrap();
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(SplitDirection::Horizontal)),
        );
        assert_ne!(model.editor().id, Some(peer));
        model.editor_mut().cursors = vec![Cursor::at(0, 0), Cursor::at(0, 1), Cursor::at(1, 1)];
        model.editor_mut().selections = model
            .editor()
            .cursors
            .iter()
            .map(|c| Selection::new(c.to_position()))
            .collect();
        model.editor_mut().active_cursor_index = 1;
        {
            let editor = model.editor_area.editors.get_mut(&peer).unwrap();
            editor.cursors = vec![Cursor::at(1, 2)];
            editor.selections = vec![Selection::from_anchor_head(
                Position::new(0, 2),
                Position::new(1, 2),
            )];
        }
        arrive(&mut model, "X\nY");
        accept(&mut model, AcceptGranularity::Line);
        assert_eq!(model.document().buffer.to_string(), "aX\nb\ncd\n");
        assert_eq!(model.editor().active_cursor_index, 1);
        assert_eq!(
            model.editor().cursors,
            vec![Cursor::at(0, 0), Cursor::at(1, 0), Cursor::at(2, 1)]
        );
        let other = &model.editor_area.editors[&peer];
        assert_eq!(other.cursors, vec![Cursor::at(2, 2)]);
        assert_eq!(
            other.selections[0],
            Selection::from_anchor_head(Position::new(1, 1), Position::new(2, 2))
        );
        assert_eq!(visible(&model).unwrap().remaining(), "Y");
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

    #[test]
    fn ghost_projection_reuses_scroll_geometry_and_reflows_on_resize_and_cycling() {
        let mut model = model_with_inline("prefix suffix\nnext\n");
        place(&mut model, 0, 7);
        model.editor_mut().soft_wrap = true;
        model.editor_mut().viewport.visible_columns = 8;
        let snapshot = arm(&mut model);
        update(
            &mut model,
            Msg::Completion(CompletionMsg::InlineReady {
                snapshot,
                texts: vec!["long words\n\tend".into(), "short".into()],
            }),
        );
        let original = model.editor().ghost_text.0.clone().unwrap();
        update(&mut model, Msg::Editor(EditorMsg::Scroll(1)));
        assert!(std::sync::Arc::ptr_eq(
            &original,
            model.editor().ghost_text.0.as_ref().unwrap()
        ));
        model.editor_mut().viewport.visible_columns = 4;
        // Native font/gutter changes can refresh geometry outside update().
        model.editor_area.refresh_wrap_caches();
        let resized = model.editor().ghost_text.0.clone().unwrap();
        assert!(!std::sync::Arc::ptr_eq(&original, &resized));
        assert!(resized.rows.len() > original.rows.len());
        update(&mut model, Msg::Editor(EditorMsg::Scroll(0)));
        assert!(std::sync::Arc::ptr_eq(
            &resized,
            model.editor().ghost_text.0.as_ref().unwrap()
        ));
        update(
            &mut model,
            Msg::Completion(CompletionMsg::CycleInline { forward: true }),
        );
        assert_eq!(visible(&model).unwrap().remaining(), "short");
        assert!(!std::sync::Arc::ptr_eq(
            &resized,
            model.editor().ghost_text.0.as_ref().unwrap()
        ));
        assert_eq!(model.document().buffer.to_string(), "prefix suffix\nnext\n");
        assert!(model.document().undo_stack.is_empty());
        update(
            &mut model,
            Msg::Editor(EditorMsg::MoveCursor(crate::messages::Direction::Down)),
        );
        assert!(model.editor().ghost_text.0.is_none());
        assert!(visible(&model).is_none());
        assert_ne!(
            model.editor().active_cursor().to_position(),
            crate::model::Position::new(0, 7)
        );
    }

    #[test]
    fn ghost_projection_tracks_type_through_partial_accept_and_undo() {
        let mut model = model_with_inline("ab\nnext");
        place(&mut model, 0, 1);
        arm(&mut model);
        arrive(&mut model, "XY\nend");
        update(&mut model, Msg::Document(DocumentMsg::InsertChar('X')));
        assert_eq!(visible(&model).unwrap().remaining(), "Y\nend");
        assert_eq!(
            model.editor().ghost_text.0.as_ref().unwrap().anchor,
            crate::model::Position::new(0, 2)
        );
        update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Line)),
        );
        assert_eq!(model.document().buffer.to_string(), "aXY\nb\nnext");
        assert_eq!(visible(&model).unwrap().remaining(), "end");
        assert_eq!(
            model.editor().ghost_text.0.as_ref().unwrap().anchor,
            crate::model::Position::new(1, 0)
        );
        assert_eq!(model.editor().viewport_map(model.document()).row_count(), 3);
        update(
            &mut model,
            Msg::Completion(CompletionMsg::AcceptInline(AcceptGranularity::Full)),
        );
        assert_eq!(model.document().buffer.to_string(), "aXY\nendb\nnext");
        assert!(model.editor().ghost_text.0.is_none());
        update(&mut model, Msg::Document(DocumentMsg::Undo));
        assert_eq!(model.document().buffer.to_string(), "aXY\nb\nnext");
        assert!(model.editor().ghost_text.0.is_none());
    }

    #[test]
    fn ghost_projection_is_removed_from_peer_panes_and_rectangle_coordinates_are_rebased() {
        use crate::messages::LayoutMsg;
        let mut model = model_with_inline("ab\nnext");
        place(&mut model, 0, 1);
        arm(&mut model);
        arrive(&mut model, "x\ny\nz");
        update(
            &mut model,
            Msg::Editor(EditorMsg::StartRectangleSelection {
                line: 3,
                visual_col: 2,
            }),
        );
        assert!(model.editor().ghost_text.0.is_none());
        assert!(visible(&model).is_none());
        assert_eq!(model.editor().rectangle_selection.start_line, 1);
        assert_eq!(model.editor().rectangle_selection.start_visual_col, 2);
        update(&mut model, Msg::Editor(EditorMsg::CancelRectangleSelection));
        arm(&mut model);
        arrive(&mut model, "x\ny\nz");
        update(
            &mut model,
            Msg::Layout(LayoutMsg::SplitFocused(
                crate::model::SplitDirection::Vertical,
            )),
        );
        assert!(model
            .editor_area
            .editors
            .values()
            .all(|e| e.ghost_text.0.is_none()));
    }
}
