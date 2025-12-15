//! Command-line argument parsing for the editor
//!
//! Supports:
//! - Opening files and directories
//! - Jump to line/column
//! - Wait mode for git integration
//! - New empty buffer mode

use clap::Parser;
use std::path::PathBuf;

/// A fast text editor
#[derive(Parser, Debug)]
#[command(name = "token", version, about = "A fast text editor")]
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
}

/// The startup mode determines what to open
#[derive(Debug, Clone)]
pub enum StartupMode {
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
        let mode = if self.new || self.paths.is_empty() {
            StartupMode::Empty
        } else if self.paths.len() == 1 {
            let path = &self.paths[0];
            if path.is_dir() {
                StartupMode::Workspace {
                    root: path.clone(),
                    initial_files: vec![],
                }
            } else {
                StartupMode::SingleFile(path.clone())
            }
        } else {
            let (dirs, files): (Vec<_>, Vec<_>) = self.paths.iter().partition(|p| p.is_dir());

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

        // Convert from 1-indexed (user input) to 0-indexed (internal)
        let initial_position = self.line.map(|line| {
            let line_0 = line.saturating_sub(1);
            let col_0 = self.column.unwrap_or(1).saturating_sub(1);
            (line_0, col_0)
        });

        Ok(StartupConfig {
            mode,
            initial_position,
            wait_mode: self.wait,
        })
    }
}

impl StartupConfig {
    /// Get file paths to open (for backward compatibility with current App::new)
    pub fn file_paths(&self) -> Vec<PathBuf> {
        match &self.mode {
            StartupMode::Empty => vec![],
            StartupMode::SingleFile(path) => vec![path.clone()],
            StartupMode::MultipleFiles(paths) => paths.clone(),
            StartupMode::Workspace { initial_files, .. } => initial_files.clone(),
        }
    }

    /// Get workspace root if this is a workspace mode
    pub fn workspace_root(&self) -> Option<&PathBuf> {
        match &self.mode {
            StartupMode::Workspace { root, .. } => Some(root),
            _ => None,
        }
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
        };
        let config = args.into_config().unwrap();
        assert!(config.wait_mode);
    }
}
