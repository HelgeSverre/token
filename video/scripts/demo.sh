#!/usr/bin/env bash
# ============================================================================
# Phase A: Demo Orchestration
# Runs a deterministic keystroke sequence in Token Editor, logging beat times.
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VIDEO_DIR="$(dirname "$SCRIPT_DIR")"
DEMO_DIR="$VIDEO_DIR/demo"
BEATS_FILE="$VIDEO_DIR/recordings/beats.jsonl"
TOKEN_BIN="${TOKEN_BIN:-token}"

# --- Helpers ----------------------------------------------------------------

log() { echo "[demo $$] $*" >&2; }

beat() {
    local label="$1"
    local now
    now=$(python3 -c "import time; print(f'{time.time() - $START_TIME:.3f}')")
    echo "{\"t\": $now, \"label\": \"$label\"}" >> "$BEATS_FILE"
    log "beat: $label @ ${now}s"
}

keystroke() {
    osascript -e "tell application \"System Events\" to keystroke \"$1\""
}

keystroke_mod() {
    local key="$1"
    shift
    local mods=""
    for m in "$@"; do
        mods="$mods $m down,"
    done
    mods="${mods%,}"  # remove trailing comma
    osascript -e "tell application \"System Events\" to keystroke \"$key\" using {$mods}"
}

key_code() {
    osascript -e "tell application \"System Events\" to key code $1"
}

key_code_mod() {
    local code="$1"
    shift
    local mods=""
    for m in "$@"; do
        mods="$mods $m down,"
    done
    mods="${mods%,}"
    osascript -e "tell application \"System Events\" to key code $code using {$mods}"
}

# Clear any text in a modal input (Select All + Backspace)
clear_modal_input() {
    keystroke_mod "a" "command"
    sleep 0.1
    key_code 51  # Backspace
    sleep 0.1
}

# Navigate to a specific line using Go to Line modal (Cmd+L)
goto_line() {
    local line_num="$1"
    keystroke_mod "l" "command"
    sleep 0.5
    clear_modal_input
    keystroke "$line_num"
    key_code 36  # Enter
    sleep 0.5
}

# --- Pre-checks -------------------------------------------------------------

if ! command -v "$TOKEN_BIN" &>/dev/null; then
    log "ERROR: Token binary not found. Set TOKEN_BIN or add 'token' to PATH."
    exit 1
fi

# --- Window Setup -----------------------------------------------------------

log "Setting up demo workspace..."

# Clear beats file
mkdir -p "$(dirname "$BEATS_FILE")"
> "$BEATS_FILE"

# Launch Token with demo workspace
log "Launching Token with demo files..."
"$TOKEN_BIN" "$DEMO_DIR" &
TOKEN_PID=$!
sleep 2

# Bring Token to front
log "Configuring window..."
osascript -e "tell application \"System Events\" to set frontmost of (first process whose unix id is $TOKEN_PID) to true" 2>/dev/null || \
osascript -e 'tell application "System Events" to set frontmost of (first process whose name is "token") to true' 2>/dev/null || true
sleep 0.5

# Set window size based on actual screen bounds, centered
osascript <<'APPLESCRIPT'
tell application "Finder"
    set b to bounds of window of desktop
end tell
set screenW to item 3 of b
set screenH to item 4 of b

-- Target: 1600x900, clamped to fit screen with margins
set targetW to 1600
set targetH to 900
set maxW to screenW - 80
set maxH to screenH - 120

if targetW > maxW then set targetW to maxW
if targetH > maxH then set targetH to maxH

set posX to (screenW - targetW) / 2
set posY to (screenH - targetH) / 2

tell application "System Events"
    tell (first process whose name is "token")
        try
            set theWindow to window 1
            set size of theWindow to {targetW, targetH}
            set position of theWindow to {posX, posY}
        end try
    end tell
end tell
APPLESCRIPT
sleep 0.8

# --- Record start time -------------------------------------------------------

START_TIME=$(python3 -c "import time; print(time.time())")
log "Demo start time: $START_TIME"

# Signal to recorder that demo is starting
echo -n "DEMO_START" > "$VIDEO_DIR/recordings/.demo_signal"

