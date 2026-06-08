//! Application state shared across UI panels.

use anyhow::Context;
use crate::audio::engine::{AudioEngine, PlaybackState as AudioPlaybackState};
use crate::config::AppConfig;
use crate::midi::MidiInput;
use crate::module::io::ModuleInfo;
use crate::undo::UndoManager;
use crate::ui::viz::AudioViz;
use std::sync::{Arc, Mutex};
use xmrs::prelude::Module;

/// Shared ring buffer for passing audio viz data from audio thread to UI.
#[derive(Clone)]
pub struct VizBuffer {
    pub data: Arc<Mutex<Vec<i16>>>,
}

impl VizBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            data: Arc::new(Mutex::new(Vec::with_capacity(capacity))),
        }
    }

    pub fn push(&self, sample: i16) {
        if let Ok(mut data) = self.data.lock() {
            if data.len() < data.capacity() {
                data.push(sample);
            }
        }
    }

    pub fn drain(&self) -> Vec<i16> {
        if let Ok(mut data) = self.data.lock() {
            std::mem::take(&mut *data)
        } else {
            Vec::new()
        }
    }
}

/// Top-level application state.
pub struct AppState {
    /// Application configuration.
    pub config: AppConfig,

    /// Loaded module info (available even when module is playing).
    pub module_info: Option<ModuleInfo>,

    /// Raw module bytes — kept for reloading after playback stops.
    pub module_data: Option<Vec<u8>>,

    /// The loaded module, when not playing.
    pub module: Option<Module>,

    /// Active audio playback engine, if playing.
    pub audio: Option<AudioEngine>,

    /// Playback state (updated from audio thread).
    pub playback: Arc<Mutex<AudioPlaybackState>>,

    /// Undo/redo manager.
    pub undo: UndoManager,

    /// Audio visualization.
    pub viz: Option<AudioViz>,

    /// MIDI input handler.
    pub midi: MidiInput,

    /// Viz buffer for mix data from audio thread to UI.
    pub viz_mix: VizBuffer,
    /// Viz buffer for channel data.
    pub viz_channels: VizBuffer,
}

impl AppState {
    pub fn new() -> Self {
        let config = AppConfig::load();

        let num_channels = 8; // default
        let viz = Some(AudioViz::new(num_channels));

        Self {
            config,
            module_info: None,
            module_data: None,
            module: None,
            audio: None,
            playback: Arc::new(Mutex::new(AudioPlaybackState::default())),
            undo: UndoManager::new(),
            viz,
            midi: MidiInput::new(),
            viz_mix: VizBuffer::new(32768),
            viz_channels: VizBuffer::new(65536),
        }
    }

    /// Load a module from a file path.
    pub fn load_module(&mut self, path: &std::path::Path) -> anyhow::Result<()> {
        let data = std::fs::read(path)?;
        let module = crate::module::io::load_module_from_bytes(&data)?;
        let info = ModuleInfo::from_module(&module);

        let num_channels = module.get_num_channels();
        self.viz = Some(AudioViz::new(num_channels));

        self.module_data = Some(data);
        self.module = Some(module);
        self.module_info = Some(info);
        self.config.add_recent(path.to_path_buf());
        self.undo.clear();

        Ok(())
    }

    /// Start playback.
    pub fn play(&mut self) -> anyhow::Result<()> {
        self.stop();

        // Keep a display copy of the module for the UI during playback.
        // The original module moves into the audio thread and is returned on stop.
        if let Some(ref data) = self.module_data {
            if let Ok(display_copy) = crate::module::io::load_module_from_bytes(data) {
                self.module = Some(display_copy);
            }
        }

        let module = self.module.take().context("No module loaded")?;
        let module_data = self.module_data.clone().context("No module data")?;

        let viz_mix = self.viz_mix.clone();
        let viz_chan = self.viz_channels.clone();

        let engine = AudioEngine::start(module, module_data, 0, viz_mix, viz_chan)?;
        self.playback = engine.state.clone();
        self.audio = Some(engine);

        Ok(())
    }

    /// Stop playback and recover the Module.
    pub fn stop(&mut self) {
        if let Some(mut engine) = self.audio.take() {
            if let Some(module) = engine.stop() {
                self.module = Some(module);
            }
        }
    }

    /// Returns true if audio is currently playing.
    pub fn is_playing(&self) -> bool {
        self.audio.is_some()
    }
}
