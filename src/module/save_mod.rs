//! MOD (ProTracker) file writer.
//!
//! Converts an xmrs Module to Amiga ProTracker MOD binary format.
//! MOD constraints: 4 channels, 31 samples max, 64 rows per pattern,
//! Amiga period-based notes, 8-bit signed sample data.

use anyhow::Result;
use std::io::Write;
use xmrs::prelude::*;

/// Write a Module to MOD binary format.
///
/// Constraints enforced (with warnings):
/// - Max 4 channels (extra channels silently ignored)
/// - Max 31 samples
/// - Max 128 patterns in order list
/// - 64 rows per pattern
pub fn save_mod(module: &Module, output: &mut impl Write) -> Result<()> {
    let num_channels = module.get_num_channels().min(4);
    let order_entries = order_entries(module);
    let order_count = order_entries.len().min(128);

    // --- Round-trip rows-per-pattern: MOD always has 64 ---
    let rows_per_pat: usize = 64;

    // === HEADER ===

    // Title (20 bytes, null-padded)
    let title = &module.name;
    let mut title_bytes = [0u8; 20];
    let title_len = title.len().min(20);
    title_bytes[..title_len].copy_from_slice(&title.as_bytes()[..title_len]);
    output.write_all(&title_bytes)?;

    // Sample headers (30 bytes × 31)
    for i in 0..31 {
        write_sample_header(module, i, output)?;
    }

    // Song length (1 byte)
    output.write_all(&[order_count as u8])?;

    // Restart position / loop back to order (1 byte) — default 0x7F (no restart)
    let restart = module
        .song_loop_to
        .and_then(|tick| {
            // Map absolute tick back to order index
            module.timeline_map.entry_at_tick(0, tick).map(|e| e.order_idx as u8)
        })
        .unwrap_or(0x7F);
    output.write_all(&[restart])?;

    // Pattern order table (128 bytes)
    let mut order_table = [0u8; 128];
    for (i, entry) in order_entries.iter().take(128).enumerate() {
        order_table[i] = entry.pattern_idx as u8;
    }
    output.write_all(&order_table)?;

    // Format tag: we always emit a 4-channel module.
    output.write_all(b"M.K.")?;

    // === PATTERNS ===

    // Count patterns referenced in the order
    let num_patterns = order_entries
        .iter()
        .map(|entry| entry.pattern_idx as usize)
        .max()
        .map(|idx| idx + 1)
        .unwrap_or(1);

    for pat_idx in 0..num_patterns {
        write_pattern(module, pat_idx, num_channels, rows_per_pat, output)?;
    }

    // === SAMPLES ===
    for i in 0..31 {
        write_sample_data(module, i, output)?;
    }

    Ok(())
}

/// Write one sample header (30 bytes).
fn write_sample_header(module: &Module, idx: usize, output: &mut impl Write) -> Result<()> {
    // Find the sample if it exists
    let sample = find_sample(module, idx);

    // Name (22 bytes)
    let mut name = [0u8; 22];
    if let Some(s) = &sample {
        let sname = &s.name;
        let len = sname.len().min(22);
        name[..len].copy_from_slice(&sname.as_bytes()[..len]);
    }
    output.write_all(&name)?;

    if let Some(s) = sample {
        let sample_len = mod_sample_len_bytes(&s);

        // Length in words (16-bit samples)
        let len_words = (sample_len / 2).min(0xFFFF) as u16;
        output.write_all(&len_words.to_be_bytes())?;

        // Finetune (0-15, lower nibble) — simplified: 0 for now
        output.write_all(&[0x00])?;

        // Volume (0-64)
        output.write_all(&[s.volume.to_byte_64().min(64)])?;

        // Loop start in words
        let loop_start_words = ((s.loop_start as usize) / 2).min(0xFFFF) as u16;
        output.write_all(&loop_start_words.to_be_bytes())?;

        // Loop length in words (if loop is on, this is the additional loop length beyond the sample)
        let loop_len = if s.loop_flag != LoopType::No && s.loop_length > 1 {
            ((s.loop_length as usize) / 2).min(0xFFFF) as u16
        } else {
            0u16
        };
        output.write_all(&loop_len.to_be_bytes())?;
    } else {
        // Empty sample header
        output.write_all(&[0u8; 8])?;
    }

    Ok(())
}

/// Find a sample by scanning instruments.
fn find_sample(module: &Module, idx: usize) -> Option<Sample> {
    for instrument in &module.instrument {
        if let InstrumentType::Default(ref instr) = instrument.instr_type {
            if let Some(Some(sample)) = instr.sample.get(idx) {
                return Some(sample.clone());
            }
        }
    }
    None
}

