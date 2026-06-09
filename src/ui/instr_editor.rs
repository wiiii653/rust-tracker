//! Instrument editor — instrument list, keyboard/sample map,
//! volume/panning envelope graphs, vibrato settings, NNA/DCT.

use egui::{Color32, ScrollArea, Sense, Ui, Vec2};
use xmrs::core::fixed::prelude::EnvValue;
use xmrs::prelude::*;

/// The instrument editor widget.
#[allow(dead_code)]
pub struct InstrEditor {
    /// Currently selected instrument index.
    pub current_instrument: usize,
    /// Whether the editor is visible.
    pub visible: bool,
    /// Sample index to assign when clicking keys in the keyboard map.
    pub assign_sample: Option<usize>,
}

impl InstrEditor {
    pub fn new() -> Self {
        Self {
            current_instrument: 0,
            visible: true,
            assign_sample: Some(0),
        }
    }

    /// Render the instrument editor. Returns true if the module was modified.
    pub fn show(&mut self, ui: &mut Ui, module: &mut Module) -> bool {
        let mut module_changed = false;

        // Instrument selector with add/remove buttons
        ui.horizontal(|ui| {
            ui.label("Instrument:");
            let instruments: Vec<String> = module
                .instrument
                .iter()
                .enumerate()
                .map(|(i, inst)| {
                    if inst.name.is_empty() {
                        format!("Instrument {:02X}", i)
                    } else {
                        format!("{:02X}: {}", i, inst.name)
                    }
                })
                .collect();

            let current_name = instruments
                .get(self.current_instrument)
                .cloned()
                .unwrap_or_else(|| "None".to_string());

            egui::ComboBox::from_id_salt("instr_edit_select")
                .selected_text(&current_name)
                .show_ui(ui, |ui| {
                    for (i, name) in instruments.iter().enumerate() {
                        if ui.selectable_label(i == self.current_instrument, name).clicked() {
                            self.current_instrument = i;
                        }
                    }
                });

            if ui.button("+ Inst.").clicked() {
                module.instrument.push(Instrument {
                    name: format!("Instrument {:02X}", module.instrument.len()),
                    instr_type: InstrumentType::Default(InstrDefault::default()),
                    muted: false,
                });
                self.current_instrument = module.instrument.len() - 1;
                module_changed = true;
            }
            if ui.button("− Inst.").clicked() && module.instrument.len() > 1 {
                if self.current_instrument < module.instrument.len() {
                    module.instrument.remove(self.current_instrument);
                }
                if self.current_instrument >= module.instrument.len() {
                    self.current_instrument = module.instrument.len().saturating_sub(1);
                }
                module_changed = true;
            }
        });

        ui.separator();

        if module.instrument.is_empty() {
            ui.label("No instruments in module.");
            return false;
        }

        if self.current_instrument >= module.instrument.len() {
            self.current_instrument = 0;
        }

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.text_edit_singleline(&mut module.instrument[self.current_instrument].name);
            let mut muted = module.instrument[self.current_instrument].muted;
            if ui.checkbox(&mut muted, "Muted").changed() {
                module.instrument[self.current_instrument].muted = muted;
                module_changed = true;
            }
        });

        ui.separator();

        match &mut module.instrument[self.current_instrument].instr_type {
            InstrumentType::Default(ref mut instr) => {
                self.show_instr_default(ui, instr);
            }
            _ => {
                ui.label("Non-sample instrument — not editable yet.");
            }
        }

        module_changed
    }

    fn show_instr_default(&mut self, ui: &mut Ui, instr: &mut InstrDefault) {
        let sample_count = instr.sample.iter().filter(|s| s.is_some()).count();
        ui.label(format!("Samples: {} ({} slots)", sample_count, instr.sample.len()));

        ui.separator();

        ui.collapsing("Voice Setup", |ui| {
            self.show_voice_setup(ui, &mut instr.voice);
        });

        ui.collapsing("Volume Envelope", |ui| {
            show_envelope_graph(ui, &mut instr.voice.volume_envelope, "Volume", Color32::GREEN);
        });

        ui.collapsing("Panning Envelope", |ui| {
            show_envelope_graph(ui, &mut instr.voice.pan_envelope, "Panning", Color32::YELLOW);
        });

        ui.collapsing("Pitch Envelope", |ui| {
            show_envelope_graph(
                ui,
                &mut instr.voice.pitch_envelope,
                "Pitch",
                Color32::from_rgb(100, 150, 255),
            );
        });

        ui.collapsing("Vibrato", |ui| {
            show_vibrato(ui, &mut instr.voice.vibrato);
        });

        ui.collapsing("Behavior (NNA / DCT)", |ui| {
            show_behavior(ui, &instr.behavior);
        });

        ui.collapsing("Keyboard Map", |ui| {
            show_keyboard_map(ui, &mut instr.keyboard, &instr.sample, &mut self.assign_sample);
        });
    }

    fn show_voice_setup(&mut self, ui: &mut Ui, voice: &mut VoiceSetup) {
        egui::Grid::new("voice_grid").striped(true).show(ui, |ui| {
            ui.label("Volume:");
            let mut vol = voice.volume.to_byte_64();
            if ui.add(egui::DragValue::new(&mut vol).range(0..=64).speed(1)).changed() {
                voice.volume = Volume::from_byte_64(vol);
            }
            ui.end_row();

            ui.label("Fadeout:");
            let mut fade = voice.volume_fadeout.to_byte_64();
            if ui.add(egui::DragValue::new(&mut fade).range(0..=64).speed(1)).changed() {
                voice.volume_fadeout = Volume::from_byte_64(fade);
            }
            ui.end_row();

            ui.label("Default Pan:");
            let mut pan = voice.default_pan.to_byte_64();
            if ui.add(egui::DragValue::new(&mut pan).range(0..=64).speed(1)).changed() {
                voice.default_pan = Panning::from_byte_64(pan);
            }
            ui.end_row();

            ui.label("Pitch-Pan Center:");
            ui.label(format!("{:?}", voice.pitch_pan_center));
            ui.end_row();

            ui.label("Filter Cutoff:");
            ui.label(format!("{}", voice.initial_filter_cutoff));
            ui.end_row();

            ui.label("Filter Resonance:");
            ui.label(format!("{}", voice.initial_filter_resonance));
            ui.end_row();
        });
    }
}

