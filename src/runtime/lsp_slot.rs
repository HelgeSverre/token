//! `FeatureSlot<P>` — one generic LSP per-feature in-flight request
//! bookkeeping mechanism (definition, hover, references), extracted from
//! the near-identical map trios `LspManager` used to hand-roll per
//! feature. Pure map bookkeeping — no server handles, no I/O — so it
//! unit-tests without a process and borrow-splits cleanly from
//! `LspManager.servers`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use token::lsp::LspServerId;
use token::model::editor_area::DocumentId;

pub(crate) type RequestKey = (LspServerId, PathBuf, i64);

/// What every feature's pending-request payload has in common — the bit
/// `take_due`'s superseded-leftover check needs, so the slot itself
/// never has to know a feature's full payload shape.
pub(crate) trait PendingRequest {
    fn document_id(&self) -> DocumentId;
}

pub(crate) struct FeatureSlot<P> {
    /// Every in-flight request, keyed by the triple the worker's response
    /// echoes back. An entry survives being superseded until its response
    /// actually arrives or its deadline fires (matching `PendingRequests`'
    /// abandoned-entry semantics) — `supersede` only clears `by_doc`.
    pub(crate) requests: HashMap<RequestKey, P>,
    /// Which key is the *current* (non-superseded) request for a
    /// document — looked up to send `$/cancelRequest` when a newer
    /// request supersedes it (design doc's "one outstanding request per
    /// feature per document").
    pub(crate) by_doc: HashMap<DocumentId, RequestKey>,
    /// UI-level abandonment deadlines for in-flight requests, keyed the
    /// same as `requests`.
    pub(crate) deadlines: HashMap<RequestKey, Instant>,
    timeout: Duration,
}

impl<P: PendingRequest> FeatureSlot<P> {
    pub(crate) fn new(timeout: Duration) -> Self {
        Self {
            requests: HashMap::new(),
            by_doc: HashMap::new(),
            deadlines: HashMap::new(),
            timeout,
        }
    }

    /// Registers a new current request for `doc` (`requests` + `by_doc`).
    pub(crate) fn insert(&mut self, key: RequestKey, doc: DocumentId, pending: P) {
        self.requests.insert(key.clone(), pending);
        self.by_doc.insert(doc, key);
    }

    /// Supersede: forget the doc's current request, returning its key so
    /// the caller can send `$/cancelRequest`. The old `requests` entry
    /// deliberately survives until its reply or deadline.
    pub(crate) fn supersede(&mut self, doc: DocumentId) -> Option<RequestKey> {
        self.by_doc.remove(&doc)
    }

    /// Response arrived: the interception-pass bookkeeping — remove from
    /// `requests` and `deadlines`, and from `by_doc` iff this response's
    /// key is still the doc's current one (a later supersede may already
    /// have replaced it).
    pub(crate) fn take_response(&mut self, key: &RequestKey) -> Option<P> {
        let pending = self.requests.remove(key)?;
        self.deadlines.remove(key);
        let doc = pending.document_id();
        if self.by_doc.get(&doc) == Some(key) {
            self.by_doc.remove(&doc);
        }
        Some(pending)
    }

    /// Deadline sweep: returns the *current* requests that expired
    /// (caller cancels + emits the feature's timeout outcome); silently
    /// drops superseded leftovers (the leak fix).
    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<(RequestKey, P)> {
        let due: Vec<RequestKey> = self
            .deadlines
            .iter()
            .filter(|(_, deadline)| now >= **deadline)
            .map(|(key, _)| key.clone())
            .collect();
        let mut out = Vec::new();
        for key in due {
            self.deadlines.remove(&key);
            let is_current = self
                .requests
                .get(&key)
                .is_some_and(|pending| self.by_doc.get(&pending.document_id()) == Some(&key));
            if !is_current {
                // Superseded: the newer request already replaced this
                // entry in `by_doc`, but the old key is still sitting in
                // `requests` — without removing it here it leaks for the
                // rest of the session.
                self.requests.remove(&key);
                continue;
            }
            let Some(pending) = self.requests.remove(&key) else {
                continue;
            };
            self.by_doc.remove(&pending.document_id());
            out.push((key, pending));
        }
        out
    }

    pub(crate) fn arm_deadline(&mut self, key: RequestKey) {
        self.deadlines.insert(key, Instant::now() + self.timeout);
    }

