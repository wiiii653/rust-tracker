//! Instrument editor — instrument list, keyboard/sample map,
//! volume/panning envelope graphs, vibrato settings, NNA/DCT.

use egui::{Color32, ScrollArea, Sense, Ui, Vec2};
use xmrs::prelude::*;

/// The instrument editor widget.
#[allow(dead_code)]
pub struct InstrEditor {
    /// Currently selected instrument index.
    pub current_instrument: usize,
    /// Whether the editor is visible.
    pub visible: bool,
}

impl InstrEditor {
    pub fn new() -> Self {
        Self {
            current_instrument: 0,
            visible: true,
        }
    }

    /// Render the instrument editor.
    pub fn show(&mut self, ui: &mut Ui, module: &Module) {
        // Instrument selector
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
        });

        ui.separator();

        if module.instrument.is_empty() {
            ui.label("No instruments in module.");
            return;
        }

        if self.current_instrument >= module.instrument.len() {
            self.current_instrument = 0;
        }

        let instrument = &module.instrument[self.current_instrument];

        // Name
        ui.horizontal(|ui| {
            ui.label("Name:");
            ui.label(&instrument.name);
            ui.label(format!("| Muted: {}", instrument.muted));
        });

        ui.separator();

        match &instrument.instr_type {
            InstrumentType::Default(instr) => {
                self.show_instr_default(ui, instr);
            }
            _ => {
                ui.label("Non-sample instrument — not editable yet.");
            }
        }
    }

    fn show_instr_default(&mut self, ui: &mut Ui, instr: &InstrDefault) {
        let sample_count = instr.sample.iter().filter(|s| s.is_some()).count();
        ui.label(format!("Samples: {} ({} slots)", sample_count, instr.sample.len()));

        ui.separator();

        ui.collapsing("Voice Setup", |ui| {
            self.show_voice_setup(ui, &instr.voice);
        });

        ui.collapsing("Volume Envelope", |ui| {
            show_envelope_graph(ui, &instr.voice.volume_envelope, "Volume", Color32::GREEN);
        });

        ui.collapsing("Panning Envelope", |ui| {
            show_envelope_graph(ui, &instr.voice.pan_envelope, "Panning", Color32::YELLOW);
        });

        ui.collapsing("Pitch Envelope", |ui| {
            show_envelope_graph(
                ui,
                &instr.voice.pitch_envelope,
                "Pitch",
                Color32::from_rgb(100, 150, 255),
            );
        });

        ui.collapsing("Vibrato", |ui| {
            show_vibrato(ui, &instr.voice.vibrato);
        });

        ui.collapsing("Behavior (NNA / DCT)", |ui| {
            show_behavior(ui, &instr.behavior);
        });

        ui.collapsing("Keyboard Map", |ui| {
            show_keyboard_map(ui, &instr.keyboard, &instr.sample);
        });
    }

    fn show_voice_setup(&mut self, ui: &mut Ui, voice: &VoiceSetup) {
        egui::Grid::new("voice_grid").striped(true).show(ui, |ui| {
            ui.label("Volume:");
            ui.label(format!("{}", voice.volume.to_byte_64()));
            ui.end_row();

            ui.label("Fadeout:");
            ui.label(format!("{}", voice.volume_fadeout.to_byte_64()));
            ui.end_row();

            ui.label("Default Pan:");
            ui.label(format!("{}", voice.default_pan.to_byte_64()));
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

/// Draw an envelope graph (standalone function, no self borrow).
fn show_envelope_graph(ui: &mut Ui, envelope: &Envelope, label: &str, color: Color32) {
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
    let (response, painter) = ui.allocate_painter(desired_size, Sense::hover());

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

    // Draw curve
    let mut last_pos: Option<egui::Pos2> = None;

    // Collect points for easier access
    let points = &envelope.point;

    for (i, p) in points.iter().enumerate() {
        let x = rect.left() + (p.frame as f32 / max_frame) * width;
        // EnvValue wraps Q15 in [-1, 1] or [0, 1]. Convert raw i16 to f32.
        let raw: f32 = p.value.raw_q15().raw() as f32 / 32768.0;
        // Map: raw in [-1,1] → bottom-to-top on the graph
        let y = rect.bottom() - (raw + 1.0) * 0.5 * height;
        let y = y.clamp(rect.top(), rect.bottom());

        let pos = egui::pos2(x, y);

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

    // Label
    painter.text(
        egui::pos2(rect.left() + 4.0, rect.top() + 2.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(10.0),
        Color32::LIGHT_GRAY,
    );
}

/// Show vibrato settings.
fn show_vibrato(ui: &mut Ui, vibrato: &Vibrato) {
    egui::Grid::new("vibrato_grid").striped(true).show(ui, |ui| {
        ui.label("Waveform:");
        ui.label(format!("{:?}", vibrato.waveform));
        ui.end_row();

        // Q8_8: raw i16 / 256.0
        ui.label("Speed:");
        ui.label(format!("{:.3}", vibrato.speed.raw() as f32 / 256.0));
        ui.end_row();

        ui.label("Depth:");
        ui.label(format!("{:.3} semitones", vibrato.depth.raw().raw() as f32 / 256.0));
        ui.end_row();

        ui.label("Sweep:");
        ui.label(format!("{:.3}", vibrato.sweep.raw() as f32 / 256.0));
        ui.end_row();
    });
}

/// Show NNA/DCT behavior.
fn show_behavior(ui: &mut Ui, behavior: &InstrumentBehavior) {
    ui.label(format!("Duplicate Check: {:?}", behavior.duplicate_check));
    ui.label("(NNA, DCT, DCA are encoded in DuplicateCheckType)");
}

/// Show keyboard map.
fn show_keyboard_map(ui: &mut Ui, keyboard: &Keyboard, samples: &[Option<Sample>]) {
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

                        let color = if sample_idx.is_some() {
                            Color32::LIGHT_GREEN
                        } else {
                            Color32::DARK_GRAY
                        };

                        ui.add_sized(
                            Vec2::new(46.0, 16.0),
                            egui::Label::new(
                                egui::RichText::new(text).size(9.0).color(color),
                            ),
                        );
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
