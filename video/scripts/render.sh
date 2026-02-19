#!/usr/bin/env bash
# ============================================================================
# Phase C (part 2): Render final video from EDL
#
# Reads dist/edl.json + recordings/raw.mp4 → produces:
#   - dist/token_launch_30s_16x9.mp4
#   - dist/token_launch_30s_9x16.mp4
#   - dist/thumbnail.png
#
# Uses ffmpeg filtergraph with:
#   - Text card generation (drawtext)
#   - Clip cutting from raw footage
#   - Fade transitions
#   - Subtle scale motion (ken burns lite)
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
VIDEO_DIR="$(dirname "$SCRIPT_DIR")"
EDL_FILE="$VIDEO_DIR/dist/edl.json"
RAW_VIDEO="$VIDEO_DIR/recordings/raw.mp4"
OUTPUT_16x9="$VIDEO_DIR/dist/token_launch_30s_16x9.mp4"
OUTPUT_9x16="$VIDEO_DIR/dist/token_launch_30s_9x16.mp4"
THUMBNAIL="$VIDEO_DIR/dist/thumbnail.png"
FONT_FILE="${FONT_FILE:-$VIDEO_DIR/../assets/JetBrainsMono.ttf}"

FPS=30
WIDTH=1920
HEIGHT=1080

# Colors (dark theme)
BG_COLOR="0x1a1a2e"  # Dark navy background
TEXT_COLOR="0xe0e0e0"  # Light grey text
ACCENT_COLOR="0x64b4ff"  # Token blue accent
SUBTEXT_COLOR="0x888888"  # Dim grey subtext

log() { echo "[render] $*" >&2; }

# --- Pre-checks ---
if [ ! -f "$EDL_FILE" ]; then
    log "ERROR: EDL not found: $EDL_FILE"
    exit 1
fi

if [ ! -f "$RAW_VIDEO" ]; then
    log "ERROR: Raw video not found: $RAW_VIDEO"
    exit 1
fi

if [ ! -f "$FONT_FILE" ]; then
    log "WARNING: Font not found at $FONT_FILE, using default"
    FONT_FILE=""
fi

mkdir -p "$VIDEO_DIR/dist"

# --- Parse EDL with Python and generate ffmpeg concat segments ---

log "Parsing EDL and generating segments..."

# Export for the Python subprocess
export VIDEO_DIR EDL_FILE RAW_VIDEO FONT_FILE

python3 << 'PYEOF'
import json
import subprocess
import os
import sys
from PIL import Image, ImageDraw, ImageFont

VIDEO_DIR = os.environ["VIDEO_DIR"]
EDL_FILE = os.environ["EDL_FILE"]
RAW_VIDEO = os.environ["RAW_VIDEO"]
FONT_FILE = os.environ.get("FONT_FILE", "")
WIDTH = 1920
HEIGHT = 1080
FPS = 30

# Colors
BG_COLOR = (0x1a, 0x1a, 0x2e)
TEXT_COLOR = (0xe0, 0xe0, 0xe0)
SUBTEXT_COLOR = (0x88, 0x88, 0x88)

# Load font
def load_font(size):
    if FONT_FILE and os.path.exists(FONT_FILE):
        return ImageFont.truetype(FONT_FILE, size)
    # Fallback: try common macOS fonts
    for fallback in ["/System/Library/Fonts/SFMono-Regular.otf",
                     "/System/Library/Fonts/Menlo.ttc",
                     "/System/Library/Fonts/Helvetica.ttc"]:
        if os.path.exists(fallback):
            try:
                return ImageFont.truetype(fallback, size)
            except Exception:
                continue
    return ImageFont.load_default()

def make_card_video(text, subtext, duration, out_file):
    """Generate a text card as an mp4 using Pillow for the image + ffmpeg for encoding."""
    # Determine font size
    if len(text) <= 6:
        fontsize = 72
    elif len(text) <= 15:
        fontsize = 56
    else:
        fontsize = 44

    # Render text card image
    img = Image.new("RGB", (WIDTH, HEIGHT), BG_COLOR)
    draw = ImageDraw.Draw(img)

    font_main = load_font(fontsize)
    font_sub = load_font(28)

    # Center main text
    bbox = draw.textbbox((0, 0), text, font=font_main)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    y_offset = -20 if subtext else 0
    x = (WIDTH - tw) // 2
    y = (HEIGHT - th) // 2 + y_offset
    draw.text((x, y), text, font=font_main, fill=TEXT_COLOR)

    # Subtext below
    if subtext:
        bbox_s = draw.textbbox((0, 0), subtext, font=font_sub)
        stw = bbox_s[2] - bbox_s[0]
        sx = (WIDTH - stw) // 2
        sy = y + th + 20
        draw.text((sx, sy), subtext, font=font_sub, fill=SUBTEXT_COLOR)

    # Save as temporary PNG
    card_png = out_file.replace(".mp4", ".png")
    img.save(card_png)

    # Encode to mp4 with fade in/out
    fade_in_dur = min(0.3, duration * 0.3)
    fade_out_start = duration - min(0.3, duration * 0.3)

    cmd = [
        "ffmpeg", "-y",
        "-loop", "1",
        "-i", card_png,
        "-t", str(duration),
        "-vf", f"fade=t=in:st=0:d={fade_in_dur},fade=t=out:st={fade_out_start:.3f}:d={min(0.3, duration * 0.3)},format=yuv420p",
        "-c:v", "libx264",
        "-preset", "fast",
        "-crf", "18",
        "-r", str(FPS),
        "-pix_fmt", "yuv420p",
        out_file,
    ]
    result = subprocess.run(cmd, capture_output=True)
    os.remove(card_png)
    if result.returncode != 0:
        print(f"    WARN: card encode failed: {result.stderr.decode()[-200:]}", file=sys.stderr)
        raise subprocess.CalledProcessError(result.returncode, cmd)


