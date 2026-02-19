# Token Editor — Launch Video Generator

Fully automated ~30s launch video pipeline for macOS. One command, deterministic output.

## Quick Start

```bash
# From project root:
./video/run.sh
```

## Prerequisites

### 1. Token Editor binary

```bash
make release
# or ensure `token` is in your PATH
```

### 2. FFmpeg

```bash
brew install ffmpeg
```

### 3. macOS Permissions

Grant your terminal app (Terminal.app or iTerm2) these permissions in **System Settings → Privacy & Security**:

- **Accessibility** — needed for simulated keystrokes via AppleScript
- **Screen Recording** — needed for ffmpeg AVFoundation capture

### 4. No additional tools required

No Hammerspoon, no OBS, no Node.js. Just ffmpeg + built-in macOS tools.

## Usage

```bash
# Full pipeline: demo → record → edit → render
./video/run.sh

# Re-render from existing recording (skip demo capture)
./video/run.sh --render

# Custom Token binary location
TOKEN_BIN=./target/release/token ./video/run.sh

# Custom screen capture device
SCREEN_INDEX=2 ./video/run.sh
```

## Outputs

```
video/dist/
├── token_launch_30s_16x9.mp4   # 1920×1080 landscape (required)
├── token_launch_30s_9x16.mp4   # 1080×1920 portrait (vertical crop)
├── thumbnail.png               # Frame grab at ~5s
└── edl.json                    # Edit decision list (full timeline spec)
```

## Architecture

### Phase A — Demo Orchestration (`scripts/demo.sh`)

Launches Token with a prepared demo workspace (`demo/`) and runs a scripted keystroke sequence via AppleScript `System Events`:

1. Open `main.ts` — syntax highlighting
2. Navigate code — scroll, go to line
3. Multi-cursor edit — Cmd+J × 4, type replacement
4. Split view — Cmd+Shift+Alt+V
5. CSV editor — open `data.csv` in split pane
6. Find/Replace — Cmd+F
7. Code outline — Cmd+7
8. File explorer — Cmd+1
9. Command palette — Cmd+Shift+A

Each feature "beat" is timestamped to `recordings/beats.jsonl`.

### Phase B — Screen Recording (`scripts/record.sh`)

Captures the screen using `ffmpeg -f avfoundation`. Starts/stops via signal file coordination with the demo script. Output: `recordings/raw.mp4`.

### Phase C — Auto-Edit + Render

**EDL Generation** (`scripts/generate_edl.py`):
- Reads beat timestamps → builds a timeline with intro/outro cards, interstitial text, and clip segments
- Outputs `dist/edl.json`

**Render** (`scripts/render.sh`):
- Generates text cards using ffmpeg `drawtext` filter (dark bg, JetBrains Mono font)
- Cuts clips from raw footage at beat boundaries
- Applies subtle fade transitions (6 frames) and gentle zoom (1.0→1.02)
- Concatenates via ffmpeg concat demuxer
- Renders 16:9 and center-cropped 9:16 versions
- Extracts thumbnail frame

## Demo Workspace

```
video/demo/
├── main.ts       # TypeScript server — syntax highlighting showcase
├── data.csv      # Employee data — CSV editor showcase
├── config.yaml   # Project config — YAML highlighting
└── README.md     # Markdown content
```

## EDL Schema

```json
{
  "fps": 30,
  "size": {"w": 1920, "h": 1080},
  "durationSec": 30.0,
  "inputs": {"video": "recordings/raw.mp4"},
  "segments": [
    {"kind": "card", "text": "Token", "subtext": "...", "start": 0, "end": 1.2},
    {"kind": "clip", "label": "open", "in": 0.0, "out": 3.5, "start": 1.2, "end": 4.7,
     "motion": {"scaleFrom": 1.0, "scaleTo": 1.02}},
    {"kind": "card", "text": "multi-cursor", "start": 4.7, "end": 5.4}
  ],
  "transitions": [
    {"type": "fade", "at": 1.2, "dur": 0.2}
  ]
}
```

## Customization

- **Demo sequence**: Edit `scripts/demo.sh` keystroke sequence
- **Interstitial text**: Edit `BEAT_LABELS` in `scripts/generate_edl.py`
- **Card styling**: Edit colors/font sizes in `scripts/render.sh`
- **Clip durations**: Edit `max_clip` and card durations in `generate_edl.py`
- **Font**: Set `FONT_FILE` env var (defaults to `assets/JetBrainsMono.ttf`)

## Troubleshooting

| Problem | Solution |
|---------|----------|
| "Token binary not found" | Run `make release` or set `TOKEN_BIN` |
| Black recording | Grant Screen Recording permission to your terminal |
| No keystrokes register | Grant Accessibility permission to your terminal |
| Wrong screen captured | Set `SCREEN_INDEX=2` (or check `ffmpeg -f avfoundation -list_devices true -i ""`) |
| Video too short/long | Adjust `sleep` durations in `demo.sh` |
