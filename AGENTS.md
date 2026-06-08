# AGENTS.md — Rust Tracker

## Project Overview

A modern Fast Tracker 2 clone for Linux, written in Rust. Faithful XM/MOD/S3M/IT
import/export and playback, with a hardware-accelerated egui-based GUI.
Compose new modules from scratch, edit with QWERTY/MIDI note entry, save as MOD.

## Architecture

```
main.rs → App (egui frame loop)
            ├── AudioEngine (xmrsplayer + cpal → ALSA)
            ├── AppState (module, audio, undo, config, viz, midi)
            ├── UndoManager (xmrs EditCommand stack)
            ├── MidiInput (midir → pattern note entry)
            └── UI panels:
                ├── PatternEditor (QWERTY grid, cursor, EditCommand output)
                ├── SampleEditor (waveform, ops, loop markers)
                ├── InstrEditor (envelope graphs, vibrato, key map)
                ├── DiskOp (file browser)
                ├── OrderList (pattern sequence)
                ├── TransportBar (play/stop/BPM/time)
                └── AudioViz (oscilloscope + VU meters)
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
| `wgpu`         | GPU abstraction                           |
| `hound`        | WAV sample loading/saving                 |
| `clap`         | CLI argument parsing                      |
| `rfd`          | Native file dialogs                       |
| `midir`        | MIDI input (ALSA sequencer)               |
| `ringbuf`      | Lock-free audio ring buffers             |
| `pollster`     | Async wgpu init                           |
| `dirs`         | XDG config directories                    |

## Coding Conventions

- **Rust 2021 edition**.
- **`anyhow::Result`** for all fallible functions.
- **One module per UI panel.** Each `src/ui/*.rs` renders a self-contained
  egui window or panel.
- **Audio runs on a dedicated thread.** Uses `ringbuf` for lock-free
  sample transfer; `std::thread::sleep` backpressure instead of spin-loop.
- **Undo/redo** wraps xmrs's `EditCommand`/`EditReceipt`. `UndoManager.execute()`
  pushes receipt onto stack; `undo()` pops and calls `Module::undo()`.
- **Pattern editor** returns `Vec<EditCommand>` each frame from user input.
  App applies them through `undo.execute()`.
- **Config** stored at `$XDG_CONFIG_HOME/rust-tracker/config.json`.
- **Themes** applied once on first frame (`current_theme` flag), toggled
  via View menu.

## File Layout

```
src/
├── main.rs              # Entry point, CLI, winit/wgpu event loop
├── app/
│   ├── mod.rs           # RustTracker struct, update loop, global keys, status bar
│   ├── menu.rs          # Menu bar (File, Module, View, Help)
│   ├── dialogs.rs       # Help dialog (F1), New Module dialog (Ctrl+N)
│   └── editors.rs       # render_transport/pattern/sample/instr/disk_op/info/viz
├── state.rs             # AppState (module, audio, undo, config, viz, midi, VizBuffer)
├── config.rs            # XDG config load/save (AppConfig)
├── undo.rs              # UndoManager (EditCommand stack, 256-step limit)
├── midi.rs              # MidiInput (midir connection, polling, MidiNoteEvent)
├── audio/
│   ├── mod.rs
│   └── engine.rs        # AudioEngine (cpal stream, player thread, ring buffer, viz feed)
├── module/
│   ├── mod.rs
│   ├── io.rs            # Module load/save, ModuleInfo
│   ├── edit.rs          # PatternPosition, cell formatting, effect to/from string
│   ├── create.rs        # create_empty_module() with MOD/XM presets
│   ├── save_mod.rs      # MOD binary exporter (31 samples, 4 channels, Amiga periods)
│   └── sample.rs        # SampleData extraction, overview, operations (normalize/reverse/etc)
└── ui/
    ├── mod.rs
    ├── transport.rs     # TransportBar (play/stop, pattern/row/BPM/time)
    ├── pattern_editor.rs # PatternEditor (QWERTY grid, cursor nav, EditCommand generation)
    ├── order_list.rs    # OrderList (clickable pattern sequence)
    ├── sample_editor.rs # SampleEditor (waveform, zoom/scroll, loop markers, operations)
    ├── instr_editor.rs  # InstrEditor (envelope graphs, vibrato, NNA/DCT, keyboard map)
    ├── disk_op.rs       # DiskOp (file browser, directory nav, filter)
    ├── viz.rs           # AudioViz (oscilloscope, VU meters per channel)
    └── theme.rs         # apply_ft2_classic() / apply_modern_dark()
```

## Phases (all complete ✅)

1. ✅ **Foundation** — scaffold, module I/O, playback, minimal GUI
2. ✅ **Pattern Editor** — pattern grid, QWERTY note entry → EditCommands, cursor nav, order list
3. ✅ **Sample Editor** — waveform display, sample ops, loop markers, instrument/sample browser
4. ✅ **Instrument Editor** — envelope graphs, vibrato, NNA/DCT, keyboard map
5. ✅ **Advanced** — undo/redo, audio viz (oscilloscope+VU), disk browser, MIDI→notes
6. ✅ **Polish** — themes, .desktop, packaging, README, help dialog, persistent view tabs, Esc to go back

## Building & Running

```bash
cargo run -- path/to/song.xm        # Open and play
cargo run -- --render output.wav song.xm  # Render to WAV
cargo run                           # Empty window, File → New Module…
```

Requires ALSA dev headers:
```bash
sudo apt install libasound2-dev libudev-dev
```

## Navigation Quick Reference

| Key | Action |
|-----|--------|
| Ctrl+1..5 | Switch views (Info/Pattern/Sample/Instr/Disk) |
| Esc | Back to Info view / close dialogs |
| F1 | Help dialog |
| Ctrl+N | New Module |
| Ctrl+S | Save as MOD |

## Commit Style

- `feat:` new feature
- `fix:` bugfix
- `refactor:` code restructuring
- `ui:` GUI changes
- `audio:` playback/mixer changes
- `docs:` documentation
