//! Bounded asynchronous stdin/stdout command formatting.
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{mpsc::Sender, Arc};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use token::config::FormatterConfig;
use token::messages::{FormattingMsg, Msg};
use token::model::{DocumentId, SaveIntent};
use token::syntax::LanguageId;
use token::util::file_validation::MAX_FILE_SIZE;
use token::util::ByteSize;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};

pub(super) struct Job {
    pub document_id: DocumentId,
    pub revision: u64,
    pub formatter: FormatterConfig,
    pub text: String,
    pub file: Option<PathBuf>,
    pub workspace: Option<PathBuf>,
    pub language: LanguageId,
    pub save: Option<SaveIntent>,
    pub request: Arc<()>,
}

pub(super) fn spawn(job: Job, tx: Sender<Msg>, wake: Option<Arc<dyn Fn() + Send + Sync>>) {
    std::thread::spawn(move || {
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .context("Starting formatter runtime")
            .and_then(|runtime| runtime.block_on(run(&job, Duration::from_secs(5))))
            .map_err(|error| format!("{}: {error:#}", job.formatter.command));
        let _ = tx.send(Msg::Formatting(FormattingMsg::ExternalResolved {
            document_id: job.document_id,
            language: job.language,
            revision: job.revision,
            request: job.request,
            result,
            save: job.save,
        }));
        if let Some(wake) = wake {
            wake();
        }
    });
}

