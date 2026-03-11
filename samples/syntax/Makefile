# Makefile Syntax Highlighting Test
# A comprehensive build system for a multi-language project.

# ============================================================
# Configuration
# ============================================================

SHELL := /bin/bash
.DEFAULT_GOAL := help
.ONESHELL:
.SHELLFLAGS := -eu -o pipefail -c

# Project metadata
PROJECT_NAME := token-editor
VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)"/\1/')
GIT_SHA := $(shell git rev-parse --short HEAD 2>/dev/null || echo "unknown")
BUILD_TIME := $(shell date -u '+%Y-%m-%dT%H:%M:%SZ')

# Directories
SRC_DIR := src
BUILD_DIR := target
DIST_DIR := dist
DOCS_DIR := docs
TEST_DIR := tests
BENCH_DIR := benches

# Tools
CARGO := cargo
RUSTFMT := rustfmt
CLIPPY := cargo clippy
NEXTEST := cargo nextest
SAMPLY := samply

# Build configuration
RELEASE_FLAGS := --release
DEBUG_FLAGS :=
CARGO_FLAGS :=
FEATURES :=

# Platform detection
UNAME_S := $(shell uname -s)
UNAME_M := $(shell uname -m)

ifeq ($(UNAME_S),Darwin)
	PLATFORM := macos
	OPEN_CMD := open
	NUM_CPUS := $(shell sysctl -n hw.ncpu)
	SED_INPLACE := sed -i ''
else ifeq ($(UNAME_S),Linux)
	PLATFORM := linux
	OPEN_CMD := xdg-open
	NUM_CPUS := $(shell nproc)
	SED_INPLACE := sed -i
else
	PLATFORM := windows
	OPEN_CMD := start
	NUM_CPUS := $(NUMBER_OF_PROCESSORS)
	SED_INPLACE := sed -i
endif

# Architecture-specific targets
ifeq ($(UNAME_M),arm64)
	RUST_TARGET := aarch64-apple-darwin
else ifeq ($(UNAME_M),x86_64)
	RUST_TARGET := x86_64-unknown-linux-gnu
endif

