//! Command types for the Elm-style architecture
//!
//! Commands represent side effects that should be performed after an update.

use std::path::PathBuf;

use crate::keymap::{Command as KeymapCommand, Keymap};
use crate::model::editor_area::DocumentId;
use crate::syntax::LanguageId;

// ============================================================================
// Command Palette Registry
// ============================================================================

/// Identifies a command that can be executed via the command palette
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    // File operations
    NewFile,
    OpenFile,
    FuzzyFileFinder,
    SaveFile,
    SaveFileAs,

    // Edit operations
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,

    // Navigation
    GotoLine,

    // View operations
    SplitHorizontal,
    SplitVertical,
    CloseGroup,
    NextTab,
    PrevTab,
    CloseTab,

    // Find/Replace
    Find,

    // UI
    ShowCommandPalette,

    // Theme
    SwitchTheme,

    // Settings
    OpenConfigDirectory,
    OpenKeybindings,
    ReloadConfiguration,

    // CSV
    ToggleCsvView,

    // Debug/Troubleshooting
    OpenLogFile,

    // Workspace
    OpenFolder,

    // Application
    Quit,

    // Debug overlays (only available in debug builds)
    #[cfg(debug_assertions)]
    TogglePerfOverlay,
    #[cfg(debug_assertions)]
    ToggleDebugOverlay,
}

/// A command definition for the command palette
#[derive(Debug, Clone)]
pub struct CommandDef {
    pub id: CommandId,
    pub label: &'static str,
    pub keybinding: Option<&'static str>,
}

/// Static registry of all available commands
pub static COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::NewFile,
        label: "New File",
        keybinding: Some("⇧⌘N"),
    },
    CommandDef {
        id: CommandId::OpenFile,
        label: "Open File...",
        keybinding: Some("⌘O"),
    },
    CommandDef {
        id: CommandId::FuzzyFileFinder,
        label: "Go to File...",
        keybinding: Some("⇧⌘O"),
    },
    CommandDef {
        id: CommandId::SaveFile,
        label: "Save File",
        keybinding: Some("⌘S"),
    },
    CommandDef {
        id: CommandId::SaveFileAs,
        label: "Save File As...",
        keybinding: Some("⇧⌘S"),
    },
    CommandDef {
        id: CommandId::Undo,
        label: "Undo",
        keybinding: Some("⌘Z"),
    },
    CommandDef {
        id: CommandId::Redo,
        label: "Redo",
        keybinding: Some("⇧⌘Z"),
    },
    CommandDef {
        id: CommandId::Cut,
        label: "Cut",
        keybinding: Some("⌘X"),
    },
    CommandDef {
        id: CommandId::Copy,
        label: "Copy",
        keybinding: Some("⌘C"),
    },
    CommandDef {
        id: CommandId::Paste,
        label: "Paste",
        keybinding: Some("⌘V"),
    },
    CommandDef {
        id: CommandId::SelectAll,
        label: "Select All",
        keybinding: Some("⌘A"),
    },
    CommandDef {
        id: CommandId::GotoLine,
        label: "Go to Line...",
        keybinding: Some("⌘L"),
    },
    CommandDef {
        id: CommandId::SplitHorizontal,
        label: "Split Editor Right",
        keybinding: Some("⇧⌥⌘H"),
    },
    CommandDef {
        id: CommandId::SplitVertical,
        label: "Split Editor Down",
        keybinding: Some("⇧⌥⌘V"),
    },
    CommandDef {
        id: CommandId::CloseGroup,
        label: "Close Editor Group",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::NextTab,
        label: "Next Tab",
        keybinding: Some("⌥⌘→"),
    },
    CommandDef {
        id: CommandId::PrevTab,
        label: "Previous Tab",
        keybinding: Some("⌥⌘←"),
    },
    CommandDef {
        id: CommandId::CloseTab,
        label: "Close Tab",
        keybinding: Some("⌘W"),
    },
    CommandDef {
        id: CommandId::Find,
        label: "Find...",
        keybinding: Some("⌘F"),
    },
    CommandDef {
        id: CommandId::ShowCommandPalette,
        label: "Show Command Palette",
        keybinding: Some("⇧⌘A"),
    },
    CommandDef {
        id: CommandId::SwitchTheme,
        label: "Switch Theme...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenConfigDirectory,
        label: "Open Config Directory",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenKeybindings,
        label: "Open Keymap",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ReloadConfiguration,
        label: "Reload Configuration",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::ToggleCsvView,
        label: "Toggle CSV View",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenLogFile,
        label: "Open Log File",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::OpenFolder,
        label: "Open Folder...",
        keybinding: None,
    },
    CommandDef {
        id: CommandId::Quit,
        label: "Quit",
        keybinding: Some("⌘Q"),
    },
];

