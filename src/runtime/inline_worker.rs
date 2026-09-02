//! The completion worker thread for inline suggestions (autocomplete.md
//! Phase 2), cloned from the syntax worker's shape: requests arrive over
//! an mpsc channel, the newest per document wins, the blocking HTTP call
//! runs here, and the result goes back as a `Msg` plus an event-loop wake.
//!
//! Supersession: a request that arrives while another is queued replaces
//! it before any HTTP happens. An in-flight request cannot be aborted
//! (std sockets have no cancel), so it simply completes within its
//! timeout and the update layer's revision guard discards the stale
//! reply. ponytail: good enough for a local server; add a cancel token
//! when a slow remote transport makes the wait visible.

use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender};

use token::completion::fim;
use token::completion::inline::{postprocess, InlineRequest};
use token::messages::{CompletionMsg, Msg};
use token::model::editor_area::DocumentId;
use winit::event_loop::EventLoopProxy;

pub(crate) fn inline_worker_loop(
    rx: Receiver<InlineRequest>,
    msg_tx: Sender<Msg>,
    event_proxy: Option<EventLoopProxy<()>>,
) {
    let mut pending: HashMap<DocumentId, InlineRequest> = HashMap::new();
    loop {
        let Ok(first) = rx.recv() else {
            return;
        };
        pending.insert(first.snapshot.document_id, first);
        while let Ok(request) = rx.try_recv() {
            pending.insert(request.snapshot.document_id, request);
        }
        for (_, request) in pending.drain() {
            let reply = run(&request);
            if msg_tx.send(Msg::Completion(reply)).is_err() {
                return;
            }
            if let Some(proxy) = &event_proxy {
                let _ = proxy.send_event(());
            }
        }
    }
}

/// One request end to end: transport, then the post-processing chain.
/// A backend answer with nothing worth showing is not a failure.
pub(crate) fn run(request: &InlineRequest) -> CompletionMsg {
    match fim::infill(request) {
        Ok(raw) => CompletionMsg::InlineReady {
            snapshot: request.snapshot.clone(),
            text: postprocess(&raw, &request.suffix).unwrap_or_default(),
        },
        Err(error) => CompletionMsg::InlineFailed {
            snapshot: request.snapshot.clone(),
            error,
        },
    }
}