with open(EDL_FILE) as f:
    edl = json.load(f)

segments = edl["segments"]
tmp_dir = os.path.join(VIDEO_DIR, "recordings", "tmp_segments")
os.makedirs(tmp_dir, exist_ok=True)

segment_files = []

for i, seg in enumerate(segments):
    out_file = os.path.join(tmp_dir, f"seg_{i:03d}.mp4")
    segment_files.append(out_file)
    duration = seg["end"] - seg["start"]

    if seg["kind"] == "card":
        text = seg["text"]
        subtext = seg.get("subtext", "")
        print(f"  Card: '{text}' ({duration:.1f}s)", file=sys.stderr)
        make_card_video(text, subtext, duration, out_file)

    elif seg["kind"] == "clip":
        clip_in = seg["in"]
        clip_out = seg["out"]
        clip_duration = clip_out - clip_in
        label = seg.get("label", "")

        fade_dur = 0.2
        fade_out_start = max(0, clip_duration - fade_dur)

        filters = [
            f"scale={WIDTH}:{HEIGHT}:force_original_aspect_ratio=decrease:flags=lanczos",
            f"pad={WIDTH}:{HEIGHT}:(ow-iw)/2:(oh-ih)/2:color=0x1a1a2e",
            f"fade=t=in:st=0:d={fade_dur}",
            f"fade=t=out:st={fade_out_start:.3f}:d={fade_dur}",
            "format=yuv420p",
        ]

        cmd = [
            "ffmpeg", "-y",
            "-ss", str(clip_in),
            "-i", RAW_VIDEO,
            "-t", str(clip_duration),
            "-vf", ",".join(filters),
            "-c:v", "libx264",
            "-preset", "fast",
            "-crf", "18",
            "-r", str(FPS),
            "-an",
            out_file,
        ]

        print(f"  Clip: '{label}' ({clip_in:.1f}→{clip_out:.1f}s, {clip_duration:.1f}s)", file=sys.stderr)
        result = subprocess.run(cmd, capture_output=True)
        if result.returncode != 0:
            print(f"    WARN: clip failed for '{label}': {result.stderr.decode()[-200:]}", file=sys.stderr)
            # Fallback: black frame
            fallback_cmd = [
                "ffmpeg", "-y", "-f", "lavfi",
                "-i", f"color=c=0x1a1a2e:s={WIDTH}x{HEIGHT}:d={clip_duration}:r={FPS}",
                "-c:v", "libx264", "-preset", "fast", "-crf", "18",
                "-pix_fmt", "yuv420p",
                "-t", str(clip_duration), out_file,
            ]
            subprocess.run(fallback_cmd, capture_output=True, check=True)

# --- Write concat list ---
concat_file = os.path.join(tmp_dir, "concat.txt")
with open(concat_file, "w") as f:
    for sf in segment_files:
        f.write(f"file '{sf}'\n")

print(f"\nGenerated {len(segment_files)} segments", file=sys.stderr)
print(f"Concat list: {concat_file}", file=sys.stderr)
PYEOF

log "Concatenating segments into final video..."

# --- Concat all segments ---
CONCAT_FILE="$VIDEO_DIR/recordings/tmp_segments/concat.txt"

ffmpeg -y \
    -f concat \
    -safe 0 \
    -i "$CONCAT_FILE" \
    -c:v libx264 \
    -preset medium \
    -crf 18 \
    -pix_fmt yuv420p \
    -movflags +faststart \
    -r "$FPS" \
    "$OUTPUT_16x9"

FINAL_DURATION=$(ffprobe -v error -show_entries format=duration -of default=noprint_wrappers=1:nokey=1 "$OUTPUT_16x9" 2>/dev/null || echo "unknown")
log "16:9 video rendered: $OUTPUT_16x9 (${FINAL_DURATION}s)"

# --- Render 9:16 (vertical) crop ---
log "Rendering 9:16 vertical crop..."

# Center-crop to 1080x1920 (portrait) — keeps the editor content centered
ffmpeg -y \
    -i "$OUTPUT_16x9" \
    -vf "crop=ih*9/16:ih:(iw-ih*9/16)/2:0,scale=1080:1920" \
    -c:v libx264 \
    -preset medium \
    -crf 20 \
    -pix_fmt yuv420p \
    -movflags +faststart \
    -r "$FPS" \
    "$OUTPUT_9x16"

log "9:16 video rendered: $OUTPUT_9x16"

# --- Extract thumbnail ---
log "Extracting thumbnail..."

# Grab a frame from ~5 seconds in (should show code with syntax highlighting)
ffmpeg -y \
    -ss 5 \
    -i "$OUTPUT_16x9" \
    -vframes 1 \
    -q:v 2 \
    "$THUMBNAIL"

log "Thumbnail: $THUMBNAIL"

# --- Cleanup temp segments ---
rm -rf "$VIDEO_DIR/recordings/tmp_segments"

log "Render complete!"
log ""
log "Outputs:"
log "  16:9 → $OUTPUT_16x9 (${FINAL_DURATION}s)"
log "  9:16 → $OUTPUT_9x16"
log "  Thumb → $THUMBNAIL"
log "  EDL  → $EDL_FILE"
