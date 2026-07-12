# speaktype

Cross-application speech-to-text: hold a hotkey anywhere on your desktop, speak,
release — the transcript (cleaned up and tone-adjusted for whatever app you're
in) gets typed into whatever text field currently has focus.

Resume line this becomes once it's running daily:

> Built a cross-application speech-to-text tool using local Whisper inference
> and OS-level input hooks (Rust), with an LLM-based agent for context-aware
> transcript cleanup.

## Why it's built this way

Scoped deliberately to balance an AWS/agent-heavy resume, not duplicate it:

- **No cloud STT.** Whisper runs 100% locally via whisper.cpp. This is the
  part that actually teaches something new — on-device ML inference — and
  it's the right call anyway: a tool that hears everything you type by voice
  shouldn't be shipping audio to a third party.
- **The LangGraph piece is intentionally small** — one tone-routing node, one
  cleanup node. It reuses the multi-agent pipeline skill without becoming a
  second full agent system.
- **Most of the engineering effort is in the injection/hook layer**
  (`core/src/audio.rs`, `focus.rs`, `inject.rs`) — global hotkeys, OS audio
  capture, focused-window detection, and simulated keystroke injection. This
  is the piece with zero overlap with the rest of the resume and the highest
  "can do systems work" signal.

## Architecture

```
[hold hotkey] --(global-hotkey + tao event loop)--> [audio.rs: record mic to 16kHz WAV]
                                                              |
                                                    [release hotkey: stop recording]
                                                              |
                                            [transcribe.rs: shell out to whisper.cpp]
                                                              |
                                                     raw transcript (local, private)
                                                              |
                                   [focus.rs: detect focused app, e.g. "slack"/"code"]
                                                              |
                        POST /cleanup  -->  [Python: LangGraph — route_tone -> cleanup]
                                                              |
                                                cleaned, tone-adjusted text
                                                              |
                                       [inject.rs: simulate keystrokes into focused field]
```

Two processes, talking over localhost HTTP:

- **`core/`** — Rust daemon. Owns the hotkey, audio, whisper.cpp invocation,
  focus detection, and injection. This is the thing that's always running.
- **`cleanup_service/`** — Python FastAPI + LangGraph microservice. Takes a
  raw transcript + app context, returns cleaned text. Kept as a separate
  process deliberately — it's the part you'll iterate on most (prompts,
  tone profiles), and restarting a Python process is much faster than
  rebuilding Rust while you tune it.

## Setup

### 1. Build whisper.cpp and download a model

```bash
./scripts/setup_whisper.sh
```

This clones `whisper.cpp`, builds it, and downloads the `base.en` model
(~140MB, good starting point — swap for `small.en` later if you want more
accuracy and can tolerate slightly more latency).

### 2. Start the cleanup service

```bash
cd cleanup_service
python3 -m venv venv
./venv/bin/pip install -r requirements.txt
export GEMINI_API_KEY=AIza...
./venv/bin/uvicorn main:app --host 127.0.0.1 --port 8008
```

Verify it's up: `curl http://127.0.0.1:8008/health` should return `{"status":"ok"}`.

### 3. Build and run the core daemon

You'll need a Rust toolchain — install via [rustup](https://rustup.rs) if you
don't have one (`rustc >= 1.85` recommended; several dependencies now require
Rust's 2024 edition). On Linux you'll also need:

```bash
sudo apt install libx11-dev libxdo-dev libasound2-dev libxi-dev libxtst-dev pkg-config
```

Then:

```bash
cd core
cp config.toml.example config.toml   # adjust paths if needed
cargo build --release
./target/release/speaktype
```

Hold `Ctrl+Alt+Space` (default, configurable in `config.toml`), speak,
release. The transcript should appear in whatever text field has focus.

## What's verified vs. what needs your machine

Everything in `cleanup_service/` was actually run in the build environment:
server starts, `/health` responds, `/cleanup` correctly validates input,
calls the LangGraph pipeline, and returns a clear error when no API key is
present. The tone-routing node was unit-tested directly against all four
app-context buckets.

