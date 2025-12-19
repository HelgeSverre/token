# Token - A Multi-Cursor Text Editor

**Multi-cursor, code editor inspired by JetBrains IDEs, Vibe-coded in Rust, using Amp Code.**
<br>
Most of the threads, prompts and conversations with the agent is available to view on
my [Amp profile](https://ampcode.com/@helgesverre).

![Rust](https://img.shields.io/badge/Rust-000000.svg?logo=rust&logoColor=white&style=flat)
[![Amp](https://img.shields.io/badge/Amp%20Code-191C19.svg?logo=data:image/svg%2bxml;base64,PHN2ZyB3aWR0aD0iMjEiIGhlaWdodD0iMjEiIHZpZXdCb3g9IjAgMCAyMSAyMSIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTMuNzY4NzkgMTguMzAxNUw4LjQ5ODM5IDEzLjUwNUwxMC4yMTk2IDIwLjAzOTlMMTIuNzIgMTkuMzU2MUwxMC4yMjg4IDkuODY3NDlMMC44OTA4NzYgNy4zMzg0NEwwLjIyNTk0IDkuODkzMzFMNi42NTEzNCAxMS42Mzg4TDEuOTQxMzggMTYuNDI4MkwzLjc2ODc5IDE4LjMwMTVaIiBmaWxsPSIjRjM0RTNGIi8+CjxwYXRoIGQ9Ik0xNy40MDc0IDEyLjc0MTRMMTkuOTA3OCAxMi4wNTc1TDE3LjQxNjcgMi41Njg5N0w4LjA3ODczIDAuMDM5OTI0Nkw3LjQxMzggMi41OTQ4TDE1LjI5OTIgNC43MzY4NUwxNy40MDc0IDEyLjc0MTRaIiBmaWxsPSIjRjM0RTNGIi8+CjxwYXRoIGQ9Ik0xMy44MTg0IDE2LjM4ODNMMTYuMzE4OCAxNS43MDQ0TDEzLjgyNzYgNi4yMTU4OEw0LjQ4OTcxIDMuNjg2ODNMMy44MjQ3NyA2LjI0MTcxTDExLjcxMDEgOC4zODM3NkwxMy44MTg0IDE2LjM4ODNaIiBmaWxsPSIjRjM0RTNGIi8+Cjwvc3ZnPg==&style=flat)](https://ampcode.com/@helgesverre)
![License: MIT](https://img.shields.io/badge/License-MIT-007ACC.svg?style=flat)
[![token-editor.com](https://img.shields.io/badge/🌐_token--editor.com-007ACC?style=flat)](https://token-editor.com)

<img src="assets/screenshot-v2-pretty.png" alt="Token Screenshot" />

---

## Installation

### Pre-built Binaries

Download the latest release for your platform from [token-editor.com](https://token-editor.com#install)
or [GitHub Releases](https://github.com/HelgeSverre/token/releases).

### Building from Source

Requires the [Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/HelgeSverre/token
cd token
make setup    # Install dependencies
make release  # Build optimized binary
make run      # Run the editor
```

For development:

```bash
make dev      # Run debug build (faster compile)
make watch    # Start bacon watch mode
```

---

## Quick Start

1. **Open a file**: `token path/to/file.rs` or use Cmd+O
2. **Open a folder**: `token path/to/project/` or use Cmd+Shift+O
3. **Command palette**: Cmd+Shift+A for all available commands
4. **Toggle sidebar**: Cmd+1 to show/hide the file tree

---

## Keyboard Shortcuts

Common shortcuts (Cmd = Command on macOS, Ctrl on Windows/Linux):

### Files & Navigation

| Action          | Shortcut    |
|-----------------|-------------|
| Save            | Cmd+S       |
| Open File       | Cmd+O       |
| Open Folder     | Cmd+Shift+O |
| Command Palette | Cmd+Shift+A |
| Go to Line      | Cmd+L       |
| Find/Replace    | Cmd+F       |
| Toggle Sidebar  | Cmd+1       |

### Editing

| Action         | Shortcut      |
|----------------|---------------|
| Undo           | Cmd+Z         |
| Redo           | Cmd+Shift+Z   |
| Copy/Cut/Paste | Cmd+C/X/V     |
| Select All     | Cmd+A         |
| Duplicate      | Cmd+D         |
| Delete Line    | Cmd+Backspace |

### Multi-Cursor

| Action            | Shortcut           |
|-------------------|--------------------|
| Add cursor        | Option+Click       |
| Add cursor above  | Option+Option+Up   |
| Add cursor below  | Option+Option+Down |
| Select next match | Cmd+J              |

_Note: Option+Option shortcuts are hardcoded and not remappable via keymap.yaml._

### Navigation

| Action             | Shortcut            |
|--------------------|---------------------|
| Word left/right    | Option+←/→          |
| Line start/end     | Cmd+←/→ or Home/End |
| Document start/end | Ctrl+Home/End       |
| Expand selection   | Option+↑            |
| Shrink selection   | Option+↓            |

For the complete keybinding reference, see [docs/KEYBINDINGS.md](docs/KEYBINDINGS.md).

---

## Configuration

Configuration files are stored in `~/.config/token-editor/`:

| File                 | Purpose            |
|----------------------|--------------------|
| `keymap.yaml`        | Custom keybindings |
| `themes/<name>.yaml` | Custom themes      |

See the documentation for details:

- [Keybindings](docs/KEYBINDINGS.md) — Customize keyboard shortcuts
- [Themes](docs/THEMES.md) — Create and customize themes

---

## Commands

Run `make help` for the full command list.

| Command        | Description                      |
|----------------|----------------------------------|
| `make build`   | Build debug binary               |
| `make release` | Build optimized release binary   |
| `make run`     | Run release build                |
| `make dev`     | Run debug build (faster compile) |
| `make test`    | Run all tests                    |
| `make fmt`     | Format code                      |
| `make lint`    | Run clippy lints                 |

---

## Built with AI

Token was built primarily through conversations with AI coding assistants, demonstrating effective human-AI
collaboration on complex software projects. The development process, methodology, and all 100+ conversation threads are
documented publicly.

- **[Building with AI](docs/BUILDING_WITH_AI.md)** — The framework used to build Token
- **[Amp Code Profile](https://ampcode.com/@helgesverre)** — View the conversation threads

---

## Documentation

| Document                                              | Description                             |
|-------------------------------------------------------|-----------------------------------------|
| [KEYBINDINGS.md](docs/KEYBINDINGS.md)                 | Complete keyboard shortcuts reference   |
| [THEMES.md](docs/THEMES.md)                           | Theme customization guide               |
| [BUILDING_WITH_AI.md](docs/BUILDING_WITH_AI.md)       | AI-assisted development framework       |
| [EDITOR_UI_REFERENCE.md](docs/EDITOR_UI_REFERENCE.md) | Technical reference for editor geometry |
| [ROADMAP.md](docs/ROADMAP.md)                         | Planned features                        |
| [CHANGELOG.md](docs/CHANGELOG.md)                     | Version history                         |
| [CONTRIBUTING.md](docs/CONTRIBUTING.md)               | Contribution guidelines                 |

---

## License

This project is licensed under the [MIT License](LICENSE.md).

The included font, [JetBrains Mono](assets/JetBrainsMono.ttf), is licensed under the [OFL-1.1](assets/OFL.txt).
