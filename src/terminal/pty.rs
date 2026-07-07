//! Cross-platform PTY spawning via `portable-pty`. Owns the shell process
//! and the read/write worker threads; the terminal-emulation core
//! (`TerminalSession`) is fed PTY output through `Msg::Terminal`, matching
//! the async-worker pattern already used for syntax highlighting and
//! file-system watching.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};

use crate::messages::{Msg, TerminalMsg};

/// How much PTY output to accumulate before forwarding it as a
/// `Msg::Terminal(PtyOutput)`. Matches the plan's "coalesce into chunks"
/// guidance to avoid flooding the message queue during large builds.
const READ_CHUNK_SIZE: usize = 32 * 1024;

/// Maximum time to hold PTY output before forwarding it, so small, infrequent
/// output (e.g. a shell prompt) still reaches the UI promptly.
const READ_FLUSH_INTERVAL: Duration = Duration::from_millis(16);

/// A spawned PTY: the shell process, its master (controlling) side, and a
/// channel for sending bytes to the shell's stdin.
pub struct PtyHandle {
    master: Box<dyn MasterPty + Send>,
    /// Channel used by the write worker to forward input bytes to the shell.
    write_tx: Sender<Vec<u8>>,
    /// Cloneable handle for terminating the child process independently of
    /// the read thread (which may be blocked in `read()`).
    child_killer: Box<dyn portable_pty::ChildKiller + Send + Sync>,
}

impl PtyHandle {
    /// Send bytes to the shell's stdin (e.g. keyboard input, paste text).
    pub fn write(&self, bytes: Vec<u8>) {
        // The write worker thread outlives the handle for as long as the
        // session is alive; a send error means the shell already exited.
        let _ = self.write_tx.send(bytes);
    }

    /// Resize the PTY (and notify the shell via SIGWINCH on Unix).
    pub fn resize(&self, rows: u16, cols: u16) -> std::io::Result<()> {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(std::io::Error::other)
    }

    /// Kill the child shell process and release the PTY master. Safe to call
    /// multiple times; subsequent calls are no-ops once the killer has run.
    pub fn kill(&mut self) {
        let _ = self.child_killer.kill();
    }
}

#[cfg(any(test, debug_assertions))]
impl PtyHandle {
    pub fn new_for_test() -> (Self, mpsc::Receiver<Vec<u8>>) {
        let (write_tx, write_rx) = mpsc::channel();
        (
            Self {
                master: Box::new(TestMasterPty),
                write_tx,
                child_killer: Box::new(TestChildKiller),
            },
            write_rx,
        )
    }
}

#[cfg(any(test, debug_assertions))]
#[derive(Debug)]
struct TestChildKiller;

#[cfg(any(test, debug_assertions))]
impl portable_pty::ChildKiller for TestChildKiller {
    fn kill(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> {
        Box::new(TestChildKiller)
    }
}

#[cfg(any(test, debug_assertions))]
struct TestMasterPty;

#[cfg(any(test, debug_assertions))]
impl MasterPty for TestMasterPty {
    fn resize(&self, _size: PtySize) -> Result<(), anyhow::Error> {
        Ok(())
    }

    fn get_size(&self) -> Result<PtySize, anyhow::Error> {
        Ok(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    fn try_clone_reader(&self) -> Result<Box<dyn Read + Send>, anyhow::Error> {
        Ok(Box::new(std::io::empty()))
    }

    fn take_writer(&self) -> Result<Box<dyn Write + Send>, anyhow::Error> {
        Ok(Box::new(std::io::sink()))
    }

    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<std::os::raw::c_int> {
        None
    }

    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<portable_pty::unix::RawFd> {
        None
    }

    #[cfg(unix)]
    fn tty_name(&self) -> Option<std::path::PathBuf> {
        None
    }
}

/// Detect the user's default shell: `$SHELL`, falling back to a
/// platform-appropriate default.
fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| {
        if cfg!(target_os = "macos") {
            "/bin/zsh".to_string()
        } else {
            "/bin/bash".to_string()
        }
    })
}

/// Spawn a PTY running the user's default shell, rooted at `cwd`.
///
/// Returns a `PtyHandle` for writing to and resizing the PTY. PTY output is
/// read on a dedicated background thread and forwarded as
/// `Msg::Terminal(PtyOutput)`; shell exit is detected on the same thread
/// (read EOF or child exit) and forwarded as `Msg::Terminal(ProcessExited)`.
pub fn spawn_pty(
    cwd: &Path,
    rows: u16,
    cols: u16,
    msg_tx: Sender<Msg>,
    session_id: usize,
) -> std::io::Result<PtyHandle> {
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(std::io::Error::other)?;

    let mut cmd = CommandBuilder::new(default_shell());
    cmd.cwd(cwd);

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(std::io::Error::other)?;
    // Keep a cloneable killer so the handle can terminate the child even
    // while the read thread may be blocked waiting for output.
    let child_killer = child.clone_killer();
    // The slave side is only needed to spawn the child; drop it so the
    // shell (not us) holds the last reference to it, matching how EOF on
    // the master read side is expected to signal shell exit.
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(std::io::Error::other)?;
    let mut writer = pair.master.take_writer().map_err(std::io::Error::other)?;

    // Write thread: keyboard input / paste -> shell stdin.
    let (write_tx, write_rx) = mpsc::channel::<Vec<u8>>();
    thread::Builder::new()
        .name(format!("pty-writer-{session_id}"))
        .spawn(move || {
            while let Ok(bytes) = write_rx.recv() {
                if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                    break;
                }
            }
        })?;

