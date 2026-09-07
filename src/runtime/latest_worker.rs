//! Replaceable speculative work, separate from the ordered file-write queue.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc::Sender, Arc, Condvar, Mutex};

use token::messages::Msg;

struct Job<T> {
    request: T,
    cancelled: Arc<AtomicBool>,
}

struct Pending<T> {
    job: Option<Job<T>>,
    active: Option<Arc<AtomicBool>>,
    stopped: bool,
}

/// At most one active computation and one replaceable pending request. Callers
/// still validate response identity in update; cancellation is only an economy.
pub(super) struct LatestWorker<T, R = Msg> {
    pending: Arc<(Mutex<Pending<T>>, Condvar)>,
    response: std::marker::PhantomData<fn() -> R>,
}

impl<T: Send + 'static, R: Send + 'static> LatestWorker<T, R> {
    pub fn start(
        name: &str,
        sender: Sender<R>,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
        mut compute: impl FnMut(&T, &AtomicBool) -> R + Send + 'static,
        failed: impl Fn(T) -> R + Send + 'static,
    ) -> std::io::Result<Self> {
        let pending = Arc::new((
            Mutex::new(Pending {
                job: None::<Job<T>>,
                active: None,
                stopped: false,
            }),
            Condvar::new(),
        ));
        let shared = Arc::clone(&pending);
        // A filesystem call may be uninterruptible. Shutdown signals cancellation
        // and detaches, never joining speculative work on the UI thread.
        std::thread::Builder::new()
            .name(name.into())
            .spawn(move || loop {
                let (mutex, changed) = &*shared;
                let state = mutex.lock().unwrap_or_else(|error| error.into_inner());
                let mut state = changed
                    .wait_while(state, |state| !state.stopped && state.job.is_none())
                    .unwrap_or_else(|error| error.into_inner());
                if state.stopped {
                    break;
                }
                let Some(job) = state.job.take() else {
                    continue;
                };
                state.active = Some(Arc::clone(&job.cancelled));
                drop(state);
                let reply = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    compute(&job.request, &job.cancelled)
                }))
                .unwrap_or_else(|_| failed(job.request));
                let mut state = mutex.lock().unwrap_or_else(|error| error.into_inner());
                state.active = None;
                let publish = !state.stopped && !job.cancelled.load(Ordering::Relaxed);
                if publish && sender.send(reply).is_err() {
                    break;
                }
                drop(state);
                if publish {
                    if let Some(wake) = &wake {
                        wake();
                    }
                }
            })?;
        Ok(Self {
            pending,
            response: std::marker::PhantomData,
        })
    }

    pub fn submit(&self, request: T) {
        let (mutex, changed) = &*self.pending;
        let mut state = mutex.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        state.job = Some(Job {
            request,
            cancelled: Arc::new(AtomicBool::new(false)),
        });
        changed.notify_one();
    }

    pub fn cancel(&self) {
        let (mutex, _) = &*self.pending;
        let mut state = mutex.lock().unwrap_or_else(|error| error.into_inner());
        state.job = None;
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
    }
}

impl<T, R> Drop for LatestWorker<T, R> {
    fn drop(&mut self) {
        let (mutex, changed) = &*self.pending;
        let mut state = mutex.lock().unwrap_or_else(|error| error.into_inner());
        state.stopped = true;
        state.job = None;
        if let Some(active) = &state.active {
            active.store(true, Ordering::Relaxed);
        }
        changed.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::Duration;
    use token::messages::CompletionMsg;

    #[test]
    fn latest_worker_cancels_running_and_replaces_pending_work() {
        let (sender, replies) = mpsc::channel();
        let (started, starts) = mpsc::channel();
        let (release, releases) = mpsc::channel();
        let caller = std::thread::current().id();
        let worker = LatestWorker::start(
            "latest-test",
            sender,
            None,
            move |request: &usize, cancelled| {
                assert_ne!(caller, std::thread::current().id());
                started.send(*request).unwrap();
                if *request == 1 {
                    releases.recv().unwrap();
                    assert!(cancelled.load(Ordering::Relaxed));
                    Msg::Completion(CompletionMsg::Dismiss)
                } else {
                    Msg::Completion(CompletionMsg::TriggerMenu)
                }
            },
            |_| panic!("unexpected worker failure"),
        )
        .unwrap();
        worker.submit(1);
        assert_eq!(starts.recv_timeout(Duration::from_secs(5)).unwrap(), 1);
        worker.submit(2);
        worker.cancel();
        worker.submit(3);
        release.send(()).unwrap();
        assert_eq!(starts.recv_timeout(Duration::from_secs(5)).unwrap(), 3);
        assert!(matches!(
            replies.recv_timeout(Duration::from_secs(5)).unwrap(),
            Msg::Completion(CompletionMsg::TriggerMenu)
        ));
        assert!(replies.try_recv().is_err());
    }

    #[test]
    fn latest_worker_reports_panic_and_remains_available() {
        let (sender, replies) = mpsc::channel();
        let worker = LatestWorker::start(
            "panic-test",
            sender,
            None,
            |request: &bool, _| {
                assert!(!request, "injected failure");
                Msg::Completion(CompletionMsg::TriggerMenu)
            },
            |_| Msg::Completion(CompletionMsg::Dismiss),
        )
        .unwrap();
        worker.submit(true);
        assert!(matches!(
            replies.recv_timeout(Duration::from_secs(5)).unwrap(),
            Msg::Completion(CompletionMsg::Dismiss)
        ));
        worker.submit(false);
        assert!(matches!(
            replies.recv_timeout(Duration::from_secs(5)).unwrap(),
            Msg::Completion(CompletionMsg::TriggerMenu)
        ));
    }

    #[test]
    fn latest_worker_drop_does_not_join_or_publish_blocked_computation() {
        let (sender, replies) = mpsc::channel();
        let (started, starts) = mpsc::channel();
        let (release, releases) = mpsc::channel();
        let worker = LatestWorker::start(
            "latest-drop-test",
            sender,
            None,
            move |request: &usize, cancelled| {
                started.send(()).unwrap();
                releases.recv().unwrap();
                assert!(cancelled.load(Ordering::Relaxed));
                *request
            },
            |_| 0,
        )
        .unwrap();
        worker.submit(42);
        starts.recv_timeout(Duration::from_secs(5)).unwrap();
        let (dropped, drops) = mpsc::channel();
        let dropper = std::thread::spawn(move || {
            drop(worker);
            dropped.send(()).unwrap();
        });
        drops
            .recv_timeout(Duration::from_secs(5))
            .expect("Drop cannot wait for a blocked file read");
        dropper.join().unwrap();
        release.send(()).unwrap();
        assert!(matches!(
            replies.recv_timeout(Duration::from_secs(5)),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }
}
