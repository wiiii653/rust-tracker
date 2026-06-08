//! Main application — integrates egui GUI with the audio engine.

pub mod dialogs;
pub mod editors;
pub mod menu;

use crate::module::create::NewModuleParams;
use crate::state::AppState;
use crate::ui::disk_op::DiskOp;
use crate::ui::instr_editor::InstrEditor;
use crate::ui::order_list::OrderList;
use crate::ui::pattern_editor::PatternEditor;
use crate::ui::sample_editor::SampleEditor;
use crate::ui::theme;
use egui::{CentralPanel, Color32, Key, SidePanel, TopBottomPanel};
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
    pub state: AppState,
    /// File to open on startup (from CLI argument).
    #[allow(dead_code)]
    startup_file: Option<PathBuf>,
    /// Error message to display (cleared after one frame).
    pub error_message: Option<String>,
    /// Status message to display (cleared after one frame).
    pub status_message: Option<String>,
    /// Pending file to open (from file dialog).
    pub pending_file: Option<PathBuf>,
    /// Pattern editor state.
    pub pattern_editor: PatternEditor,
    /// Order list state.
    pub order_list: OrderList,
    /// Sample editor state.
    pub sample_editor: SampleEditor,
    /// Instrument editor state.
    pub instr_editor: InstrEditor,
    /// Disk operations panel.
    pub disk_op: DiskOp,
    /// Currently active editor view.
    pub active_view: EditorView,
    /// Show help dialog.
    pub show_help: bool,
    /// Show new module dialog.
    pub show_new_dialog: bool,
    /// Parameters for new module dialog.
    pub new_module_params: NewModuleParams,
    /// Current theme (0 = not applied, 1 = FT2, 2 = modern, 255 = already applied).
    pub current_theme: u8,
    /// Window title for display.
    window_title: String,
}