/// Debug-only commands (only available in debug builds)
#[cfg(debug_assertions)]
pub static DEBUG_COMMANDS: &[CommandDef] = &[
    CommandDef {
        id: CommandId::TogglePerfOverlay,
        label: "Toggle Performance Overlay",
        keybinding: Some("F2"),
    },
    CommandDef {
        id: CommandId::ToggleDebugOverlay,
        label: "Toggle Debug Overlay",
        keybinding: Some("F8"),
    },
];

/// Calculate fuzzy match score. Returns None if no match, Some(score) if matches.
/// Higher score = better match. Consecutive matches and word-start matches score higher.
fn fuzzy_match_score(query: &str, target: &str) -> Option<i32> {
    let query_chars: Vec<char> = query.to_lowercase().chars().collect();
    let target_lower = target.to_lowercase();
    let target_chars: Vec<char> = target_lower.chars().collect();

    if query_chars.is_empty() {
        return Some(0);
    }

    let mut query_idx = 0;
    let mut score = 0;
    let mut prev_matched = false;
    let mut prev_was_separator = true; // Start of string counts as separator

    for (i, &tc) in target_chars.iter().enumerate() {
        let is_separator = tc == ' ' || tc == '_' || tc == '-';

        if query_idx < query_chars.len() && tc == query_chars[query_idx] {
            // Match found
            score += 1;

            // Bonus for consecutive matches
            if prev_matched {
                score += 2;
            }

            // Bonus for matching at word start (after separator or at beginning)
            if prev_was_separator {
                score += 3;
            }

            // Bonus for matching at string start
            if i == 0 {
                score += 5;
            }

            query_idx += 1;
            prev_matched = true;
        } else {
            prev_matched = false;
        }

        prev_was_separator = is_separator;
    }

    // All query chars must be found
    if query_idx == query_chars.len() {
        Some(score)
    } else {
        None
    }
}

/// Get all available commands (including debug commands in debug builds)
fn all_commands() -> Vec<&'static CommandDef> {
    #[allow(unused_mut)]
    let mut cmds: Vec<&'static CommandDef> = COMMANDS.iter().collect();
    #[cfg(debug_assertions)]
    cmds.extend(DEBUG_COMMANDS.iter());
    cmds
}

/// Filter commands by a search query (fuzzy match on label)
pub fn filter_commands(query: &str) -> Vec<&'static CommandDef> {
    let all = all_commands();

    if query.is_empty() {
        return all;
    }

    let mut matches: Vec<(&'static CommandDef, i32)> = all
        .into_iter()
        .filter_map(|cmd| fuzzy_match_score(query, cmd.label).map(|score| (cmd, score)))
        .collect();

    // Sort by score descending (best matches first)
    matches.sort_by(|a, b| b.1.cmp(&a.1));

    matches.into_iter().map(|(cmd, _)| cmd).collect()
}