/// Draw an envelope graph with draggable points.
fn show_envelope_graph(ui: &mut Ui, envelope: &mut Envelope, label: &str, color: Color32) {
    if !envelope.enabled {
        ui.label(format!("{} envelope: disabled", label));
        return;
    }

    ui.label(format!(
        "{} points | Sustain: {} | Loop: {}",
        envelope.point.len(),
        if envelope.sustain_enabled { "ON" } else { "off" },
        if envelope.loop_enabled { "ON" } else { "off" }
    ));

    if envelope.point.len() < 2 {
        ui.label("  (need ≥ 2 points)");
        return;
    }

    let desired_size = Vec2::new(ui.available_width(), 120.0);
    let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());

    let rect = response.rect;
    let width = rect.width();
    let height = rect.height();

    // Background
    painter.rect_filled(rect, 0.0, Color32::from_rgb(22, 22, 32));

    // Grid
    for i in 1..4 {
        let y = rect.top() + height * i as f32 / 4.0;
        painter.line_segment(
            [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
            egui::Stroke::new(1.0, Color32::from_rgb(32, 32, 42)),
        );
    }

    // Find max frame for X scaling
    let max_frame = envelope
        .point
        .iter()
        .map(|p| p.frame)
        .max()
        .unwrap_or(1)
        .max(1) as f32;

    // Helper to get point screen positions
    let point_pos = |p: &EnvelopePoint| -> egui::Pos2 {
        let x = rect.left() + (p.frame as f32 / max_frame) * width;
        let raw: f32 = p.value.raw_q15().raw() as f32 / 32768.0;
        let y = rect.bottom() - (raw + 1.0) * 0.5 * height;
        egui::pos2(x, y.clamp(rect.top(), rect.bottom()))
    };

    // --- Drag handling ---
    if response.dragged() {
        if let Some(pointer) = response.hover_pos() {
            // Find nearest point
            let mut nearest_idx = 0usize;
            let mut nearest_dist = f32::MAX;
            for (i, p) in envelope.point.iter().enumerate() {
                let pos = point_pos(p);
                let dist = pos.distance(pointer);
                if dist < nearest_dist {
                    nearest_dist = dist;
                    nearest_idx = i;
                }
            }
            if nearest_dist < 18.0 && !envelope.point.is_empty() {
                let new_frame = ((pointer.x - rect.left()) / width * max_frame)
                    .round()
                    .max(0.0) as usize;
                // Keep points in order: clamp between neighbors
                let min_frame = if nearest_idx > 0 {
                    envelope.point[nearest_idx - 1].frame + 1
                } else {
                    0
                };
                let max_frame = if nearest_idx + 1 < envelope.point.len() {
                    envelope.point[nearest_idx + 1].frame.saturating_sub(1)
                } else {
                    usize::MAX
                };
                let clamped_frame = new_frame.clamp(min_frame, max_frame);

                let fraction_from_top = (pointer.y - rect.top()) / height;
                let raw = (1.0 - 2.0 * fraction_from_top).clamp(-1.0, 1.0);
                let raw_i16 = (raw * 32767.0) as i16;

                envelope.point[nearest_idx].frame = clamped_frame;
                envelope.point[nearest_idx].value =
                    EnvValue::from_q15_i16(raw_i16);
            }
        }
    }

    // --- Right-click to delete a point ---
    if response.secondary_clicked() {
        if let Some(pointer) = response.hover_pos() {
            let mut nearest_idx = None;
            let mut nearest_dist = f32::MAX;
            for (i, p) in envelope.point.iter().enumerate() {
                let pos = point_pos(p);
                let dist = pos.distance(pointer);
                if dist < nearest_dist {
                    nearest_dist = dist;
                    nearest_idx = Some(i);
                }
            }
            if let Some(idx) = nearest_idx {
                if nearest_dist < 18.0 && envelope.point.len() > 2 {
                    envelope.point.remove(idx);
                    // Adjust sustain/loop indices
                    if envelope.sustain_enabled {
                        if idx < envelope.sustain_start_point {
                            envelope.sustain_start_point =
                                envelope.sustain_start_point.saturating_sub(1);
                        }
                        if idx < envelope.sustain_end_point {
                            envelope.sustain_end_point =
                                envelope.sustain_end_point.saturating_sub(1);
                        }
                    }
                    if envelope.loop_enabled {
                        if idx < envelope.loop_start_point {
                            envelope.loop_start_point =
                                envelope.loop_start_point.saturating_sub(1);
                        }
                        if idx < envelope.loop_end_point {
                            envelope.loop_end_point =
                                envelope.loop_end_point.saturating_sub(1);
                        }
                    }
                }
            }
        }
    }

    // --- Double-click to add a point ---
    if response.double_clicked() {
        if let Some(pointer) = response.hover_pos() {
            let new_frame = ((pointer.x - rect.left()) / width * max_frame)
                .round()
                .max(0.0) as usize;
            let fraction_from_top = (pointer.y - rect.top()) / height;
            let raw = (1.0 - 2.0 * fraction_from_top).clamp(-1.0, 1.0);
            let raw_i16 = (raw * 32767.0) as i16;

            let new_point = EnvelopePoint {
                frame: new_frame,
                value: EnvValue::from_q15_i16(raw_i16),
            };

            // Insert at the right position to keep sorted order
            let insert_idx = envelope
                .point
                .iter()
                .position(|p| p.frame > new_frame)
                .unwrap_or(envelope.point.len());
            envelope.point.insert(insert_idx, new_point);

            // Adjust sustain/loop indices
            if envelope.sustain_enabled {
                if insert_idx <= envelope.sustain_start_point {
                    envelope.sustain_start_point += 1;
                }
                if insert_idx <= envelope.sustain_end_point {
                    envelope.sustain_end_point += 1;
                }
            }
            if envelope.loop_enabled {
                if insert_idx <= envelope.loop_start_point {
                    envelope.loop_start_point += 1;
                }
                if insert_idx <= envelope.loop_end_point {
                    envelope.loop_end_point += 1;
                }
            }
        }
    }

    // --- Draw curve ---
    let points = &envelope.point;
    let mut last_pos: Option<egui::Pos2> = None;

    for (i, p) in points.iter().enumerate() {
        let pos = point_pos(p);

        // Point dot
        painter.circle_filled(pos, 3.5, color);

        // Line segment
        if let Some(last) = last_pos {
            painter.line_segment([last, pos], egui::Stroke::new(1.5, color));
        }

        // Sustain markers
        if envelope.sustain_enabled {
            if i == envelope.sustain_start_point {
                painter.circle_stroke(pos, 6.0, egui::Stroke::new(2.0, Color32::YELLOW));
            }
            if i == envelope.sustain_end_point {
                painter.circle_stroke(pos, 6.0, egui::Stroke::new(2.0, Color32::RED));
            }
        }

        // Loop markers
        if envelope.loop_enabled {
            if i == envelope.loop_start_point {
                painter.circle_stroke(pos, 6.0, egui::Stroke::new(2.0, Color32::LIGHT_BLUE));
            }
            if i == envelope.loop_end_point {
                painter.circle_stroke(
                    pos,
                    6.0,
                    egui::Stroke::new(2.0, Color32::from_rgb(255, 100, 255)),
                );
            }
        }

        last_pos = Some(pos);
    }

    // Highlight nearest point on hover
    if let Some(pointer) = response.hover_pos() {
        let mut nearest_pos = None;
        let mut nearest_dist = 18.0f32;
        for p in envelope.point.iter() {
            let pos = point_pos(p);
            let dist = pos.distance(pointer);
            if dist < nearest_dist {
                nearest_dist = dist;
                nearest_pos = Some(pos);
            }
        }
        if let Some(pos) = nearest_pos {
            painter.circle_stroke(pos, 7.0, egui::Stroke::new(1.5, Color32::WHITE));
        }
    }

    // Label
    painter.text(
        egui::pos2(rect.left() + 4.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(10.0),
        Color32::LIGHT_GRAY,
    );
}

/// Show vibrato settings (editable).
fn show_vibrato(ui: &mut Ui, vibrato: &mut Vibrato) {
    egui::Grid::new("vibrato_grid").striped(true).show(ui, |ui| {
        ui.label("Waveform:");
        egui::ComboBox::from_id_salt("vibrato_waveform")
            .selected_text(format!("{:?}", vibrato.waveform))
            .show_ui(ui, |ui| {
                // VibratoWaveform variants — show all available
                for wf in &["Sine", "Square", "SawUp", "SawDown", "Triangle", "Noise"] {
                    if ui.selectable_label(false, *wf).clicked() {
                        // Can't easily convert string back to enum; keep display-only for now
                    }
                }
            });
        ui.end_row();

        // Q8_8: raw i16 / 256.0
        ui.label("Speed:");
        let mut speed_raw = vibrato.speed.raw();
        let mut speed_f32 = speed_raw as f32 / 256.0;
        if ui.add(egui::DragValue::new(&mut speed_f32).range(0.0..=255.996).speed(0.1)).changed() {
            speed_raw = (speed_f32 * 256.0).round().clamp(-32768.0, 32767.0) as i16;
            vibrato.speed = xmrs::core::fixed::prelude::Q8_8::from_raw(speed_raw);
        }
        ui.end_row();

        ui.label("Depth:");
        ui.label(format!("{:.3} semitones", vibrato.depth.raw().raw() as f32 / 256.0));
        ui.end_row();

        ui.label("Sweep:");
        let mut sweep_raw = vibrato.sweep.raw();
        let mut sweep_f32 = sweep_raw as f32 / 256.0;
        if ui.add(egui::DragValue::new(&mut sweep_f32).range(0.0..=255.996).speed(0.1)).changed() {
            sweep_raw = (sweep_f32 * 256.0).round().clamp(-32768.0, 32767.0) as i16;
            vibrato.sweep = xmrs::core::fixed::prelude::Q8_8::from_raw(sweep_raw);
        }
        ui.end_row();
    });
}

/// Show NNA/DCT behavior.
fn show_behavior(ui: &mut Ui, behavior: &InstrumentBehavior) {
    ui.label(format!("Duplicate Check: {:?}", behavior.duplicate_check));
    ui.label("(NNA, DCT, DCA are encoded in DuplicateCheckType)");
}

/// Show keyboard map with interactive note assignment.
fn show_keyboard_map(
    ui: &mut Ui,
    keyboard: &mut Keyboard,
    samples: &[Option<Sample>],
    assign_sample: &mut Option<usize>,
) {
    let sample_names: Vec<String> = samples
        .iter()
        .enumerate()
        .map(|(i, s)| {
            s.as_ref()
                .map(|s| {
                    if s.name.is_empty() {
                        format!("{:02X}", i)
                    } else {
                        s.name.clone()
                    }
                })
                .unwrap_or_else(|| "--".to_string())
        })
        .collect();

    // Sample selector for assignment
    if !sample_names.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Assign:");
            for (i, name) in sample_names.iter().enumerate() {
                let selected = *assign_sample == Some(i);
                if ui.selectable_label(selected, name).clicked() {
                    *assign_sample = Some(i);
                }
            }
            if ui.selectable_label(assign_sample.is_none(), "(none)").clicked() {
                *assign_sample = None;
            }
        });
    }

    let note_names = [
        "C-", "C#", "D-", "D#", "E-", "F-", "F#", "G-", "G#", "A-", "A#", "B-",
    ];

    // Piano-roll style display
    ScrollArea::vertical()
        .max_height(300.0)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            for octave in (0..=9).rev() {
                ui.horizontal(|ui| {
                    ui.label(format!("O{}", octave));
                    for note in 0..12 {
                        let midi_note = octave * 12 + note;
                        let sample_idx = keyboard.sample_for_pitch[midi_note];
                        let transpose = keyboard.note_for_pitch[midi_note];

                        let text = match (sample_idx, transpose) {
                            (Some(si), Some(tr)) => {
                                let sn = sample_names.get(si).map(|s| s.as_str()).unwrap_or("??");
                                format!("{}{}:{}", note_names[note], sn, tr as i8 - midi_note as i8)
                            }
                            (Some(si), None) => {
                                let sn = sample_names.get(si).map(|s| s.as_str()).unwrap_or("??");
                                format!("{}{}", note_names[note], sn)
                            }
                            (None, _) => format!("{}-", note_names[note]),
                        };

                        let is_assigned = sample_idx.is_some();
                        let color = if is_assigned {
                            Color32::LIGHT_GREEN
                        } else {
                            Color32::DARK_GRAY
                        };

                        let response = ui.add_sized(
                            Vec2::new(46.0, 16.0),
                            egui::SelectableLabel::new(is_assigned, egui::RichText::new(text).size(9.0).color(color)),
                        );

                        // Left-click: assign selected sample
                        if response.clicked() {
                            if let Some(si) = *assign_sample {
                                keyboard.sample_for_pitch[midi_note] = Some(si);
                            }
                        }
                        // Right-click: clear assignment
                        if response.secondary_clicked() {
                            keyboard.sample_for_pitch[midi_note] = None;
                            keyboard.note_for_pitch[midi_note] = None;
                        }
                        response.on_hover_text("Click to assign sample, right-click to clear");
                    }
                });
            }
        });

    ui.separator();

    // Sample list summary
    ui.label("Sample assignments:");
    for (i, name) in sample_names.iter().enumerate() {
        // Find notes mapped to this sample
        let notes: Vec<usize> = (0..120)
            .filter(|&n| keyboard.sample_for_pitch[n] == Some(i))
            .collect();

        let range = if notes.is_empty() {
            "unmapped".to_string()
        } else if notes.len() == 1 {
            let p = Pitch::try_from(notes[0] as u8).unwrap_or(Pitch::C4);
            format!("{:?}", p)
        } else {
            let first = Pitch::try_from(notes[0] as u8).unwrap_or(Pitch::C4);
            let last = Pitch::try_from(*notes.last().unwrap() as u8).unwrap_or(Pitch::C4);
            format!("{:?} → {:?}", first, last)
        };

        ui.label(format!("  {}: {}", name, range));
    }
}
