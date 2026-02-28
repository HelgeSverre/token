# Justfile Syntax Highlighting Test
# This file demonstrates all major justfile features and syntax constructs.

# ─── Settings ────────────────────────────────────────────────────────────────

set shell := ["bash", "-cu"]
set dotenv-load
set dotenv-filename := ".env.local"
set positional-arguments
set export
set fallback
set ignore-comments
set tempdir := "/tmp/just-builds"
set windows-shell := ["powershell.exe", "-NoLogo", "-Command"]
set unstable

# ─── Variables & Constants ───────────────────────────────────────────────────

project_name := "my-project"
version := "2.4.1"
build_dir := "target" / "release"
timestamp := `date +%Y-%m-%d_%H:%M:%S`
git_hash := `git rev-parse --short HEAD`
git_branch := `git branch --show-current`
num_cpus := num_cpus()
os_family := os_family()
arch_val := arch()
home_dir := env("HOME", "/root")
ci_mode := env("CI", "false")
config_file := join(config_directory(), project_name, "config.toml")

# String operations
upper_name := uppercase(project_name)
snake_name := snakecase(project_name)
kebab_name := kebabcase("MyProject")
camel_name := camelcase("my-project")
trimmed := trim("  hello world  ")
replaced := replace(version, ".", "_")
quoted := quote(build_dir)

# Path operations
abs_build := absolute_path(build_dir)
parent := parent_directory(build_dir)
stem := file_stem("archive.tar.gz")
ext := extension("archive.tar.gz")
filename := file_name("/path/to/file.txt")
clean := clean("/foo//bar/../baz")

# Conditional expressions
profile := if ci_mode == "true" { "release" } else { "debug" }
linker := if os_family == "unix" { "mold" } else { "link" }
test_flag := if env("COVERAGE", "") != "" { "--coverage" } else { "" }
cc := if os() == "macos" { "clang" } else if os() == "linux" { "gcc" } else { "cl" }

# Regular expressions
is_semver := if regex_match(version, '^\d+\.\d+\.\d+$') == "true" { "valid" } else { "invalid" }

# UUID and random
build_id := uuid()
random_port := "80" + random(10, 99)

# ─── Aliases ─────────────────────────────────────────────────────────────────

alias b := build
alias t := test
alias r := run
alias c := clean-all
alias fmt := format
alias l := lint

# ─── Default Recipe ──────────────────────────────────────────────────────────

# List available recipes (default when running `just`)
default:
    @just --list --unsorted

# ─── Build Recipes ───────────────────────────────────────────────────────────

# Build the project in the specified profile
build profile=profile: check-deps
    @echo "Building {{project_name}} ({{profile}}) on {{os()}} {{arch_val}}..."
    cargo build --profile {{profile}}
    @echo "Build ID: {{build_id}}"

# Build with all features enabled
build-all: (build "release")
    cargo build --release --all-features

# Build a specific crate from the workspace
build-crate crate:
    cargo build -p {{crate}} --release

# Cross-compile for a target
cross-compile target="x86_64-unknown-linux-musl":
    cross build --release --target {{target}}
    @echo "Output: target/{{target}}/release/{{project_name}}"

# ─── Test Recipes ────────────────────────────────────────────────────────────

# Run the test suite
test *args='': check-deps
    cargo test {{test_flag}} {{args}}

# Run tests matching a pattern
test-filter pattern:
    cargo test --release -- --test-threads={{num_cpus}} '{{pattern}}'

# Run tests with coverage report
test-coverage:
    #!/usr/bin/env bash
    set -euo pipefail
    echo "Running tests with coverage..."
    cargo llvm-cov --html --output-dir coverage/
    if [[ "{{os()}}" == "macos" ]]; then
        open coverage/html/index.html
    elif command -v xdg-open &>/dev/null; then
        xdg-open coverage/html/index.html
    fi

# Run integration tests only
test-integration: build
    cargo test --test '*' -- --ignored

# ─── Lint & Format ───────────────────────────────────────────────────────────

# Format all source files
format:
    cargo fmt --all
    prettier --write "**/*.{js,ts,json,md,yaml}"

