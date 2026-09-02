//! Command-line argument parsing for the editor
//!
//! Supports:
//! - Opening files and directories
//! - Jump to line/column
//! - Wait mode for git integration
//! - New empty buffer mode

use clap::Parser;
use std::path::{Path, PathBuf};

/// A fast text editor
#[derive(Parser, Debug)]
#[command(name = "token", version = env!("TOKEN_VERSION"), about = "A fast text editor")]
pub struct CliArgs {
    /// Files or directories to open
    #[arg(value_name = "PATHS")]
    pub paths: Vec<PathBuf>,

    /// Start with empty buffer (ignore session restore)
    #[arg(short = 'n', long)]
    pub new: bool,

    /// Wait for files to close before exiting (for git/svn integration)
    #[arg(short = 'w', long)]
    pub wait: bool,

    /// Go to line N in the first file
    #[arg(long, value_name = "N")]
    pub line: Option<usize>,

    /// Go to column N (used with --line)
    #[arg(long, value_name = "N")]
    pub column: Option<usize>,

    /// Launch deterministic content for automation and screenshot testing
    #[arg(long)]
    pub demo: bool,

    /// Always start a separate editor process instead of opening in a
    /// running one
    #[arg(long)]
    pub new_window: bool,

    /// Run the editor in this process (the launcher sets this on the
    /// detached child; useful for seeing logs in the terminal)
    #[arg(long, hide = true)]
    pub foreground: bool,
}

/// Split a trailing `:line[:column]` suffix off a CLI path argument.
///
/// A path that exists on disk is returned verbatim, so a file literally
/// named `notes:1` still opens. Positions are 1-indexed as typed.
pub fn split_position(arg: &Path) -> (PathBuf, Option<(usize, usize)>) {
    if arg.exists() {
        return (arg.to_path_buf(), None);
    }
    let Some(text) = arg.to_str() else {
        return (arg.to_path_buf(), None);
    };
    let mut parts = text.rsplitn(3, ':');
    let last = parts.next().unwrap_or_default();
    let Ok(first_number) = last.parse::<usize>() else {
        return (arg.to_path_buf(), None);
    };
    let Some(rest) = parts.next() else {
        return (arg.to_path_buf(), None);
    };
    match parts.next() {
        Some(path) if rest.parse::<usize>().is_ok() => {
            let line = rest.parse().unwrap_or(1);
            (PathBuf::from(path), Some((line, first_number)))
        }
        Some(path) => (
            PathBuf::from(format!("{path}:{rest}")),
            Some((first_number, 1)),
        ),
        None => (PathBuf::from(rest), Some((first_number, 1))),
    }
}

/// The startup mode determines what to open
#[derive(Debug, Clone)]
pub enum StartupMode {
    /// Deterministic in-memory document for automation and profiling
    Demo,
    /// Start with an empty buffer
    Empty,
    /// Open a single file
    SingleFile(PathBuf),
    /// Open multiple files as tabs
    MultipleFiles(Vec<PathBuf>),
    /// Open a directory as workspace
    Workspace {
        root: PathBuf,
        initial_files: Vec<PathBuf>,
    },
}

/// Configuration derived from CLI arguments
#[derive(Debug, Clone)]
pub struct StartupConfig {
    /// What files/folders to open
    pub mode: StartupMode,
    /// Initial cursor position (line, column) - 1-indexed from user, converted to 0-indexed
    pub initial_position: Option<(usize, usize)>,
    /// Wait for files to close before process exits
    pub wait_mode: bool,
}

