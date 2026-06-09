//! Menu bar rendering.

use crate::app::RustTracker;
use crate::ui::theme::{self, Theme};

impl RustTracker {
    pub fn render_menu_bar_app(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Module…").clicked() {
                    self.show_new_dialog = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Open Module…").clicked() {
                    let file = rfd::FileDialog::new()
                        .add_filter("Tracker Modules", &["xm", "mod", "s3m", "it"])
                        .add_filter("All Files", &["*"])
                        .pick_file();
                    if let Some(path) = file {
                        self.pending_file = Some(path);
                    }
                    ui.close_menu();
                }

                ui.separator();

                let recent_files = self.state.config.recent_files.clone();
                if !recent_files.is_empty() {
                    ui.label(egui::RichText::new("Recent:").size(12.0).weak());
                    for path in &recent_files {
                        let label = path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        if ui.button(label).clicked() {
                            self.pending_file = Some(path.clone());
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                }

                if ui.button("Save As MOD…").clicked() {
                    let default_name = self.state.module_info.as_ref()
                        .map(|i| format!("{}.mod", i.name))
                        .unwrap_or_else(|| "untitled.mod".to_string());
                    if let Some(path) = rfd::FileDialog::new()
                        .set_file_name(&default_name)
                        .add_filter("MOD Files", &["mod"])
                        .save_file()
                    {
                        match self.save_module_as_mod(&path) {
                            Ok(()) => self.status_message = Some(format!("Saved: {}", path.display())),
                            Err(e) => self.error_message = Some(format!("Save failed: {}", e)),
                        }
                    }
                    ui.close_menu();
                }

                if ui.button("Quit").clicked() {
                    self.quit_requested = true;
                    ui.close_menu();
                }
            });

            ui.menu_button("Module", |ui| {
                let loaded = self.state.module_info.is_some();

                if ui.add_enabled(loaded, egui::Button::new("Module Info")).clicked() {
                    self.active_view = crate::app::EditorView::Info;
                    ui.close_menu();
                }

                ui.separator();

                if ui.add_enabled(loaded && !self.state.is_playing(), egui::Button::new("▶ Play")).clicked() {
                    if let Err(e) = self.state.play() {
                        self.error_message = Some(format!("Playback error: {}", e));
                    }
                    ui.close_menu();
                }

                if ui.add_enabled(self.state.is_playing(), egui::Button::new("⏹ Stop")).clicked() {
                    self.state.stop();
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                ui.label("Switch to:");
                if ui.button("📋 Info (Ctrl+1)").clicked() {
                    self.active_view = crate::app::EditorView::Info;
                    ui.close_menu();
                }
                if ui.button("🎵 Patterns (Ctrl+2)").clicked() {
                    self.active_view = crate::app::EditorView::Pattern;
                    ui.close_menu();
                }
                if ui.button("🔊 Samples (Ctrl+3)").clicked() {
                    self.active_view = crate::app::EditorView::Samples;
                    ui.close_menu();
                }
                if ui.button("🎛 Instruments (Ctrl+4)").clicked() {
                    self.active_view = crate::app::EditorView::Instruments;
                    ui.close_menu();
                }
                if ui.button("💾 Disk (Ctrl+5)").clicked() {
                    self.active_view = crate::app::EditorView::DiskOp;
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("FT2 Classic Theme").clicked() {
                    theme::apply_ft2_classic(ui.ctx());
                    self.current_theme = Some(Theme::Ft2Classic);
                    ui.close_menu();
                }
                if ui.button("Modern Dark Theme").clicked() {
                    theme::apply_modern_dark(ui.ctx());
                    self.current_theme = Some(Theme::ModernDark);
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Reset Layout").clicked() {
                    ui.close_menu();
                }
            });

            ui.menu_button("Help", |ui| {
                if ui.button("Keyboard Shortcuts").clicked() {
                    self.show_help = true;
                    ui.close_menu();
                }
                if ui.button("About rust-tracker").clicked() {
                    self.status_message = Some(
                        "rust-tracker v0.1.0 — A modern Fast Tracker 2 clone for Linux".to_string(),
                    );
                    ui.close_menu();
                }
            });
        });
    }
}
