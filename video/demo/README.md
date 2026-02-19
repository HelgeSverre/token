# Token Editor

A fast, minimal multi-cursor text editor built with Rust.

## Features

- **Syntax Highlighting** — 17 languages via tree-sitter
- **Multi-Cursor** — Select next occurrence, edit everywhere
- **Split View** — Side-by-side editing
- **CSV Editor** — Spreadsheet-like view for data files
- **Code Outline** — Tree-sitter powered symbol navigation
- **Themes** — 9 built-in themes, custom YAML themes
- **Configurable** — YAML keybindings, per-project settings

## Quick Start

```bash
# Install
brew install helgesverre/tap/token

# Open a file
token main.ts

# Open a workspace
token ./my-project
```

## Keyboard Shortcuts

| Action             | Shortcut        |
|--------------------|-----------------|
| Command Palette    | Cmd+Shift+A     |
| Find/Replace       | Cmd+F           |
| Multi-Cursor       | Cmd+J           |
| Split View         | Cmd+Shift+Alt+V |
| File Explorer      | Cmd+1           |
| Code Outline       | Cmd+7           |

## License

MIT
