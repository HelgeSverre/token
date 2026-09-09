//! At most one running Find scan and one replaceable pending snapshot.
use std::sync::{mpsc::Sender, Arc};

use token::messages::{Msg, UiMsg};
use token::model::ui::{FindResults, FindSearchRequest};

pub(super) struct FindWorker(super::latest_worker::LatestWorker<Arc<FindSearchRequest>>);

impl FindWorker {
    pub fn start(
        sender: Sender<Msg>,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> std::io::Result<Self> {
        Self::start_with(sender, wake, |request| request.compute())
    }

    fn start_with(
        sender: Sender<Msg>,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
        compute: impl Fn(Arc<FindSearchRequest>) -> Arc<FindResults> + Send + 'static,
    ) -> std::io::Result<Self> {
        super::latest_worker::LatestWorker::start(
            "find-search",
            sender,
            wake,
            move |request: &Arc<FindSearchRequest>, _| {
                Msg::Ui(UiMsg::FindSearchCompleted {
                    request: Arc::clone(request),
                    result: Ok(compute(Arc::clone(request))),
                })
            },
            |request| {
                Msg::Ui(UiMsg::FindSearchCompleted {
                    request,
                    result: Err("Find worker panicked".into()),
                })
            },
        )
        .map(Self)
    }

    pub fn submit(&self, request: Arc<FindSearchRequest>) {
        self.0.submit(request);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    use token::model::{AppModel, FindReplaceState};
    use token::Cmd;

    fn request(pattern: &str) -> Arc<FindSearchRequest> {
        fn extract(cmd: Cmd) -> Option<Arc<FindSearchRequest>> {
            match cmd {
                Cmd::RunFindSearch(request) => Some(request),
                Cmd::Batch(cmds) => cmds.into_iter().find_map(extract),
                _ => None,
            }
        }
        let mut model = AppModel::new(80, 60, 1.0);
        model.document_mut().buffer = "foo ".repeat(70_000).into();
        let mut state = FindReplaceState::default();
        state.set_query(pattern);
        model.ui.open_find(state);
        extract(token::update::update(&mut model, Msg::Ui(UiMsg::BlinkCursor)).unwrap()).unwrap()
    }

    #[test]
    fn find_async_worker_coalesces_pending_requests_and_runs_off_thread() {
        let (sender, replies) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let caller = std::thread::current().id();
        let worker = FindWorker::start_with(sender, None, move |request| {
            assert_ne!(std::thread::current().id(), caller);
            started_tx.send(Arc::clone(&request)).unwrap();
            release_rx.recv().unwrap();
            request.compute()
        })
        .unwrap();
        let first = request("foo");
        let discarded = request("bar");
        let latest = request("absent");
        worker.submit(Arc::clone(&first));
        assert!(Arc::ptr_eq(
            &started_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            &first
        ));
        worker.submit(discarded);
        worker.submit(Arc::clone(&latest));
        release_tx.send(()).unwrap();
        assert!(Arc::ptr_eq(
            &started_rx.recv_timeout(Duration::from_secs(5)).unwrap(),
            &latest
        ));
        assert!(
            replies.try_recv().is_err(),
            "superseded running result must not publish"
        );
        release_tx.send(()).unwrap();
        let Msg::Ui(UiMsg::FindSearchCompleted { request, result }) =
            replies.recv_timeout(Duration::from_secs(5)).unwrap()
        else {
            panic!("search reply")
        };
        assert!(Arc::ptr_eq(&request, &latest));
        assert!(result.is_ok());
    }

    #[test]
    fn find_async_worker_drop_does_not_wait_for_computation_or_publish_after_stop() {
        let (sender, replies) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let worker = FindWorker::start_with(sender, None, move |request| {
            started_tx.send(()).unwrap();
            release_rx.recv().unwrap();
            request.compute()
        })
        .unwrap();
        worker.submit(request("foo"));
        started_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        let (dropped_tx, dropped_rx) = mpsc::channel();
        let dropper = std::thread::spawn(move || {
            drop(worker);
            dropped_tx.send(()).unwrap();
        });
        dropped_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("Drop must not join the blocked scan");
        dropper.join().unwrap();
        release_tx.send(()).unwrap();
        assert!(matches!(
            replies.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
