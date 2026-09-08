//! A bounded latest-request worker. A new watch value cancels the active
//! provider future, including its socket wait; no per-request threads are spawned.

use std::sync::{mpsc::Sender, Arc};
use token::completion::fim::{self, FimProvider};
use token::completion::inline::{postprocess, MAX_ALTERNATIVES};
use token::completion::postprocess::InlinePostprocessor;
use token::completion::provider::{InlineJob, InlineProvider};
use token::messages::{CompletionMsg, Msg};
use tokio::sync::watch;
use winit::event_loop::EventLoopProxy;

use super::inline_cache::InlineCache;

pub(crate) fn inline_worker_loop(
    rx: watch::Receiver<Option<Arc<InlineJob>>>,
    msg_tx: Sender<Msg>,
    event_proxy: Option<EventLoopProxy<()>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!("cannot start inline worker runtime: {error}");
            return;
        }
    };
    runtime.block_on(work(rx, msg_tx, event_proxy));
    // DNS may use Tokio's blocking resolver; never wait for it during app exit.
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
}

async fn work(
    mut rx: watch::Receiver<Option<Arc<InlineJob>>>,
    msg_tx: Sender<Msg>,
    event_proxy: Option<EventLoopProxy<()>>,
) {
    let client = fim::client();
    let mut cache = InlineCache::default();
    let mut postprocessor = InlinePostprocessor::default();
    let mut wait_for_change = false;
    loop {
        if wait_for_change && rx.changed().await.is_err() {
            return;
        }
        let job = { rx.borrow_and_update().clone() };
        let Some(job) = job else {
            wait_for_change = true;
            continue;
        };
        wait_for_change = false;
        let reply = tokio::select! {
            biased;
            changed = rx.changed() => {
                if changed.is_err() { return; }
                continue;
            }
            reply = async {
                let provider = match &client {
                    Ok(client) => FimProvider::new(client.clone(), job.provider.clone()),
                    Err(_) => Err(token::completion::provider::ProviderError::Transport),
                };
                match provider {
                    Ok(mut provider) => run(&mut provider, &job, &mut cache, &mut postprocessor).await,
                    Err(error) => CompletionMsg::InlineFailed {
                        snapshot: job.request.snapshot.clone(), error: error.to_string(),
                    },
                }
            } => reply,
        };
        if msg_tx.send(Msg::Completion(reply)).is_err() {
            return;
        }
        if let Some(proxy) = &event_proxy {
            let _ = proxy.send_event(());
        }
        wait_for_change = true;
    }
}

