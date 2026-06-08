# AGENTS.md — Rust Tracker

## Project Overview

A modern Fast Tracker 2 clone for Linux, written in Rust. Faithful XM/MOD/S3M/IT
import/export and playback, with a hardware-accelerated egui-based GUI.

## Architecture

```
main.rs → App (egui frame loop)
            ├── AudioEngine (xmrsplayer + cpal → ALSA)
            ├── ModuleStore (xmrs Module + I/O + editing ops)
            ├── AppState (undo, clipboard, config, keybinds)
            └── UI panels (pattern ed, sample ed, instr ed, transport, disk op)
```

## Key Dependencies

| Crate          | Role                                      |
|----------------|-------------------------------------------|
| `xmrs`         | Module data model & file format I/O       |
| `xmrsplayer`   | Playback engine (effects, mixing, voices) |
| `cpal`         | Cross-platform audio I/O (ALSA on Linux)  |
| `egui`         | Immediate-mode GUI                        |
| `egui-winit`   | egui ↔ winit integration                  |
| `egui-wgpu`    | egui GPU rendering (Vulkan/Metal/DX12)    |
| `winit`        | Cross-platform window + input             |
| `hound`        | WAV sample loading/saving                 |
| `clap`         | CLI argument parsing                      |
| `rfd`          | Native file dialogs                       |
| `midir`        | MIDI input                                |

## Coding Conventions

- **Rust 2024 edition** (if stable), otherwise 2021.
- **`anyhow` / `thiserror`** for error handling — library code returns
  structured errors, binary code uses `anyhow::Result`.
- **One module per UI panel.** Each `src/ui/*.rs` renders a self-contained
  egui window or panel.
- **Audio runs on a dedicated thread.** The cpal callback must never block;
  use `ringbuf` or `crossbeam::channel` for lock-free communication.
- **Undo/redo** uses the command pattern. Every mutation to `Module` goes
  through an `EditCommand` trait so it can be pushed onto the undo stack.
- **Config** is stored under `$XDG_CONFIG_HOME/rust-tracker/` (usually
  `~/.config/rust-tracker/`).

## File Layout

```
src/
├── main.rs              # Entry point
├── app.rs               # Top-level App, egui frame loop
├── state.rs             # Shared state (undo, config)
├── config.rs            # XDG config load/save
├── audio/
│   ├── mod.rs
│   └── engine.rs        # Playback engine wrapper
├── module/
│   ├── mod.rs
│   ├── io.rs            # Module load/save
│   ├── edit.rs          # Pattern editing helpers
│   └── sample.rs        # Sample data extraction & operations
└── ui/
    ├── mod.rs
    ├── transport.rs     # Play/stop controls
    ├── pattern_editor.rs
    ├── sample_editor.rs
    ├── instr_editor.rs
    ├── disk_op.rs
    ├── order_list.rs
    └── theme.rs
```

## Phases

1. ✅ **Foundation** — scaffold, module I/O, playback, minimal GUI
2. ✅ **Pattern Editor** — pattern grid, QWERTY note entry, cursor navigation, order list
3. ✅ **Sample Editor** — waveform display, sample operations, loop point markers
4. ✅ **Instrument Editor** — envelope graphs, vibrato display, NNA/DCT, keyboard map
5. ✅ **Advanced** — undo/redo, audio viz (oscilloscope + VU), disk ops browser, MIDI input
6. ✅ **Polish** — FT2/modern themes, .desktop integration, packaging scripts, README, help dialog, window title

## Building & Running

```bash
cargo run -- path/to/song.xm        # Open and play
cargo run -- --render output.wav song.xm  # Render to WAV
```

Requires ALSA dev headers on Linux:
```bash
sudo apt install libasound2-dev libudev-dev
```

## Commit Style

- `feat:` new feature
- `fix:` bugfix
- `refactor:` code restructuring
- `ui:` GUI changes
- `audio:` playback/mixer changes
- `docs:` documentation