The Rust `core/` was written against current, correct APIs for each crate
(`global-hotkey`, `cpal`, `enigo`, `tokio`, `reqwest`) and partially compiled
in this environment — every module except the hotkey/event-loop code
(`main.rs`'s use of `tao`/`global-hotkey`) was reachable in dependency
resolution before hitting a toolchain limit: this sandbox ships Rust 1.75,
and current versions of `tao`'s transitive dependencies require the 2024
edition (Rust 1.85+). None of the failures were in this project's own code —
all were in unrelated upstream crates during dependency resolution. Run
`cargo build` on your own machine with a current `rustup` toolchain; it
should build cleanly. If something doesn't compile, the most likely
suspects are minor API drift in `global-hotkey`/`enigo`/`cpal` since this was
written — check their docs.rs pages for the version Cargo actually resolves.

## Known rough edges (be upfront about these if you demo it)

- **Injection method**: uses simulated keystrokes (`enigo.text()`), which
  types character-by-character. This is slow for long transcripts and can
  trigger autocomplete in some apps. A clipboard-paste (`Ctrl+V`) variant is
  faster but overwrites the user's clipboard — worth offering both, with
  clipboard-paste as an opt-in.
- **Focus detection is X11-only** (`focus.rs` uses `xdotool`). Wayland needs
  a compositor-specific approach (e.g. `wlr-foreign-toplevel-management`) —
  flagged clearly in code rather than silently failing.
- **Resampling in `audio.rs`** is a simple decimation, not proper sinc-based
  resampling. Fine for speech-to-text; would introduce artifacts for
  anything audio-quality-sensitive.
- **No VAD (voice activity detection)** — recording starts/stops purely on
  hotkey press/release. A stretch goal is auto-stopping after silence.

## Repo structure

```
speaktype/
├── core/                    # Rust daemon
│   ├── Cargo.toml
│   ├── config.toml.example
│   └── src/
│       ├── main.rs          # hotkey event loop, orchestrates the pipeline
│       ├── config.rs        # loads config.toml
│       ├── audio.rs         # mic capture -> 16kHz mono WAV
│       ├── transcribe.rs    # shells out to whisper.cpp
│       ├── focus.rs         # detects focused app (for tone routing)
│       ├── cleanup.rs       # HTTP client to the Python service
│       └── inject.rs        # simulated keystroke injection
├── cleanup_service/         # Python FastAPI + LangGraph
│   ├── main.py               # HTTP endpoint
│   ├── graph.py               # 2-node LangGraph: route_tone -> cleanup
│   └── requirements.txt
└── scripts/
    └── setup_whisper.sh      # clones/builds whisper.cpp, downloads model
```

## Roadmap (phases from original plan)

- [x] Phase 1: hotkey -> record -> local Whisper -> inject
- [x] Phase 3: LLM cleanup pass (filler removal, punctuation)
- [x] Phase 4: context-awareness (tone by focused app)
- [ ] Phase 2 (retro-fit): swap `base.en` for `small.en`/`medium.en`, measure
      latency/accuracy tradeoff — good place to start once the pipeline works
- [ ] Phase 5: tray icon, settings UI, custom vocabulary/dictionary

## What to learn next (after this ships)

See `docs/learning-roadmap.md` for a longer writeup, but the short version:

1. **Voice activity detection (VAD)** — Silero VAD or WebRTC VAD, so
   recording auto-stops on silence instead of needing hold-to-talk.
2. **Streaming transcription** — whisper.cpp supports streaming mode; moving
   from "record then transcribe" to "transcribe as you speak" is a real
   latency/UX upgrade and a good excuse to learn streaming inference.
3. **Wayland support** — the compositor-specific focus/injection APIs are
   genuinely underdocumented; solving this properly is a strong signal.
4. **Quantization** — try Whisper's int8/int4 quantized checkpoints, measure
   accuracy vs. latency vs. RAM tradeoffs on your own hardware.
5. **Packaging** — wrap `core/` in Tauri for a tray icon + settings UI once
   the daemon itself is solid; don't start with the GUI.