# === DEMO SEQUENCE ============================================================
# Total target: ~22 seconds of raw footage
# Uses Go to Line (Cmd+L) for all navigation — avoids hardware-dependent keys.

# --- Beat 1: File opens with syntax highlighting (0s) -------------------------
beat "open"
sleep 1.5

# Open main.ts via fuzzy file finder (opens fresh, no retained state)
keystroke_mod "o" "command" "shift"
sleep 1.0
keystroke "main.ts"
sleep 0.5
key_code 36  # Enter
sleep 1.0

beat "code_visible"
sleep 1.0

# --- Beat 2: Navigate and show code ("focus") --------------------------------
# Jump to an interesting area (route handlers ~line 80)
goto_line "80"
sleep 1.0

beat "navigate"

# Jump back to top (line 1)
goto_line "1"
sleep 0.5

# --- Beat 3: Multi-cursor edit ("multi-cursor") --------------------------------
# Jump to area with "logger" occurrences
goto_line "41"
sleep 0.5

# Open Find, clear any old state, search for "logger" to position cursor
keystroke_mod "f" "command"
sleep 0.5
clear_modal_input
keystroke "logger"
sleep 0.3
key_code 36  # Enter — find first occurrence
sleep 0.3
key_code 53  # Escape — close find (cursor now on "logger")
sleep 0.3

# Now Cmd+J to add multi-cursors (first press selects word under cursor)
keystroke_mod "j" "command"
sleep 0.4
beat "multicursor_start"

keystroke_mod "j" "command"  # 2nd occurrence
sleep 0.4
keystroke_mod "j" "command"  # 3rd
sleep 0.4
keystroke_mod "j" "command"  # 4th
sleep 0.4

beat "multicursor_active"
sleep 0.8

# Type replacement to show multi-cursor in action
keystroke "log"
sleep 0.8

beat "multicursor_typed"

# Undo to restore original text, then clear multi-cursors
keystroke_mod "z" "command"
sleep 0.3
key_code 53  # Escape — collapse to single cursor
sleep 0.5

# --- Beat 4: Split View ("split") --------------------------------------------
keystroke_mod "v" "command" "shift" "option"  # Split Vertical
sleep 1.0
beat "split_view"
sleep 1.0

# --- Beat 5: Open CSV in second pane ("csv") ----------------------------------
keystroke_mod "o" "command" "shift"  # Fuzzy file finder (fresh state)
sleep 1.0
keystroke "data.csv"
sleep 0.5
key_code 36  # Enter
sleep 1.5

beat "csv_view"
sleep 1.5

# --- Beat 6: Find/Replace ("search") ------------------------------------------
# Switch to first pane
key_code_mod 48 "control"  # Ctrl+Tab — focus next group
sleep 0.5

# Open Find, clear old state, search for "config"
keystroke_mod "f" "command"
sleep 0.5
clear_modal_input
keystroke "config"
sleep 0.5
key_code 36  # Enter — find next
sleep 0.3
key_code 36  # Enter — find next
sleep 0.5

beat "search"
sleep 0.5
key_code 53  # Escape — close find
sleep 0.3

# --- Beat 7: Code Outline Panel ("outline") -----------------------------------
keystroke_mod "7" "command"  # Toggle Outline
sleep 1.0
beat "outline"
sleep 1.5

# --- Beat 8: File Explorer ("workspace") --------------------------------------
keystroke_mod "1" "command"  # Toggle File Explorer
sleep 0.5
beat "workspace"
sleep 1.0

# --- Beat 9: Command Palette ("command_palette") ------------------------------
keystroke_mod "a" "command" "shift"  # Command Palette
sleep 0.8
clear_modal_input  # Clear any retained input so it opens clean
sleep 0.3
beat "command_palette"
sleep 1.0
key_code 53  # Escape
sleep 0.5

beat "end"

# === END DEMO =================================================================

log "Demo sequence complete"
echo -n "DEMO_END" > "$VIDEO_DIR/recordings/.demo_signal"

# Give a moment for the last frame
sleep 0.5

# Close Token
kill $TOKEN_PID 2>/dev/null || true
wait $TOKEN_PID 2>/dev/null || true

log "Token closed. Beats logged to $BEATS_FILE"
