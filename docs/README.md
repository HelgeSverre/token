# Token Editor Documentation

Start here to understand, use, and contribute to Token.

---

## Quick Links

| I want to...                | Go to                                        |
| --------------------------- | -------------------------------------------- |
| Visit the website           | [token-editor.com](https://token-editor.com) |
| Learn the keybindings       | [KEYBINDINGS.md](KEYBINDINGS.md)             |
| Customize themes            | [User Guide: Themes](user/config-theme.md)   |
| See what's shipped          | [CHANGELOG.md](CHANGELOG.md)                 |
| Understand the architecture | [Architecture](#architecture)                |
| Contribute a feature        | [Contributing](#contributing)                |

---

## User Documentation

Configuration and usage guides for end users.

| Document                                        | Description                                                  |
| ----------------------------------------------- | ------------------------------------------------------------ |
| [config-editor.md](user/config-editor.md)       | Editor settings reference                                    |
| [language-servers.md](user/language-servers.md) | Language server setup, preferences, and troubleshooting      |
| [editorconfig.md](user/editorconfig.md)         | Per-file indentation, line endings, and save cleanup         |
| [previews.md](user/previews.md)                 | Markdown/HTML resources, local links, and directory boundaries |
| [folding.md](user/folding.md)                   | Gutter controls, commands, language support, and persistence |
| [config-keymap.md](user/config-keymap.md)       | Keymap configuration reference                               |
| [config-theme.md](user/config-theme.md)         | Theme configuration reference                                |
| [KEYBINDINGS.md](KEYBINDINGS.md)               | Complete keyboard shortcut reference                          |
| [THEMES.md](THEMES.md)                         | Theme format reference and built-in theme catalog             |
| [AUTOMATION.md](AUTOMATION.md)                 | Automation endpoint for tests, measurements, and MCP clients  |

---

## Developer Documentation

Architecture, contracts, and implementation guides.

Shipped plans and past analyses (refactoring audits, profiling reports, the
completion refactoring plan) live in [`archived/`](archived/).
Language integration guide: [Adding Tree-sitter languages](feature/adding-languages.md)
(maintained reference, not an unfinished feature plan).

| Document                                                        | Description                                                        |
| ---------------------------------------------------------------- | -------------------------------------------------------------------- |
| [automation-input.md](dev/automation-input.md)                   | Window-local pointer/wheel/focus injection and the native smoke check |
| [contracts-selection.md](dev/contracts-selection.md)             | Selection semantics and multi-cursor rules (behavior contract)      |
| [contracts-undo.md](dev/contracts-undo.md)                       | Undo/redo behavior and edit grouping (behavior contract)            |
| [lsp-tools-overview.md](dev/lsp-tools-overview.md)               | Catalog of LSP servers and formatters, with Token rollout status    |
| [ui-gallery.md](dev/ui-gallery.md)                               | Native UI gallery (`just ui-gallery`) for component specimens       |
| [ui-component-inventory.md](dev/ui-component-inventory.md)       | UI component inventory and design discussion (2026-09-11)           |
| [release-0.7.0-readiness.md](dev/release-0.7.0-readiness.md)      | 0.7.0 release preparation checklist                                 |
| [agent-archive.md](dev/agent-archive.md)                         | Private agent-session archive tooling                                |
| [agent-viewer.md](dev/agent-viewer.md)                           | Website agent-viewer design fixtures                                 |
| [agent-transcript-research.md](dev/agent-transcript-research.md)  | Agent transcript viewer research (2026-09-16)                        |

### UI Component Reference

The native UI component manuals live in [ui/](ui/README.md), following the
[technical standard](ui/TECHNICAL-STANDARD.md). Start at
[FOUNDATIONS.md](ui/FOUNDATIONS.md); the adoption roadmap is in
[IMPLEMENTATION-ROADMAP.md](ui/IMPLEMENTATION-ROADMAP.md), with an
IntelliJ-feature crosswalk in [INTELLIJ-CROSSWALK.md](ui/INTELLIJ-CROSSWALK.md).

## Feature Design Documents

Active specifications live in `docs/feature/` and `docs/future/`. Implemented or
superseded plans live in `docs/archived/`, with status notes distinguishing shipped
scope from deferred ideas and manual verification. Archival is not a release.

Plan sweeps: the initial external-file protection slice was implemented and
verified on native macOS (2026-09-08), and the coordinated
[auto-save, EditorConfig, and code folding plan](archived/file-policy-and-folding-plan.md)
was implemented and reviewed on 2026-09-09. Earlier completed Settings v1,
terminal, context-menu, Find, soft-wrap and LSP plans are already archived. See
the [reconciliation record](archived/refactoring-audit-2026-09-06.md#plan-reconciliation--2026-09-08)
for partial implementations and suggested next work.

### Completed Features

| Feature                                 | Status                                                               | Design Doc                                                                    |
| --------------------------------------- | -------------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Syntax Highlighting                     | ✅ MVP                                                               | [syntax-highlighting.md](archived/syntax-highlighting.md)                     |
| Workspace Management                    | ✅ P0-6                                                              | [workspace-management.md](archived/workspace-management.md)                   |
| CSV Viewer/Editor                       | ✅ P1-2                                                              | [csv-editor.md](archived/csv-editor.md)                                       |
| Recent Files                            | ✅ MVP                                                               | [recent-files.md](archived/recent-files.md)                                   |
| Markdown Preview (incl. offline Mermaid)| ✅                                                                  | [markdown-preview.md](archived/markdown-preview.md)                          |
| Syntax-Aware Expand Selection           | ✅ Base phase                                                        | [syntax-aware-expand-selection.md](archived/syntax-aware-expand-selection.md) |
| Overlay Surface & Search Everywhere     | ✅ P1-5                                                              | [overlay-surface.md](archived/overlay-surface.md)                             |
| Editor Decorations & Gutter Lanes       | ✅ P1-3                                                              | [editor-decorations.md](archived/editor-decorations.md)                       |
| LSP Integration                         | ✅ P1-5: hover, completion, signature help, rename, code actions, format, usages, workspace symbols | [lsp-integration.md](archived/lsp-integration.md)               |
| Context Menu                            | ✅                                                                   | [context-menu.md](archived/context-menu.md)                                   |
| Embedded Terminal (tabs, selection, links) | ✅ MVP                                                            | [embedded-terminal.md](archived/embedded-terminal.md)                         |
| File Dialogs                            | ✅ via `rfd`                                                         | [file-dialogs.md](archived/file-dialogs.md)                                   |
| File Drag-and-Drop                      | ✅                                                                   | [handle-file-dropping.md](archived/handle-file-dropping.md)                   |
| Image Viewer & Binary Placeholder       | ✅ v0.4.0                                                            | [2026-02-27-image-viewer-and-binary-placeholder.md](archived/plans/2026-02-27-image-viewer-and-binary-placeholder.md) |
| External File Changes                   | ✅ Initial text-file protection; native macOS verified               | [file-change-detection.md](archived/file-change-detection.md)                 |
| Session Restore                         | ✅ Saved-file tabs/splits/pane state; native macOS restarts verified | [session-restore.md](archived/session-restore.md)                             |
| Find Enhancements                       | ✅                                                                   | [find-enhancements.md](archived/find-enhancements.md)                         |
| Docked Find and Replace                 | ✅ v0.7.0                                                            | [find-enhancements.md](archived/find-enhancements.md)                         |
| Damage Tracking                         | ✅ Coarse regions + cursor lines                                     | [DAMAGE-TRACKING.md](archived/DAMAGE-TRACKING.md)                             |
| Command Palette History, Pins & Ranking | ✅ Via OverlaySurface P4                                             | [command-palette-enhancements.md](archived/command-palette-enhancements.md)   |
| Select Next Occurrence                  | ✅                                                                   | [select-next-occurrence.md](archived/select-next-occurrence.md)               |
| Column Selection                        | ✅                                                                   | [column-selection.md](archived/column-selection.md)                           |
| Soft Wrap                               | ✅ Implemented, unreleased                                           | [soft-wrap.md](archived/soft-wrap.md)                                         |
| Settings v1 (presets + LSP)             | ✅ Implemented, unreleased                                           | [settings-page.md](archived/settings-page.md)                                 |
| Pixel Scrolling                         | ✅ v0.7.0; native macOS verified                                     | [pixel-scrolling.md](archived/pixel-scrolling.md)                             |
| Auto-Save                               | ✅ Implemented (2026-09-09), unreleased                              | [file-policy-and-folding-plan.md](archived/file-policy-and-folding-plan.md)   |
| EditorConfig Integration                | ✅ Implemented (2026-09-09), unreleased                              | [file-policy-and-folding-plan.md](archived/file-policy-and-folding-plan.md)   |
| Code Folding                            | ✅ Implemented (2026-09-09), unreleased                              | [file-policy-and-folding-plan.md](archived/file-policy-and-folding-plan.md)   |
| Find Usages panel                       | ✅ v0.7.0                                                            | [lsp-integration.md](archived/lsp-integration.md)                             |
| Search Everywhere workspace symbols    | ✅ v0.7.0                                                            | [overlay-surface.md](archived/overlay-surface.md)                             |
| Settings Keymap Tab                     | ✅ Implemented; cross-platform validation open                       | [settings-keymap.md](archived/settings-keymap.md)                             |
| Shared Syntax Parsing                   | ✅ Verified host-tree parsing across highlight paths                 | [shared-syntax-parsing.md](archived/shared-syntax-parsing.md)                 |
| Completion Refactoring                  | ✅ Handoff plan and progress log shipped (2026-09-15)                | [completion-refactoring-plan.md](archived/completion-refactoring-plan.md)     |

### Active and Planned Features

| Feature                                                       | Milestone                   | Design Doc                                                             |
| ------------------------------------------------------------- | --------------------------- | ---------------------------------------------------------------------- |
| Command History Follow-ups                                    | Deferred polish             | [command-history-followups.md](future/command-history-followups.md)    |
| File Finder Enhancements (basic finder implemented)           | 1 - Navigation              | [enhanced-file-finder.md](future/enhanced-file-finder.md)              |
| Go to Line Enhancements                                       | 1 - Navigation              | [goto-line-enhancements.md](future/goto-line-enhancements.md)          |
| Replace Enhancements (selection scope implemented)            | 2 - Search & Editing        | [replace-enhancements.md](feature/replace-enhancements.md)             |
| Line Operations Follow-ups (duplication implemented)          | 2 - Search & Editing        | [line-operations.md](future/line-operations.md)                        |
| Whitespace Rendering                                          | 2 - Search & Editing        | [whitespace-rendering.md](future/whitespace-rendering.md)              |
| Whitespace Management (conversion and cleanup)                | 2 - Search & Editing        | [whitespace-management.md](future/whitespace-management.md)            |
| Indentation Guides (basic implemented; advanced scope planned) | 2 - Refinement             | [indent-guides.md](feature/indent-guides.md)                           |
| Folding follow-ups (basic and syntax folding implemented via the coordinated plan; deferred ideas remain) | Deferred scope | [folding-advanced.md](feature/folding-advanced.md) |
| Configurable Double-Tap Gestures                              | 3 - Keybinding Enhancements | [gesture-bindings.md](feature/gesture-bindings.md)                     |
| Diff Gutter                                                   | 5 - Insight Tools           | [diff-gutter.md](feature/diff-gutter.md)                               |
| Snippets                                                      | 6 - Productivity            | [snippets.md](future/snippets.md)                                     |
| Autocomplete (inline / FIM)                                   | 6 - Productivity — phases 1, 2, 4 and most of 3/5 shipped; IME/platform verification open | [autocomplete.md](feature/autocomplete.md) |
| Macros                                                        | 7 - Productivity            | [macros.md](future/macros.md)                                         |
| Keymap Enhancements (chords partly implemented)               | Future follow-ups           | [keymap-enhancements.md](future/keymap-enhancements.md)                |
| Sema Scripting Integration                                    | Future extensibility        | [sema-scripting-integration.md](future/sema-scripting-integration.md) |
| WASM / Web Target                                             | Feasibility proposal        | [wasm-web-target.md](future/wasm-web-target.md)                        |
| Performance Panel                                             | Planned                     | [performance-panel.md](feature/performance-panel.md)                   |
| Editor Visual Polish                                          | Plan only                   | [editor-visual-polish.md](feature/editor-visual-polish.md)              |

---

## Architecture

Token follows the **Elm Architecture** pattern:

```
User Input → Message → Update → Command → Render
     ↑                                      │
     └──────────────────────────────────────┘
```

### Core Modules

| Module   | Location          | Purpose                         |
| -------- | ----------------- | ------------------------------- |
| Model    | `src/model/`      | AppModel, Document, EditorState |
| Messages | `src/messages.rs` | Msg, EditorMsg, DocumentMsg     |
| Update   | `src/update/`     | Pure state transformation       |
| View     | `src/view/`       | CPU rendering pipeline          |
| Keymap   | `src/keymap/`     | Configurable keybindings        |

### Module Map

```
src/
├── main.rs / lib.rs     # Entry point, library root
├── runtime/             # Event loop, windowing, app lifecycle
│   └── app.rs           # winit ApplicationHandler, event dispatch
├── model/               # State structures
│   ├── document.rs      # Text buffer, undo/redo
│   ├── editor.rs        # Cursor, Selection, Viewport
│   ├── editor_area.rs   # Splits, tabs, groups
│   └── workspace.rs     # File tree
├── update/              # Pure state transformation (one module per feature)
│   ├── editor.rs        # Cursor movement, selection
│   ├── document.rs      # Text editing
│   ├── layout.rs        # Splits, tabs
│   ├── lsp.rs           # Language-server requests and replies
│   ├── folding.rs       # Code folding
│   ├── file_change.rs   # External-file protection
│   └── ...
├── view/                # CPU rendering pipeline
│   ├── frame.rs         # Drawing primitives, clip stack
│   ├── editor_text.rs   # Editor surface
│   ├── overlay_surface.rs # Command palette / overlays
│   ├── settings_page.rs # Settings page
│   └── ...
├── keymap/              # Key handling
│   ├── keymap.rs        # Lookup engine
│   └── defaults.rs      # Default bindings (embedded keymap.yaml)
├── layout/              # Clay-based declarative layout engine
├── syntax/              # Highlighting
│   └── worker.rs        # Background parser
├── completion/          # Dropdown menu, LSP items, inline/FIM suggestions
├── lsp/                 # Language Server Protocol client
├── terminal/            # Embedded terminal
├── csv/                 # CSV viewer/editor
├── folding/             # Fold-region detection
├── outline/             # Symbol outline
├── image/               # Image viewer
├── markdown/            # Markdown preview
├── editable/            # Shared text-editing primitives (modals, CSV cells)
├── context_menu/        # Right-click menus
├── settings/            # Settings model and persistence
├── config/              # Configuration files
├── panel/               # Dock layout
├── panels/              # Dock panel contents (terminal, placeholder)
├── editorconfig.rs      # .editorconfig parsing
├── session.rs           # Session restore
├── recent_files.rs      # Recent file tracking
├── fs_watcher.rs        # File system watching
├── automation.rs / mcp.rs # Automation bridge and MCP server
└── cli.rs               # Command-line interface
```

---

## Contributing

### Before You Start

1. Read the relevant feature design doc in `docs/feature/`
2. Understand the behavior contracts in `docs/dev/`

### Creating a Feature Doc

1. Create the design doc under `docs/feature/` (or `docs/future/` for deferred ideas), modelled on an existing spec there
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
- [EDITOR_UI_REFERENCE.md](EDITOR_UI_REFERENCE.md) - Implementer's guide to
  editor UI geometry, coordinate transformations, and edge cases
- [PROFILING.md](PROFILING.md) - Performance analysis tools and workflows (macOS)
- [CONTRIBUTING.md](CONTRIBUTING.md) - Contribution guide
- [BUILDING_WITH_AI.md](BUILDING_WITH_AI.md) - Methodology for developing Token
  with AI assistants, with the [Amp conversation archive](ampcode-threads/README.md)
- [Benchmarking and reports](benchmark/README.md) - Performance testing guide,
  historical baselines and current optimized measurements
