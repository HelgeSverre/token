# Token Editor Documentation

Start here to understand, use, and contribute to Token.

---

## Quick Links

| I want to... | Go to |
|--------------|-------|
| Visit the website | [token-editor.com](https://token-editor.com) |
| Learn the keybindings | [User Guide: Keymap](user/config-keymap.md) |
| Customize themes | [User Guide: Themes](user/config-theme.md) |
| See what's planned | [ROADMAP.md](ROADMAP.md) |
| See what's shipped | [CHANGELOG.md](CHANGELOG.md) |
| Understand the architecture | [Architecture](#architecture) |
| Contribute a feature | [Contributing](#contributing) |

---

## User Documentation

Configuration and usage guides for end users.

| Document | Description |
|----------|-------------|
| [config-keymap.md](user/config-keymap.md) | Keymap configuration reference |
| [config-theme.md](user/config-theme.md) | Theme configuration reference |

---

## Developer Documentation

Architecture, contracts, and implementation guides.

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

Detailed specifications for major features, located in `docs/feature/`:

### Completed Features

| Feature | Status | Design Doc |
|---------|--------|------------|
| Syntax Highlighting | ✅ MVP | [syntax-highlighting.md](archived/syntax-highlighting.md) |
| Workspace Management | ✅ P0-6 | [workspace-management.md](archived/workspace-management.md) |
| CSV Viewer/Editor | ✅ P1-2 | [csv-editor.md](archived/csv-editor.md) |

### Planned Features

| Feature | Milestone | Design Doc |
|---------|-----------|------------|
| Command Palette Enhancements | 1 - Navigation | [command-palette-enhancements.md](future/command-palette-enhancements.md) |
| Quick Open | 1 - Navigation | planned |
| Go to Line Enhancements | 1 - Navigation | [goto-line-enhancements.md](future/goto-line-enhancements.md) |
| Recent Files | 1 - Navigation | [recent-files.md](feature/recent-files.md) |
| Find Enhancements | 2 - Search & Editing | [find-enhancements.md](feature/find-enhancements.md) |
| Replace Enhancements | 2 - Search & Editing | [replace-enhancements.md](feature/replace-enhancements.md) |
| Select Next Occurrence | 2 - Search & Editing | [select-next-occurrence.md](archived/select-next-occurrence.md) |
| Line Operations | 2 - Search & Editing | [line-operations.md](archived/line-operations.md) |
| Whitespace Rendering | 2 - Search & Editing | [whitespace-rendering.md](future/whitespace-rendering.md) |
| Auto-Save | 3 - File Lifecycle | [auto-save.md](feature/auto-save.md) |
| File Change Detection | 3 - File Lifecycle | [file-change-detection.md](feature/file-change-detection.md) |
| Session Restore | 3 - File Lifecycle | [session-restore.md](feature/session-restore.md) |
| Column Selection | 4 - Hard Problems | [column-selection.md](archived/column-selection.md) |
| Soft Wrap | 4 - Hard Problems | [soft-wrap.md](feature/soft-wrap.md) |
| Code Folding | 4 - Hard Problems | planned |
| Diff Gutter | 5 - Insight Tools | [diff-gutter.md](feature/diff-gutter.md) |
| Markdown Preview | 5 - Insight Tools | [markdown-preview.md](feature/markdown-preview.md) |
| Snippets | 6 - Productivity | [snippets.md](feature/snippets.md) |

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

1. Check [ROADMAP.md](ROADMAP.md) for planned work
2. Read the relevant feature design doc in `docs/feature/`
3. Understand the behavior contracts in `docs/dev/`

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

- [ROADMAP.md](ROADMAP.md) - Feature roadmap and milestones
- [CHANGELOG.md](CHANGELOG.md) - Version history
- [EDITOR_UI_REFERENCE.md](EDITOR_UI_REFERENCE.md) - UI component reference
- [BENCHMARKING.md](BENCHMARKING.md) - Performance testing guide
