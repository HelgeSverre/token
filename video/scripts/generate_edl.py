#!/usr/bin/env python3
"""
Phase C (part 1): Generate EDL (Edit Decision List) from beat timestamps.

Reads recordings/beats.jsonl → produces dist/edl.json
The EDL describes the final ~30s timeline with:
  - intro card
  - interstitial text cards between feature beats
  - clip segments from raw footage
  - outro card
  - transition fade specifications
"""

import json
import sys
import os
import subprocess

SCRIPT_DIR = os.path.dirname(os.path.abspath(__file__))
VIDEO_DIR = os.path.dirname(SCRIPT_DIR)
BEATS_FILE = os.path.join(VIDEO_DIR, "recordings", "beats.jsonl")
RAW_VIDEO = os.path.join(VIDEO_DIR, "recordings", "raw.mp4")
EDL_FILE = os.path.join(VIDEO_DIR, "dist", "edl.json")

# --- Configuration ---

FPS = 30
SIZE = {"w": 1920, "h": 1080}
TARGET_DURATION = 30.0

# Card durations
INTRO_DURATION = 1.2
INTERSTITIAL_DURATION = 0.7
OUTRO_DURATION = 1.5
FADE_DURATION = 0.2  # 6 frames at 30fps

# Feature labels for interstitial cards (beat_label → display text)
BEAT_LABELS = {
    "open": None,  # No card before first clip
    "code_visible": None,
    "navigate": None,
    "multicursor_start": "multi-cursor",
    "multicursor_active": None,
    "multicursor_typed": None,
    "split_view": "split view",
    "csv_view": "csv editor",
    "search": "search",
    "outline": "outline",
    "workspace": "workspace",
    "command_palette": "command palette",
    "end": None,
}

# Which beats define clip boundaries (start of a new "scene")
SCENE_BEATS = [
    "open",           # opening the file
    "multicursor_start",  # multi-cursor feature
    "split_view",     # split view
    "csv_view",       # csv editor
    "search",         # find/replace
    "outline",        # code outline
    "command_palette", # command palette
    "end",            # end marker
]


def get_video_duration(path):
    """Get duration of video file in seconds."""
    try:
        result = subprocess.run(
            ["ffprobe", "-v", "error", "-show_entries", "format=duration",
             "-of", "default=noprint_wrappers=1:nokey=1", path],
            capture_output=True, text=True
        )
        return float(result.stdout.strip())
    except Exception:
        return 25.0  # fallback


def load_beats():
    """Load beat timestamps from JSONL file."""
    beats = []
    with open(BEATS_FILE, "r") as f:
        for line in f:
            line = line.strip()
            if line:
                beats.append(json.loads(line))
    return beats


def build_edl(beats, raw_duration):
    """Build the EDL timeline from beats."""
    # Index beats by label
    beat_map = {}
    for b in beats:
        beat_map[b["label"]] = b["t"]

    segments = []
    transitions = []
    timeline_pos = 0.0

    # --- Intro Card ---
    segments.append({
        "kind": "card",
        "text": "Token",
        "subtext": "a minimal text editor",
        "start": round(timeline_pos, 3),
        "end": round(timeline_pos + INTRO_DURATION, 3),
    })
    transitions.append({
        "type": "fade",
        "at": round(timeline_pos + INTRO_DURATION, 3),
        "dur": FADE_DURATION,
    })
    timeline_pos += INTRO_DURATION

    # --- Build scenes from beats ---
    scene_times = []
    for label in SCENE_BEATS:
        if label in beat_map:
            scene_times.append((label, beat_map[label]))

    for i in range(len(scene_times) - 1):
        label, start_t = scene_times[i]
        _, end_t = scene_times[i + 1]

        # Add interstitial card if this beat has a display label
        display_text = BEAT_LABELS.get(label)
        if display_text:
            segments.append({
                "kind": "card",
                "text": display_text,
                "start": round(timeline_pos, 3),
                "end": round(timeline_pos + INTERSTITIAL_DURATION, 3),
            })
            transitions.append({
                "type": "fade",
                "at": round(timeline_pos + INTERSTITIAL_DURATION, 3),
                "dur": FADE_DURATION,
            })
            timeline_pos += INTERSTITIAL_DURATION

        # Calculate clip duration — use raw footage timestamps
        clip_in = start_t
        clip_out = min(end_t, raw_duration)
        clip_duration = clip_out - clip_in

        # Limit individual clips to keep total ~30s
        max_clip = 5.0
        if clip_duration > max_clip:
            clip_out = clip_in + max_clip
            clip_duration = max_clip

        segments.append({
            "kind": "clip",
            "label": label,
            "in": round(clip_in, 3),
            "out": round(clip_out, 3),
            "start": round(timeline_pos, 3),
            "end": round(timeline_pos + clip_duration, 3),
            "motion": {
                "scaleFrom": 1.0,
                "scaleTo": 1.02,
            },
        })
        transitions.append({
            "type": "fade",
            "at": round(timeline_pos + clip_duration, 3),
            "dur": FADE_DURATION,
        })
        timeline_pos += clip_duration

    # --- Outro Card ---
    segments.append({
        "kind": "card",
        "text": "token-editor.com",
        "subtext": "",
        "start": round(timeline_pos, 3),
        "end": round(timeline_pos + OUTRO_DURATION, 3),
    })
    timeline_pos += OUTRO_DURATION

    # Build final EDL
    total_duration = round(timeline_pos, 3)

    edl = {
        "fps": FPS,
        "size": SIZE,
        "durationSec": total_duration,
        "inputs": {
            "video": "recordings/raw.mp4",
        },
        "segments": segments,
        "transitions": transitions,
    }

    return edl


def main():
    if not os.path.exists(BEATS_FILE):
        print(f"ERROR: Beats file not found: {BEATS_FILE}", file=sys.stderr)
        sys.exit(1)

    if not os.path.exists(RAW_VIDEO):
        print(f"ERROR: Raw video not found: {RAW_VIDEO}", file=sys.stderr)
        sys.exit(1)

    beats = load_beats()
    print(f"Loaded {len(beats)} beats", file=sys.stderr)

    raw_duration = get_video_duration(RAW_VIDEO)
    print(f"Raw video duration: {raw_duration:.1f}s", file=sys.stderr)

    edl = build_edl(beats, raw_duration)
    print(f"Timeline duration: {edl['durationSec']:.1f}s ({len(edl['segments'])} segments)", file=sys.stderr)

    os.makedirs(os.path.dirname(EDL_FILE), exist_ok=True)
    with open(EDL_FILE, "w") as f:
        json.dump(edl, f, indent=2)

    print(f"EDL written to {EDL_FILE}", file=sys.stderr)


if __name__ == "__main__":
    main()
