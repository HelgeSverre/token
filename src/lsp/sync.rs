//! Pure document-sync bookkeeping: the `didChange` debounce/max-wait
//! map and the LSP `languageId` string mapping
//! (docs/feature/lsp-integration.md "Document Synchronization").
//!
//! No I/O, no threads — the runtime's `LspManager`
//! (`runtime/app.rs`) owns the actual sends; this module is just the
//! bookkeeping, kept testable in isolation the same way `client.rs`'s
//! `HandshakeGate`/`PendingRequests` are.

use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};

use crate::syntax::LanguageId;

/// Debounce class for `didChange`, matching the syntax worker's
/// `SYNTAX_DEBOUNCE_MS`.
pub const DID_CHANGE_DEBOUNCE_MS: u64 = 30;

/// Hard cap: a `didChange` fires at latest this long after the *first*
/// pending edit, even under continuous typing. An uncapped trailing
/// debounce (what the syntax worker does) would let the server drift
/// unboundedly behind the buffer during a long typing burst.
pub const DID_CHANGE_MAX_WAIT_MS: u64 = 300;

/// LSP-conventional `languageId` string for `didOpen`
/// (docs/feature/lsp-integration.md "URIs and Paths"). `None` for
/// languages with no registered server def — nothing else needs one.
pub fn language_id_str(language: LanguageId) -> Option<&'static str> {
    use LanguageId::*;
    Some(match language {
        Rust => "rust",
        TypeScript => "typescript",
        Tsx => "typescriptreact",
        JavaScript => "javascript",
        Jsx => "javascriptreact",
        Python => "python",
        Php => "php",
        Sema => "sema",
        _ => return None,
    })
}

#[derive(Debug, Clone, Copy)]
struct PendingChange {
    first_edit_at: Instant,
    deadline: Instant,
    revision: u64,
}

/// Deadline map for debounced `didChange`, keyed by an arbitrary
/// `Copy + Eq + Hash` document key (`DocumentId` in the runtime — kept
/// generic here so this stays unit-testable without pulling in the
/// model). Shaped like `App::syntax_deadlines` but adds the max-wait cap
/// the syntax debounce doesn't need.
pub struct DidChangeDeadlines<K: Eq + Hash + Copy> {
    entries: HashMap<K, PendingChange>,
}

impl<K: Eq + Hash + Copy> Default for DidChangeDeadlines<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Eq + Hash + Copy> DidChangeDeadlines<K> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Records an edit at `now`. Extends the debounce deadline but never
    /// past `first_edit_at + max_wait` — the cap described above.
    pub fn record_edit(
        &mut self,
        key: K,
        revision: u64,
        now: Instant,
        debounce: Duration,
        max_wait: Duration,
    ) {
        let first_edit_at = self.entries.get(&key).map_or(now, |p| p.first_edit_at);
        let deadline = (now + debounce).min(first_edit_at + max_wait);
        self.entries.insert(
            key,
            PendingChange {
                first_edit_at,
                deadline,
                revision,
            },
        );
    }

    pub fn is_pending(&self, key: K) -> bool {
        self.entries.contains_key(&key)
    }

    /// Removes and returns the pending revision for `key`, regardless of
    /// whether its deadline has elapsed yet — the flush-before-request
    /// (and flush-before-close) path.
    pub fn take(&mut self, key: K) -> Option<u64> {
        self.entries.remove(&key).map(|p| p.revision)
    }

    /// Removes and returns every entry whose deadline has elapsed.
    pub fn take_expired(&mut self, now: Instant) -> Vec<(K, u64)> {
        let due: Vec<K> = self
            .entries
            .iter()
            .filter(|(_, p)| now >= p.deadline)
            .map(|(k, _)| *k)
            .collect();
        due.into_iter()
            .map(|k| (k, self.entries.remove(&k).unwrap().revision))
            .collect()
    }

    /// Earliest deadline across all pending documents, folded into
    /// `about_to_wait`'s `next_wake` computation.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.entries.values().map(|p| p.deadline).min()
    }
}