async fn read_bounded(reader: impl AsyncRead + Unpin, limit: ByteSize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(limit.as_u64() + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() as u64 > limit.as_u64() {
        bail!("Formatter output exceeds {limit}");
    }
    Ok(bytes)
}

fn context_paths(job: &Job) -> Result<(PathBuf, PathBuf)> {
    let base = job
        .workspace
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let file = job
        .file
        .clone()
        .unwrap_or_else(|| base.join(job.language.untitled_filename()));
    let file = if file.is_absolute() {
        file
    } else {
        std::env::current_dir()?.join(file)
    };
    let cwd = file
        .parent()
        .context("Formatter filename has no parent")?
        .to_path_buf();
    Ok((file, cwd))
}

async fn run(job: &Job, timeout: Duration) -> Result<String> {
    let command = &job.formatter.command;
    if command.trim().is_empty() || command.contains(['\0', '\n', '\r']) {
        bail!("Choose a valid formatter executable in Settings > Formatting");
    }
    let (file, cwd) = context_paths(job)?;
    let filename = file.to_str().context("Formatter filename is not UTF-8")?;
    let args: Vec<_> = job
        .formatter
        .args
        .iter()
        .map(|arg| arg.replace("{file}", filename))
        .collect();
    let executable = token::lsp::client::resolve_command(command);
    let mut child = tokio::process::Command::new(executable)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .context(
            "Starting formatter; install it or configure its executable in Settings > Formatting",
        )?;
    let mut stdin = child.stdin.take().context("Opening formatter stdin")?;
    let stdout = child.stdout.take().context("Opening formatter stdout")?;
    let stderr = child.stderr.take().context("Opening formatter stderr")?;
    let result = {
        let work = async {
            let writer = async {
                stdin.write_all(job.text.as_bytes()).await?;
                stdin.shutdown().await?;
                drop(stdin);
                Ok::<_, anyhow::Error>(())
            };
            let wait = async { child.wait().await.map_err(anyhow::Error::from) };
            let (_, out, err, status) = tokio::try_join!(
                writer,
                read_bounded(stdout, MAX_FILE_SIZE),
                read_bounded(stderr, ByteSize::kibibytes(64)),
                wait
            )?;
            if !status.success() {
                bail!(
                    "Exited with {status}: {}",
                    String::from_utf8_lossy(&err).trim()
                );
            }
            String::from_utf8(out).context("Formatter returned invalid UTF-8")
        };
        let cancelled = async {
            loop {
                if Arc::strong_count(&job.request) == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
        };
        tokio::select! {
            result = work => result,
            _ = tokio::time::sleep(timeout) => Err(anyhow::anyhow!("Timed out after {} seconds", timeout.as_secs_f32())),
            _ = cancelled => Err(anyhow::anyhow!("Formatting request superseded or document closed")),
        }
    };
    if result.is_err() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
    result
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn job(command: &str, args: &[&str]) -> (tempfile::TempDir, Arc<()>, Job) {
        let dir = tempfile::tempdir().unwrap();
        let request = Arc::new(());
        let job = Job {
            document_id: DocumentId(1),
            revision: 0,
            formatter: FormatterConfig {
                preset_id: None,
                enabled: true,
                command: command.into(),
                args: args.iter().map(|arg| (*arg).into()).collect(),
            },
            text: "unsaved 🐍 = 1\n".into(),
            file: Some(dir.path().join("file with spaces.py")),
            workspace: None,
            language: LanguageId::Python,
            save: None,
            request: request.clone(),
        };
        (dir, request, job)
    }

    #[tokio::test]
    async fn command_formatting_round_trips_unsaved_unicode_buffer() {
        let (_dir, _request, job) = job("/bin/cat", &[]);
        assert_eq!(run(&job, Duration::from_secs(5)).await.unwrap(), job.text);
    }

    #[tokio::test]
    async fn command_formatting_substitutes_filename_as_one_argument_and_sets_cwd() {
        let (dir, _request, job) = job(
            "/bin/sh",
            &[
                "-c",
                "cat >/dev/null; printf '%s\\n' \"$1\"; pwd",
                "fixture",
                "{file}",
            ],
        );
        let result = run(&job, Duration::from_secs(5)).await.unwrap();
        let mut lines = result.lines();
        assert_eq!(lines.next(), job.file.as_ref().unwrap().to_str());
        assert_eq!(
            std::fs::canonicalize(lines.next().unwrap()).unwrap(),
            std::fs::canonicalize(dir.path()).unwrap()
        );
    }

    #[tokio::test]
    async fn command_formatting_preserves_literal_shell_metacharacters() {
        let (_dir, _request, mut job) = job(
            "/bin/sh",
            &[
                "-c",
                "cat >/dev/null; printf '%s' \"$1\"",
                "fixture",
                "$(this-is-not-a-command); `literal`",
            ],
        );
        job.text.clear();
        assert_eq!(
            run(&job, Duration::from_secs(5)).await.unwrap(),
            "$(this-is-not-a-command); `literal`"
        );
    }

    #[tokio::test]
    async fn command_formatting_reports_exit_utf8_and_spawn_errors() {
        for (command, args, expected) in [
            (
                "/bin/sh",
                vec!["-c", "cat >/dev/null; echo 'invalid syntax' >&2; exit 2"],
                "invalid syntax",
            ),
            (
                "/bin/sh",
                vec!["-c", "cat >/dev/null; printf '\\377'"],
                "invalid UTF-8",
            ),
            ("/does/not/exist/formatter", vec![], "Starting formatter"),
        ] {
            let (_dir, _request, job) = job(command, &args);
            assert!(
                format!("{:#}", run(&job, Duration::from_secs(5)).await.unwrap_err())
                    .contains(expected)
            );
        }
    }

    #[tokio::test]
    async fn command_formatting_timeout_kills_and_reaps_child() {
        let (dir, _request, job) = job("/bin/sh", &["-c", "echo $$ > child.pid; exec sleep 30"]);
        let result = run(&job, Duration::from_millis(200)).await.unwrap_err();
        assert!(result.to_string().contains("Timed out"));
        let pid = std::fs::read_to_string(dir.path().join("child.pid")).unwrap();
        let status = std::process::Command::new("/bin/kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(!status.success(), "child must have been reaped");
    }

    #[tokio::test]
    async fn command_formatting_cancels_superseded_jobs() {
        let (_dir, request, job) = job("/bin/sh", &["-c", "exec sleep 30"]);
        drop(request);
        let result = run(&job, Duration::from_secs(5)).await.unwrap_err();
        assert!(result.to_string().contains("superseded"));
    }

    #[tokio::test]
    async fn command_formatting_drains_output_while_writing_stdin() {
        let (_dir, _request, mut job) = job("/bin/sh", &["-c", "head -c 131072 /dev/zero; cat"]);
        job.text = "x".repeat(ByteSize::kibibytes(256).as_u64() as usize);
        let output = run(&job, Duration::from_secs(5)).await.unwrap();
        assert_eq!(output.len(), ByteSize::kibibytes(384).as_u64() as usize);
        assert!(output.ends_with(&job.text));
    }

    #[tokio::test]
    async fn command_formatting_bounds_capture_and_reports_excessive_stderr() {
        assert!(read_bounded(&b"12345"[..], ByteSize::bytes(4))
            .await
            .is_err());
        assert_eq!(
            read_bounded(&b"1234"[..], ByteSize::bytes(4))
                .await
                .unwrap(),
            b"1234"
        );
        let (_dir, _request, job) = job("/bin/sh", &["-c", "head -c 131072 /dev/zero >&2; cat"]);
        assert!(run(&job, Duration::from_secs(5))
            .await
            .unwrap_err()
            .to_string()
            .contains("exceeds"));
    }

    #[test]
    fn command_formatting_untitled_python_uses_workspace_and_language_extension() {
        let (dir, _request, mut job) = job("/bin/cat", &[]);
        job.file = None;
        job.workspace = Some(dir.path().to_path_buf());
        let (file, cwd) = context_paths(&job).unwrap();
        assert_eq!(file, dir.path().join("untitled.py"));
        assert_eq!(cwd, dir.path());
    }
}