impl CliArgs {
    /// Convert parsed CLI args into startup configuration
    pub fn into_config(self) -> Result<StartupConfig, String> {
        let (paths, positions): (Vec<PathBuf>, Vec<Option<(usize, usize)>>) =
            self.paths.iter().map(|p| split_position(p)).unzip();
        let first_position = positions.first().copied().flatten();
        let mode = if self.demo {
            StartupMode::Demo
        } else if self.new || paths.is_empty() {
            StartupMode::Empty
        } else if paths.len() == 1 {
            let path = &paths[0];
            if path.is_dir() {
                StartupMode::Workspace {
                    root: path.clone(),
                    initial_files: vec![],
                }
            } else {
                StartupMode::SingleFile(path.clone())
            }
        } else {
            let (dirs, files): (Vec<_>, Vec<_>) = paths.iter().partition(|p| p.is_dir());

            if dirs.len() > 1 {
                return Err("Cannot open multiple directories".to_string());
            }

            if let Some(dir) = dirs.first() {
                StartupMode::Workspace {
                    root: (*dir).clone(),
                    initial_files: files.into_iter().cloned().collect(),
                }
            } else {
                StartupMode::MultipleFiles(files.into_iter().cloned().collect())
            }
        };

        // Convert from 1-indexed (user input) to 0-indexed (internal).
        // `--line` wins over a `path:line:col` suffix on the first path.
        let initial_position = self
            .line
            .map(|line| (line, self.column.unwrap_or(1)))
            .or(first_position)
            .map(|(line, column)| (line.saturating_sub(1), column.saturating_sub(1)));

        Ok(StartupConfig {
            mode,
            initial_position,
            wait_mode: self.wait,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_args_gives_empty_mode() {
        let args = CliArgs {
            paths: vec![],
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(matches!(config.mode, StartupMode::Empty));
    }

    #[test]
    fn test_new_flag_gives_empty_mode() {
        let args = CliArgs {
            paths: vec![PathBuf::from("file.txt")],
            new: true,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(matches!(config.mode, StartupMode::Empty));
    }

    #[test]
    fn test_single_file() {
        let args = CliArgs {
            paths: vec![PathBuf::from("file.txt")],
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(matches!(config.mode, StartupMode::SingleFile(_)));
    }

    #[test]
    fn test_multiple_files() {
        let args = CliArgs {
            paths: vec![PathBuf::from("a.txt"), PathBuf::from("b.txt")],
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        if let StartupMode::MultipleFiles(files) = config.mode {
            assert_eq!(files.len(), 2);
        } else {
            panic!("Expected MultipleFiles mode");
        }
    }

    #[test]
    fn test_line_column_conversion() {
        let args = CliArgs {
            paths: vec![PathBuf::from("file.txt")],
            new: false,
            wait: false,
            line: Some(42),
            column: Some(10),
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        // 1-indexed to 0-indexed: line 42 → 41, column 10 → 9
        assert_eq!(config.initial_position, Some((41, 9)));
    }

    #[test]
    fn test_line_without_column() {
        let args = CliArgs {
            paths: vec![PathBuf::from("file.txt")],
            new: false,
            wait: false,
            line: Some(10),
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        // Column defaults to 1, so 0-indexed: line 10 → 9, column 1 → 0
        assert_eq!(config.initial_position, Some((9, 0)));
    }

    #[test]
    fn test_wait_mode() {
        let args = CliArgs {
            paths: vec![],
            new: false,
            wait: true,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(config.wait_mode);
    }

    #[test]
    fn demo_mode_takes_precedence_over_paths() {
        let args = CliArgs {
            paths: vec![PathBuf::from("ignored.txt")],
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: true,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(matches!(config.mode, StartupMode::Demo));
    }

    #[test]
    fn split_position_parses_line_and_column() {
        let missing = PathBuf::from("definitely/missing/file.rs");
        assert_eq!(
            split_position(&missing.join("x:12")),
            (missing.join("x"), Some((12, 1)))
        );
        assert_eq!(
            split_position(&missing.join("x:12:3")),
            (missing.join("x"), Some((12, 3)))
        );
        assert_eq!(
            split_position(&missing.join("x:")),
            (missing.join("x:"), None)
        );
        assert_eq!(
            split_position(&missing.join("x:a:3")),
            (missing.join("x:a"), Some((3, 1)))
        );
    }

    #[test]
    fn split_position_keeps_existing_path_verbatim() {
        let dir = tempfile::tempdir().unwrap();
        let literal = dir.path().join("notes:1");
        std::fs::write(&literal, "").unwrap();
        assert_eq!(split_position(&literal), (literal.clone(), None));
    }

    #[test]
    fn path_suffix_feeds_initial_position() {
        let args = CliArgs {
            paths: vec![PathBuf::from("definitely/missing.rs:7:2")],
            new: false,
            wait: false,
            line: None,
            column: None,
            demo: false,
            new_window: false,
            foreground: false,
        };
        let config = args.into_config().unwrap();
        assert!(
            matches!(config.mode, StartupMode::SingleFile(ref p) if p == Path::new("definitely/missing.rs"))
        );
        assert_eq!(config.initial_position, Some((6, 1)));
    }

    #[test]
    fn launcher_flags_parse() {
        let args =
            CliArgs::try_parse_from(["token", "--new-window", "--foreground", "a.rs"]).unwrap();
        assert!(args.new_window && args.foreground);
    }
}
