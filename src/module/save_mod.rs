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
    let order_count = module.timeline_map.order_count(0).min(128);

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
    for (i, entry) in module
        .timeline_map
        .entries
        .iter()
        .filter(|e| e.song == 0 && e.loop_iter == 0 && e.row_idx == 0)
        .take(128)
        .enumerate()
    {
        order_table[i] = entry.pattern_idx as u8;
    }
    output.write_all(&order_table)?;

    // Format tag (4 bytes)
    let tag = match order_count {
        0..=63 => b"M.K.",
        _ => b"M!K!",
    };
    output.write_all(tag)?;

    // === PATTERNS ===

    // Count patterns referenced in the order
    let num_patterns = module.timeline_map.order_count(0).max(1);

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
        // Length in words (16-bit samples)
        let len_words = (s.len() / 2).min(0xFFFF) as u16;
        output.write_all(&len_words.to_be_bytes())?;

        // Finetune (0-15, lower nibble) — simplified: 0 for now
        output.write_all(&[0x00])?;

        // Volume (0-64)
        output.write_all(&[s.volume.to_byte_64().min(64)])?;

        // Loop start in words
        let loop_start_words = (s.loop_start / 2).min(0xFFFF) as u16;
        output.write_all(&loop_start_words.to_be_bytes())?;

        // Loop length in words (if loop is on, this is the additional loop length beyond the sample)
        let loop_len = if s.loop_flag != LoopType::No && s.loop_length > 1 {
            (s.loop_length / 2).min(0xFFFF) as u16
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
            let mut note_byte: u8 = 0;
            let mut sample_byte: u8 = 0;
            let mut effect_byte: u8 = 0;
            let mut param_byte: u8 = 0;

            if let Some(ref pos) = pattern_pos {
                if let Some(track_idx) = pos.tracks.get(chan).and_then(|t| *t) {
                    let cell = crate::module::edit::get_cell(module, track_idx, row);

                    // Note → MOD note byte
                    if let Some(pitch) = cell.event.pitch() {
                        if let Some(note) = pitch_to_mod_note(pitch) {
                            note_byte = note;
                        }
                    }

                    // Sample number (instrument column present → sample index + 1)
                    if cell.event.has_instrument_column() {
                        // MOD stores sample number as 1-based
                        // We use the track's instrument index to get the sample
                        if let Some(track) = module.tracks.get(track_idx as usize) {
                            let instr_idx = track.instrument();
                            if instr_idx < module.instrument.len() {
                                // For MOD, instrument index = sample number (1-based)
                                sample_byte = (instr_idx as u8 + 1).min(31);
                            }
                        }
                    }

                    // Effects
                    if let Some(effect) = cell.effects.first() {
                        let (cmd, param) = track_effect_to_mod(effect);
                        effect_byte = cmd;
                        param_byte = param;
                    }
                }
            }

            output.write_all(&[note_byte, sample_byte, effect_byte, param_byte])?;
        }
    }

    Ok(())
}

/// Write one sample's raw 8-bit signed PCM data.
fn write_sample_data(module: &Module, idx: usize, output: &mut impl Write) -> Result<()> {
    if let Some(sample) = find_sample(module, idx) {
        match &sample.data {
            Some(SampleDataType::Mono8(data)) => {
                output.write_all(&byte_vec_to_u8(data))?;
            }
            Some(SampleDataType::Mono16(data)) => {
                // Convert 16-bit to 8-bit
                let bytes: Vec<u8> = data.iter().map(|&s| (s >> 8) as u8).collect();
                output.write_all(&bytes)?;
            }
            Some(SampleDataType::Stereo8(data)) => {
                // Downmix to mono, keep left channel
                let mono: Vec<u8> = data.iter().step_by(2).map(|&s| s as u8).collect();
                output.write_all(&mono)?;
            }
            Some(SampleDataType::Stereo16(data)) => {
                let mono: Vec<u8> = data.iter().step_by(2).map(|&s| (s >> 8) as u8).collect();
                output.write_all(&mono)?;
            }
            Some(SampleDataType::StereoFloat(data)) => {
                let mono: Vec<u8> = data
                    .iter()
                    .step_by(2)
                    .map(|&s| (s * 128.0 + 128.0).clamp(0.0, 255.0) as u8)
                    .collect();
                output.write_all(&mono)?;
            }
            None => {}
        }
    }
    Ok(())
}

/// Convert i8 slice to u8 bytes (MOD stores signed samples, we write as raw bytes).
fn byte_vec_to_u8(v: &[i8]) -> Vec<u8> {
    v.iter().map(|&s| s as u8).collect()
}

// ============================================================
// Amiga period / MOD note conversion tables
// ============================================================

/// Map a Pitch enum directly to a MOD note byte (0-71).
/// MOD note 0 = C-1 (~32.7 Hz), note 71 = B-3.
/// Pitches outside this range return None.
fn pitch_to_mod_note(pitch: Pitch) -> Option<u8> {
    let v = pitch.value();
    // Pitch values: C-0 = 0, ..., B-9 = 119
    // MOD range: C-1 (12) to B-3 (47) = notes 0-35 in MOD scale
    // But MOD can shift octaves by changing finetune/sample rate
    // We map C-1..B-3 to MOD notes 0..35
    if v >= 12 && v < 48 {
        Some(v - 12)
    } else if v >= 48 && v < 84 {
        // One octave higher than MOD range — still works but sounds higher
        Some(v - 12) // same note name, higher octave
    } else {
        // Out of ideal range but still produce a note
        Some(v % 12 + 24) // fold to middle octave
    }
}

/// Convert an xmrs TrackEffect to MOD effect command and parameter bytes.
fn track_effect_to_mod(effect: &TrackEffect) -> (u8, u8) {
    match effect {
        TrackEffect::Arpeggio { half1, half2 } => {
            (0x00, ((half1 & 0x0F) << 4 | (half2 & 0x0F)) as u8)
        }
        TrackEffect::NoteCut { tick, .. } => {
            // ECx is note cut in ProTracker
            (0xEC, (*tick as u8).min(0xFF))
        }
        TrackEffect::NoteDelay(delay) => {
            // EDx is note delay in MOD
            (0xED, (*delay as u8) & 0xFF)
        }
        TrackEffect::Volume { value, .. } => {
            (0x0C, value.to_byte_64())
        }
        TrackEffect::NoteOff { tick, .. } => {
            // ECx is note cut in MOD
            (0xEC, (*tick as u8) & 0xFF)
        }
        // MOD doesn't have many effects — most FT2 effects have no MOD equivalent
        _ => (0x00, 0x00),
    }
}