/// All providers share response snapshots and post-processing. The worker owns
/// cancellation by dropping this future; it never reports cancellation as failure.
async fn run(
    provider: &mut dyn InlineProvider,
    job: &InlineJob,
    cache: &mut InlineCache,
    postprocessor: &mut InlinePostprocessor,
) -> CompletionMsg {
    let request = &job.request;
    let texts = if let Some(texts) = cache.get(job) {
        texts
    } else {
        match provider.suggest(request).await {
            Ok(raw) => raw
                .into_iter()
                .take(MAX_ALTERNATIVES)
                .filter_map(|text| postprocess(&text, &request.suffix))
                .collect::<Vec<_>>(),
            Err(error) => {
                return CompletionMsg::InlineFailed {
                    snapshot: request.snapshot.clone(),
                    error: error.to_string(),
                }
            }
        }
    };
    let served = postprocessor.serve_all(&texts, job.context.as_ref());
    cache.insert(job, &texts, &served);
    CompletionMsg::InlineReady {
        snapshot: request.snapshot.clone(),
        texts: served,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::time::Duration;
    use token::completion::inline::{InlineRequest, RequestSnapshot};
    use token::completion::provider::SuggestionFuture;
    use token::config::ProviderConfig;
    use token::model::DocumentId;

    fn job(url: String, id: u64) -> Arc<InlineJob> {
        Arc::new(InlineJob {
            context: None,
            request: InlineRequest {
                snapshot: RequestSnapshot {
                    document_id: DocumentId(1),
                    revision: 1,
                    line: 0,
                    column: 0,
                    request_id: id,
                },
                prefix: String::new(),
                suffix: "\n".into(),
                language: None,
                file_path: None,
                extra_context: Vec::new(),
                explicit: false,
            },
            provider: ProviderConfig {
                url,
                timeout_ms: 15_000,
                ..Default::default()
            },
        })
    }

    /// A server that either answers once, or waits for the client to disconnect.
    /// Handshakes avoid sleeps and prove cancellation of an active socket read.
    fn server(stall: bool) -> (String, mpsc::Receiver<()>, mpsc::Receiver<bool>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let (seen_tx, seen_rx) = mpsc::channel();
        let (closed_tx, closed_rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert_ne!(reader.read_line(&mut line).unwrap(), 0);
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
                if line == "\r\n" {
                    break;
                }
            }
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let _ = seen_tx.send(());
            if stall {
                let closed = match reader.read(&mut [0]) {
                    Ok(0) => true,
                    Err(error) => matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::ConnectionAborted
                    ),
                    _ => false,
                };
                let _ = closed_tx.send(closed);
            } else {
                let body = r#"{"content":"new answer"}"#;
                stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).as_bytes()).unwrap();
            }
        });
        (url, seen_rx, closed_rx)
    }

    fn worker() -> (
        watch::Sender<Option<Arc<InlineJob>>>,
        mpsc::Receiver<Msg>,
        mpsc::Receiver<()>,
    ) {
        let (tx, rx) = watch::channel(None);
        let (reply_tx, reply_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::spawn(move || {
            inline_worker_loop(rx, reply_tx, None);
            let _ = done_tx.send(());
        });
        (tx, reply_rx, done_rx)
    }

    #[test]
    fn supersession_disconnects_old_socket_and_serves_new_request() {
        let (tx, replies, done) = worker();
        let (url, seen, closed) = server(true);
        tx.send(Some(job(url, 1))).unwrap();
        seen.recv_timeout(Duration::from_secs(3)).unwrap();
        let (url, _, _) = server(false);
        tx.send(Some(job(url, 2))).unwrap();
        assert!(closed.recv_timeout(Duration::from_secs(3)).unwrap());
        let reply = replies.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            matches!(reply, Msg::Completion(CompletionMsg::InlineReady { snapshot, texts }) if snapshot.request_id == 2 && texts == vec!["new answer"])
        );
        assert!(
            replies.try_recv().is_err(),
            "cancellation is not a backend failure"
        );
        drop(tx);
        done.recv_timeout(Duration::from_secs(3)).unwrap();
    }

    #[test]
    fn dismissal_and_sender_drop_cancel_socket_waits() {
        assert_dismissal_and_sender_drop_cancel_socket_waits(
            token::config::TransportKind::LlamaCpp,
        );
    }

    #[test]
    fn tabby_dismissal_and_sender_drop_cancel_socket_waits() {
        assert_dismissal_and_sender_drop_cancel_socket_waits(token::config::TransportKind::Tabby);
    }

    fn assert_dismissal_and_sender_drop_cancel_socket_waits(
        transport: token::config::TransportKind,
    ) {
        for explicit_cancel in [true, false] {
            let (tx, replies, done) = worker();
            let (url, seen, closed) = server(true);
            let mut request = job(url, 1);
            Arc::make_mut(&mut request).provider.transport = transport;
            tx.send(Some(request)).unwrap();
            seen.recv_timeout(Duration::from_secs(3)).unwrap();
            if explicit_cancel {
                tx.send(None).unwrap();
                assert!(closed.recv_timeout(Duration::from_secs(3)).unwrap());
                assert!(replies.try_recv().is_err());
                drop(tx);
            } else {
                drop(tx);
                assert!(closed.recv_timeout(Duration::from_secs(3)).unwrap());
            }
            done.recv_timeout(Duration::from_secs(3)).unwrap();
        }
    }

    #[tokio::test]
    async fn cache_replays_normalized_choices_with_current_snapshots_and_explicit_refresh() {
        struct Counting(usize);
        impl InlineProvider for Counting {
            fn suggest<'a>(&'a mut self, _request: &'a InlineRequest) -> SuggestionFuture<'a> {
                self.0 += 1;
                Box::pin(async {
                    Ok(vec![
                        "hello_world<|fim_suffix|>leak".into(),
                        "hello_again".into(),
                    ])
                })
            }
        }
        let mut provider = Counting(0);
        let mut cache = InlineCache::default();
        let mut processor = InlinePostprocessor::default();
        let mut request = (*job(String::new(), 1)).clone();
        let first = run(&mut provider, &request, &mut cache, &mut processor).await;
        assert!(
            matches!(first, CompletionMsg::InlineReady { texts, .. } if texts == ["hello_world", "hello_again"])
        );
        request.request.prefix = "hello_".into();
        request.request.snapshot.column = 6;
        request.request.snapshot.revision = 2;
        request.request.snapshot.request_id = 2;
        let replay = run(&mut provider, &request, &mut cache, &mut processor).await;
        assert!(
            matches!(replay, CompletionMsg::InlineReady { snapshot, texts } if snapshot == request.request.snapshot && texts == ["world", "again"])
        );
        assert_eq!(provider.0, 1);
        request.request.explicit = true;
        run(&mut provider, &request, &mut cache, &mut processor).await;
        assert_eq!(provider.0, 2, "explicit refresh calls the provider");
        request.request.explicit = false;
        request.provider.max_tokens += 1;
        run(&mut provider, &request, &mut cache, &mut processor).await;
        assert_eq!(provider.0, 3, "configuration changes miss");
    }

    #[test]
    fn worker_cache_serves_a_new_revision_after_the_backend_has_closed() {
        let (tx, replies, done) = worker();
        let (url, seen, _) = server(false);
        let first = job(url, 1);
        tx.send(Some(first.clone())).unwrap();
        seen.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(
            matches!(replies.recv_timeout(Duration::from_secs(3)).unwrap(), Msg::Completion(CompletionMsg::InlineReady { texts, .. }) if texts == ["new answer"])
        );
        let mut next = (*first).clone();
        next.request.snapshot.request_id = 2;
        next.request.snapshot.revision = 8;
        next.request.prefix = "new ".into();
        next.request.snapshot.column = 4;
        tx.send(Some(Arc::new(next))).unwrap();
        assert!(
            matches!(replies.recv_timeout(Duration::from_secs(3)).unwrap(), Msg::Completion(CompletionMsg::InlineReady { snapshot, texts }) if snapshot.request_id == 2 && snapshot.revision == 8 && texts == ["answer"])
        );
        drop(tx);
        done.recv_timeout(Duration::from_secs(3)).unwrap();
    }

    #[tokio::test]
    async fn cache_hits_reapply_filters_with_current_local_context() {
        use token::completion::postprocess::InlineContext;
        use token::model::Document;
        use token::syntax::LanguageId;

        struct Counting(usize);
        impl InlineProvider for Counting {
            fn suggest<'a>(&'a mut self, _request: &'a InlineRequest) -> SuggestionFuture<'a> {
                self.0 += 1;
                Box::pin(async { Ok(vec!["next();\n\tmore();".into()]) })
            }
        }
        let mut provider = Counting(0);
        let mut cache = InlineCache::default();
        let mut processor = InlinePostprocessor::default();
        let mut request = (*job(String::new(), 1)).clone();
        // The bounded generation context is identical; style elsewhere in the
        // local document has changed since generation and must be re-evaluated.
        for (indent, expected) in [
            ("    ", "next();\n    more();"),
            ("\t", "next();\n\tmore();"),
        ] {
            let mut document = Document::with_text(&format!("fn f() {{\n{indent}old();\n\n}}\n"));
            document.language = LanguageId::Rust;
            request.context = InlineContext::capture(&document, (2, 0));
            let result = run(&mut provider, &request, &mut cache, &mut processor).await;
            assert!(
                matches!(result, CompletionMsg::InlineReady { texts, .. } if texts == [expected])
            );
        }
        assert_eq!(provider.0, 1);
    }

    #[tokio::test]
    async fn failures_are_not_cached() {
        struct Failing(usize);
        impl InlineProvider for Failing {
            fn suggest<'a>(&'a mut self, _request: &'a InlineRequest) -> SuggestionFuture<'a> {
                self.0 += 1;
                Box::pin(async { Err(token::completion::provider::ProviderError::Transport) })
            }
        }
        let mut provider = Failing(0);
        let mut cache = InlineCache::default();
        let mut processor = InlinePostprocessor::default();
        let request = job(String::new(), 1);
        for _ in 0..2 {
            assert!(matches!(
                run(&mut provider, &request, &mut cache, &mut processor).await,
                CompletionMsg::InlineFailed { .. }
            ));
        }
        assert_eq!(provider.0, 2);
    }

    #[tokio::test]
    async fn a_non_http_provider_uses_the_same_response_pipeline() {
        struct Heuristic;
        impl InlineProvider for Heuristic {
            fn suggest<'a>(&'a mut self, _request: &'a InlineRequest) -> SuggestionFuture<'a> {
                Box::pin(async {
                    Ok(vec![
                        "close_block();<|fim_suffix|>leaked".into(),
                        "<|fim_middle|>not a suggestion".into(),
                        "other_block();".into(),
                    ])
                })
            }
        }
        let job = job(String::new(), 42);
        let reply = run(
            &mut Heuristic,
            &job,
            &mut InlineCache::default(),
            &mut InlinePostprocessor::default(),
        )
        .await;
        assert!(
            matches!(reply, CompletionMsg::InlineReady { snapshot, texts } if snapshot.request_id == 42 && texts == vec!["close_block();", "other_block();"])
        );
    }
}