/// Map CommandId to keymap::Command for keybinding lookup
impl CommandId {
    pub fn to_keymap_command(self) -> Option<KeymapCommand> {
        match self {
            CommandId::NewFile => Some(KeymapCommand::NewTab), // NewFile maps to NewTab
            CommandId::OpenFile => Some(KeymapCommand::OpenFile),
            CommandId::FuzzyFileFinder => Some(KeymapCommand::FuzzyFileFinder),
            CommandId::SaveFile => Some(KeymapCommand::SaveFile),
            CommandId::SaveFileAs => Some(KeymapCommand::SaveFileAs),
            CommandId::Undo => Some(KeymapCommand::Undo),
            CommandId::Redo => Some(KeymapCommand::Redo),
            CommandId::Cut => Some(KeymapCommand::Cut),
            CommandId::Copy => Some(KeymapCommand::Copy),
            CommandId::Paste => Some(KeymapCommand::Paste),
            CommandId::SelectAll => Some(KeymapCommand::SelectAll),
            CommandId::GotoLine => Some(KeymapCommand::ToggleGotoLine),
            CommandId::SplitHorizontal => Some(KeymapCommand::SplitHorizontal),
            CommandId::SplitVertical => Some(KeymapCommand::SplitVertical),
            CommandId::CloseGroup => None, // No direct mapping yet
            CommandId::NextTab => Some(KeymapCommand::NextTab),
            CommandId::PrevTab => Some(KeymapCommand::PrevTab),
            CommandId::CloseTab => Some(KeymapCommand::CloseTab),
            CommandId::Find => Some(KeymapCommand::ToggleFindReplace),
            CommandId::ShowCommandPalette => Some(KeymapCommand::ToggleCommandPalette),
            CommandId::SwitchTheme => None,
            CommandId::OpenConfigDirectory => None,
            CommandId::OpenKeybindings => None,
            CommandId::ReloadConfiguration => None,
            CommandId::ToggleCsvView => Some(KeymapCommand::CsvToggle),
            CommandId::OpenLogFile => Some(KeymapCommand::OpenLogFile),
            CommandId::OpenFolder => None,
            CommandId::Quit => Some(KeymapCommand::Quit),
            #[cfg(debug_assertions)]
            CommandId::TogglePerfOverlay => None,
            #[cfg(debug_assertions)]
            CommandId::ToggleDebugOverlay => None,
        }
    }
}

/// Get keybinding display string for a command from the keymap
pub fn keybinding_for_command(id: CommandId, keymap: &Keymap) -> Option<String> {
    let keymap_cmd = id.to_keymap_command()?;
    keymap.display_for(keymap_cmd)
}

/// Get keybinding display string using the static fallback (for when keymap isn't available)
pub fn keybinding_for_command_static(id: CommandId) -> Option<&'static str> {
    COMMANDS
        .iter()
        .find(|cmd| cmd.id == id)
        .and_then(|cmd| cmd.keybinding)
}

// ============================================================================
// Damage Tracking (partial redraw optimization)
// ============================================================================

/// Represents which parts of the UI need redrawing
///
/// Used for partial redraw optimization to avoid full-frame rendering
/// on every update. When in doubt, use `Damage::Full` for correctness.
#[derive(Debug, Clone, Default)]
pub enum Damage {
    /// No redraw needed (default state for accumulation)
    #[default]
    None,
    /// Redraw everything (always safe fallback)
    Full,
    /// Redraw specific areas only
    Areas(Vec<DamageArea>),
}

/// High-level UI regions that can be independently redrawn
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DamageArea {
    /// All editor groups, tab bars, gutters, text areas, splitters
    EditorArea,
    /// Bottom status bar only
    StatusBar,
    /// Specific lines for cursor blink optimization (line numbers are document-relative)
    /// This enables the most fine-grained optimization for cursor blink which happens
    /// at 2Hz and would otherwise require full EditorArea redraws.
    CursorLines(Vec<usize>),
}

