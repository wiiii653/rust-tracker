//! Dialog windows: help and new module.

use crate::app::RustTracker;
use crate::module::create;
use crate::module::create::NewModuleParams;
use crate::module::io::ModuleInfo;
use egui::Window;

impl RustTracker {
    pub fn render_help_dialog_app(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }

        let mut show = true;
        Window::new("Keyboard Shortcuts — rust-tracker")
            .collapsible(true)
            .resizable(true)
            .default_size([520.0, 480.0])
            .show(ctx, |ui| {
                ui.heading("Keyboard Shortcuts");
                ui.separator();

                ui.collapsing("Transport", |ui| {
                    shortcuts_table(ui, &[
                        ("Play", "Ctrl+Enter"),
                        ("Stop", "Escape / Ctrl+Space"),
                    ]);
                });

                ui.collapsing("Pattern Editor — Navigation", |ui| {
                    shortcuts_table(ui, &[
                        ("Move cursor up/down", "↑ ↓"),
                        ("Move cursor left/right (columns)", "← →"),
                        ("Next/previous channel", "Tab / Shift+Tab"),
                        ("Go to row start", "Home"),
                        ("Go to first order", "Ctrl+Home"),
                        ("Go to last order", "Ctrl+End"),
                        ("Page up/down (16 rows)", "Page Up / Page Down"),
                    ]);
                });

                ui.collapsing("Pattern Editor — Note Entry (QWERTY)", |ui| {
                    ui.label("Lower octave (C-4):");
                    ui.monospace("  Z= C   S= C#  X= D   D= D#  C= E   V= F");
                    ui.monospace("  G= F#  B= G   H= G#  N= A   J= A#  M= B");
                    ui.label("Upper octave (C-5):");
                    ui.monospace("  Q= C   2= C#  W= D   3= D#  E= E   R= F");
                    ui.monospace("  5= F#  T= G   6= G#  Y= A   7= A#  U= B");
                    ui.label("Ctrl + note = raise one octave");
                });

                ui.collapsing("Pattern Editor — Editing", |ui| {
                    shortcuts_table(ui, &[
                        ("Delete note", "Delete / Backspace"),
                        ("Toggle edit mode", "Space"),
                        ("Next order", "F12"),
                        ("Previous order", "F11"),
                    ]);
                });

                ui.collapsing("Global", |ui| {
                    shortcuts_table(ui, &[
                        ("Undo", "Ctrl+Z"),
                        ("Redo", "Ctrl+Y / Ctrl+Shift+Z"),
                        ("Open file", "Ctrl+O"),
                        ("Save", "Ctrl+S"),
                        ("Connect MIDI", "Ctrl+M"),
                        ("Toggle help", "F1"),
                        ("Quit", "Ctrl+Q"),
                    ]);
                });

                ui.collapsing("MIDI", |ui| {
                    ui.label("MIDI notes are mapped directly to the tracker keyboard.");
                    ui.label("Note On → enters note and advances cursor.");
                    ui.label("Note Off → stops note.");
                    ui.label("Connect via Ctrl+M or Help → Connect MIDI.");
                });

                ui.separator();
                if ui.button("Close").clicked() {
                    show = false;
                }
            });
        self.show_help = show;
    }

    pub fn render_new_module_dialog_app(&mut self, ctx: &egui::Context) {
        let mut show = true;
        Window::new("New Module")
            .collapsible(false)
            .resizable(false)
            .default_size([350.0, 420.0])
            .show(ctx, |ui| {
                ui.heading("Create New Module");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("Name:");
                    ui.text_edit_singleline(&mut self.new_module_params.name);
                });

                ui.add(egui::Slider::new(&mut self.new_module_params.channels, 1..=32)
                    .text("Channels")
                    .step_by(2.0));

                ui.add(egui::Slider::new(&mut self.new_module_params.patterns, 1..=128)
                    .text("Patterns")
                    .step_by(1.0));

                ui.add(egui::Slider::new(&mut self.new_module_params.rows, 16..=256)
                    .text("Rows per pattern")
                    .step_by(16.0));

                ui.add(egui::Slider::new(&mut self.new_module_params.bpm, 32..=255)
                    .text("BPM")
                    .step_by(1.0));

                ui.add(egui::Slider::new(&mut self.new_module_params.tempo, 1..=32)
                    .text("Tempo (ticks/row)")
                    .step_by(1.0));

                ui.checkbox(&mut self.new_module_params.linear_freq, "Linear frequencies");

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("MOD Preset").clicked() {
                        self.new_module_params = NewModuleParams {
                            name: "untitled".to_string(),
                            channels: 4,
                            patterns: 1,
                            rows: 64,
                            bpm: 125,
                            tempo: 6,
                            linear_freq: false,
                        };
                    }
                    if ui.button("XM Preset").clicked() {
                        self.new_module_params = NewModuleParams {
                            name: "untitled".to_string(),
                            channels: 8,
                            patterns: 1,
                            rows: 64,
                            bpm: 125,
                            tempo: 6,
                            linear_freq: true,
                        };
                    }
                });

                ui.separator();

                ui.horizontal(|ui| {
                    if ui.button("Create").clicked() {
                        match create::create_empty_module(self.new_module_params.clone()) {
                            Ok(module) => {
                                let info = ModuleInfo::from_module(&module);
                                self.state.module = Some(module);
                                self.state.module_info = Some(info);
                                self.state.module_data = Some(Vec::new());
                                self.state.undo.clear();
                                self.status_message = Some(format!(
                                    "Created: {} ({} ch, {} patterns)",
                                    self.new_module_params.name,
                                    self.new_module_params.channels,
                                    self.new_module_params.patterns
                                ));
                                show = false;
                            }
                            Err(e) => {
                                self.error_message = Some(format!("Failed: {}", e));
                            }
                        }
                    }
                    if ui.button("Cancel").clicked() {
                        show = false;
                    }
                });
            });
        self.show_new_dialog = show;
    }
}

/// Helper: render a table of keyboard shortcuts.
fn shortcuts_table(ui: &mut egui::Ui, entries: &[(&str, &str)]) {
    egui::Grid::new("shortcuts_grid")
        .striped(true)
        .show(ui, |ui| {
            for (action, shortcut) in entries {
                ui.label(*action);
                ui.monospace(*shortcut);
                ui.end_row();
            }
        });
}
