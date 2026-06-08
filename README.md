# rust-tracker

A modern **Fast Tracker 2** clone for Linux, written in Rust.

Load, edit, and play XM, MOD, S3M, and IT tracker modules — or **compose from scratch** with QWERTY note entry, MIDI input, and export to ProTracker-compatible MOD files. Hardware-accelerated GUI, low-latency ALSA audio, Amiga-style workflow.

## Features

- 🎵 **Module playback** — Load and play XM, MOD, S3M, IT files with xmrsplayer engine
- 🎹 **Pattern editor** — QWERTY note entry (FT2 layout), cursor navigation, effect columns, **notes actually modify the module** via undoable EditCommands
- 🔊 **Sample editor** — Waveform display with zoom/scroll, normalize/reverse/amplify/fade, loop point markers
- 🎛 **Instrument editor** — Volume/panning/pitch envelope graphs with sustain/loop markers, vibrato, NNA/DCT, keyboard sample map
- 📊 **Audio visualization** — Real-time oscilloscope and per-channel VU meters during playback
- 🎚 **MIDI input** — Connect external MIDI keyboards, notes enter directly at cursor position (Ctrl+M)
- ↩️ **Undo/Redo** — Full edit history via xmrs EditCommand stack (Ctrl+Z / Ctrl+Y)
- 💾 **Disk browser** — Built-in file browser with directory navigation and tracker file filter
- 🎨 **Themes** — FT2 classic dark blue + modern dark theme (View menu)
- 🎼 **Compose from scratch** — File → New Module… with MOD/XM presets (Ctrl+N)
- 💿 **Save as MOD** — Export to ProTracker-compatible .mod files (Ctrl+S)
- 🖥 **Headless render** — Export any module to WAV via `--render`
- 🖱 **Persistent view tabs** — 📋 Info | 🎵 Patterns | 🔊 Samples | 🎛 Instr. | 💾 Disk always visible (Ctrl+1..5)
- 🔙 **Escape to go back** — Press Esc to jump to Info view or close dialogs

## Installation

### From source

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install build-essential libasound2-dev libudev-dev

# Clone and build
git clone https://github.com/user/rust-tracker.git
cd rust-tracker
cargo build --release

# Run
./target/release/rust-tracker song.xm
```

### .deb package

```bash
./packaging/build-deb.sh
sudo dpkg -i packaging/deb/rust-tracker_0.1.0_amd64.deb
```

### AppImage

```bash
./packaging/build-appimage.sh
```

## Usage

### GUI mode

```bash
rust-tracker                    # Open empty window
rust-tracker song.xm            # Open and play a module
rust-tracker song.mod           # MOD files
rust-tracker song.s3m           # S3M files
rust-tracker song.it            # IT files
```

### Headless render

```bash
rust-tracker --render output.wav song.xm
```

Exports the module to a 44.1 kHz 16-bit stereo WAV file.

### Keyboard Shortcuts

| Key | Action |
|---|---|
| **Pattern Editor** | |
| ZSXDCVGBHNJM | Notes C through B (octave 4) |
| Q2W3ER5T6Y7U | Notes C through B (octave 5) |
| Ctrl + note | Raise one octave |
| ↑ ↓ ← → | Move cursor / change column |
| Tab / Shift+Tab | Next/previous channel |
| Home / End | Go to row start/end |
| Ctrl+Home/End | Go to first/last order |
| Page Up/Down | Page 16 rows |
| Delete / Backspace | **Clear note** (creates undoable edit) |
| F11 / F12 | Previous/next order |
| **View Switching** | |
| Ctrl+1 | 📋 Info view |
| Ctrl+2 | 🎵 Pattern editor |
| Ctrl+3 | 🔊 Sample editor |
| Ctrl+4 | 🎛 Instrument editor |
| Ctrl+5 | 💾 Disk browser |
| Esc | Back to Info view / close dialogs |
| **Global** | |
| Ctrl+N | New Module… |
| Ctrl+O | Open Module… |
| Ctrl+S | Save As MOD… |
| Ctrl+Z | Undo |
| Ctrl+Y / Ctrl+Shift+Z | Redo |
| Ctrl+M | Connect MIDI device |
| F1 | Keyboard shortcuts help |

## Project Structure

```
src/
├── main.rs              # Entry point, CLI, winit/wgpu event loop
├── app/
│   ├── mod.rs           # Top-level App state, update loop, global keys
│   ├── menu.rs          # Menu bar (File, Module, View, Help)
│   ├── dialogs.rs       # Help dialog, New Module dialog
│   └── editors.rs       # Editor view rendering methods
├── state.rs             # Shared state (module, audio, undo, config, viz, midi)
├── config.rs            # XDG config (~/.config/rust-tracker/)
├── undo.rs              # Undo/Redo stack (wraps xmrs EditCommand)
├── midi.rs              # MIDI input handler (midir + ALSA)
├── audio/
│   └── engine.rs        # cpal + xmrsplayer playback thread
├── module/
│   ├── io.rs            # Module load/save
│   ├── edit.rs          # Pattern editing helpers (cell formatting, effects)
│   ├── create.rs        # New empty module creation (MOD/XM presets)
│   ├── save_mod.rs      # MOD (ProTracker) binary exporter
│   └── sample.rs        # Sample data extraction & operations
└── ui/
    ├── transport.rs     # Play/Stop controls
    ├── pattern_editor.rs # Pattern grid widget (QWERTY entry, cursor, selection)
    ├── order_list.rs    # Pattern sequence order
    ├── sample_editor.rs # Waveform display & sample operations
    ├── instr_editor.rs  # Instrument & envelope editor
    ├── disk_op.rs       # File browser
    ├── viz.rs           # Oscilloscope & VU meters
    └── theme.rs         # FT2 classic & modern dark themes
```

## Dependencies

| Crate | Purpose |
|---|---|
| `xmrs` + `xmrsplayer` | Module data model, file I/O, playback engine |
| `cpal` | Cross-platform audio I/O (ALSA on Linux) |
| `egui` + `egui-wgpu` + `egui-winit` | Immediate-mode GUI with GPU rendering |
| `winit` + `wgpu` | Windowing + GPU backend (Vulkan/Metal/DX12) |
| `hound` | WAV sample loading/saving |
| `midir` | MIDI input |
| `clap` | CLI argument parsing |
| `rfd` | Native file dialogs (open/save) |
| `ringbuf` | Lock-free audio ring buffers |
| `dirs` | XDG directory resolution |
| `pollster` | Async runtime for wgpu init |
| `anyhow` + `thiserror` | Error handling |

## Building

Requires Rust 1.75+ and ALSA development headers:

```bash
# Ubuntu/Debian
sudo apt install build-essential libasound2-dev libudev-dev

# Fedora
sudo dnf install gcc gcc-c++ alsa-lib-devel systemd-devel

# Build
cargo build --release
```

## License

MIT

## Acknowledgements

- Fast Tracker 2 by Triton (Magnus Högdahl & Fredrik Huss)
- [xmrs](https://codeberg.org/sbechet/xmrs) & [xmrsplayer](https://codeberg.org/sbechet/xmrsplayer) by Sébastien Bechet
- [ft2-clone](https://github.com/8bitbubsy/ft2-clone) by Olav Sørensen (8bitbubsy)
- [MilkyTracker](https://milkytracker.org/)
