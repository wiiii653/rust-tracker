# rust-tracker

A modern **Fast Tracker 2** clone for Linux, written in Rust.

Load, edit, and play XM, MOD, S3M, and IT tracker modules with a hardware-accelerated GUI, low-latency ALSA audio, and MIDI input support.

## Features

- 🎵 **Module playback** — Load and play XM, MOD, S3M, IT files with accurate xmrsplayer engine
- 🎹 **Pattern editor** — QWERTY note entry, cursor navigation, effect columns
- 🔊 **Sample editor** — Waveform display with zoom/scroll, normalize/reverse/fade/amplify
- 🎛 **Instrument editor** — Volume/panning/pitch envelope graphs, vibrato, NNA/DCT, keyboard map
- 📊 **Audio visualization** — Real-time oscilloscope and per-channel VU meters
- 🎚 **MIDI input** — Connect external MIDI keyboards for note entry (Ctrl+M)
- ↩️ **Undo/Redo** — Full edit history (Ctrl+Z / Ctrl+Y)
- 💾 **Disk browser** — Built-in file browser for module and sample files
- 🎨 **Themes** — FT2 classic dark blue + modern dark theme
- 🖥 **Headless render** — Export modules to WAV from command line

## Screenshots

*Screenshots coming soon — see [16-bits.org](https://16-bits.org) for FT2 reference.*

## Installation

### From source

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt install build-essential libasound2-dev libudev-dev cargo

# Clone and build
git clone https://github.com/user/rust-tracker.git
cd rust-tracker
cargo build --release

# Run
./target/release/rust-tracker song.xm
```

### Arch Linux (AUR)

```bash
paru -S rust-tracker
```

### .deb package

```bash
./packaging/build-deb.sh
sudo dpkg -i packaging/deb/rust-tracker_0.1.0_amd64.deb
```

### AppImage

```bash
./packaging/build-appimage.sh
# Requires appimagetool: https://github.com/AppImage/AppImageKit
```

### Flatpak

```bash
# Coming soon — flatpak manifest at packaging/flatpak/
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

### Controls

| Key | Action |
|---|---|
| **Pattern Editor** | |
| ZSXDCVGBHNJM | Notes C through B (octave 4) |
| Q2W3ER5T6Y7U | Notes C through B (octave 5) |
| Ctrl + note | Raise one octave |
| ↑ ↓ ← → | Move cursor |
| Tab / Shift+Tab | Next/previous channel |
| Home / End | Go to row start/end |
| Ctrl+Home/End | Go to first/last order |
| Page Up/Down | Page 16 rows |
| Delete / Backspace | Clear note |
| F11 / F12 | Previous/next order |
| **Global** | |
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Ctrl+O | Open file |
| Ctrl+S | Save |
| Ctrl+M | Connect MIDI |
| F1 | Keyboard shortcuts help |

## Project Structure

```
src/
├── main.rs              # Entry point, CLI, winit event loop
├── app.rs               # Top-level App state, menu, UI dispatch
├── state.rs             # Shared state (module, audio, undo, config)
├── config.rs            # XDG config (~/.config/rust-tracker/)
├── undo.rs              # Undo/Redo stack
├── midi.rs              # MIDI input handler
├── audio/
│   └── engine.rs        # cpal + xmrsplayer playback thread
├── module/
│   ├── io.rs            # Module load/save
│   ├── edit.rs          # Pattern editing helpers
│   └── sample.rs        # Sample data extraction & operations
└── ui/
    ├── transport.rs     # Play/Stop controls
    ├── pattern_editor.rs # Pattern grid widget
    ├── order_list.rs    # Pattern sequence order
    ├── sample_editor.rs # Waveform display & sample ops
    ├── instr_editor.rs  # Instrument & envelope editor
    ├── disk_op.rs       # File browser
    ├── viz.rs           # Oscilloscope & VU meters
    └── theme.rs         # FT2 classic & modern dark themes
```

## Dependencies

| Crate | Purpose |
|---|---|
| `xmrs` | Module data model & file format I/O |
| `xmrsplayer` | Playback engine (effects, mixing, voices) |
| `cpal` | Cross-platform audio I/O (ALSA on Linux) |
| `egui` + `egui-wgpu` + `winit` | GUI framework & windowing |
| `wgpu` | GPU rendering backend |
| `hound` | WAV sample loading/saving |
| `midir` | MIDI input |
| `clap` | CLI argument parsing |
| `rfd` | Native file dialogs |
| `ringbuf` | Lock-free audio ring buffers |
| `dirs` | XDG directory resolution |

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

MIT — See [LICENSE](LICENSE) for details.

## Acknowledgements

- Fast Tracker 2 by Triton (Magnus Högdahl & Fredrik Huss)
- [xmrs](https://codeberg.org/sbechet/xmrs) & [xmrsplayer](https://codeberg.org/sbechet/xmrsplayer) by Sébastien Bechet
- [ft2-clone](https://github.com/8bitbubsy/ft2-clone) by Olav Sørensen (8bitbubsy)
- [MilkyTracker](https://milkytracker.org/)
