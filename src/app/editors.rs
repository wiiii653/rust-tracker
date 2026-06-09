//! Editor view rendering methods.

use crate::app::RustTracker;
use crate::ui::transport::TransportBar;
use egui::{Color32, Frame};

impl RustTracker {
    pub fn render_transport_app(&mut self, ui: &mut egui::Ui) {
        let is_playing = self.state.is_playing();
        let is_loaded = self.state.module_info.is_some();
        let playback = self.state.playback.lock().unwrap().clone();

        let mut want_play = false;
        let mut want_stop = false;

        TransportBar::show(
            ui,
            is_playing,
            is_loaded,
            &playback,
            &mut || { want_play = true; },
            &mut || { want_stop = true; },
        );

        if want_play {
            if let Err(e) = self.state.play() {
                self.error_message = Some(format!("Playback error: {}", e));
            }
        }
        if want_stop {
            self.state.stop();
        }
    }

    pub fn render_pattern_editor_app(&mut self, ui: &mut egui::Ui) {
        if let Some(ref module) = self.state.module {
            if self.state.is_playing() {
                let playback = self.state.playback.lock().unwrap();
                self.pattern_editor.current_row = playback.current_row;
                self.pattern_editor.current_order = playback.current_order;
            }
            let commands = self.pattern_editor.show(ui, module);
            // Apply edit commands via undo manager
            for cmd in commands {
                let live_cmd = cmd.clone();
                let result = if let Some(ref mut module) = self.state.module {
                    self.state.undo.execute(module, cmd)
                } else {
                    Ok(())
                };
                if let Err(e) = result {
                    self.error_message = Some(format!("Edit failed: {}", e));
                } else {
                    self.state.bump_module_revision();
                    if let Err(e) = self.state.apply_live_edit(live_cmd) {
                        self.error_message = Some(format!("Playback sync failed: {}", e));
                    }
                }
            }
        } else {
            ui.label("No module loaded for editing.");
        }
    }

    pub fn render_module_info_app(&mut self, ui: &mut egui::Ui) {
        Frame::none().show(ui, |ui| {
            if let Some(ref info) = self.state.module_info {
                ui.heading(&info.name);
                ui.separator();

                egui::Grid::new("module_info").striped(true).show(ui, |ui| {
                    ui.label("Channels:"); ui.label(format!("{}", info.channels)); ui.end_row();
                    ui.label("Instruments:"); ui.label(format!("{}", info.instruments)); ui.end_row();
                    ui.label("Patterns:"); ui.label(format!("{}", info.patterns)); ui.end_row();
                    ui.label("Orders:"); ui.label(format!("{}", info.orders)); ui.end_row();
                    ui.label("BPM:"); ui.label(format!("{}", info.bpm)); ui.end_row();
                    ui.label("Tempo:"); ui.label(format!("{}", info.tempo)); ui.end_row();
                });

                ui.add_space(16.0);

                if self.state.is_playing() {
                    let playback = self.state.playback.lock().unwrap();
                    let sr = if playback.sample_rate > 0 { playback.sample_rate as f64 } else { 44100.0 };
                    let total_secs = playback.elapsed_samples as f64 / sr;
                    ui.label(egui::RichText::new(format!(
                        "▶ Playing…  Pattern: {:02X}  Row: {:02X}  Time: {:02}:{:02}",
                        playback.current_pattern, playback.current_row,
                        total_secs as u64 / 60, total_secs as u64 % 60
                    )).color(Color32::LIGHT_GREEN));
                } else {
                    ui.label("Ready — click ▶ to play.");
                }
            }
        });
    }

    pub fn render_empty_state_app(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.label(egui::RichText::new(
                "No module loaded\n\nFile → New Module… or File → Open Module…",
            ).size(16.0).color(Color32::DARK_GRAY));
        });
    }

    pub fn render_sample_editor_app(&mut self, ui: &mut egui::Ui) {
        if let Some(ref mut module) = self.state.module {
            let changed = self.sample_editor.show(ui, module);
            if changed {
                self.state.bump_module_revision();
                if self.state.is_playing() {
                    if let Err(e) = self.state.sync_playback_module() {
                        self.error_message = Some(format!("Playback sync failed: {}", e));
                    }
                }
            }
        } else {
            ui.label("No module loaded.");
        }
    }

    pub fn render_instr_editor_app(&mut self, ui: &mut egui::Ui) {
        if let Some(ref mut module) = self.state.module {
            let changed = self.instr_editor.show(ui, module);
            if changed {
                self.state.bump_module_revision();
                if self.state.is_playing() {
                    if let Err(e) = self.state.sync_playback_module() {
                        self.error_message = Some(format!("Playback sync failed: {}", e));
                    }
                }
            }
        } else {
            ui.label("No module loaded.");
        }
    }

    pub fn render_disk_op_app(&mut self, ui: &mut egui::Ui) {
        self.disk_op.show(ui);
        if let Some(path) = self.disk_op.pending_load.take() {
            self.pending_file = Some(path);
        }
    }

    pub fn render_viz_app(&mut self, ui: &mut egui::Ui) {
        if let Some(ref viz) = self.state.viz {
            viz.render_oscilloscope(ui, 60.0);
            ui.separator();
            viz.render_vu_meters(ui);
        }
    }
}