impl Damage {
    /// Create damage for specific areas
    pub fn areas(areas: Vec<DamageArea>) -> Self {
        if areas.is_empty() {
            Damage::Full // Empty areas means nothing to redraw, but treat as full for safety
        } else {
            Damage::Areas(areas)
        }
    }

    /// Create damage for just the editor area
    pub fn editor_area() -> Self {
        Damage::Areas(vec![DamageArea::EditorArea])
    }

    /// Create damage for just the status bar
    pub fn status_bar() -> Self {
        Damage::Areas(vec![DamageArea::StatusBar])
    }

    /// Create damage for specific cursor lines
    pub fn cursor_lines(lines: Vec<usize>) -> Self {
        if lines.is_empty() {
            Damage::Areas(vec![]) // No lines to redraw
        } else {
            Damage::Areas(vec![DamageArea::CursorLines(lines)])
        }
    }

    /// Merge another damage into this one
    ///
    /// If either damage is Full, the result is Full.
    /// If either damage is None, the other takes precedence.
    /// Otherwise, areas are combined with deduplication.
    pub fn merge(&mut self, other: Damage) {
        match (&mut *self, other) {
            // None is identity for merge
            (Damage::None, other) => *self = other,
            (_, Damage::None) => {} // Nothing to merge
            // Full absorbs everything
            (Damage::Full, _) => {} // Already full, nothing to do
            (this, Damage::Full) => *this = Damage::Full,
            // Merge areas
            (Damage::Areas(areas), Damage::Areas(other_areas)) => {
                for area in other_areas {
                    // Merge CursorLines specially (combine line lists)
                    if let DamageArea::CursorLines(ref lines) = area {
                        if let Some(existing) = areas.iter_mut().find_map(|a| {
                            if let DamageArea::CursorLines(ref mut l) = a {
                                Some(l)
                            } else {
                                Option::None
                            }
                        }) {
                            // Merge line numbers, avoiding duplicates
                            for &line in lines {
                                if !existing.contains(&line) {
                                    existing.push(line);
                                }
                            }
                            continue;
                        }
                    }
                    // For EditorArea/StatusBar, just add if not present
                    if !areas.contains(&area) {
                        areas.push(area);
                    }
                }
            }
        }
    }

    /// Check if this damage is a full redraw
    pub fn is_full(&self) -> bool {
        matches!(self, Damage::Full)
    }

