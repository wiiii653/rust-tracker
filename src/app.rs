//! Main application — integrates egui GUI with the audio engine.

#![allow(dead_code)]

use crate::module::create::{self, NewModuleParams};
use crate::state::AppState;
use crate::ui::disk_op::DiskOp;
use crate::ui::instr_editor::InstrEditor;
use crate::ui::order_list::OrderList;
use crate::ui::pattern_editor::PatternEditor;
use crate::ui::sample_editor::SampleEditor;
use crate::ui::theme;
use crate::ui::transport::TransportBar;
use egui::{CentralPanel, Color32, Frame, Key, SidePanel, TopBottomPanel, Window};
use log::info;
use std::path::PathBuf;

/// Which editor view is currently active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorView {
    Info,
    Pattern,
    Samples,
    Instruments,
    DiskOp,
}

/// The top-level application struct.
pub struct RustTracker {
    state: AppState,
    /// File to open on startup (from CLI argument).
    startup_file: Option<PathBuf>,
    /// Error message to display (cleared after one frame).
    error_message: Option<String>,
    /// Status message to display (cleared after one frame).
    status_message: Option<String>,
    /// Pending file to open (from file dialog).
    pending_file: Option<PathBuf>,
    /// Pattern editor state.
    pattern_editor: PatternEditor,
    /// Order list state.
    order_list: OrderList,
    /// Sample editor state.
    sample_editor: SampleEditor,
    /// Instrument editor state.
    instr_editor: InstrEditor,
    /// Disk operations panel.
    disk_op: DiskOp,
    /// Currently active editor view.
    active_view: EditorView,
    /// Show help dialog.
    show_help: bool,
    /// Show new module dialog.
    show_new_dialog: bool,
    /// Parameters for new module dialog.
    new_module_params: NewModuleParams,
    /// Current theme (0 = FT2 classic, 1 = modern dark).
    current_theme: u8,
    /// Window title for display.
    window_title: String,
}

impl RustTracker {
    pub fn new(startup_file: Option<PathBuf>) -> Self {
        let mut state = AppState::new();
        let mut status = None;
        let mut error = None;

        // Load startup file if provided
        if let Some(ref path) = startup_file {
            match state.load_module(path) {
                Ok(()) => {
                    status = Some(format!("Loaded: {}", path.display()));
                    info!("Loaded module: {}", path.display());
                }
                Err(e) => {
                    error = Some(format!("Failed to load {}: {}", path.display(), e));
                    log::error!("{}", error.as_ref().unwrap());
                }
            }
        }

        Self {
            state,
            startup_file,
            error_message: error,
            status_message: status,
            pending_file: None,
            pattern_editor: PatternEditor::new(),
            order_list: OrderList::new(),
            sample_editor: SampleEditor::new(),
            instr_editor: InstrEditor::new(),
            disk_op: DiskOp::new(),
            active_view: EditorView::Info,
            show_help: false,
            show_new_dialog: false,
            new_module_params: NewModuleParams::default(),
            current_theme: 0,
            window_title: "rust-tracker".to_string(),
        }
    }