# Colors for output
RED := \033[0;31m
GREEN := \033[0;32m
YELLOW := \033[0;33m
BLUE := \033[0;34m
CYAN := \033[0;36m
NC := \033[0m

# Verbose mode
ifdef VERBOSE
	Q :=
	CARGO_FLAGS += -v
else
	Q := @
endif

# ============================================================
# Phony targets
# ============================================================

.PHONY: all build release debug run dev test test-fast lint fmt \
        clean distclean bench profile coverage doc help \
        install check ci setup watch

# ============================================================
# Build targets
# ============================================================

## Build debug binary
build: $(BUILD_DIR)/debug/$(PROJECT_NAME)

$(BUILD_DIR)/debug/$(PROJECT_NAME): $(shell find $(SRC_DIR) -name '*.rs') Cargo.toml
	@printf "$(CYAN)Building debug...$(NC)\n"
	$(Q)$(CARGO) build $(DEBUG_FLAGS) $(CARGO_FLAGS)

## Build optimized release binary
release: export RUSTFLAGS=-C target-cpu=native
release:
	@printf "$(CYAN)Building release ($(VERSION))...$(NC)\n"
	$(Q)$(CARGO) build $(RELEASE_FLAGS) $(CARGO_FLAGS)
	@printf "$(GREEN)Built: $(BUILD_DIR)/release/$(PROJECT_NAME)$(NC)\n"
	@ls -lh $(BUILD_DIR)/release/$(PROJECT_NAME) | awk '{print "  Size:", $$5}'

## Build with all features
all:
	$(Q)$(CARGO) build --all-features $(RELEASE_FLAGS)

# ============================================================
# Run targets
# ============================================================

## Run release build with sample file
run: release
	$(Q)$(BUILD_DIR)/release/$(PROJECT_NAME) samples/syntax/sample.rs

## Run debug build (faster compile)
dev: build
	$(Q)$(BUILD_DIR)/debug/$(PROJECT_NAME) samples/syntax/sample.rs

## Start bacon watch mode
watch:
	$(Q)bacon

## Watch with clippy
watch-lint:
	$(Q)bacon clippy

# ============================================================
# Test targets
# ============================================================

## Run all tests
test:
	@printf "$(CYAN)Running tests...$(NC)\n"
	$(Q)$(CARGO) test $(CARGO_FLAGS) 2>&1 | tail -n 20
	@printf "$(GREEN)All tests passed!$(NC)\n"

## Run specific test
test-one:
ifndef TEST
	$(error TEST is not set. Usage: make test-one TEST=test_name)
endif
	$(Q)$(CARGO) test $(TEST) -- --nocapture

## Run tests with output
test-verbose:
	$(Q)$(CARGO) test -- --nocapture

## Fast parallel tests (nextest)
test-fast:
	$(Q)$(NEXTEST) run --jobs $(NUM_CPUS)

## Tests with retries for flaky tests
test-retry:
	$(Q)$(NEXTEST) run --retries 2

# ============================================================
# Quality targets
# ============================================================

## Run clippy lints (mirrors CI)
lint:
	@printf "$(CYAN)Running clippy...$(NC)\n"
	$(Q)$(CLIPPY) --all-targets -- -D warnings
	@printf "$(GREEN)No lint issues!$(NC)\n"

## Format Rust code and markdown
fmt:
	@printf "$(CYAN)Formatting...$(NC)\n"
	$(Q)$(CARGO) fmt
	@printf "$(GREEN)Formatted!$(NC)\n"

## Check formatting without modifying
fmt-check:
	$(Q)$(CARGO) fmt -- --check

## Run all checks (format + lint + test)
check: fmt-check lint test
	@printf "$(GREEN)All checks passed!$(NC)\n"

# ============================================================
# Benchmark and profiling
# ============================================================

## Run all benchmarks
bench:
	@printf "$(CYAN)Running benchmarks...$(NC)\n"
	$(Q)$(CARGO) bench

## Rope operation benchmarks
bench-rope:
	$(Q)$(CARGO) bench --bench rope_bench

## Rendering benchmarks
bench-render:
	$(Q)$(CARGO) bench --bench render_bench

## Glyph cache benchmarks
bench-glyph:
	$(Q)$(CARGO) bench --bench glyph_bench

## Generate CPU flamegraph
flamegraph: release
	@printf "$(CYAN)Generating flamegraph...$(NC)\n"
	$(Q)$(CARGO) flamegraph --bin $(PROJECT_NAME) -- samples/syntax/sample.rs
	$(Q)$(OPEN_CMD) flamegraph.svg

## Interactive profiling
profile-samply: release
	$(Q)$(SAMPLY) record $(BUILD_DIR)/release/$(PROJECT_NAME) samples/syntax/sample.rs

## Heap profiling with dhat
profile-memory:
	$(Q)$(CARGO) run --features dhat-heap $(RELEASE_FLAGS) -- samples/syntax/sample.rs

## Generate HTML coverage report
coverage:
	@printf "$(CYAN)Generating coverage report...$(NC)\n"
	$(Q)$(CARGO) llvm-cov --html --output-dir $(BUILD_DIR)/coverage
	@printf "$(GREEN)Report: $(BUILD_DIR)/coverage/index.html$(NC)\n"
	$(Q)$(OPEN_CMD) $(BUILD_DIR)/coverage/index.html

# ============================================================
# Documentation
# ============================================================

## Build documentation
doc:
	$(Q)$(CARGO) doc --no-deps --open

# ============================================================
# Distribution
# ============================================================

## Create distribution package
dist: release
	@printf "$(CYAN)Creating distribution...$(NC)\n"
	$(Q)mkdir -p $(DIST_DIR)
	$(Q)cp $(BUILD_DIR)/release/$(PROJECT_NAME) $(DIST_DIR)/
	$(Q)cp -r assets themes $(DIST_DIR)/
	$(Q)tar czf $(DIST_DIR)/$(PROJECT_NAME)-$(VERSION)-$(PLATFORM)-$(UNAME_M).tar.gz \
		-C $(DIST_DIR) $(PROJECT_NAME) assets themes
	@printf "$(GREEN)Package: $(DIST_DIR)/$(PROJECT_NAME)-$(VERSION)-$(PLATFORM)-$(UNAME_M).tar.gz$(NC)\n"
	@ls -lh $(DIST_DIR)/*.tar.gz | awk '{print "  Size:", $$5}'

## Build for all platforms
compile-all:
	$(Q)$(CARGO) build $(RELEASE_FLAGS) --target x86_64-unknown-linux-gnu
	$(Q)$(CARGO) build $(RELEASE_FLAGS) --target aarch64-apple-darwin
	$(Q)$(CARGO) build $(RELEASE_FLAGS) --target x86_64-pc-windows-msvc

# ============================================================
# CI
# ============================================================

## Test GitHub Actions locally with act
ci:
	$(Q)act -j test

# ============================================================
# Maintenance
# ============================================================

## Install all dev tools
setup:
	@printf "$(CYAN)Installing development tools...$(NC)\n"
	$(Q)cargo install cargo-nextest cargo-watch cargo-flamegraph \
		cargo-llvm-cov bacon samply
	@printf "$(GREEN)All tools installed!$(NC)\n"

## Clean build artifacts
clean:
	@printf "$(YELLOW)Cleaning build artifacts...$(NC)\n"
	$(Q)$(CARGO) clean
	$(Q)rm -rf $(DIST_DIR)

## Deep clean including caches
distclean: clean
	$(Q)rm -rf .cargo-cache
	$(Q)rm -f flamegraph.svg perf.data

# ============================================================
# Help
# ============================================================

## Show this help message
help:
	@printf "$(CYAN)$(PROJECT_NAME) v$(VERSION)$(NC) ($(GIT_SHA))\n"
	@printf "Platform: $(PLATFORM)/$(UNAME_M), CPUs: $(NUM_CPUS)\n\n"
	@printf "$(YELLOW)Available targets:$(NC)\n\n"
	@grep -E '^## ' $(MAKEFILE_LIST) | \
		sed 's/^## //' | \
		awk 'BEGIN {FS = ":"}; {printf "  $(GREEN)%-20s$(NC) %s\n", prev, $$0; prev=""} /^[a-zA-Z]/ {prev=$$0}' | \
		grep -v '^  \s*$$'