    /// Check if this damage includes the editor area (or is full)
    pub fn includes_editor(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => areas.iter().any(|a| {
                matches!(a, DamageArea::EditorArea) || matches!(a, DamageArea::CursorLines(_))
            }),
        }
    }

    /// Check if this damage includes the status bar (or is full)
    pub fn includes_status_bar(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => areas.iter().any(|a| matches!(a, DamageArea::StatusBar)),
        }
    }

    /// Get cursor lines if this is a cursor-lines-only damage
    pub fn cursor_lines_only(&self) -> Option<&[usize]> {
        match self {
            Damage::Areas(areas) if areas.len() == 1 => {
                if let Some(DamageArea::CursorLines(lines)) = areas.first() {
                    Some(lines)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Check if any redraw is needed
    pub fn needs_redraw(&self) -> bool {
        match self {
            Damage::None => false,
            Damage::Full => true,
            Damage::Areas(areas) => !areas.is_empty(),
        }
    }
}

// ============================================================================
// Side-Effect Commands (returned from update)
// ============================================================================

/// Commands returned by update functions
#[derive(Debug, Clone, Default)]
pub enum Cmd {
    /// No command - do nothing
    #[default]
    None,
    /// Request a full redraw of the UI (legacy, always safe)
    Redraw,
    /// Request a partial redraw of specific areas (optimization)
    RedrawAreas(Vec<DamageArea>),
    /// Save file asynchronously
    SaveFile { path: PathBuf, content: String },
    /// Load file asynchronously
    LoadFile { path: PathBuf },
    /// Open a path in the system file explorer/finder
    OpenInExplorer { path: PathBuf },
    /// Open a file in a new tab for editing
    OpenFileInEditor { path: PathBuf },
    /// Execute multiple commands
    Batch(Vec<Cmd>),

    // File dialogs
    /// Show native open file dialog
    ShowOpenFileDialog {
        /// Allow selecting multiple files
        allow_multi: bool,
        /// Starting directory for the dialog
        start_dir: Option<PathBuf>,
    },
    /// Show native save file dialog
    ShowSaveFileDialog {
        /// Suggested file path (for pre-filling name/directory)
        suggested_path: Option<PathBuf>,
    },
    /// Show native open folder dialog
    ShowOpenFolderDialog {
        /// Starting directory for the dialog
        start_dir: Option<PathBuf>,
    },

    // === Syntax Highlighting Commands ===
    /// Start debounce timer for syntax parsing
    /// After delay_ms, sends Msg::Syntax(ParseReady)
    DebouncedSyntaxParse {
        document_id: DocumentId,
        revision: u64,
        delay_ms: u64,
    },
    /// Run syntax parsing in background worker
    /// Sends Msg::Syntax(ParseCompleted) when done
    RunSyntaxParse {
        document_id: DocumentId,
        revision: u64,
        source: String,
        language: LanguageId,
    },

    // === Display Commands ===
    /// Reinitialize the renderer (e.g., after scale factor change)
    ReinitializeRenderer,

    // === Application Commands ===
    /// Request application exit
    Quit,

    // === Debug Commands ===
    /// Toggle performance overlay (debug builds only)
    #[cfg(debug_assertions)]
    TogglePerfOverlay,
}

impl Cmd {
    /// Create a batch of commands
    pub fn batch(cmds: Vec<Cmd>) -> Self {
        Cmd::Batch(cmds)
    }

    /// Check if this command requires a redraw
    pub fn needs_redraw(&self) -> bool {
        match self {
            Cmd::None => false,
            Cmd::Redraw => true,
            Cmd::RedrawAreas(areas) => !areas.is_empty(),
            Cmd::SaveFile { .. } => true,
            Cmd::LoadFile { .. } => true,
            Cmd::OpenInExplorer { .. } => true,
            Cmd::OpenFileInEditor { .. } => true,
            Cmd::Batch(cmds) => cmds.iter().any(|c| c.needs_redraw()),
            // Dialogs don't need immediate redraw - they'll trigger messages when done
            Cmd::ShowOpenFileDialog { .. } => false,
            Cmd::ShowSaveFileDialog { .. } => false,
            Cmd::ShowOpenFolderDialog { .. } => false,
            // Syntax commands don't need immediate redraw - ParseCompleted triggers redraw
            Cmd::DebouncedSyntaxParse { .. } => false,
            Cmd::RunSyntaxParse { .. } => false,
            // Reinitialize triggers a full redraw after renderer is recreated
            Cmd::ReinitializeRenderer => true,
            // Quit doesn't need redraw - app is exiting
            Cmd::Quit => false,
            // Debug overlay toggle triggers redraw
            #[cfg(debug_assertions)]
            Cmd::TogglePerfOverlay => true,
        }
    }

    /// Get the damage for this command
    ///
    /// Returns the combined damage from this command. For batch commands,
    /// merges all sub-command damages.
    pub fn damage(&self) -> Damage {
        match self {
            Cmd::None => Damage::Areas(vec![]), // No damage
            Cmd::Redraw => Damage::Full,
            Cmd::RedrawAreas(areas) => {
                if areas.is_empty() {
                    Damage::Areas(vec![])
                } else {
                    Damage::Areas(areas.clone())
                }
            }
            // File operations may cause full redraw (file load changes content)
            Cmd::SaveFile { .. } => Damage::Full,
            Cmd::LoadFile { .. } => Damage::Full,
            Cmd::OpenInExplorer { .. } => Damage::Full,
            Cmd::OpenFileInEditor { .. } => Damage::Full,
            // Batch: merge all damages
            Cmd::Batch(cmds) => {
                let mut damage = Damage::Areas(vec![]);
                for cmd in cmds {
                    damage.merge(cmd.damage());
                    // Short-circuit if we hit Full
                    if damage.is_full() {
                        break;
                    }
                }
                damage
            }
            // Dialogs don't need immediate redraw
            Cmd::ShowOpenFileDialog { .. } => Damage::Areas(vec![]),
            Cmd::ShowSaveFileDialog { .. } => Damage::Areas(vec![]),
            Cmd::ShowOpenFolderDialog { .. } => Damage::Areas(vec![]),
            // Syntax commands don't need immediate redraw
            Cmd::DebouncedSyntaxParse { .. } => Damage::Areas(vec![]),
            Cmd::RunSyntaxParse { .. } => Damage::Areas(vec![]),
            // Reinitialize triggers full redraw
            Cmd::ReinitializeRenderer => Damage::Full,
            // Quit doesn't need redraw - app is exiting
            Cmd::Quit => Damage::Areas(vec![]),
            // Debug overlay toggle triggers full redraw
            #[cfg(debug_assertions)]
            Cmd::TogglePerfOverlay => Damage::Full,
        }
    }

    /// Create a command to redraw the editor area and status bar
    pub fn redraw_editor() -> Self {
        Cmd::RedrawAreas(vec![DamageArea::EditorArea, DamageArea::StatusBar])
    }

    /// Create a command to redraw just the status bar
    pub fn redraw_status_bar() -> Self {
        Cmd::RedrawAreas(vec![DamageArea::StatusBar])
    }

    /// Create a command to redraw specific cursor lines
    pub fn redraw_cursor_lines(lines: Vec<usize>) -> Self {
        if lines.is_empty() {
            Cmd::None
        } else {
            Cmd::RedrawAreas(vec![DamageArea::CursorLines(lines)])
        }
    }

    /// Convert Option<Cmd> with None to Cmd::None
    pub fn from_option(opt: Option<Cmd>) -> Self {
        opt.unwrap_or(Cmd::None)
    }
}

// Allow converting Option<Cmd> to Cmd
impl From<Option<Cmd>> for Cmd {
    fn from(opt: Option<Cmd>) -> Self {
        opt.unwrap_or(Cmd::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_damage_computation() {
        // None -> empty areas
        assert!(matches!(Cmd::None.damage(), Damage::Areas(a) if a.is_empty()));

        // Redraw -> Full
        assert!(matches!(Cmd::Redraw.damage(), Damage::Full));

        // RedrawAreas preserves areas
        let cmd = Cmd::RedrawAreas(vec![DamageArea::EditorArea]);
        assert!(matches!(cmd.damage(), Damage::Areas(a) if a.contains(&DamageArea::EditorArea)));

        // Empty RedrawAreas -> empty areas (not Full)
        let cmd = Cmd::RedrawAreas(vec![]);
        assert!(matches!(cmd.damage(), Damage::Areas(a) if a.is_empty()));
    }

    #[test]
    fn test_cmd_damage_batch_full() {
        // Batch with Full -> Full
        let batch = Cmd::Batch(vec![
            Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
            Cmd::Redraw,
        ]);
        assert!(matches!(batch.damage(), Damage::Full));
    }

    #[test]
    fn test_cmd_damage_batch_merge() {
        // Batch without Full -> merged areas
        let batch = Cmd::Batch(vec![
            Cmd::RedrawAreas(vec![DamageArea::StatusBar]),
            Cmd::RedrawAreas(vec![DamageArea::EditorArea]),
        ]);
        let damage = batch.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 2);
                assert!(a.contains(&DamageArea::StatusBar));
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas, got {:?}", damage),
        }
    }

    #[test]
    fn test_cmd_damage_cursor_lines() {
        // CursorLines damage
        let cmd = Cmd::redraw_cursor_lines(vec![5, 10, 15]);
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                if let DamageArea::CursorLines(lines) = &a[0] {
                    assert_eq!(lines.len(), 3);
                    assert!(lines.contains(&5));
                    assert!(lines.contains(&10));
                    assert!(lines.contains(&15));
                } else {
                    panic!("Expected DamageArea::CursorLines");
                }
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_cursor_lines() {
        // Merge cursor lines with existing cursor lines
        let mut damage = Damage::Areas(vec![DamageArea::CursorLines(vec![1, 2])]);
        damage.merge(Damage::Areas(vec![DamageArea::CursorLines(vec![2, 3])]));
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                if let DamageArea::CursorLines(lines) = &a[0] {
                    assert_eq!(lines.len(), 3);
                    assert!(lines.contains(&1));
                    assert!(lines.contains(&2));
                    assert!(lines.contains(&3));
                } else {
                    panic!("Expected DamageArea::CursorLines");
                }
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_editor_area() {
        // Merge EditorArea with existing EditorArea (should dedupe)
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea]);
        damage.merge(Damage::Areas(vec![DamageArea::EditorArea]));
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_none() {
        // None is identity for merge
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea]);
        damage.merge(Damage::None);
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::EditorArea));
            }
            _ => panic!("Expected Damage::Areas"),
        }
    }

    #[test]
    fn test_cmd_damage_merge_full_absorbs() {
        // Full absorbs everything
        let mut damage = Damage::Areas(vec![DamageArea::EditorArea, DamageArea::StatusBar]);
        damage.merge(Damage::Full);
        assert!(damage.is_full());

        // Full is also absorbed (stays Full)
        let mut damage = Damage::Full;
        damage.merge(Damage::Areas(vec![DamageArea::EditorArea]));
        assert!(damage.is_full());
    }

    #[test]
    fn test_damage_editor_area_helper() {
        let damage = Damage::editor_area();
        assert!(damage.includes_editor());
        assert!(!damage.includes_status_bar());
    }

    #[test]
    fn test_damage_status_bar_helper() {
        let damage = Damage::status_bar();
        assert!(damage.includes_status_bar());
        assert!(!damage.includes_editor());
    }

    #[test]
    fn test_damage_cursor_lines_helper() {
        let damage = Damage::cursor_lines(vec![5, 10]);
        assert!(damage.includes_editor());
        if let Some(lines) = damage.cursor_lines_only() {
            assert_eq!(lines.len(), 2);
        } else {
            panic!("Expected cursor_lines_only to return Some");
        }
    }

    #[test]
    fn test_damage_needs_redraw() {
        assert!(!Damage::None.needs_redraw());
        assert!(Damage::Full.needs_redraw());
        assert!(Damage::Areas(vec![DamageArea::EditorArea]).needs_redraw());
        assert!(!Damage::Areas(vec![]).needs_redraw());
    }

    #[test]
    fn test_cmd_redraw_helpers() {
        // redraw_editor includes both EditorArea and StatusBar
        let cmd = Cmd::redraw_editor();
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 2);
                assert!(a.contains(&DamageArea::EditorArea));
                assert!(a.contains(&DamageArea::StatusBar));
            }
            _ => panic!("Expected Damage::Areas"),
        }

        // redraw_status_bar only includes StatusBar
        let cmd = Cmd::redraw_status_bar();
        let damage = cmd.damage();
        match damage {
            Damage::Areas(a) => {
                assert_eq!(a.len(), 1);
                assert!(a.contains(&DamageArea::StatusBar));
            }
            _ => panic!("Expected Damage::Areas"),
        }

        // redraw_cursor_lines with empty vec returns None
        let cmd = Cmd::redraw_cursor_lines(vec![]);
        assert!(matches!(cmd, Cmd::None));
    }
}
