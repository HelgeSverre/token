//! Per-document auto-save deadlines. One event-loop wake serves every document.

use std::{collections::HashMap, path::PathBuf, time::Instant};

use token::{
    config::{AutoSaveConfig, AutoSaveMode},
    messages::AutoSaveRequest,
    model::{AppModel, DocumentId, SaveReason},
};

struct Pending {
    path: PathBuf,
    revision: u64,
    idle: Option<Instant>,
    focus_loss: bool,
}

pub(super) struct AutoSaveScheduler {
    policy: AutoSaveConfig,
    documents: HashMap<DocumentId, Pending>,
    focused: bool,
    pub composition: Option<DocumentId>,
}

impl Default for AutoSaveScheduler {
    fn default() -> Self {
        Self {
            policy: AutoSaveConfig::default(),
            documents: HashMap::new(),
            focused: true,
            composition: None,
        }
    }
}

impl AutoSaveScheduler {
    pub fn cancel(&mut self, id: DocumentId) {
        self.documents.remove(&id);
    }

    pub fn edited(&mut self, model: &AppModel, id: DocumentId, revision: u64, now: Instant) {
        self.sync(model, now);
        if let Some(doc) = model
            .editor_area
            .documents
            .get(&id)
            .filter(|doc| doc.revision == revision)
        {
            self.record(id, doc, now);
        }
    }

    fn record(&mut self, id: DocumentId, doc: &token::model::Document, now: Instant) {
        let Some(path) = doc.file_path.as_ref().filter(|_| doc.is_modified) else {
            self.documents.remove(&id);
            return;
        };
        if self.policy.mode == AutoSaveMode::Off {
            return;
        }
        if self
            .documents
            .get(&id)
            .is_some_and(|pending| pending.revision == doc.revision && &pending.path == path)
        {
            return;
        }
        let focus_loss = self.policy.mode.on_focus_loss()
            && (!self.focused
                || self
                    .documents
                    .get(&id)
                    .is_some_and(|pending| pending.focus_loss));
        self.documents.insert(
            id,
            Pending {
                path: path.clone(),
                revision: doc.revision,
                idle: self
                    .policy
                    .mode
                    .after_delay()
                    .then(|| now + self.policy.delay()),
                focus_loss,
            },
        );
    }

    fn sync(&mut self, model: &AppModel, now: Instant) {
        if self.policy != model.config.auto_save {
            self.policy = model.config.auto_save.clone();
            self.documents.clear();
        }
        self.documents
            .retain(|id, _| model.editor_area.documents.contains_key(id));
        if self
            .composition
            .is_some_and(|id| !model.editor_area.documents.contains_key(&id))
        {
            self.composition = None;
        }
        for (&id, doc) in &model.editor_area.documents {
            self.record(id, doc, now);
        }
    }

    pub fn focus_changed(&mut self, model: &AppModel, focused: bool, now: Instant) {
        self.sync(model, now);
        let lost = self.focused && !focused;
        self.focused = focused;
        if lost && self.policy.mode.on_focus_loss() {
            for pending in self.documents.values_mut() {
                pending.focus_loss = true;
            }
        }
    }

    /// Due but temporarily blocked work stays pending. Excluding past deadlines
    /// from `next_deadline` lets normal input/worker/tick events retry without spin.
    pub fn take_due(&mut self, model: &AppModel, now: Instant) -> Vec<AutoSaveRequest> {
        self.sync(model, now);
        let mut requests = Vec::new();
        for (&id, pending) in &mut self.documents {
            let reason = if pending.focus_loss {
                SaveReason::FocusLoss
            } else if pending.idle.is_some_and(|deadline| deadline <= now) {
                SaveReason::Idle
            } else {
                continue;
            };
            if self.composition == Some(id) || !token::update::auto_save::eligible(model, id) {
                continue;
            }
            pending.idle = None;
            pending.focus_loss = false;
            requests.push(AutoSaveRequest {
                document_id: id,
                revision: pending.revision,
                path: pending.path.clone(),
                policy: self.policy.clone(),
                reason,
            });
        }
        requests.sort_by_key(|request| request.document_id.0);
        requests
    }

