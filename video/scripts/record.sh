#!/usr/bin/env bash
# ============================================================================
# Phase B: Screen Recording
# Captures the screen using ffmpeg + AVFoundation on macOS.
#
# Strategy: Record for a fixed duration (-t) that exceeds the demo length.
# ffmpeg -f avfoundation on macOS is notoriously hard to stop via signals,
# but -t produces a clean, finalized file every time.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VIDEO_DIR="$(dirname "$SCRIPT_DIR")"
RAW_VIDEO="$VIDEO_DIR/recordings/raw.mp4"
SIGNAL_FILE="$VIDEO_DIR/recordings/.demo_signal"

FPS="${FPS:-30}"
# Max recording duration in seconds — must exceed demo length
# Demo runs ~38s, add margin for startup/shutdown
MAX_DURATION="${MAX_DURATION:-50}"

log() { echo "[record $$] $*" >&2; }

# --- Find the correct AVFoundation screen device ---

log "Available capture devices:"
ffmpeg -f avfoundation -list_devices true -i "" 2>&1 | grep -E "^\[AVFoundation" || true

SCREEN_INDEX="${SCREEN_INDEX:-1}"

log "Using screen device index: $SCREEN_INDEX"
log "Recording at ${FPS}fps, max ${MAX_DURATION}s"

# Clean up
rm -f "$RAW_VIDEO"
rm -f "$SIGNAL_FILE"

# --- Helper to read signal file ---
read_signal() {
    if [ -f "$SIGNAL_FILE" ]; then
        tr -d '\r\n ' < "$SIGNAL_FILE" 2>/dev/null || true
    fi
}

# --- Wait for demo to signal start ---
log "Waiting for demo to start..."
while [ "$(read_signal)" != "DEMO_START" ]; do
    sleep 0.1
done

log "Demo started — beginning recording (max ${MAX_DURATION}s)"

# --- Record with fixed duration ---
# -nostdin: don't read from terminal
# -t: stop after MAX_DURATION seconds (produces a clean, finalized file)
# The demo will finish within this window; excess footage is trimmed in Phase C.
ffmpeg -y -nostdin \
    -f avfoundation \
    -capture_cursor 1 \
    -framerate "$FPS" \
    -pixel_format uyvy422 \
    -i "${SCREEN_INDEX}:none" \
    -c:v libx264 \
    -preset ultrafast \
    -crf 18 \
    -pix_fmt yuv420p \
    -r "$FPS" \
    -t "$MAX_DURATION" \
    "$RAW_VIDEO" </dev/null 2>&1 &
FFMPEG_PID=$!

log "ffmpeg recording started (PID: $FFMPEG_PID)"

# --- Wait for demo to end, then let ffmpeg run a bit longer for trailing frames ---
while [ "$(read_signal)" != "DEMO_END" ]; do
    sleep 0.2
done

log "Demo ended — capturing trailing frames..."
sleep 1

# Now kill ffmpeg (we have all the footage we need; no point waiting for MAX_DURATION)
log "Stopping ffmpeg..."
kill -TERM $FFMPEG_PID 2>/dev/null || true
sleep 2
# Force kill if still running
if kill -0 $FFMPEG_PID 2>/dev/null; then
    kill -9 $FFMPEG_PID 2>/dev/null || true
fi
wait $FFMPEG_PID 2>/dev/null || true

# --- Verify output ---
# If SIGTERM truncated the file, ffmpeg with -t should have already written
# enough valid frames. Check if the file is usable.
if [ -f "$RAW_VIDEO" ]; then
    DURATION=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$RAW_VIDEO" 2>/dev/null || echo "0")
    SIZE=$(du -h "$RAW_VIDEO" | cut -f1)
    log "Recording saved: $RAW_VIDEO (duration: ${DURATION}s, size: ${SIZE})"

    # If duration is too short (< 5s), the file is likely corrupt
    DURATION_INT=$(printf "%.0f" "$DURATION" 2>/dev/null || echo "0")
    if [ "$DURATION_INT" -lt 5 ]; then
        log "WARNING: Recording seems too short (${DURATION}s). File may be corrupt."
        log "Tip: Ensure Screen Recording permission is granted to your terminal."
    fi
else
    log "ERROR: Recording failed — $RAW_VIDEO not found"
    exit 1
fi

rm -f "$SIGNAL_FILE"
