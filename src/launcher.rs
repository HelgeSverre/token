//! Shell-facing startup: hand paths to a running editor, or start one
//! detached from the terminal, and implement `--wait`.
//!
//! The automation socket doubles as the single-instance rendezvous: if a
//! connection succeeds an editor is running and gets an `OpenPaths`
//! request; otherwise this process spawns the editor as a detached child
//! (`--foreground`) and sends the same request once the child listens.
//! `--wait` simply keeps that connection open until the editor answers,
//! which it does when the opened documents close or it exits.
//!
//! Directories never hand off: each gets its own editor process, since a
//! window owns exactly one workspace.

use std::ffi::OsString;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use token::cli::CliArgs;

use crate::automation::{self, AutomationRequest, OpenPath, RequestError, Target};

/// How long to wait for a freshly spawned editor to start listening.
const STARTUP_TIMEOUT: Duration = Duration::from_secs(10);
const STARTUP_POLL: Duration = Duration::from_millis(100);

/// What the launcher decided to do with this invocation.
#[derive(Debug, PartialEq, Eq)]
enum LaunchPlan {
    /// Run the editor in this process.
    Foreground,
    /// Start a separate editor process for these arguments; `wait`
    /// keeps it attached (`--wait` on a directory waits for its window).
    /// `reuse_dir` is the one directory named, when an editor already
    /// showing that workspace should be focused instead.
    NewProcess {
        child_args: Vec<OsString>,
        files: Vec<OpenPath>,
        wait: bool,
        reuse_dir: Option<PathBuf>,
    },
    /// Open `files` in the running editor, starting one if needed.
    HandOff {
        files: Vec<OpenPath>,
        child_args: Vec<OsString>,
        wait: bool,
    },
}

/// Decide what to do and do it. `Some(code)` means this process is done
/// and should exit with `code`; `None` means run the GUI here.
pub(crate) fn maybe_hand_off(args: &CliArgs) -> Option<i32> {
    let stdin_file = read_stdin_if_requested(args);
    let plan = plan(args, stdin_file.as_deref());
    let code = match plan {
        LaunchPlan::Foreground => None,
        LaunchPlan::NewProcess {
            child_args,
            files,
            wait,
            reuse_dir: Some(dir),
        } => Some(open_directory(&dir, files, &child_args, wait, None)),
        LaunchPlan::NewProcess {
            child_args, wait, ..
        } => Some(start_new_process(&child_args, wait)),
        LaunchPlan::HandOff {
            files,
            child_args,
            wait,
        } => hand_off(files, &child_args, wait),
    };
    // Without `--wait` the editor may not have read the spool yet, so it
    // is left behind like VS Code's `code-stdin-*` files.
    if code.is_some() && args.wait {
        if let Some(path) = stdin_file {
            let _ = std::fs::remove_file(path);
        }
    }
    code
}