    pub(crate) fn is_empty_deadlines(&self) -> bool {
        self.deadlines.is_empty()
    }

    pub(crate) fn earliest_deadline(&self) -> Option<Instant> {
        self.deadlines.values().min().copied()
    }

    /// The clear-on-restart/exit/toggle triple retain.
    pub(crate) fn clear_for_roots(&mut self, server_id: &LspServerId, roots: &[PathBuf]) {
        self.requests
            .retain(|(id, root, _), _| !(id == server_id && roots.contains(root)));
        self.by_doc
            .retain(|_, (id, root, _)| !(id == server_id && roots.contains(root)));
        self.deadlines
            .retain(|(id, root, _), _| !(id == server_id && roots.contains(root)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Clone)]
    struct Pending {
        doc: DocumentId,
    }

    impl PendingRequest for Pending {
        fn document_id(&self) -> DocumentId {
            self.doc
        }
    }

    fn key(id: i64) -> RequestKey {
        (
            LspServerId::from("rust-analyzer"),
            PathBuf::from("/tmp/root"),
            id,
        )
    }

    fn doc(n: u64) -> DocumentId {
        DocumentId(n)
    }

    #[test]
    fn supersede_keeps_the_request_entry() {
        let mut slot: FeatureSlot<Pending> = FeatureSlot::new(Duration::from_secs(30));
        slot.insert(key(1), doc(1), Pending { doc: doc(1) });

        let old_key = slot.supersede(doc(1));

        assert_eq!(old_key, Some(key(1)));
        assert!(
            slot.requests.contains_key(&key(1)),
            "the superseded request must survive until its reply or deadline"
        );
        assert!(!slot.by_doc.contains_key(&doc(1)));
    }

    #[test]
    fn take_due_drops_superseded_leftovers_without_emitting_them() {
        let mut slot: FeatureSlot<Pending> = FeatureSlot::new(Duration::from_secs(30));
        slot.insert(key(1), doc(1), Pending { doc: doc(1) });
        slot.arm_deadline(key(1));
        // A newer request now owns doc(1); key(1) is superseded but its
        // deadline is still ticking.
        slot.supersede(doc(1));
        slot.insert(key(2), doc(1), Pending { doc: doc(1) });

        let due = slot.take_due(Instant::now() + Duration::from_secs(60));

        assert!(due.is_empty(), "a superseded entry must not be emitted");
        assert!(
            !slot.requests.contains_key(&key(1)),
            "its bookkeeping must still be cleaned up, not leaked"
        );
        assert!(
            slot.requests.contains_key(&key(2)),
            "the current request for the doc must be untouched"
        );
    }

    #[test]
    fn take_due_returns_current_requests_past_their_deadline() {
        let mut slot: FeatureSlot<Pending> = FeatureSlot::new(Duration::from_secs(30));
        slot.insert(key(1), doc(1), Pending { doc: doc(1) });
        slot.arm_deadline(key(1));

        let due = slot.take_due(Instant::now() + Duration::from_secs(60));

        assert_eq!(due, vec![(key(1), Pending { doc: doc(1) })]);
        assert!(slot.requests.is_empty());
        assert!(slot.by_doc.is_empty());
        assert!(slot.deadlines.is_empty());
    }

    #[test]
    fn clear_for_roots_only_removes_the_matching_scope() {
        let mut slot: FeatureSlot<Pending> = FeatureSlot::new(Duration::from_secs(30));
        let other_root_key = (
            LspServerId::from("rust-analyzer"),
            PathBuf::from("/tmp/other"),
            1,
        );
        slot.insert(key(1), doc(1), Pending { doc: doc(1) });
        slot.arm_deadline(key(1));
        slot.insert(other_root_key.clone(), doc(2), Pending { doc: doc(2) });
        slot.arm_deadline(other_root_key.clone());

        slot.clear_for_roots(
            &LspServerId::from("rust-analyzer"),
            &[PathBuf::from("/tmp/root")],
        );

        assert!(!slot.requests.contains_key(&key(1)));
        assert!(!slot.by_doc.contains_key(&doc(1)));
        assert!(!slot.deadlines.contains_key(&key(1)));
        assert!(slot.requests.contains_key(&other_root_key));
        assert!(slot.by_doc.contains_key(&doc(2)));
        assert!(slot.deadlines.contains_key(&other_root_key));
    }
}
