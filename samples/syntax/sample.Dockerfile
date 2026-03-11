# Dockerfile Syntax Highlighting Test
# Multi-stage build for a Rust application with optimized layers.

# syntax=docker/dockerfile:1

# ============================================================
# Stage 1: Chef - prepare recipe for caching
# ============================================================
FROM rust:1.77-bookworm AS chef

RUN cargo install cargo-chef --locked
WORKDIR /app

# ============================================================
# Stage 2: Planner - create dependency recipe
# ============================================================
FROM chef AS planner

COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY tests/ tests/
COPY benches/ benches/

RUN cargo chef prepare --recipe-path recipe.json

# ============================================================
# Stage 3: Builder - compile with cached dependencies
# ============================================================
FROM chef AS builder

# Install system dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
        cmake \
        pkg-config \
        libfontconfig1-dev \
        libfreetype-dev \
        libxkbcommon-dev \
        libwayland-dev \
    && rm -rf /var/lib/apt/lists/*

# Cache dependencies
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build application
COPY . .

ARG BUILD_VERSION=dev
ARG BUILD_SHA=unknown
ENV BUILD_VERSION=${BUILD_VERSION}
ENV BUILD_SHA=${BUILD_SHA}

RUN --mount=type=cache,target=/app/target \
    --mount=type=cache,target=/usr/local/cargo/registry \
    cargo build --release \
    && cp target/release/token /usr/local/bin/token

# Run tests
RUN --mount=type=cache,target=/app/target \
    cargo test --release 2>&1 | tee /tmp/test-results.txt

# ============================================================
# Stage 4: Runtime - minimal production image
# ============================================================
FROM debian:bookworm-slim AS runtime

# Labels
LABEL maintainer="helge@example.com" \
      org.opencontainers.image.title="Token Editor" \
      org.opencontainers.image.description="A minimal text editor" \
      org.opencontainers.image.version="${BUILD_VERSION}" \
      org.opencontainers.image.source="https://github.com/example/token-editor"

# Install runtime dependencies only
RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        libfontconfig1 \
        libfreetype6 \
        libxkbcommon0 \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd -r token \
    && useradd -r -g token -d /home/token -s /sbin/nologin token \
    && mkdir -p /home/token/.config/token-editor \
    && chown -R token:token /home/token

# Copy binary from builder
COPY --from=builder --chown=token:token /usr/local/bin/token /usr/local/bin/token

# Copy default assets
COPY --chown=token:token assets/ /usr/share/token-editor/assets/
COPY --chown=token:token themes/ /usr/share/token-editor/themes/

# Environment
ENV RUST_LOG=token=info \
    TERM=xterm-256color \
    TOKEN_THEME_DIR=/usr/share/token-editor/themes \
    TOKEN_FONT_DIR=/usr/share/token-editor/assets

# Health check
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["token", "--version"] || exit 1

# Switch to non-root user
USER token
WORKDIR /workspace

# Volumes for persistent data
VOLUME ["/workspace", "/home/token/.config/token-editor"]

# Use tini as init
ENTRYPOINT ["tini", "--"]
CMD ["token"]

# ============================================================
# Stage 5: Development image (optional)
# ============================================================
FROM builder AS dev

RUN cargo install cargo-watch cargo-nextest

# Install additional dev tools
RUN apt-get update && apt-get install -y --no-install-recommends \
        gdb \
        lldb \
        valgrind \
        heaptrack \
        git \
        curl \
        ripgrep \
        fd-find \
    && rm -rf /var/lib/apt/lists/*

ENV RUST_BACKTRACE=1 \
    RUST_LOG=token=debug

WORKDIR /app
EXPOSE 9999

CMD ["cargo", "watch", "-x", "run"]
