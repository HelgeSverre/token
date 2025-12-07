<center>

# Token

**A minimal text editor for editing code, inspired by Jetbrains, built with Rust**

[![Amp](https://img.shields.io/badge/Amp-191C19.svg?logo=data:image/svg%2bxml;base64,PHN2ZyB3aWR0aD0iMjEiIGhlaWdodD0iMjEiIHZpZXdCb3g9IjAgMCAyMSAyMSIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTMuNzY4NzkgMTguMzAxNUw4LjQ5ODM5IDEzLjUwNUwxMC4yMTk2IDIwLjAzOTlMMTIuNzIgMTkuMzU2MUwxMC4yMjg4IDkuODY3NDlMMC44OTA4NzYgNy4zMzg0NEwwLjIyNTk0IDkuODkzMzFMNi42NTEzNCAxMS42Mzg4TDEuOTQxMzggMTYuNDI4MkwzLjc2ODc5IDE4LjMwMTVaIiBmaWxsPSIjRjM0RTNGIi8+CjxwYXRoIGQ9Ik0xNy40MDc0IDEyLjc0MTRMMTkuOTA3OCAxMi4wNTc1TDE3LjQxNjcgMi41Njg5N0w4LjA3ODczIDAuMDM5OTI0Nkw3LjQxMzggMi41OTQ4TDE1LjI5OTIgNC43MzY4NUwxNy40MDc0IDEyLjc0MTRaIiBmaWxsPSIjRjM0RTNGIi8+CjxwYXRoIGQ9Ik0xMy44MTg0IDE2LjM4ODNMMTYuMzE4OCAxNS43MDQ0TDEzLjgyNzYgNi4yMTU4OEw0LjQ4OTcxIDMuNjg2ODNMMy44MjQ3NyA2LjI0MTcxTDExLjcxMDEgOC4zODM3NkwxMy44MTg0IDE2LjM4ODNaIiBmaWxsPSIjRjM0RTNGIi8+Cjwvc3ZnPg==)](https://ampcode.com/@helgesverre)
![License: MIT](https://img.shields.io/badge/License-MIT-teal.svg?style=flat-square)
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
|----------------|--------------------------------------------------|
| `make build`   | Build debug binary                               |
| `make release` | Build optimized release binary                   |
| `make run`     | Run with default sample file (indentation.txt)   |
| `make dev`     | Run debug build (faster compile, slower runtime) |
| `make clean`   | Remove build artifacts                           |
| `make fmt`     | Format Rust code and markdown files              |

### Testing

| Command                   | Description           |
|---------------------------|-----------------------|
| `make test`               | Run all tests         |
| `make test-one TEST=name` | Run a specific test   |
| `make test-verbose`       | Run tests with output |

### Sample File Runners

| Command             | Description                                    |
|---------------------|------------------------------------------------|
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
|---------------------|----------------------------------|
| `make sample-files` | Generate large/binary test files |

## License

This project is licensed under the [MIT License](LICENSE.md)

The included font, [JetBrains Mono](assets/JetBrainsMono.ttf), is licensed under the [OFL-1.1](assets/OFL.txt).