# Run all linters
lint: format
    cargo clippy --all-targets --all-features -- -D warnings
    shellcheck scripts/*.sh

# Check formatting without modifying files
check-format:
    cargo fmt --all -- --check

# ─── Run & Execute ───────────────────────────────────────────────────────────

# Run the application with arguments
run *args:
    cargo run --release -- {{args}}

# Run with environment variable overrides
run-dev:
    RUST_LOG=debug \
    DATABASE_URL="postgres://localhost/dev" \
    cargo run

# Run a specific example
run-example name:
    cargo run --example {{name}} --release

# ─── Docker Recipes ──────────────────────────────────────────────────────────

# Docker image tag
docker_tag := git_hash

# Build Docker image
docker-build tag=docker_tag:
    docker build \
        --build-arg VERSION={{version}} \
        --build-arg GIT_HASH={{git_hash}} \
        --tag {{project_name}}:{{tag}} \
        --tag {{project_name}}:latest \
        .

# Push Docker image to registry
docker-push registry="ghcr.io/myorg": (docker-build)
    docker tag {{project_name}}:latest {{registry}}/{{project_name}}:{{version}}
    docker push {{registry}}/{{project_name}}:{{version}}

# Run in Docker
docker-run *args: (docker-build)
    docker run --rm -it \
        -p 8080:8080 \
        -v "{{invocation_directory()}}:/data" \
        {{project_name}}:latest {{args}}

# ─── Database Recipes ────────────────────────────────────────────────────────

db_url := env("DATABASE_URL", "postgres://localhost:5432/mydb")

# Run database migrations
db-migrate:
    sqlx migrate run --database-url {{db_url}}

# Rollback the last migration
db-rollback:
    sqlx migrate revert --database-url {{db_url}}

# Reset and reseed the database
db-reset: db-rollback db-migrate
    sqlx migrate run --database-url {{db_url}}
    cargo run --bin seed

# ─── Release & Deploy ────────────────────────────────────────────────────────

# Create a new release
release kind="patch":
    #!/usr/bin/env bash
    set -euo pipefail

    current="{{version}}"
    IFS='.' read -r major minor patch <<< "$current"

    case "{{kind}}" in
        major) major=$((major + 1)); minor=0; patch=0 ;;
        minor) minor=$((minor + 1)); patch=0 ;;
        patch) patch=$((patch + 1)) ;;
        *)     echo "Unknown release kind: {{kind}}"; exit 1 ;;
    esac

    new_version="${major}.${minor}.${patch}"
    echo "Bumping version: $current → $new_version"

    sed -i'' -e "s/version = \"$current\"/version = \"$new_version\"/" Cargo.toml
    cargo check
    git add Cargo.toml Cargo.lock
    git commit -m "chore: release v${new_version}"
    git tag -a "v${new_version}" -m "Release v${new_version}"
    echo "Tagged v${new_version}. Push with: git push && git push --tags"

# Deploy to staging
deploy-staging: test lint (docker-push "ghcr.io/myorg")
    kubectl set image deployment/{{project_name}} \
        app=ghcr.io/myorg/{{project_name}}:{{version}} \
        --namespace=staging

# Deploy to production (requires confirmation)
deploy-prod: deploy-staging
    @echo "⚠ Deploying {{version}} to PRODUCTION"
    @read -p "Are you sure? (y/N) " confirm && [ "$confirm" = "y" ] || exit 1
    kubectl set image deployment/{{project_name}} \
        app=ghcr.io/myorg/{{project_name}}:{{version}} \
        --namespace=production

# ─── Utility Recipes ─────────────────────────────────────────────────────────

# Check that all dependencies are installed
[private]
check-deps:
    @command -v cargo >/dev/null 2>&1 || { echo "cargo is required"; exit 1; }

# Clean all build artifacts
[confirm("This will delete all build artifacts. Continue?")]
clean-all:
    cargo clean
    rm -rf node_modules/ coverage/ dist/ .cache/

# Print project information
info:
    @echo "Project:   {{upper_name}}"
    @echo "Version:   {{version}} ({{is_semver}})"
    @echo "Profile:   {{profile}}"
    @echo "Commit:    {{git_hash}} ({{git_branch}})"
    @echo "Timestamp: {{timestamp}}"
    @echo "OS:        {{os()}} / {{os_family}} / {{arch_val}}"
    @echo "CPUs:      {{num_cpus}}"
    @echo "Build Dir: {{abs_build}}"
    @echo "Config:    {{config_file}}"
    @echo "Home:      {{home_dir}}"
    @echo "Build ID:  {{build_id}}"

# Watch for changes and rebuild
watch *args:
    cargo watch -x 'build {{args}}' -c -w src/

# Generate documentation
docs:
    cargo doc --no-deps --open

# ─── Multi-line Strings & Heredocs ───────────────────────────────────────────

# Generate a configuration file from a template
generate-config:
    #!/usr/bin/env python3
    import json
    import os

    config = {
        "name": "{{project_name}}",
        "version": "{{version}}",
        "debug": {{profile}} == "debug",
        "build": {
            "hash": "{{git_hash}}",
            "branch": "{{git_branch}}",
            "timestamp": "{{timestamp}}"
        }
    }

    os.makedirs("generated", exist_ok=True)
    with open("generated/config.json", "w") as f:
        json.dump(config, f, indent=2)
    print("Generated config.json")

# Generate a systemd service file
generate-service:
    #!/usr/bin/env bash
    cat > "generated/{{project_name}}.service" << 'UNIT'
    [Unit]
    Description={{upper_name}} Service
    After=network.target

    [Service]
    Type=simple
    ExecStart=/usr/local/bin/{{project_name}}
    Restart=on-failure
    RestartSec=5
    Environment=RUST_LOG=info

    [Install]
    WantedBy=multi-user.target
    UNIT
    echo "Generated {{project_name}}.service"

# ─── Conditional Recipes (OS-specific) ───────────────────────────────────────

# Install dependencies based on OS
install-deps:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{os()}}" in
        macos)
            brew install protobuf openssl cmake
            ;;
        linux)
            sudo apt-get update
            sudo apt-get install -y protobuf-compiler libssl-dev cmake
            ;;
        windows)
            choco install protoc openssl cmake
            ;;
        *)
            echo "Unsupported OS: {{os()}}"
            exit 1
            ;;
    esac

# ─── Recipes with Special Attributes ────────────────────────────────────────

# Recipe that runs on Linux only
[linux]
setup-systemd: generate-service
    sudo cp "generated/{{project_name}}.service" /etc/systemd/system/
    sudo systemctl daemon-reload
    sudo systemctl enable {{project_name}}

# Recipe that runs on macOS only
[macos]
setup-launchd:
    @echo "Setting up launchd for {{project_name}}..."

# Recipe that runs on Windows only
[windows]
setup-service:
    @echo "Setting up Windows service for {{project_name}}..."

# Recipe that doesn't appear in `just --list`
[private]
_internal-helper arg:
    @echo "Internal: {{arg}}"

# Recipe that doesn't print commands before running
[no-cd]
run-from-anywhere:
    pwd
    echo "Running from invocation directory: {{invocation_directory()}}"

# Recipe that continues even if a line fails
[no-exit-message]
optional-cleanup:
    -rm -f temp_file_1
    -rm -f temp_file_2
    -rm -rf .cache/

# ─── Variadic Parameters ────────────────────────────────────────────────────

# Run arbitrary cargo commands
cargo +args:
    cargo {{args}}

# Run multiple shell commands
exec +cmds:
    {{cmds}}

# Pass positional arguments to a script
script *args:
    #!/usr/bin/env bash
    echo "Script args: $@"
    for arg in "$@"; do
        echo "  - $arg"
    done

# ─── Dependency Recipes with Arguments ───────────────────────────────────────

# Full CI pipeline
ci: check-format (test "--release") lint
    @echo "CI passed!"

# Parameterized dependency chain
full-build target="native": (cross-compile target) test lint
    @echo "Full build for {{target}} complete."

# Multiple parameterized dependencies
deploy env="staging" tag=docker_tag: (docker-build tag) (docker-push "ghcr.io/myorg")
    @echo "Deployed {{tag}} to {{env}}"

# ─── Error Handling & Control Flow ───────────────────────────────────────────

# Recipe demonstrating error handling in shebang scripts
validate:
    #!/usr/bin/env bash
    set -euo pipefail

    errors=0

    if [[ ! -f "Cargo.toml" ]]; then
        echo "ERROR: Cargo.toml not found"
        errors=$((errors + 1))
    fi

    if ! cargo check 2>/dev/null; then
        echo "ERROR: cargo check failed"
        errors=$((errors + 1))
    fi

    if [[ $errors -gt 0 ]]; then
        echo "Validation failed with $errors error(s)"
        exit 1
    fi

    echo "All validations passed!"

# ─── Working with Files and Paths ────────────────────────────────────────────

# Source directory for operations
src_dir := justfile_directory() / "src"
test_dir := justfile_directory() / "tests"
justfile_loc := justfile()
source_hash := sha256_file(justfile())

# Count lines of code
loc:
    @echo "Source: {{src_dir}}"
    @find {{quoted}} -name '*.rs' | xargs wc -l | tail -1
    @echo "Justfile: {{justfile_loc}}"
    @echo "Justfile SHA256: {{source_hash}}"

# ─── Grouped Recipes ─────────────────────────────────────────────────────────

[group('build')]
build-debug:
    cargo build

[group('build')]
build-release:
    cargo build --release

[group('test')]
test-unit:
    cargo test --lib

[group('test')]
test-doc:
    cargo test --doc

[group('quality')]
audit:
    cargo audit

[group('quality')]
outdated:
    cargo outdated

# ─── XDG and Standard Directories ───────────────────────────────────────────

cache_dir := cache_directory()
data_dir := data_directory()
data_local_dir := data_local_directory()
executable_dir := executable_directory()

show-dirs:
    @echo "Cache:      {{cache_dir}}"
    @echo "Config:     {{config_file}}"
    @echo "Data:       {{data_dir}}"
    @echo "Data Local: {{data_local_dir}}"
    @echo "Executable: {{executable_dir}}"

# ─── Numeric Functions ───────────────────────────────────────────────────────

# Demonstrate just's error function
check-version:
    #!/usr/bin/env bash
    if [[ "{{is_semver}}" != "valid" ]]; then
        echo "Invalid version: {{version}}"
        exit 1
    fi
    echo "Version {{version}} is valid"

# ─── Backtick Expressions & Command Evaluation ──────────────────────────────

rust_version := `rustc --version | cut -d' ' -f2`
kernel := `uname -r`
shell_name := `basename "$SHELL"`

show-env:
    @echo "Rust:   {{rust_version}}"
    @echo "Kernel: {{kernel}}"
    @echo "Shell:  {{shell_name}}"
    @echo "User:   {{env("USER", "unknown")}}"
    @echo "Path:   {{env("PATH")}}"

# ─── Imports and Modules ────────────────────────────────────────────────────

# Import recipes from other files (just 1.14+)
import 'justfiles/docker.just'
import? 'justfiles/local.just'

# Module declaration (just 1.19+)
mod tools 'justfiles/tools.just'
mod? local

# ─── String Escaping & Raw Strings ──────────────────────────────────────────

escaped := "\t\n\r\\\""
raw_string := 'no escapes here: \t \n \r'
multiline := "line one
line two
line three"

show-strings:
    @printf "Escaped: {{escaped}}"
    @echo "Raw: {{raw_string}}"
    @echo '{{multiline}}'

# ─── Assert & Error ─────────────────────────────────────────────────────────

check-ci:
    {{ if ci_mode == "true" { "" } else { error("This recipe must run in CI") } }}
    @echo "Running in CI mode"

# ─── Complex Real-World Example ─────────────────────────────────────────────

# Full release workflow combining multiple features
do-release kind="patch": validate check-format (test "--release") lint
    #!/usr/bin/env bash
    set -euo pipefail

    echo "=== Release Pipeline ==="
    echo "Project: {{upper_name}}"
    echo "Current: v{{version}}"
    echo "Kind:    {{kind}}"
    echo "Date:    {{timestamp}}"
    echo "Commit:  {{git_hash}}"
    echo "OS:      {{os()}} ({{arch_val}})"
    echo "Rust:    {{rust_version}}"
    echo ""

    # Build for multiple targets
    for target in x86_64-unknown-linux-musl aarch64-unknown-linux-musl; do
        echo "Building for $target..."
        cross build --release --target "$target"
        tar czf "dist/{{project_name}}-{{version}}-${target}.tar.gz" \
            -C "target/${target}/release" "{{project_name}}"
    done

    echo ""
    echo "Release artifacts:"
    ls -lh dist/

    echo ""
    echo "=== Release Pipeline Complete ==="
