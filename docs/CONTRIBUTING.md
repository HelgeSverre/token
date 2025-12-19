# Contributing to Token

Thank you for your interest in contributing to Token!

---

## Development Setup

1. **Install Rust**: Get the [Rust toolchain](https://rustup.rs/)

2. **Clone and build**:
   ```bash
   git clone https://github.com/HelgeSverre/token
   cd token
   make setup    # Install dev dependencies
   make build    # Build debug binary
   ```

3. **Run**:
   ```bash
   make dev      # Run debug build
   make run      # Run release build
   ```

---

## Running Tests

```bash
make test           # Run all tests
make test-verbose   # Run with output
make test-fast      # Fast parallel tests (nextest)
```

---

## Code Style

- **Format before committing**: Run `make fmt`
- **Lint check**: Run `make lint` (mirrors CI)
- **Follow existing patterns**: Match the style of surrounding code

---

## Smart Commits

This project includes a `/commit` slash command for Claude Code that:
- Analyzes staged and unstaged changes
- Creates atomic commits following Conventional Commits format
- Groups related changes logically

Usage in Claude Code:
```
/commit
/commit refactoring the authentication flow
```

---

## Pull Requests

- **One feature/fix per PR**: Keep changes focused
- **Include tests**: Add tests for new functionality
- **Update docs**: If your change affects user-facing behavior, update relevant docs
- **Descriptive commits**: Use clear, concise commit messages

---

## Architecture Overview

Token follows the **Elm Architecture** pattern:

```
Message → Update → Command → Render
```

Key directories:
- `src/model/` - Data structures (AppModel, Document, EditorState)
- `src/update/` - Pure state transformation
- `src/view/` - Rendering (Frame, TextPainter, widgets)
- `src/runtime/` - App lifecycle and input handling
- `src/keymap/` - Configurable keybindings

For detailed architecture documentation, see:
- [EDITOR_UI_REFERENCE.md](EDITOR_UI_REFERENCE.md) - Viewport, coordinates, scrolling
- [ROADMAP.md](ROADMAP.md) - Planned features
- [CHANGELOG.md](CHANGELOG.md) - Version history

---

## Getting Help

- Visit [token-editor.com](https://token-editor.com) for documentation and downloads
- Open an issue for bugs or feature requests
- Check existing issues before creating new ones
