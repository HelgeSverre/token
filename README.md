# Token

A minimal text editor for editing code, inspired by Jetbrains behaviour, built with Rust.

<!-- Placeholder for Logo/Screenshot -->

## About

Token is a lightweight, high-performance text editor built with Rust. It is designed to be a simple, distraction-free
environment for writing beautiful and efficient code.

## Building from Source

To build Token from source, you will need the Rust toolchain installed.

```bash
git clone https://github.com/HelgeSverre/token
cd token
cargo build --release
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