    // Shared accumulator for PTY output coalescing, protected so both the
    // read thread and a flush timer thread can append/flush safely.
    let accumulator = Arc::new(Mutex::new(Vec::with_capacity(READ_CHUNK_SIZE)));
    let flush_done = Arc::new(AtomicBool::new(false));

    // Read thread: PTY output -> shared accumulator.
    {
        let accumulator = Arc::clone(&accumulator);
        let flush_done = Arc::clone(&flush_done);
        let msg_tx = msg_tx.clone();
        thread::Builder::new()
            .name(format!("pty-reader-{session_id}"))
            .spawn(move || {
                let mut buf = [0u8; READ_CHUNK_SIZE];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut acc = accumulator.lock().unwrap();
                            acc.extend_from_slice(&buf[..n]);
                            if acc.len() >= READ_CHUNK_SIZE {
                                let data = std::mem::take(&mut *acc);
                                acc.reserve(READ_CHUNK_SIZE);
                                let _ = msg_tx.send(Msg::Terminal(TerminalMsg::PtyOutput {
                                    session_id,
                                    data,
                                }));
                            }
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                flush_done.store(true, Ordering::Relaxed);
            })?;
    }

    // Flush timer thread: forwards accumulated output every 16 ms so small
    // amounts of data (e.g. a shell prompt) don't sit in the buffer while
    // the read thread is blocked waiting for more PTY output.
    {
        let accumulator = Arc::clone(&accumulator);
        let flush_done = Arc::clone(&flush_done);
        thread::Builder::new()
            .name(format!("pty-flush-{session_id}"))
            .spawn(move || {
                while !flush_done.load(Ordering::Relaxed) {
                    thread::sleep(READ_FLUSH_INTERVAL);
                    let mut acc = accumulator.lock().unwrap();
                    if !acc.is_empty() {
                        let data = std::mem::take(&mut *acc);
                        acc.reserve(READ_CHUNK_SIZE);
                        let _ =
                            msg_tx.send(Msg::Terminal(TerminalMsg::PtyOutput { session_id, data }));
                    }
                }

                // Final flush after the read thread has exited.
                let mut acc = accumulator.lock().unwrap();
                if !acc.is_empty() {
                    let data = std::mem::take(&mut *acc);
                    let _ = msg_tx.send(Msg::Terminal(TerminalMsg::PtyOutput { session_id, data }));
                }

                let code = match child.try_wait() {
                    Ok(Some(status)) => status.exit_code() as i32,
                    _ => 0,
                };
                let _ = msg_tx.send(Msg::Terminal(TerminalMsg::ProcessExited {
                    session_id,
                    code,
                }));
            })?;
    }

    Ok(PtyHandle {
        master: pair.master,
        write_tx,
        child_killer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// Poll `msg_rx` until a `PtyOutput` chunk is received whose bytes
    /// contain `needle`, or the timeout elapses.
    fn recv_output_containing(
        msg_rx: &mpsc::Receiver<Msg>,
        needle: &str,
        timeout: Duration,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        let mut collected = Vec::new();
        while Instant::now() < deadline {
            match msg_rx.recv_timeout(Duration::from_millis(50)) {
                Ok(Msg::Terminal(TerminalMsg::PtyOutput { data, .. })) => {
                    collected.extend_from_slice(&data);
                    if String::from_utf8_lossy(&collected).contains(needle) {
                        return true;
                    }
                }
                Ok(_) => {}
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        false
    }

    #[test]
    fn spawns_shell_and_echoes_output() {
        let (msg_tx, msg_rx) = mpsc::channel();
        let cwd = std::env::temp_dir();
        let pty = spawn_pty(&cwd, 24, 80, msg_tx, 0).expect("failed to spawn PTY");

        pty.write(b"echo hello-from-pty\n".to_vec());

        assert!(
            recv_output_containing(&msg_rx, "hello-from-pty", Duration::from_secs(5)),
            "expected PTY output to contain the echoed string within 5s"
        );
    }

    #[test]
    fn process_exited_is_sent_after_shell_quits() {
        let (msg_tx, msg_rx) = mpsc::channel();
        let cwd = std::env::temp_dir();
        let pty = spawn_pty(&cwd, 24, 80, msg_tx, 7).expect("failed to spawn PTY");

        pty.write(b"exit\n".to_vec());

        let deadline = Instant::now() + Duration::from_secs(5);
        let mut saw_exit = false;
        while Instant::now() < deadline {
            match msg_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(Msg::Terminal(TerminalMsg::ProcessExited { session_id, .. })) => {
                    assert_eq!(session_id, 7);
                    saw_exit = true;
                    break;
                }
                Ok(_) => {}
                Err(_) => {}
            }
        }
        assert!(saw_exit, "expected ProcessExited within 5s of shell exit");
    }
}