/// The flush-before-request invariant
/// (docs/feature/lsp-integration.md "Document Synchronization"): if
/// there's a pending `didChange` for the document a request targets, it
/// must be written to the outbound channel *before* the request frame —
/// never after, and never dropped. Pure ordering logic; the runtime just
/// forwards each frame to the worker's `outbound_tx` in the order
/// returned here.
pub fn flush_before_request<T>(pending_did_change: Option<T>, request: T) -> Vec<T> {
    let mut frames = Vec::with_capacity(2);
    frames.extend(pending_did_change);
    frames.push(request);
    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_id_maps_ts_family_to_lsp_conventional_strings() {
        assert_eq!(language_id_str(LanguageId::Rust), Some("rust"));
        assert_eq!(language_id_str(LanguageId::TypeScript), Some("typescript"));
        assert_eq!(
            language_id_str(LanguageId::Tsx),
            Some("typescriptreact")
        );
        assert_eq!(language_id_str(LanguageId::JavaScript), Some("javascript"));
        assert_eq!(
            language_id_str(LanguageId::Jsx),
            Some("javascriptreact")
        );
        assert_eq!(language_id_str(LanguageId::Python), Some("python"));
    }

    #[test]
    fn language_id_is_none_for_unregistered_languages() {
        assert_eq!(language_id_str(LanguageId::PlainText), None);
        assert_eq!(language_id_str(LanguageId::Markdown), None);
    }

    #[test]
    fn first_edit_sets_a_plain_debounce_deadline() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        let now = Instant::now();
        let debounce = Duration::from_millis(30);
        let max_wait = Duration::from_millis(300);
        map.record_edit(1, 5, now, debounce, max_wait);
        assert!(map.is_pending(1));

        let expired = map.take_expired(now + Duration::from_millis(31));
        assert_eq!(expired, vec![(1, 5)]);
        assert!(!map.is_pending(1));
    }

    #[test]
    fn continuous_edits_do_not_fire_before_the_plain_debounce_elapses() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        let debounce = Duration::from_millis(30);
        let max_wait = Duration::from_millis(300);
        let start = Instant::now();

        // Re-edit every 10ms, well under the 30ms debounce each time.
        for i in 0..5 {
            map.record_edit(1, i, start + Duration::from_millis(i * 10), debounce, max_wait);
        }
        // 20ms after the last edit (t=40): debounce not yet elapsed for the
        // latest edit (would need t=70).
        assert!(map.take_expired(start + Duration::from_millis(60)).is_empty());
    }

    #[test]
    fn max_wait_cap_fires_under_continuous_typing() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        let debounce = Duration::from_millis(30);
        let max_wait = Duration::from_millis(300);
        let start = Instant::now();

        // An edit every 20ms for 400ms — the plain debounce would never
        // fire, but the max-wait cap forces it at start+300ms.
        for i in 0..20 {
            let revision = i;
            map.record_edit(
                1,
                revision,
                start + Duration::from_millis(i * 20),
                debounce,
                max_wait,
            );
        }

        // Just before the cap: still pending.
        assert!(map
            .take_expired(start + Duration::from_millis(299))
            .is_empty());

        // At the cap: fires, carrying whatever revision was current then.
        let expired = map.take_expired(start + Duration::from_millis(300));
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, 1);
    }

    #[test]
    fn take_flushes_a_not_yet_expired_entry() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        let now = Instant::now();
        map.record_edit(
            7,
            3,
            now,
            Duration::from_millis(30),
            Duration::from_millis(300),
        );
        // Not expired yet, but `take` (flush) returns it anyway.
        assert!(map.take_expired(now).is_empty());
        assert_eq!(map.take(7), Some(3));
        assert!(!map.is_pending(7));
    }

    #[test]
    fn take_on_absent_key_is_none_not_a_panic() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        assert_eq!(map.take(42), None);
    }

    #[test]
    fn revisions_only_increase_across_a_burst() {
        let mut map: DidChangeDeadlines<u64> = DidChangeDeadlines::new();
        let start = Instant::now();
        let mut last_seen = None;
        for i in 1..=10u64 {
            map.record_edit(
                1,
                i,
                start + Duration::from_millis(i * 40), // outpaces the 30ms debounce each time
                Duration::from_millis(30),
                Duration::from_millis(300),
            );
            for (_, revision) in map.take_expired(start + Duration::from_millis(i * 40 + 31)) {
                if let Some(prev) = last_seen {
                    assert!(revision > prev, "revisions must strictly increase");
                }
                last_seen = Some(revision);
            }
        }
        assert_eq!(last_seen, Some(10));
    }

    #[test]
    fn flush_before_request_orders_pending_change_first() {
        let frames = flush_before_request(Some("didChange"), "definition");
        assert_eq!(frames, vec!["didChange", "definition"]);
    }

    #[test]
    fn flush_before_request_with_nothing_pending_sends_only_the_request() {
        let frames = flush_before_request(None, "definition");
        assert_eq!(frames, vec!["definition"]);
    }
}