    /// Called every frame by the egui integration.
    pub fn update(&mut self, ctx: &egui::Context) {
        // Apply theme on first frame
        if self.current_theme == 0 {
            theme::apply_ft2_classic(ctx);
        }

        // Update window title based on loaded module
        let title = if let Some(ref info) = self.state.module_info {
            format!("rust-tracker — {}", info.name)
        } else {
            "rust-tracker".to_string()
        };
        if title != self.window_title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        // Handle global keyboard shortcuts
        self.handle_global_keys(ctx);

        // Update audio viz
        if let Some(ref mut viz) = self.state.viz {
            let mix_data = self.state.viz_mix.drain();
            if !mix_data.is_empty() {
                viz.feed_mix(&mix_data);
            }
            let chan_data = self.state.viz_channels.drain();
            if !chan_data.is_empty() {
                viz.feed_channels(&chan_data);
            }
        }
        // --- Top menu bar ---
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar(ui);
        });

        // --- Transport bar ---
        TopBottomPanel::top("transport_bar").show(ctx, |ui| {
            self.render_transport(ui);
        });

        // --- Viz panel (oscilloscope) ---
        if self.state.is_playing() {
            TopBottomPanel::bottom("viz_panel")
                .resizable(true)
                .default_height(100.0)
                .min_height(60.0)
                .show(ctx, |ui| {
                    self.render_viz(ui);
                });
        }

        // --- Central area ---
        if self.state.module_info.is_some() {
            match self.active_view {
                EditorView::Pattern => {
                    // Side panel for order list
                    SidePanel::left("order_panel")
                        .resizable(true)
                        .default_width(200.0)
                        .min_width(100.0)
                        .show(ctx, |ui| {
                            if let Some(ref module) = self.state.module {
                                if let Some(new_order) = self.order_list.show(ui, module) {
                                    self.pattern_editor.current_order = new_order;
                                }
                            }
                        });

                    CentralPanel::default().show(ctx, |ui| {
                        self.render_pattern_editor(ui);
                    });
                }
                EditorView::Samples => {
                    CentralPanel::default().show(ctx, |ui| {
                        self.render_sample_editor(ui);
                    });
                }
                EditorView::Instruments => {
                    CentralPanel::default().show(ctx, |ui| {
                        self.render_instr_editor(ui);
                    });
                }
                EditorView::DiskOp => {
                    CentralPanel::default().show(ctx, |ui| {
                        self.render_disk_op(ui);
                    });
                }
                EditorView::Info => {
                    CentralPanel::default().show(ctx, |ui| {
                        self.render_module_info(ui);
                    });
                }
            }
        } else {
            CentralPanel::default().show(ctx, |ui| {
                self.render_empty_state(ui);
            });
        }

        // --- Bottom status bar ---
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            self.render_status_bar(ui);
        });

        // --- Help dialog ---
        if self.show_help {
            self.render_help_dialog(ctx);
        }

        // --- New module dialog ---
        if self.show_new_dialog {
            self.render_new_module_dialog(ctx);
        }

        // --- Process pending file ---
        if let Some(path) = self.pending_file.take() {
            match self.state.load_module(&path) {
                Ok(()) => {
                    self.status_message = Some(format!("Loaded: {}", path.display()));
                }
                Err(e) => {
                    self.error_message = Some(format!("Error loading {}: {}", path.display(), e));
                }
            }
        }

        // Request repaint while playing (for position updates)
        if self.state.is_playing() {
            ctx.request_repaint();
        }

        // Clear transient messages after showing them
        // (they were set above and will be shown this frame, then cleared next)
    }

    fn render_menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New Module…").clicked() {
                    self.show_new_dialog = true;
                    ui.close_menu();
                }

                ui.separator();

                if ui.button("Open Module…").clicked() {
                    // Open file dialog (blocking — acceptable for Phase 1)
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

                // Recent files
                let recent_files = self.state.config.recent_files.clone();
                if !recent_files.is_empty() {
                    ui.label(egui::RichText::new("Recent:").size(12.0).weak());
                    for path in &recent_files {
                        let label = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| path.to_string_lossy().to_string());
                        if ui.button(label).clicked() {
                            self.pending_file = Some(path.clone());
                            ui.close_menu();
                        }
                    }
                    ui.separator();
                }

                if ui.button("Quit").clicked() {
                    std::process::exit(0);
                }
            });

            ui.menu_button("Module", |ui| {
                let loaded = self.state.module_info.is_some();

                if ui.add_enabled(loaded, egui::Button::new("Module Info")).clicked() {
                    self.active_view = EditorView::Info;
                    ui.close_menu();
                }

                ui.separator();

                if ui
                    .add_enabled(loaded && !self.state.is_playing(), egui::Button::new("▶ Play"))
                    .clicked()
                {
                    if let Err(e) = self.state.play() {
                        self.error_message = Some(format!("Playback error: {}", e));
                    }
                    ui.close_menu();
                }

                if ui
                    .add_enabled(self.state.is_playing(), egui::Button::new("⏹ Stop"))
                    .clicked()
                {
                    self.state.stop();
                    ui.close_menu();
                }
            });

            ui.menu_button("View", |ui| {
                if ui.button("FT2 Classic Theme").clicked() {
                    theme::apply_ft2_classic(ui.ctx());
                    self.current_theme = 0;
                    ui.close_menu();
                }
                if ui.button("Modern Dark Theme").clicked() {
                    theme::apply_modern_dark(ui.ctx());
                    self.current_theme = 1;
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

    fn render_transport(&mut self, ui: &mut egui::Ui) {
        let is_playing = self.state.is_playing();
        let is_loaded = self.state.module_info.is_some();
        let playback = self.state.playback.lock().unwrap().clone();

        // We need separate closures that don't both borrow self.state.
        // Use local flags to signal actions after rendering.
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

    fn render_pattern_editor(&mut self, ui: &mut egui::Ui) {
        if let Some(ref module) = self.state.module {
            // Sync pattern editor with playback position when playing
            if self.state.is_playing() {
                let playback = self.state.playback.lock().unwrap();
                self.pattern_editor.current_row = playback.current_row;
                // Note: we'd need to map playing_pattern to order_idx
            }

            self.pattern_editor.show(ui, module);
        } else {
            ui.label("No module loaded for editing.");
        }
    }

    fn render_module_info(&mut self, ui: &mut egui::Ui) {
        Frame::central_panel(&egui::Style::default()).show(ui, |ui| {
            if let Some(ref info) = self.state.module_info {
                ui.heading(&info.name);
                ui.separator();

                // Module info grid
                egui::Grid::new("module_info").striped(true).show(ui, |ui| {
                    ui.label("Channels:");
                    ui.label(format!("{}", info.channels));
                    ui.end_row();

                    ui.label("Instruments:");
                    ui.label(format!("{}", info.instruments));
                    ui.end_row();

                    ui.label("Patterns:");
                    ui.label(format!("{}", info.patterns));
                    ui.end_row();

                    ui.label("Orders:");
                    ui.label(format!("{}", info.orders));
                    ui.end_row();

                    ui.label("BPM:");
                    ui.label(format!("{}", info.bpm));
                    ui.end_row();

                    ui.label("Tempo:");
                    ui.label(format!("{}", info.tempo));
                    ui.end_row();
                });

                ui.add_space(16.0);

                // Playback status
                if self.state.is_playing() {
                    let playback = self.state.playback.lock().unwrap();
                    let total_secs = playback.elapsed_samples as f64 / 44100.0;
                    ui.label(
                        egui::RichText::new(format!(
                            "▶ Playing…  Pattern: {:02X}  Row: {:02X}  Time: {:02}:{:02}",
                            playback.current_pattern,
                            playback.current_row,
                            total_secs as u64 / 60,
                            total_secs as u64 % 60
                        ))
                        .color(Color32::LIGHT_GREEN),
                    );
                } else {
                    ui.label("Ready — click ▶ to play.");
                }

                ui.add_space(16.0);

                // Button to open pattern editor
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(
                            self.active_view == EditorView::Info,
                            "📋 Info",
                        )
                        .clicked()
                    {
                        self.active_view = EditorView::Info;
                    }
                    if ui
                        .selectable_label(
                            self.active_view == EditorView::Pattern,
                            "🎵 Patterns",
                        )
                        .clicked()
                    {
                        self.active_view = EditorView::Pattern;
                    }
                    if ui
                        .selectable_label(
                            self.active_view == EditorView::Samples,
                            "🔊 Samples",
                        )
                        .clicked()
                    {
                        self.active_view = EditorView::Samples;
                    }
                    if ui
                        .selectable_label(
                            self.active_view == EditorView::Instruments,
                            "🎛 Instruments",
                        )
                        .clicked()
                    {
                        self.active_view = EditorView::Instruments;
                    }
                    if ui
                        .selectable_label(
                            self.active_view == EditorView::DiskOp,
                            "💾 Disk",
                        )
                        .clicked()
                    {
                        self.active_view = EditorView::DiskOp;
                    }
                });
            }
        });
    }

    fn render_empty_state(&mut self, ui: &mut egui::Ui) {
        ui.centered_and_justified(|ui| {
            ui.label(
                egui::RichText::new(
                    "No module loaded\n\nFile → Open Module… or drag & drop an .xm/.mod/.s3m/.it file",
                )
                .size(16.0)
                .color(Color32::DARK_GRAY),
            );
        });
    }

    fn render_sample_editor(&mut self, ui: &mut egui::Ui) {
        if let Some(ref module) = self.state.module {
            self.sample_editor.show(ui, module);
        } else {
            ui.label("No module loaded.");
        }
    }

    fn render_instr_editor(&mut self, ui: &mut egui::Ui) {
        if let Some(ref module) = self.state.module {
            self.instr_editor.show(ui, module);
        } else {
            ui.label("No module loaded.");
        }
    }

    fn render_disk_op(&mut self, ui: &mut egui::Ui) {
        self.disk_op.show(ui);

        // Handle pending loads from disk op
        if let Some(path) = self.disk_op.pending_load.take() {
            self.pending_file = Some(path);
        }
    }

    fn render_viz(&mut self, ui: &mut egui::Ui) {
        if let Some(ref viz) = self.state.viz {
            viz.render_oscilloscope(ui, 60.0);
            ui.separator();
            viz.render_vu_meters(ui);
        }
    }

    fn render_help_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_help {
            return;
        }

        let mut show = true;
        egui::Window::new("Keyboard Shortcuts — rust-tracker")
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

    fn render_new_module_dialog(&mut self, ctx: &egui::Context) {
        let mut show = true;
        egui::Window::new("New Module")
            .collapsible(false)
            .resizable(false)
            .default_size([350.0, 370.0])
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
                    if ui.button("Create").clicked() {
                        match create::create_empty_module(self.new_module_params.clone()) {
                            Ok(module) => {
                                let info = crate::module::io::ModuleInfo::from_module(&module);
                                self.state.module = Some(module);
                                self.state.module_info = Some(info);
                                self.state.module_data = Some(Vec::new()); // empty data
                                self.state.undo.clear();
                                self.status_message = Some(
                                    format!("Created: {} ({} ch, {} patterns)",
                                        self.new_module_params.name,
                                        self.new_module_params.channels,
                                        self.new_module_params.patterns)
                                );
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

    fn handle_global_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Undo: Ctrl+Z
            if i.key_pressed(Key::Z) && i.modifiers.ctrl && !i.modifiers.shift {
                if let Some(ref mut module) = self.state.module {
                    if let Err(e) = self.state.undo.undo(module) {
                        self.error_message = Some(format!("Undo failed: {}", e));
                    }
                }
            }
            // Redo: Ctrl+Y or Ctrl+Shift+Z
            if (i.key_pressed(Key::Y) && i.modifiers.ctrl)
                || (i.key_pressed(Key::Z) && i.modifiers.ctrl && i.modifiers.shift)
            {
                if let Some(ref mut module) = self.state.module {
                    if let Err(e) = self.state.undo.redo(module) {
                        self.error_message = Some(format!("Redo failed: {}", e));
                    }
                }
            }
            // Save: Ctrl+S
            if i.key_pressed(Key::S) && i.modifiers.ctrl && !i.modifiers.shift {
                if self.state.module_data.is_some() {
                    // For now, just print status
                    self.status_message = Some("Module data in memory (save to XM not yet implemented)".to_string());
                    // TODO: implement Module::save_xm
                }
            }
            // New: Ctrl+N
            if i.key_pressed(Key::N) && i.modifiers.ctrl && !i.modifiers.shift {
                self.show_new_dialog = true;
            }
            // Help: F1
            if i.key_pressed(Key::F1) {
                self.show_help = !self.show_help;
            }
            if i.key_pressed(Key::M) && i.modifiers.ctrl && !i.modifiers.shift {
                match self.state.midi.connect(None) {
                    Ok(()) => {
                        self.status_message = Some(format!(
                            "MIDI connected: {}",
                            self.state.midi.device_name.as_deref().unwrap_or("unknown")
                        ));
                    }
                    Err(e) => {
                        self.error_message = Some(format!("MIDI: {}", e));
                    }
                }
            }
        });
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(ref err) = self.error_message {
                ui.label(
                    egui::RichText::new(err)
                        .size(12.0)
                        .color(Color32::RED),
                );
            } else if let Some(ref status) = self.status_message {
                ui.label(
                    egui::RichText::new(status)
                        .size(12.0)
                        .color(Color32::LIGHT_GREEN),
                );
            } else {
                // Show undo depth and MIDI status
                let mut parts = Vec::new();
                let undo_n = self.state.undo.undo_depth();
                if undo_n > 0 {
                    parts.push(format!("Undo: {} steps", undo_n));
                }
                if self.state.midi.enabled {
                    parts.push(format!(
                        "MIDI: {}",
                        self.state.midi.device_name.as_deref().unwrap_or("on")
                    ));
                }
                let status = if parts.is_empty() {
                    "Ready".to_string()
                } else {
                    parts.join(" | ")
                };
                ui.label(
                    egui::RichText::new(status)
                        .size(12.0)
                        .color(Color32::DARK_GRAY),
                );
            }
        });

        // Clear messages after displaying them for one frame
        self.error_message = None;
        self.status_message = None;
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
