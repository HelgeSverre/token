#!/usr/bin/env bash
# ============================================================================
# Token Editor — Launch Video Generator
# ============================================================================
#
# Fully automated ~30s launch video pipeline:
#   Phase A: Demo orchestration (scripted keystrokes in Token)
#   Phase B: Screen recording (ffmpeg AVFoundation)
#   Phase C: Auto-edit + render (EDL generation + ffmpeg concat)
#
# Usage:
#   ./video/run.sh              # Full pipeline
#   ./video/run.sh --render     # Re-render from existing recording + beats
#   ./video/run.sh --help       # Show help
#
# Outputs:
#   video/dist/token_launch_30s_16x9.mp4   (required)
#   video/dist/token_launch_30s_9x16.mp4   (optional vertical)
#   video/dist/thumbnail.png               (frame grab)
#   video/dist/edl.json                    (edit decision list)
#
# ============================================================================
set -euo pipefail

VIDEO_DIR="$(cd "$(dirname "$0")" && pwd)"
SCRIPTS_DIR="$VIDEO_DIR/scripts"

# --- Help ---
if [[ "${1:-}" == "--help" || "${1:-}" == "-h" ]]; then
    echo "Token Editor Launch Video Generator"
    echo ""
    echo "Usage:"
    echo "  ./video/run.sh              Full pipeline (demo + record + render)"
    echo "  ./video/run.sh --render     Re-render from existing recording"
    echo "  ./video/run.sh --help       Show this help"
    echo ""
    echo "Environment variables:"
    echo "  TOKEN_BIN      Path to Token binary (default: 'token' from PATH)"
    echo "  SCREEN_INDEX   AVFoundation screen device index (default: 1)"
    echo "  FPS            Recording framerate (default: 30)"
    echo "  FONT_FILE      Path to font file for text cards"
    echo ""
    echo "Prerequisites:"
    echo "  - macOS with Accessibility + Screen Recording permissions"
    echo "  - ffmpeg (brew install ffmpeg)"
    echo "  - Token Editor installed (token binary in PATH)"
    echo ""
    echo "Outputs → video/dist/"
    exit 0
fi

# Export VIDEO_DIR for child scripts
export VIDEO_DIR
export SCRIPT_DIR="$SCRIPTS_DIR"

# --- Banner ---
echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   Token Editor — Launch Video Generator          ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""

# --- Pre-flight checks ---
echo "[pre-flight] Checking dependencies..."

check_cmd() {
    if ! command -v "$1" &>/dev/null; then
        echo "  ✗ $1 not found. $2"
        exit 1
    else
        echo "  ✓ $1"
    fi
}

check_cmd ffmpeg "Install with: brew install ffmpeg"
check_cmd python3 "Install Python 3"
check_cmd osascript "macOS only (uses AppleScript for automation)"

TOKEN_BIN="${TOKEN_BIN:-token}"
if ! command -v "$TOKEN_BIN" &>/dev/null; then
    # Try local build
    if [ -f "$VIDEO_DIR/../target/release/token" ]; then
        TOKEN_BIN="$VIDEO_DIR/../target/release/token"
        echo "  ✓ token (local build: $TOKEN_BIN)"
    else
        echo "  ✗ Token binary not found. Build with 'make release' or set TOKEN_BIN"
        exit 1
    fi
else
    echo "  ✓ token ($TOKEN_BIN)"
fi
export TOKEN_BIN

echo ""

# --- Permissions check ---
echo "[pre-flight] Checking macOS permissions..."
echo "  ℹ  Ensure Terminal/iTerm has Accessibility permission"
echo "     (System Settings → Privacy & Security → Accessibility)"
echo "  ℹ  Ensure Terminal/iTerm has Screen Recording permission"
echo "     (System Settings → Privacy & Security → Screen Recording)"
echo ""

# --- Ensure scripts are executable ---
chmod +x "$SCRIPTS_DIR/demo.sh"
chmod +x "$SCRIPTS_DIR/record.sh"
chmod +x "$SCRIPTS_DIR/render.sh"

# --- Create output directories ---
mkdir -p "$VIDEO_DIR/recordings"
mkdir -p "$VIDEO_DIR/dist"

# --- Check if --render only ---
if [[ "${1:-}" == "--render" ]]; then
    echo "[mode] Re-render only (skipping demo + recording)"
    echo ""

    if [ ! -f "$VIDEO_DIR/recordings/raw.mp4" ]; then
        echo "ERROR: No raw recording found at recordings/raw.mp4"
        echo "Run without --render to capture a new demo first."
        exit 1
    fi

    if [ ! -f "$VIDEO_DIR/recordings/beats.jsonl" ]; then
        echo "ERROR: No beats file found at recordings/beats.jsonl"
        exit 1
    fi

    echo "═══ Phase C: Generate EDL ═══"
    python3 "$SCRIPTS_DIR/generate_edl.py"
    echo ""

    echo "═══ Phase C: Render ═══"
    bash "$SCRIPTS_DIR/render.sh"
    echo ""

    echo "╔══════════════════════════════════════════════════╗"
    echo "║   ✓ Render complete!                             ║"
    echo "╚══════════════════════════════════════════════════╝"
    echo ""
    echo "Outputs:"
    ls -lh "$VIDEO_DIR/dist/" 2>/dev/null
    exit 0
fi

# --- Full Pipeline ---

echo "═══ Phase A+B: Demo + Recording ═══"
echo ""
echo "Starting screen recorder in background..."

# Start recorder in background
bash "$SCRIPTS_DIR/record.sh" &
RECORD_PID=$!

# Small delay to let recorder initialize
sleep 1

echo "Running demo sequence..."
bash "$SCRIPTS_DIR/demo.sh"

# Wait for recorder to finish (with timeout)
echo "Waiting for recorder to finish..."
WAIT_COUNT=0
while kill -0 $RECORD_PID 2>/dev/null; do
    sleep 1
    WAIT_COUNT=$((WAIT_COUNT + 1))
    if [ $WAIT_COUNT -ge 30 ]; then
        echo "  Recorder timeout — force stopping"
        kill $RECORD_PID 2>/dev/null || true
        # Also kill any lingering ffmpeg
        FFMPEG_PID_FILE="$VIDEO_DIR/recordings/.ffmpeg_pid"
        if [ -f "$FFMPEG_PID_FILE" ]; then
            kill "$(cat "$FFMPEG_PID_FILE")" 2>/dev/null || true
            rm -f "$FFMPEG_PID_FILE"
        fi
        sleep 1
        break
    fi
done
wait $RECORD_PID 2>/dev/null || true

echo ""
echo "═══ Phase C: Generate EDL ═══"
python3 "$SCRIPTS_DIR/generate_edl.py"

echo ""
echo "═══ Phase C: Render ═══"
bash "$SCRIPTS_DIR/render.sh"

echo ""
echo "╔══════════════════════════════════════════════════╗"
echo "║   ✓ Video generation complete!                   ║"
echo "╚══════════════════════════════════════════════════╝"
echo ""
echo "Outputs:"
ls -lh "$VIDEO_DIR/dist/" 2>/dev/null
echo ""
echo "Watch: open video/dist/token_launch_30s_16x9.mp4"
