//! Pattern-editing operations built on xmrs's `EditCommand` API.
//!
//! Note: xmrs separates continuous effects (portamento, vibrato, volume slide,
//! etc.) into `AutomationLane`s rather than storing them on `Cell.effects`.
//! Only discrete/trigger effects appear in `Cell.effects`.

use xmrs::prelude::*;

/// Information about one pattern position in the order list.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PatternPosition {
    /// Index into `Module.tracks` for each channel, in channel order.
    /// `tracks[channel] = Some(track_index)`
    pub tracks: Vec<Option<u32>>,
    /// Number of rows in this pattern (max across all channels).
    pub num_rows: usize,
    /// Absolute tick at which this pattern starts.
    pub start_tick: u32,
    /// The order index for this pattern.
    pub order_idx: usize,
}

/// Reconstruct the pattern layout for a given order position.
pub fn get_pattern_position(
    module: &Module,
    song: usize,
    order_idx: usize,
) -> Option<PatternPosition> {
    let entry = module.timeline_map.find_order_entry(song, order_idx)?;
    let start_tick = entry.tick;
    let num_channels = module.get_num_channels();

    let next_order_tick = module
        .timeline_map
        .find_order_entry(song, order_idx + 1)
        .map(|e| e.tick)
        .unwrap_or(u32::MAX);

    let mut tracks: Vec<Option<u32>> = vec![None; num_channels];

    for (_clip_idx, clip) in module.clips.iter().enumerate() {
        if clip.song as usize == song
            && clip.position_tick >= start_tick
            && clip.position_tick < next_order_tick
        {
            let ch = clip.target_channel as usize;
            if ch < num_channels {
                tracks[ch] = Some(clip.track);
            }
        }
    }

    let mut num_rows = 0;
    for track_idx in tracks.iter().flatten() {
        if let Some(track) = module.tracks.get(*track_idx as usize) {
            num_rows = num_rows.max(track.natural_length());
        }
    }
    if num_rows == 0 {
        num_rows = 64;
    }

    Some(PatternPosition {
        tracks,
        num_rows,
        start_tick,
        order_idx,
    })
}

/// Get the Cell at a specific (track, row) position, or default empty cell.
pub fn get_cell(module: &Module, track_idx: u32, row: usize) -> Cell {
    module
        .tracks
        .get(track_idx as usize)
        .map(|t| t.cell_at(row as u32))
        .unwrap_or_default()
}

/// Format a Pitch as a note string like "C-4", "F#3", etc.
pub fn pitch_to_string(pitch: Pitch) -> String {
    let note_names = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];
    let value = pitch.value();
    let octave = value / 12;
    let note = (value % 12) as usize;
    format!("{}{}", note_names[note], octave)
}

/// Format a Cell's note column for display.
pub fn cell_note_string(cell: &Cell) -> String {
    match cell.event {
        CellEvent::None | CellEvent::InstrReset => "---".to_string(),
        CellEvent::NoteOn { pitch, .. } | CellEvent::NoteOnGhost { pitch, .. } => {
            pitch_to_string(pitch)
        }
        CellEvent::NoteOff { .. } => "===".to_string(),
        CellEvent::NoteCut => "^^^".to_string(),
        CellEvent::NoteFade => "~~~".to_string(),
    }
}

/// Format the instrument/volume column for display.
pub fn cell_instr_vol_string(cell: &Cell, default_instrument: Option<usize>) -> String {
    let instr = if cell.event.has_instrument_column() {
        default_instrument
            .map(|i| format!("{:02X}", i))
            .unwrap_or("..".to_string())
    } else {
        "..".to_string()
    };
    let vol = if cell.event.velocity() != Volume::FULL {
        format!("{:02X}", cell.event.velocity().to_byte_64())
    } else {
        "..".to_string()
    };
    format!("{} {}", instr, vol)
}

/// Format effects for display.
pub fn cell_effects_string(cell: &Cell) -> String {
    if cell.effects.is_empty() {
        return "... .. ... ..".to_string();
    }
    let mut parts = Vec::new();
    for effect in cell.effects.iter().take(2) {
        parts.push(effect_to_string(effect));
    }
    while parts.len() < 2 {
        parts.push("... ..".to_string());
    }
    parts.join(" ")
}

