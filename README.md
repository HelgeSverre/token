<center>

# Token

**A minimal text editor for editing code, inspired by Jetbrains, built with Rust**

![License: MIT](https://img.shields.io/badge/License-MIT-teal.svg?style=flat-square)
[![Powered by (accent)](https://img.shields.io/badge/Powered%20by-Amp-F34E3F.svg?style=flat-square&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iNDAwIiBoZWlnaHQ9IjQwMCIgdmlld0JveD0iMCAwIDI4IDI4IiBmaWxsPSJub25lIiB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciPgo8cGF0aCBkPSJNMTMuOTE5NyAxMy42MUwxNy4zODE2IDI2LjU2NkwxNC4yNDIgMjcuNDA0OUwxMS4yNjQ1IDE2LjI2NDNMMC4xMTk5MjYgMTMuMjkwNkwwLjk1NzgxNyAxMC4xNUwxMy45MTk3IDEzLjYxWiIgZmlsbD0iI0ZGRkZGRiIvPgo8cGF0aCBkPSJNMTMuNzM5MSAxNi4wODkyTDQuODgxNjkgMjQuOTA1NkwyLjU4ODcyIDIyLjYwMTlMMTEuNDQ2MSAxMy43ODY1TDEzLjczOTEgMTYuMDg5MloiIGZpbGw9IiNGRkZGRkYiLz4KPHBhdGggZD0iTTE4LjkzODYgOC41ODMxNUwyMi40MDA1IDIxLjUzOTJMMTkuMjYwOSAyMi4zNzgxTDE2LjI4MzMgMTEuMjM3NEw1LjEzODc5IDguMjYzODFMNS45NzY2OCA1LjEyMzE4TDE4LjkzODYgOC41ODMxNVoiIGZpbGw9IiNGRkZGRkYiLz4KPHBhdGggZD0iTTIzLjk4MDMgMy41NTYzMkwyNy40NDIyIDE2LjUxMjRMMjQuMzAyNSAxNy4zNTEyTDIxLjMyNSA2LjIxMDYyTDEwLjE4MDUgMy4yMzY5OEwxMS4wMTgzIDAuMDk2MzU5M0wyMy45ODAzIDMuNTU2MzJaIiBmaWxsPSIjRkZGRkZGIi8%2BCjwvc3ZnPgo%3D)](https://ampcode.com/@helgesverre)

</center>

---

## About

Token is a lightweight, high-performance text editor built with Rust. It is designed to be a simple, distraction-free
environment for writing beautiful and efficient code.

## Building from Source

To build Token from source, you will need the Rust toolchain installed.

```bash
git clone https://github.com/HelgeSverre/token
cd token

# --- Makefile for convenience ---
# Install dependencies
make setup


make setup

# Or build and run manually with cargo
cargo build --release
cargo run
```

## Commands

### Build & Run

| Command        | Description                                      |
| -------------- | ------------------------------------------------ |
| `make build`   | Build debug binary                               |
| `make release` | Build optimized release binary                   |
| `make run`     | Run with default sample file (indentation.txt)   |
| `make dev`     | Run debug build (faster compile, slower runtime) |
| `make clean`   | Remove build artifacts                           |
| `make fmt`     | Format Rust code and markdown files              |

### Testing

| Command                   | Description           |
| ------------------------- | --------------------- |
| `make test`               | Run all tests         |
| `make test-one TEST=name` | Run a specific test   |
| `make test-verbose`       | Run tests with output |

### Sample File Runners

| Command             | Description                                    |
| ------------------- | ---------------------------------------------- |
| `make run-indent`   | Test smart home/end with indented code         |
| `make run-large`    | Test with large file (10k lines)               |
| `make run-mixed`    | Test mixed tabs/spaces                         |
| `make run-trailing` | Test trailing whitespace                       |
| `make run-long`     | Test long lines (horizontal scroll)            |
| `make run-binary`   | Test binary file handling                      |
| `make run-unicode`  | Test unicode/emoji content                     |
| `make run-emoji`    | Test mixed languages, emojis, accents, box art |
| `make run-zalgo`    | Test progressive Zalgo text corruption         |
| `make run-empty`    | Test empty file                                |
| `make run-single`   | Test single line file                          |
| `make run-code`     | Test with Rust source code                     |

### Setup

| Command             | Description                      |
| ------------------- | -------------------------------- |
| `make sample-files` | Generate large/binary test files |

## License

This project is licensed under the [MIT License](LICENSE.md)

The included font, [JetBrains Mono](assets/JetBrainsMono.ttf), is licensed under the [OFL-1.1](assets/OFL.txt).
