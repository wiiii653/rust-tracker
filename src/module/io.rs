#![allow(dead_code)]

use anyhow::{Context, Result};
use std::path::Path;
use xmrs::prelude::*;

/// Load a tracker module from disk, auto-detecting format.
pub fn load_module(path: &Path) -> Result<Module> {
    let data = std::fs::read(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    Module::load(&data).map_err(|e| anyhow::anyhow!("Failed to load module: {:?}", e))
}

/// Load a module from in-memory bytes.
pub fn load_module_from_bytes(data: &[u8]) -> Result<Module> {
    Module::load(data).map_err(|e| anyhow::anyhow!("Failed to load module: {:?}", e))
}

/// Info about a loaded module (extracted for display).
#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub channels: usize,
    pub instruments: usize,
    pub patterns: usize,
    pub orders: usize,
    pub bpm: usize,
    pub tempo: usize,
}

impl ModuleInfo {
    pub fn from_module(module: &Module) -> Self {
        let name = if module.name.is_empty() {
            "Untitled".to_string()
        } else {
            module.name.clone()
        };

        let channels = module.get_num_channels();
        let instruments = module.instrument.len();

        // Count patterns = number of tracks (each track has a pattern)
        let num_patterns = module.tracks.len();

        // Number of entries in the timeline map (≈ orders)
        let num_orders = module.timeline_map.entries.len();

        let bpm = module.default_bpm;
        let tempo = module.default_tempo;

        ModuleInfo {
            name,
            channels,
            instruments,
            patterns: num_patterns,
            orders: num_orders,
            bpm,
            tempo,
        }
    }
}
