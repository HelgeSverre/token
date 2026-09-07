# Token Editor Documentation

Start here to understand, use, and contribute to Token.

---

## Quick Links

| I want to... | Go to |
|--------------|-------|
| Visit the website | [token-editor.com](https://token-editor.com) |
| Learn the keybindings | [User Guide: Keymap](user/config-keymap.md) |
| Customize themes | [User Guide: Themes](user/config-theme.md) |
| See what's shipped | [CHANGELOG.md](CHANGELOG.md) |
| Understand the architecture | [Architecture](#architecture) |
| Contribute a feature | [Contributing](#contributing) |

---

## User Documentation

Configuration and usage guides for end users.

| Document | Description |
|----------|-------------|
| [config-editor.md](user/config-editor.md) | Editor settings reference |
| [config-keymap.md](user/config-keymap.md) | Keymap configuration reference |
| [config-theme.md](user/config-theme.md) | Theme configuration reference |

---

## Developer Documentation

Architecture, contracts, and implementation guides.

Recent analysis: [Refactoring and production-path profiling, 2026-09-05](dev/refactoring-profile-2026-09-05.md).
Implementation and verification follow-ups, including completion profiling:
[Refactoring audit, 2026-09-06](dev/refactoring-audit-2026-09-06.md).

### Behavior Contracts

These documents define invariants that implementations must preserve:

| Contract | Description |
|----------|-------------|
| [contracts-selection.md](dev/contracts-selection.md) | Selection semantics and multi-cursor rules |
| [contracts-undo.md](dev/contracts-undo.md) | Undo/redo behavior and edit grouping |

### Templates

| Template | Use for |
|----------|---------|
| [FEATURE_SPEC.md](templates/FEATURE_SPEC.md) | New feature design documents |

---

## Feature Design Documents

Active specifications live in `docs/feature/` and `docs/future/`. Implemented or
superseded plans live in `docs/archived/`, with status notes distinguishing shipped
scope from deferred ideas and manual verification. Archival is not a release.

### Completed Features

| Feature | Status | Design Doc |
|---------|--------|------------|
| Syntax Highlighting | ✅ MVP | [syntax-highlighting.md](archived/syntax-highlighting.md) |
| Workspace Management | ✅ P0-6 | [workspace-management.md](archived/workspace-management.md) |
| CSV Viewer/Editor | ✅ P1-2 | [csv-editor.md](archived/csv-editor.md) |
| Recent Files | ✅ MVP | [recent-files.md](archived/recent-files.md) |
| Markdown Preview | ✅ | [markdown-preview.md](archived/markdown-preview.md) |
| Syntax-Aware Expand Selection | ✅ Base phase | [syntax-aware-expand-selection.md](archived/syntax-aware-expand-selection.md) |
| Overlay Surface & Search Everywhere | ✅ P1-5 | [overlay-surface.md](archived/overlay-surface.md) |
| Editor Decorations & Gutter Lanes | ✅ P1-3 | [editor-decorations.md](archived/editor-decorations.md) |
| LSP Integration | ✅ P1-5 | [lsp-integration.md](archived/lsp-integration.md) |
| Context Menu | ✅ | [context-menu.md](archived/context-menu.md) |
| Embedded Terminal | ✅ MVP | [embedded-terminal.md](archived/embedded-terminal.md) |
| File Dialogs | ✅ via `rfd` | [file-dialogs.md](archived/file-dialogs.md) |
| Find Enhancements | ✅ | [find-enhancements.md](archived/find-enhancements.md) |
| Damage Tracking | ✅ Coarse regions + cursor lines | [DAMAGE-TRACKING.md](archived/DAMAGE-TRACKING.md) |
| Command Palette History, Pins & Ranking | ✅ Via OverlaySurface P4 | [command-palette-enhancements.md](archived/command-palette-enhancements.md) |
| Select Next Occurrence | ✅ | [select-next-occurrence.md](archived/select-next-occurrence.md) |
| Column Selection | ✅ | [column-selection.md](archived/column-selection.md) |
| Soft Wrap | ✅ Implemented, unreleased | [soft-wrap.md](archived/soft-wrap.md) |
| Settings v1 (presets + LSP) | ✅ Implemented, unreleased | [settings-page.md](archived/settings-page.md) |

### Active and Planned Features

| Feature | Milestone | Design Doc |
|---------|-----------|------------|
| Command History Follow-ups | Deferred polish | [command-history-followups.md](future/command-history-followups.md) |
| File Finder Enhancements (basic finder implemented) | 1 - Navigation | [enhanced-file-finder.md](future/enhanced-file-finder.md) |
| Go to Line Enhancements | 1 - Navigation | [goto-line-enhancements.md](future/goto-line-enhancements.md) |
| Replace Enhancements | 2 - Search & Editing | [replace-enhancements.md](feature/replace-enhancements.md) |
| Line Operations Follow-ups (duplication implemented) | 2 - Search & Editing | [line-operations.md](future/line-operations.md) |
| Whitespace Rendering | 2 - Search & Editing | [whitespace-rendering.md](future/whitespace-rendering.md) |
| Auto-Save | 3 - File Lifecycle | [auto-save.md](feature/auto-save.md) |
| File Change Detection | 3 - File Lifecycle | [file-change-detection.md](feature/file-change-detection.md) |
| Session Restore | 3 - File Lifecycle | [session-restore.md](feature/session-restore.md) |
| Code Folding | 4 - Hard Problems | [folding-basic.md](feature/folding-basic.md) |
| Diff Gutter | 5 - Insight Tools | [diff-gutter.md](feature/diff-gutter.md) |
| Snippets | 6 - Productivity | [snippets.md](feature/snippets.md) |
| Settings Keymap Tab | 6 - Productivity | [settings-keymap.md](future/settings-keymap.md) |
| Autocomplete (inline / FIM) | 6 - Productivity | [autocomplete.md](feature/autocomplete.md) |

---

## Architecture

Token follows the **Elm Architecture** pattern:

```
User Input → Message → Update → Command → Render
     ↑                                      │
     └──────────────────────────────────────┘
```

### Core Modules

| Module | Location | Purpose |
|--------|----------|---------|
| Model | `src/model/` | AppModel, Document, EditorState |
| Messages | `src/messages.rs` | Msg, EditorMsg, DocumentMsg |
| Update | `src/update/` | Pure state transformation |
| View | `src/view/` | CPU rendering pipeline |
| Keymap | `src/keymap/` | Configurable keybindings |

### Module Map

```
src/
├── main.rs              # Entry point
├── lib.rs               # Library root
├── model/               # State structures
│   ├── document.rs      # Text buffer, undo/redo
│   ├── editor.rs        # Cursor, Selection, Viewport
│   ├── editor_area.rs   # Splits, tabs, groups
│   └── workspace.rs     # File tree
├── update/              # State transformation
│   ├── editor.rs        # Cursor movement, selection
│   ├── document.rs      # Text editing
│   └── layout.rs        # Splits, tabs
├── view/                # Rendering
│   ├── mod.rs           # Renderer
│   └── frame.rs         # Drawing primitives
├── keymap/              # Key handling
│   ├── keymap.rs        # Lookup engine
│   └── defaults.rs      # Default bindings
└── syntax/              # Highlighting
    └── worker.rs        # Background parser
```

---

## Contributing

### Before You Start

1. Read the relevant feature design doc in `docs/feature/`
2. Understand the behavior contracts in `docs/dev/`

### Creating a Feature Doc

1. Copy [templates/FEATURE_SPEC.md](templates/FEATURE_SPEC.md)
2. Fill in all sections
3. Submit for review before implementation

### Definition of Done

For each feature:

- [ ] Commands appear in palette + documented
- [ ] Default keybinding + configurable override
- [ ] Tests cover 3+ edge cases
- [ ] No regression to selection/cursor invariants
- [ ] Screenshot/GIF for website (if user-visible)

---

## References

- [CHANGELOG.md](CHANGELOG.md) - Version history
- [EDITOR_UI_REFERENCE.md](EDITOR_UI_REFERENCE.md) - UI component reference
- [Benchmarking and reports](benchmark/README.md) - Performance testing guide,
  historical baselines and current optimized measurements
