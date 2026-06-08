//! Create new empty modules suitable for editing.

use anyhow::Result;
use xmrs::prelude::*;
use xmrs::core::daw::sorted_clips::SortedClips;
use xmrs::core::daw::clip::Clip;
use xmrs::core::daw::timeline::{TimelineEntry, TimelineMap};

/// Parameters for creating a new module.
#[derive(Debug, Clone)]
pub struct NewModuleParams {
    /// Module name.
    pub name: String,
    /// Number of channels.
    pub channels: usize,
    /// Number of patterns (order list length).
    pub patterns: usize,
    /// Rows per pattern.
    pub rows: usize,
    /// Initial BPM.
    pub bpm: usize,
    /// Initial tempo (ticks per row).
    pub tempo: usize,
    /// Frequency type.
    pub linear_freq: bool,
}

impl Default for NewModuleParams {
    fn default() -> Self {
        Self {
            name: "untitled".to_string(),
            channels: 8,
            patterns: 1,
            rows: 64,
            bpm: 125,
            tempo: 6,
            linear_freq: true,
        }
    }
}

/// Create a new empty module ready for editing.
pub fn create_empty_module(params: NewModuleParams) -> Result<Module> {
    let num_channels = params.channels.max(1).min(32);
    let num_patterns = params.patterns.max(1).min(256);
    let rows = params.rows.max(1).min(256);

    // Start from default and set known fields
    let mut module = Module::default();

    module.name = params.name;
    module.comment = "Created with rust-tracker".to_string();
    module.quirks = PlaybackQuirks::default();
    module.origin = None;
    module.frequency_type = if params.linear_freq {
        FrequencyType::LinearFrequencies
    } else {
        FrequencyType::AmigaFrequencies
    };
    module.default_tempo = params.tempo;
    module.default_bpm = params.bpm;
    module.channel_names = (0..num_channels)
        .map(|i| format!("Channel {}", i + 1))
        .collect();
    module.instrument = vec![Instrument {
        name: "Instrument 1".to_string(),
        instr_type: InstrumentType::Default(InstrDefault::default()),
        muted: false,
    }];
    module.pitch_wheel_depth = 2;
    module.mix_volume = Volume::FULL;
    module.mix_plugins = None;

    // Create tracks: one per channel per pattern
    let empty_cell = Cell::default();
    let empty_row: Vec<Cell> = vec![empty_cell; rows];

    let ticks_per_row = params.tempo as u32;
    let ticks_per_pattern = rows as u32 * ticks_per_row;

    let mut clips_vec: Vec<Clip> = Vec::new();

    for pat in 0..num_patterns {
        for ch in 0..num_channels {
            let track = Track::Notes {
                name: format!("Pattern {:02X} Ch {:02}", pat, ch),
                instrument: 0,
                rows: empty_row.clone(),
                muted: false,
            };
            module.tracks.push(track);
            let track_idx = module.tracks.len() as u32 - 1;

            clips_vec.push(Clip {
                track: track_idx,
                song: 0,
                target_channel: ch as u8,
                position_tick: pat as u32 * ticks_per_pattern,
                speed_at_start: params.tempo as u8,
                track_row_offset: 0,
                source_start_row: 0,
                end_tick: (pat as u32 + 1) * ticks_per_pattern,
            });
        }
    }

    module.clips = SortedClips::from_unsorted(clips_vec);

    // Build minimal timeline_map
    let mut entries = Vec::new();
    let mut tick: u32 = 0;
    for pat in 0..num_patterns {
        for row in 0..rows {
            entries.push(TimelineEntry {
                song: 0,
                order_idx: pat as u32,
                pattern_idx: pat as u32,
                row_idx: row as u32,
                loop_iter: 0,
                tick,
                speed_at_row: params.tempo as u8,
                bpm_at_row: params.bpm as u16,
            });
            tick += ticks_per_row;
        }
    }
    module.timeline_map = TimelineMap { entries };

    module.verify_layers_consistent()
        .map_err(|e| anyhow::anyhow!("Module consistency check failed: {:?}", e))?;

    Ok(module)
}