/// Format a single TrackEffect as a string.
fn effect_to_string(effect: &TrackEffect) -> String {
    match effect {
        TrackEffect::Arpeggio { half1, half2 } => format!("0{:X}{:X}", half1, half2),
        TrackEffect::NoteDelay(delay) => format!("D{:02X}", delay),
        TrackEffect::NoteCut { tick, .. } => format!("S{:02X}C", tick),
        TrackEffect::NoteOff { tick, .. } => format!("K{:02X}", tick),
        TrackEffect::NoteFadeOut { tick, .. } => format!("S{:02X}F", tick),
        TrackEffect::Volume { value, tick } => format!("C{:02X}T{:02X}", value.to_byte_64(), tick),
        TrackEffect::ChannelVolume(vol) => format!("M{:02X}", vol.to_byte_64()),
        TrackEffect::Panning(pan) => format!("8{:02X}", pan.to_byte_64()),
        TrackEffect::NoteRetrig {
            speed,
            volume_modifier: _, // NoteRetrigOperator enum — skip for now
        } => format!("R{:X}..", speed),
        TrackEffect::Glissando(on) => {
            if *on {
                "Gxx On".to_string()
            } else {
                "Gxx Off".to_string()
            }
        }
        TrackEffect::Tremor {
            on_time,
            off_time,
        } => format!("T{:X}{:X}", on_time, off_time),
        TrackEffect::InstrumentFineTune(_tune) => "E5x".to_string(),
        _ => "... ..".to_string(),
    }
}

/// Convert an effect string like "A04" back to a TrackEffect (simplified).
#[allow(dead_code)]
pub fn parse_effect(s: &str) -> Option<TrackEffect> {
    if s.len() < 3 {
        return None;
    }
    let chars: Vec<char> = s.chars().collect();
    let cmd = chars[0].to_ascii_uppercase();
    let x1 = chars.get(1).and_then(|c| c.to_digit(16))? as usize;
    let x2 = chars.get(2).and_then(|c| c.to_digit(16))? as usize;

    match cmd {
        '0' => Some(TrackEffect::Arpeggio {
            half1: x1,
            half2: x2,
        }),
        'D' => Some(TrackEffect::NoteDelay(x1 * 16 + x2)),
        'K' => Some(TrackEffect::NoteOff {
            tick: x1 * 16 + x2,
            past: false,
        }),
        'C' => Some(TrackEffect::Volume {
            value: Volume::from_byte_64((x1 * 16 + x2) as u8),
            tick: 0,
        }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::module::create::{create_empty_module, NewModuleParams};

    #[test]
    fn test_get_pattern_position_created_module() {
        let params = NewModuleParams {
            channels: 4,
            patterns: 2,
            rows: 64,
            ..Default::default()
        };
        let module = create_empty_module(params).expect("Failed to create module");

        // Check pattern 0
        let pos = get_pattern_position(&module, 0, 0);
        assert!(pos.is_some(), "Pattern 0 should exist");
        let pos = pos.unwrap();
        assert_eq!(pos.tracks.len(), 4, "Should have 4 channels");
        assert!(pos.tracks.iter().all(|t| t.is_some()), "All channels should have tracks");
        assert!(pos.num_rows > 0, "Should have rows");

        // Check pattern 1
        let pos1 = get_pattern_position(&module, 0, 1);
        assert!(pos1.is_some(), "Pattern 1 should exist");
    }

    #[test]
    fn test_cell_note_display() {
        let cell = Cell::default();
        assert_eq!(cell_note_string(&cell), "---");

        let cell = Cell {
            event: CellEvent::NoteOn {
                pitch: Pitch::C4,
                velocity: Volume::FULL,
            },
            effects: vec![],
        };
        assert_eq!(cell_note_string(&cell), "C-4");
    }

    #[test]
    fn test_effect_formatting() {
        let cell = Cell {
            event: CellEvent::default(),
            effects: vec![TrackEffect::Arpeggio { half1: 4, half2: 7 }],
        };
        let fx = cell_effects_string(&cell);
        assert!(fx.contains("047"), "Arpeggio should format as 047, got: {}", fx);
    }
}