impl RustTracker {
    pub fn new(startup_file: Option<PathBuf>) -> Self {
        let mut state = AppState::new();
        let mut status = None;
        let mut error = None;

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

        // Auto-open pattern view if a module was loaded
        let initial_view = if state.module.is_some() {
            EditorView::Pattern
        } else {
            EditorView::Info
        };

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
            active_view: initial_view,
            show_help: false,
            show_new_dialog: false,
            new_module_params: NewModuleParams::default(),
            current_theme: 0,
            window_title: "rust-tracker".to_string(),
        }
    }

    /// Called every frame by the egui integration.
    pub fn update(&mut self, ctx: &egui::Context) {
        // Apply theme on first frame only
        if self.current_theme == 0 {
            theme::apply_ft2_classic(ctx);
            self.current_theme = 255;
        }

        // Update window title
        let title = if let Some(ref info) = self.state.module_info {
            format!("rust-tracker — {}", info.name)
        } else {
            "rust-tracker".to_string()
        };
        if title != self.window_title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }

        self.handle_global_keys(ctx);

        // Update audio viz
        if let Some(ref mut viz) = self.state.viz {
            let mix_data = self.state.viz_mix.drain();
            if !mix_data.is_empty() { viz.feed_mix(&mix_data); }
            let chan_data = self.state.viz_channels.drain();
            if !chan_data.is_empty() { viz.feed_channels(&chan_data); }
        }

        // Process MIDI input
        if self.state.midi.enabled {
            for event in self.state.midi.poll() {
                if event.on && event.velocity > 0 {
                    // Convert MIDI note to internal pitch and enter it
                    if let Ok(pitch) = xmrs::prelude::Pitch::try_from(event.note) {
                        if let Some(ref mut module) = self.state.module {
                            if let Some(pat_pos) = crate::module::edit::get_pattern_position(
                                module,
                                self.pattern_editor.current_song,
                                self.pattern_editor.current_order,
                            ) {
                                if let Some(track_idx) = pat_pos
                                    .tracks
                                    .get(self.pattern_editor.current_channel)
                                    .and_then(|t| *t)
                                {
                                    let cell = xmrs::prelude::Cell {
                                        event: xmrs::prelude::CellEvent::NoteOn {
                                            pitch,
                                            velocity: xmrs::prelude::Volume::from_byte_64(
                                                (event.velocity as u8 / 2).min(64),
                                            ),
                                        },
                                        effects: vec![],
                                    };
                                    let cmd = xmrs::edit::EditCommand::SetCell {
                                        track: track_idx,
                                        row_offset: self.pattern_editor.current_row as u32,
                                        content: cell,
                                    };
                                    if let Err(e) = self.state.undo.execute(module, cmd) {
                                        self.error_message = Some(format!("MIDI edit: {}", e));
                                    } else {
                                        self.pattern_editor.current_row += 1;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // --- Menu bar ---
        TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            self.render_menu_bar_app(ui);
        });

        // --- Transport bar ---
        TopBottomPanel::top("transport_bar").show(ctx, |ui| {
            self.render_transport_app(ui);
        });

        // --- View tabs (persistent, always visible when module loaded) ---
        if self.state.module_info.is_some() {
            TopBottomPanel::top("view_tabs")
                .resizable(false)
                .show(ctx, |ui| {
                    self.render_view_tabs(ui);
                });
        }

        // --- Viz panel ---
        if self.state.is_playing() {
            TopBottomPanel::bottom("viz_panel")
                .resizable(true)
                .default_height(100.0)
                .min_height(60.0)
                .show(ctx, |ui| { self.render_viz_app(ui); });
        }

        // --- Central area ---
        if self.state.module_info.is_some() {
            match self.active_view {
                EditorView::Pattern => {
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
                    CentralPanel::default().show(ctx, |ui| { self.render_pattern_editor_app(ui); });
                }
                EditorView::Samples => {
                    CentralPanel::default().show(ctx, |ui| { self.render_sample_editor_app(ui); });
                }
                EditorView::Instruments => {
                    CentralPanel::default().show(ctx, |ui| { self.render_instr_editor_app(ui); });
                }
                EditorView::DiskOp => {
                    CentralPanel::default().show(ctx, |ui| { self.render_disk_op_app(ui); });
                }
                EditorView::Info => {
                    CentralPanel::default().show(ctx, |ui| { self.render_module_info_app(ui); });
                }
            }
        } else {
            CentralPanel::default().show(ctx, |ui| { self.render_empty_state_app(ui); });
        }

        // --- Status bar ---
        TopBottomPanel::bottom("status_bar").show(ctx, |ui| { self.render_status_bar(ui); });

        // --- Dialogs ---
        if self.show_help { self.render_help_dialog_app(ctx); }
        if self.show_new_dialog { self.render_new_module_dialog_app(ctx); }

        // --- Process pending file ---
        if let Some(path) = self.pending_file.take() {
            match self.state.load_module(&path) {
                Ok(()) => {
                    self.status_message = Some(format!("Loaded: {}", path.display()));
                    // Auto-switch to Pattern view so user sees the notes
                    self.active_view = EditorView::Pattern;
                    // Reset pattern editor cursor
                    self.pattern_editor.current_order = 0;
                    self.pattern_editor.current_row = 0;
                    self.pattern_editor.current_channel = 0;
                }
                Err(e) => self.error_message = Some(format!("Error loading {}: {}", path.display(), e)),
            }
        }

        if self.state.is_playing() { ctx.request_repaint(); }
    }

    fn handle_global_keys(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(Key::Z) && i.modifiers.ctrl && !i.modifiers.shift {
                if let Some(ref mut module) = self.state.module {
                    if let Err(e) = self.state.undo.undo(module) {
                        self.error_message = Some(format!("Undo failed: {}", e));
                    }
                }
            }
            if (i.key_pressed(Key::Y) && i.modifiers.ctrl)
                || (i.key_pressed(Key::Z) && i.modifiers.ctrl && i.modifiers.shift)
            {
                if let Some(ref mut module) = self.state.module {
                    if let Err(e) = self.state.undo.redo(module) {
                        self.error_message = Some(format!("Redo failed: {}", e));
                    }
                }
            }
            if i.key_pressed(Key::S) && i.modifiers.ctrl && !i.modifiers.shift {
                if self.state.module.is_some() {
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
                }
            }
            if i.key_pressed(Key::N) && i.modifiers.ctrl && !i.modifiers.shift {
                self.show_new_dialog = true;
            }

            // View switching: Ctrl+1..5
            if i.modifiers.ctrl && !i.modifiers.shift && !i.modifiers.alt {
                if i.key_pressed(Key::Num1) { self.active_view = EditorView::Info; }
                if i.key_pressed(Key::Num2) { self.active_view = EditorView::Pattern; }
                if i.key_pressed(Key::Num3) { self.active_view = EditorView::Samples; }
                if i.key_pressed(Key::Num4) { self.active_view = EditorView::Instruments; }
                if i.key_pressed(Key::Num5) { self.active_view = EditorView::DiskOp; }
            }

            if i.key_pressed(Key::F1) {
                self.show_help = !self.show_help;
            }
            // Escape: go back to Info view or close dialogs
            if i.key_pressed(Key::Escape) {
                if self.show_help || self.show_new_dialog {
                    self.show_help = false;
                    self.show_new_dialog = false;
                } else if self.active_view != EditorView::Info {
                    self.active_view = EditorView::Info;
                }
            }
            if i.key_pressed(Key::M) && i.modifiers.ctrl && !i.modifiers.shift {
                match self.state.midi.connect(None) {
                    Ok(()) => self.status_message = Some(format!(
                        "MIDI connected: {}", self.state.midi.device_name.as_deref().unwrap_or("unknown")
                    )),
                    Err(e) => self.error_message = Some(format!("MIDI: {}", e)),
                }
            }
        });
    }

    fn save_module_as_mod(&self, path: &std::path::Path) -> anyhow::Result<()> {
        if let Some(ref module) = self.state.module {
            let mut file = std::fs::File::create(path)?;
            crate::module::save_mod::save_mod(module, &mut file)?;
        }
        Ok(())
    }

    fn render_view_tabs(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().button_padding = egui::vec2(8.0, 2.0);
            for (label, shortcut, view) in &[
                ("📋 Info", "Ctrl+1", EditorView::Info),
                ("🎵 Patterns", "Ctrl+2", EditorView::Pattern),
                ("🔊 Samples", "Ctrl+3", EditorView::Samples),
                ("🎛 Instr.", "Ctrl+4", EditorView::Instruments),
                ("💾 Disk", "Ctrl+5", EditorView::DiskOp),
            ] {
                let selected = self.active_view == *view;
                let response = ui.selectable_label(selected, *label);
                if response.clicked() {
                    self.active_view = *view;
                }
                response.on_hover_text(format!("Switch to {} view ({})", label, shortcut));
            }
        });
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let Some(ref err) = self.error_message {
                ui.label(egui::RichText::new(err).size(12.0).color(Color32::RED));
            } else if let Some(ref status) = self.status_message {
                ui.label(egui::RichText::new(status).size(12.0).color(Color32::LIGHT_GREEN));
            } else {
                let mut parts = Vec::new();
                let undo_n = self.state.undo.undo_depth();
                if undo_n > 0 { parts.push(format!("Undo: {}", undo_n)); }
                if self.state.undo.can_redo() { parts.push("Redo available".to_string()); }
                if self.state.midi.enabled {
                    parts.push(format!("MIDI: {}", self.state.midi.device_name.as_deref().unwrap_or("on")));
                }
                let status = if parts.is_empty() { "Ready".to_string() } else { parts.join(" | ") };
                ui.label(egui::RichText::new(status).size(12.0).color(Color32::DARK_GRAY));
            }
        });
        self.error_message = None;
        self.status_message = None;
    }
}
