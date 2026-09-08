//! Ordered clipboard effects with a live owner for Linux selection requests.

use std::sync::{mpsc, Arc};
use std::thread::JoinHandle;

use token::messages::{AppMsg, Msg};

pub(super) enum Request {
    Copy(String),
    Paste,
}

pub(super) struct ClipboardWorker {
    sender: Option<mpsc::Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}

impl ClipboardWorker {
    pub fn start(
        replies: mpsc::Sender<Msg>,
        wake: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("clipboard".into())
            .spawn(move || {
                let mut clipboard = None;
                for request in receiver {
                    if clipboard.is_none() {
                        clipboard = arboard::Clipboard::new()
                            .map_err(|error| {
                                tracing::warn!("Failed to initialize clipboard: {error}")
                            })
                            .ok();
                    }
                    match request {
                        Request::Copy(text) => {
                            if let Some(clipboard) = clipboard.as_mut() {
                                if let Err(error) = clipboard.set_text(text) {
                                    tracing::warn!("Failed to copy to clipboard: {error}");
                                }
                            }
                        }
                        Request::Paste => {
                            let text = clipboard
                                .as_mut()
                                .and_then(|clipboard| clipboard.get_text().ok())
                                .unwrap_or_default();
                            if replies
                                .send(Msg::App(AppMsg::PasteFromClipboard(text)))
                                .is_ok()
                            {
                                if let Some(wake) = &wake {
                                    wake();
                                }
                            }
                        }
                    }
                }
                // Drop only after the queue closes, not after each copy. X11
                // serves the retained data until another app takes ownership.
            })?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
        })
    }

    pub fn send(&self, request: Request) -> Result<(), mpsc::SendError<Request>> {
        self.sender
            .as_ref()
            .expect("sender exists until exclusive Drop")
            .send(request)
    }
}

impl Drop for ClipboardWorker {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            if thread.join().is_err() {
                tracing::error!("Clipboard worker panicked during shutdown");
            }
        }
    }
}