/// Write one pattern (4 channels × 64 rows, 1024 bytes).
fn write_pattern(
    module: &Module,
    pat_idx: usize,
    num_channels: usize,
    rows: usize,
    output: &mut impl Write,
) -> Result<()> {
    let ch = num_channels.min(4);

    // Cache pattern position — called once per pattern, not per row
    let pattern_pos = crate::module::edit::get_pattern_position(module, 0, pat_idx);

    for row in 0..rows {
        for chan in 0..ch {
            let mut period: u16 = 0;
            let mut sample_number: u8 = 0;
            let mut effect_cmd: u8 = 0;
            let mut effect_param: u8 = 0;

            if let Some(ref pos) = pattern_pos {
                if let Some(track_idx) = pos.tracks.get(chan).and_then(|t| *t) {
                    let cell = crate::module::edit::get_cell(module, track_idx, row);

                    // Note → MOD Amiga period
                    if let Some(pitch) = cell.event.pitch() {
                        period = pitch_to_mod_period(pitch);
                    }

                    // Sample number (instrument column present → sample index + 1)
                    if cell.event.has_instrument_column() {
                        // MOD stores sample number as 1-based
                        // We use the track's instrument index to get the sample
                        if let Some(track) = module.tracks.get(track_idx as usize) {
                            let instr_idx = track.instrument();
                            if instr_idx < module.instrument.len() {
                                // For MOD, instrument index = sample number (1-based)
                                sample_number = (instr_idx as u8 + 1).min(31);
                            }
                        }
                    }

                    // Effects
                    if let Some(effect) = cell.effects.first() {
                        let (cmd, param) = track_effect_to_mod(effect);
                        effect_cmd = cmd;
                        effect_param = param;
                    }
                }
            }

            output.write_all(&pack_mod_event(
                sample_number,
                period,
                effect_cmd,
                effect_param,
            ))?;
        }

        for _ in ch..4 {
            output.write_all(&[0u8; 4])?;
        }
    }

    Ok(())
}

/// Write one sample's raw 8-bit signed PCM data.
fn write_sample_data(module: &Module, idx: usize, output: &mut impl Write) -> Result<()> {
    if let Some(sample) = find_sample(module, idx) {
        let bytes = sample_to_mod_bytes(&sample);
        output.write_all(&bytes)?;
    }
    Ok(())
}

fn order_entries(module: &Module) -> Vec<&TimelineEntry> {
    module
        .timeline_map
        .entries
        .iter()
        .filter(|e| e.song == 0 && e.loop_iter == 0 && e.row_idx == 0)
        .collect()
}

fn mod_sample_len_bytes(sample: &Sample) -> usize {
    sample_to_mod_bytes(sample).len()
}

fn sample_to_mod_bytes(sample: &Sample) -> Vec<u8> {
    let mut bytes = match &sample.data {
        Some(SampleDataType::Mono8(data)) => data.iter().map(|&s| s as u8).collect(),
        Some(SampleDataType::Mono16(data)) => data.iter().map(|&s| (s >> 8) as u8).collect(),
        Some(SampleDataType::Stereo8(data)) => data.iter().step_by(2).map(|&s| s as u8).collect(),
        Some(SampleDataType::Stereo16(data)) => {
            data.iter().step_by(2).map(|&s| (s >> 8) as u8).collect()
        }
        Some(SampleDataType::StereoFloat(data)) => data
            .iter()
            .step_by(2)
            .map(|&s| (s * 128.0).round().clamp(-128.0, 127.0) as i8 as u8)
            .collect(),
        None => Vec::new(),
    };

    let max_len = 0xFFFF * 2;
    if bytes.len() > max_len {
        bytes.truncate(max_len);
    }
    if bytes.len() % 2 != 0 {
        bytes.push(0);
    }

    bytes
}

fn pitch_to_mod_period(pitch: Pitch) -> u16 {
    xmrs::core::fixed::tables::amiga_period_from_pitch(PitchQ::from_semitone(pitch.value() as i16))
        .raw()
}

fn pack_mod_event(sample_number: u8, period: u16, effect_cmd: u8, effect_param: u8) -> [u8; 4] {
    let sample = sample_number.min(31);
    let period = period.min(0x0FFF);
    let effect = effect_cmd & 0x0F;

    [
        ((sample & 0xF0) >> 4) | ((period >> 8) as u8 & 0x0F),
        (period & 0xFF) as u8,
        ((sample & 0x0F) << 4) | effect,
        effect_param,
    ]
}

/// Convert an xmrs TrackEffect to MOD effect command and parameter bytes.
fn track_effect_to_mod(effect: &TrackEffect) -> (u8, u8) {
    match effect {
        TrackEffect::Arpeggio { half1, half2 } => {
            (0x0, ((half1 & 0x0F) << 4 | (half2 & 0x0F)) as u8)
        }
        TrackEffect::NoteCut { tick, .. } => {
            (0xE, 0xC0 | ((*tick as u8) & 0x0F))
        }
        TrackEffect::NoteDelay(delay) => {
            (0xE, 0xD0 | ((*delay as u8) & 0x0F))
        }
        TrackEffect::Volume { value, .. } => {
            (0xC, value.to_byte_64())
        }
        TrackEffect::NoteOff { tick, .. } => {
            (0xE, 0xC0 | ((*tick as u8) & 0x0F))
        }
        // MOD doesn't have many effects — most FT2 effects have no MOD equivalent
        _ => (0x00, 0x00),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_mod_packs_pattern_events_as_mod_cells() {
        let mut module = crate::module::create::create_empty_module(
            crate::module::create::NewModuleParams {
                channels: 4,
                patterns: 1,
                rows: 64,
                linear_freq: false,
                ..Default::default()
            },
        )
        .expect("create module");

        module
            .apply(xmrs::edit::EditCommand::SetCell {
                track: 0,
                row_offset: 0,
                content: Cell {
                    event: CellEvent::NoteOn {
                        pitch: Pitch::C4,
                        velocity: Volume::FULL,
                    },
                    effects: vec![TrackEffect::Arpeggio { half1: 1, half2: 2 }],
                },
            })
            .expect("set cell");

        let mut bytes = Vec::new();
        save_mod(&module, &mut bytes).expect("export mod");

        let pattern_offset = 1084;
        let period = pitch_to_mod_period(Pitch::C4);
        let expected = pack_mod_event(1, period, 0x0, 0x12);
        assert_eq!(&bytes[pattern_offset..pattern_offset + 4], &expected);
    }
}