/// Start `current_exe() --foreground <args>` detached from this terminal;
/// returns the child's pid, which is also its automation instance id.
pub(crate) fn spawn_detached(extra: &[OsString]) -> io::Result<u32> {
    let mut command = child_command(extra)?;
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // SAFETY: `setsid` is async-signal-safe and touches no Rust state;
        // it only detaches the child from the terminal's session so a
        // closing terminal does not SIGHUP the editor.
        unsafe {
            command.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    command.spawn().map(|child| child.id())
}

/// Open `dir` (plus `files`) in the editor already showing that
/// workspace, or start one. `exclude` is the calling editor's own id
/// when this runs inside an editor, so discovery never waits on itself.
pub(crate) fn open_directory(
    dir: &Path,
    files: Vec<OpenPath>,
    child_args: &[OsString],
    wait: bool,
    exclude: Option<u32>,
) -> i32 {
    let root = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
    let existing = automation::discover_in(&automation::instances_dir(), exclude)
        .into_iter()
        .find(|instance| instance.info.workspace_root.as_deref() == Some(root.as_path()));
    match existing {
        Some(instance) => {
            let request = AutomationRequest::OpenPaths { paths: files, wait };
            let timeout = (!wait).then_some(Duration::from_secs(30));
            let target = Target::Instance(instance.info.instance_id);
            exit_code(
                automation::request_with_timeout(&target, request, timeout),
                wait,
            )
        }
        None => start_new_process(child_args, wait),
    }
}

fn plan(args: &CliArgs, stdin_file: Option<&Path>) -> LaunchPlan {
    if args.foreground || args.demo {
        return LaunchPlan::Foreground;
    }
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    for arg in &args.paths {
        let mut open = if arg == Path::new("-") {
            match stdin_file {
                Some(file) => OpenPath {
                    path: file.to_path_buf(),
                    line: None,
                    column: None,
                },
                None => continue,
            }
        } else {
            OpenPath::from_arg(arg)
        };
        if open.path.is_dir() {
            dirs.push(open.path);
        } else {
            if files.is_empty() && args.line.is_some() {
                open.line = args.line;
                open.column = args.column.or(Some(1));
            }
            files.push(open);
        }
    }
    let mut child_args: Vec<OsString> = Vec::new();
    if args.new {
        child_args.push("--new".into());
    }
    child_args.extend(dirs.iter().map(OsString::from));
    child_args.extend(files.iter().map(|file| {
        let mut arg = file.path.clone().into_os_string();
        if let Some(line) = file.line {
            arg.push(format!(":{line}:{}", file.column.unwrap_or(1)));
        }
        arg
    }));
    if args.new_window || !dirs.is_empty() {
        let reuse_dir = match dirs.as_slice() {
            [dir] if !args.new_window => Some(dir.clone()),
            _ => None,
        };
        LaunchPlan::NewProcess {
            child_args,
            files,
            wait: args.wait,
            reuse_dir,
        }
    } else {
        LaunchPlan::HandOff {
            files,
            child_args,
            wait: args.wait,
        }
    }
}

fn start_new_process(child_args: &[OsString], wait: bool) -> i32 {
    // `--wait` on a directory means "until that window closes", so the
    // child stays attached; otherwise it is detached whatever started us.
    let result = if wait {
        child_command(child_args)
            .and_then(|mut command| command.status().map(|status| status.code().unwrap_or(1)))
    } else {
        spawn_detached(child_args).map(|_pid| 0)
    };
    result.unwrap_or_else(|error| {
        eprintln!("token: failed to start the editor: {error}");
        1
    })
}

fn hand_off(files: Vec<OpenPath>, child_args: &[OsString], wait: bool) -> Option<i32> {
    // The editor whose workspace holds the first file, else the most
    // recently focused one.
    let target = files
        .first()
        .map_or(Target::Default, |file| Target::ForPath(file.path.clone()));
    let request = AutomationRequest::OpenPaths { paths: files, wait };
    let timeout = (!wait).then_some(Duration::from_secs(30));
    match automation::request_with_timeout(&target, request.clone(), timeout) {
        Err(RequestError::NotRunning) => {}
        outcome => return Some(exit_code(outcome, wait)),
    }
    if !launched_from_terminal() {
        // Started by a desktop launcher: this process is the editor.
        return None;
    }
    let child = match spawn_detached(child_args) {
        Ok(pid) => Target::Instance(pid),
        Err(error) => {
            eprintln!("token: failed to start the editor: {error}");
            return Some(1);
        }
    };
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    loop {
        match automation::request_with_timeout(&child, request.clone(), timeout) {
            Err(RequestError::NotRunning | RequestError::NoSuchInstance(_))
                if Instant::now() < deadline =>
            {
                std::thread::sleep(STARTUP_POLL);
            }
            Err(RequestError::NotRunning | RequestError::NoSuchInstance(_)) => {
                eprintln!("token: the editor started but did not answer; it may still be opening");
                return Some(1);
            }
            outcome => return Some(exit_code(outcome, wait)),
        }
    }
}

fn exit_code(outcome: Result<automation::AutomationResponse, RequestError>, wait: bool) -> i32 {
    match outcome {
        Ok(response) if response.ok => 0,
        Ok(response) => {
            eprintln!("token: {}", response.message);
            1
        }
        // The editor answers and exits in the same breath; losing the
        // race to its exit still means the wait is over.
        Err(RequestError::Eof) if wait => 0,
        Err(error) => {
            eprintln!("token: {error}");
            1
        }
    }
}

fn child_command(extra: &[OsString]) -> io::Result<Command> {
    let mut command = Command::new(std::env::current_exe()?);
    command
        .arg("--foreground")
        .args(extra)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    Ok(command)
}

/// `echo text | token -`: spool stdin to a temp file before anything
/// else happens, so both the child and a running editor see a real path.
fn read_stdin_if_requested(args: &CliArgs) -> Option<PathBuf> {
    if args.foreground || !args.paths.iter().any(|path| path == Path::new("-")) {
        return None;
    }
    let mut stdin = io::stdin();
    if stdin.is_terminal() {
        return None;
    }
    let path = std::env::temp_dir().join(format!("token-stdin-{}.txt", std::process::id()));
    let mut contents = Vec::new();
    if let Err(error) = stdin
        .read_to_end(&mut contents)
        .and_then(|_| std::fs::write(&path, &contents))
    {
        eprintln!("token: could not read stdin: {error}");
        return None;
    }
    Some(path)
}

/// Whether a shell is waiting on this process. Windows console apps
/// always report a terminal (Explorer allocates one), which is fine: a
/// detached child is never worse than today's blocking launch.
fn launched_from_terminal() -> bool {
    io::stdin().is_terminal() || io::stdout().is_terminal()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(paths: &[&str]) -> CliArgs {
        CliArgs {
            paths: paths.iter().map(PathBuf::from).collect(),
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        }
    }

    #[test]
    fn foreground_and_demo_run_here() {
        assert_eq!(
            plan(
                &CliArgs {
                    foreground: true,
                    ..args(&["a.rs"])
                },
                None
            ),
            LaunchPlan::Foreground
        );
        assert_eq!(
            plan(
                &CliArgs {
                    demo: true,
                    ..args(&[])
                },
                None
            ),
            LaunchPlan::Foreground
        );
    }

    #[test]
    fn files_hand_off_with_absolute_paths_and_positions() {
        let plan = plan(
            &CliArgs {
                line: Some(4),
                ..args(&["missing/a.rs", "missing/b.rs:12:3"])
            },
            None,
        );
        let LaunchPlan::HandOff {
            files, child_args, ..
        } = plan
        else {
            panic!("expected a handoff");
        };
        assert!(files.iter().all(|file| file.path.is_absolute()));
        assert_eq!((files[0].line, files[0].column), (Some(4), Some(1)));
        assert_eq!((files[1].line, files[1].column), (Some(12), Some(3)));
        assert_eq!(child_args.len(), 2);
        assert!(child_args[1].to_string_lossy().ends_with("b.rs:12:3"));
    }

    #[test]
    fn directories_and_new_window_start_a_process() {
        let dir = tempfile::tempdir().unwrap();
        let plan_dir = plan(
            &CliArgs {
                wait: true,
                ..args(&[dir.path().to_str().unwrap()])
            },
            None,
        );
        assert!(matches!(
            plan_dir,
            LaunchPlan::NewProcess {
                wait: true,
                reuse_dir: Some(_),
                ..
            }
        ));
        let plan_new = plan(
            &CliArgs {
                new_window: true,
                ..args(&["missing.rs"])
            },
            None,
        );
        assert!(matches!(
            plan_new,
            LaunchPlan::NewProcess {
                wait: false,
                reuse_dir: None,
                ..
            }
        ));
    }

    #[test]
    fn two_directories_never_reuse_and_files_ride_along() {
        let a = tempfile::tempdir().unwrap();
        let b = tempfile::tempdir().unwrap();
        let two = plan(
            &args(&[a.path().to_str().unwrap(), b.path().to_str().unwrap()]),
            None,
        );
        assert!(matches!(
            two,
            LaunchPlan::NewProcess {
                reuse_dir: None,
                ..
            }
        ));
        let with_file = plan(&args(&[a.path().to_str().unwrap(), "missing/x.rs:3"]), None);
        let LaunchPlan::NewProcess {
            files, reuse_dir, ..
        } = with_file
        else {
            panic!("expected a new process");
        };
        assert!(reuse_dir.is_some());
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].line, Some(3));
    }

    #[test]
    fn stdin_dash_is_replaced_by_the_spooled_file() {
        let spooled = PathBuf::from("/tmp/token-stdin-test.txt");
        let LaunchPlan::HandOff { files, .. } = plan(&args(&["-"]), Some(&spooled)) else {
            panic!("expected a handoff");
        };
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].path, spooled);
        assert!(
            matches!(plan(&args(&["-"]), None), LaunchPlan::HandOff { files, .. } if files.is_empty())
        );
    }
}