    pub fn next_deadline(&self, now: Instant) -> Option<Instant> {
        self.documents
            .values()
            .filter_map(|pending| pending.idle)
            .filter(|deadline| *deadline > now)
            .min()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use token::{
        messages::{DocumentMsg, LayoutMsg, Msg},
        update::update,
    };

    fn model(mode: AutoSaveMode) -> AppModel {
        let mut model = AppModel::new(800, 600, 1.0);
        model.config.auto_save.mode = mode;
        model.document_mut().file_path = Some("/fixture/a.txt".into());
        edit(&mut model);
        model
    }

    fn edit(model: &mut AppModel) {
        update(model, Msg::Document(DocumentMsg::InsertChar('x')));
    }

    #[test]
    fn auto_save_deadlines_are_independent_and_noops_do_not_delay_them() {
        let mut model = model(AutoSaveMode::AfterDelay);
        let a = model.document().id.unwrap();
        let now = Instant::now();
        let mut scheduler = AutoSaveScheduler::default();
        assert!(scheduler.take_due(&model, now).is_empty());
        let first = now + Duration::from_secs(1);
        assert_eq!(scheduler.next_deadline(now), Some(first));
        // An unchanged revision does not reset the deadline even if its effect repeats.
        scheduler.edited(
            &model,
            a,
            model.document().revision,
            now + Duration::from_millis(100),
        );
        update(&mut model, Msg::Layout(LayoutMsg::NewTab));
        model.document_mut().file_path = Some("/fixture/b.txt".into());
        edit(&mut model);
        let b = model.document().id.unwrap();
        let second_edit = now + Duration::from_millis(500);
        scheduler.edited(&model, b, model.document().revision, second_edit);
        assert!(scheduler
            .take_due(&model, first - Duration::from_millis(1))
            .is_empty());
        let requests = scheduler.take_due(&model, first);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].document_id, a);
        assert_eq!(
            scheduler.next_deadline(first),
            Some(second_edit + Duration::from_secs(1))
        );
        let requests = scheduler.take_due(&model, second_edit + Duration::from_secs(1));
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].document_id, b);
        assert!(scheduler
            .take_due(&model, now + Duration::from_secs(5))
            .is_empty());
    }

    #[test]
    fn auto_save_focus_and_idle_modes_do_not_duplicate_a_trigger() {
        for mode in [
            AutoSaveMode::Off,
            AutoSaveMode::OnFocusLoss,
            AutoSaveMode::AfterDelay,
            AutoSaveMode::OnFocusLossAndDelay,
        ] {
            let model = model(mode);
            let now = Instant::now();
            let mut scheduler = AutoSaveScheduler::default();
            scheduler.focus_changed(&model, false, now);
            let first = scheduler.take_due(&model, now);
            assert_eq!(first.len(), usize::from(mode.on_focus_loss()));
            let later = scheduler.take_due(&model, now + Duration::from_secs(2));
            assert_eq!(later.len(), usize::from(mode == AutoSaveMode::AfterDelay));
        }
    }

    #[test]
    fn auto_save_composition_and_pending_writes_defer_without_busy_loop() {
        let mut model = model(AutoSaveMode::OnFocusLossAndDelay);
        let id = model.document().id.unwrap();
        let mut scheduler = AutoSaveScheduler::default();
        let now = Instant::now();
        scheduler.composition = Some(id);
        scheduler.focus_changed(&model, false, now);
        let due = now + Duration::from_secs(1);
        assert!(scheduler.take_due(&model, due).is_empty());
        assert_eq!(scheduler.next_deadline(due), None);
        scheduler.composition = None;
        let requests = scheduler.take_due(&model, due);
        assert_eq!(requests.len(), 1);
        let cmd = update(
            &mut model,
            Msg::App(token::messages::AppMsg::AutoSave(requests[0].clone())),
        )
        .unwrap();
        edit(&mut model);
        scheduler.edited(&model, id, model.document().revision, due);
        assert!(scheduler
            .take_due(&model, due + Duration::from_secs(2))
            .is_empty());
        assert_eq!(scheduler.next_deadline(due + Duration::from_secs(2)), None);
        fn reply(cmd: token::Cmd) -> Option<Msg> {
            match cmd {
                token::Cmd::SaveFile {
                    target,
                    path,
                    content,
                } => Some(Msg::App(token::messages::AppMsg::SaveCompleted {
                    target,
                    path,
                    content,
                    identity: None,
                    result: Ok(()),
                })),
                token::Cmd::Batch(cmds) => cmds.into_iter().find_map(reply),
                _ => None,
            }
        }
        update(&mut model, reply(cmd).unwrap());
        assert_eq!(
            scheduler
                .take_due(&model, due + Duration::from_secs(2))
                .len(),
            1
        );
    }

    #[test]
    fn auto_save_policy_changes_and_closed_documents_drop_old_deadlines() {
        let mut model = model(AutoSaveMode::AfterDelay);
        let mut scheduler = AutoSaveScheduler::default();
        let now = Instant::now();
        scheduler.take_due(&model, now);
        model.config.auto_save.mode = AutoSaveMode::Off;
        assert!(scheduler
            .take_due(&model, now + Duration::from_secs(2))
            .is_empty());
        assert_eq!(scheduler.next_deadline(now), None);
        model.config.auto_save.mode = AutoSaveMode::AfterDelay;
        scheduler.take_due(&model, now);
        let id = model.document().id.unwrap();
        model.editor_area.documents.remove(&id);
        assert!(scheduler
            .take_due(&model, now + Duration::from_secs(2))
            .is_empty());
        assert!(scheduler.documents.is_empty());
    }

    #[test]
    fn auto_save_focus_loss_is_remembered_for_new_edits_while_unfocused() {
        let mut model = model(AutoSaveMode::OnFocusLoss);
        let mut scheduler = AutoSaveScheduler::default();
        let now = Instant::now();
        scheduler.focus_changed(&model, false, now);
        assert_eq!(scheduler.take_due(&model, now).len(), 1);
        edit(&mut model);
        assert_eq!(scheduler.take_due(&model, now).len(), 1);
        assert!(scheduler.take_due(&model, now).is_empty());
    }
}
