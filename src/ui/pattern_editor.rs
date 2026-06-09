//! Pattern editor — scrollable grid showing channels × rows with
//! note, instrument, volume, and effect columns.
//!
//! FT2-style QWERTY keyboard input for note entry, cursor navigation,
//! and basic editing.

use crate::module::edit;
use egui::{Color32, Key, ScrollArea, Sense, Ui, Vec2};
use xmrs::edit::EditCommand;
use xmrs::prelude::*;

/// Which column the cursor is in within a cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorColumn {
    Note,
    Instrument,
    Volume,
    Effect(usize), // 0, 1, 2, 3 (effect number)
}

impl Default for CursorColumn {
    fn default() -> Self {
        CursorColumn::Note
    }
}

/// The pattern editor widget.
#[allow(dead_code)]
pub struct PatternEditor {
    /// Current song index.
    pub current_song: usize,
    /// Current order position.
    pub current_order: usize,
    /// Current row within the pattern.
    pub current_row: usize,
    /// Current channel (0-based).
    pub current_channel: usize,
    /// Cursor column within the cell.
    pub cursor_column: CursorColumn,
    /// If true, the pattern grid is being displayed.
    pub visible: bool,
    /// Scroll position for the row view.
    pub scroll_y: f32,
    /// Pending note/effect value being entered (for multi-key sequences).
    pub input_buffer: String,
    /// Whether we're in edit mode (inserting notes).
    pub edit_mode: bool,
}

impl PatternEditor {
    pub fn new() -> Self {
        Self {
            current_song: 0,
            current_order: 0,
            current_row: 0,
            current_channel: 0,
            cursor_column: CursorColumn::Note,
            visible: true,
            scroll_y: 0.0,
            input_buffer: String::new(),
            edit_mode: false,
        }
    }

    /// Render the pattern editor for the given module.
    /// Returns EditCommands for any edits made this frame.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        module: &Module,
    ) -> Vec<EditCommand> {
        let mut commands = Vec::new();

        // Get the pattern layout for the current position
        let pattern_pos = edit::get_pattern_position(module, self.current_song, self.current_order);

        if pattern_pos.is_none() {
            ui.label("No pattern data at this position.");
            return commands;
        }

        let pattern_pos = pattern_pos.unwrap();
        let num_channels = pattern_pos.tracks.len();
        let num_rows = pattern_pos.num_rows;

        // Clamp cursor to valid range (prevents out-of-bounds after module change)
        if self.current_channel >= num_channels {
            self.current_channel = 0;
        }
        if self.current_row >= num_rows {
            self.current_row = 0;
        }
        // Handle keyboard input
        self.handle_keyboard_input(ui, &pattern_pos, &mut commands);

        // Layout constants
        let row_height = 18.0;
        let channel_width = 160.0; // note + instr/vol + effects
        let header_height = 22.0;

        // Calculate total size
        let total_width = channel_width * num_channels as f32 + 40.0; // + row numbers
        let total_height = row_height * num_rows as f32 + header_height;

        let available_size = ui.available_size();

        // Scroll area
        ScrollArea::vertical()
            .id_salt("pattern_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(total_width.max(available_size.x), total_height),
                    Sense::click(),
                );

                let rect = response.rect;

                // --- Draw header row ---
                let header_rect = egui::Rect::from_min_size(
                    rect.min,
                    Vec2::new(rect.width(), header_height),
                );
                painter.rect_filled(header_rect, 0.0, Color32::from_rgb(40, 40, 50));
                painter.text(
                    header_rect.left_center() + Vec2::new(4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    "Ch",
                    egui::FontId::monospace(12.0),
                    Color32::LIGHT_GRAY,
                );

                for ch in 0..num_channels {
                    let x = 40.0 + ch as f32 * channel_width;
                    painter.text(
                        header_rect.left_top() + Vec2::new(x + 4.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        format!("{:02}", ch),
                        egui::FontId::monospace(12.0),
                        Color32::LIGHT_GRAY,
                    );
                }

                // --- Draw rows ---
                // Only render visible rows for performance
                let clip_top = ui.clip_rect().top() - rect.top();
                let clip_bottom = ui.clip_rect().bottom() - rect.top();
                let first_visible_row = ((clip_top - header_height) / row_height).max(0.0) as usize;
                let last_visible_row = ((clip_bottom - header_height) / row_height).ceil() as usize + 1;
                let last_visible_row = last_visible_row.min(num_rows);

                for row in first_visible_row..last_visible_row {
                    let y = header_height + row as f32 * row_height;
                    let row_rect = egui::Rect::from_min_size(
                        rect.min + Vec2::new(0.0, y),
                        Vec2::new(rect.width(), row_height),
                    );

                    // Row background (alternating)
                    let row_bg = if row % 16 == 0 {
                        Color32::from_rgb(30, 30, 40) // beat marker
                    } else if row == self.current_row && self.edit_mode {
                        Color32::from_rgb(50, 50, 70) // cursor row (editing)
                    } else if row == self.current_row {
                        Color32::from_rgb(20, 50, 20) // playing row highlight
                    } else if row % 2 == 0 {
                        Color32::from_rgb(25, 25, 35)
                    } else {
                        Color32::from_rgb(22, 22, 32)
                    };
                    painter.rect_filled(row_rect, 0.0, row_bg);

                    // Row number
                    if row % 16 == 0 {
                        painter.text(
                            row_rect.left_center() + Vec2::new(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{:02X}", row),
                            egui::FontId::monospace(11.0),
                            Color32::from_rgb(180, 180, 100),
                        );
                    } else {
                        painter.text(
                            row_rect.left_center() + Vec2::new(4.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{:02X}", row),
                            egui::FontId::monospace(10.0),
                            Color32::from_rgb(100, 100, 100),
                        );
                    }

                    // Draw cells for each channel
                    for ch in 0..num_channels {
                        if let Some(track_idx) = pattern_pos.tracks[ch] {
                            let cell = edit::get_cell(module, track_idx, row);
                            let x = 40.0 + ch as f32 * channel_width;

                            // Note
                            let note_str = edit::cell_note_string(&cell);
                            let note_color = if row == self.current_row
                                && ch == self.current_channel
                                && self.cursor_column == CursorColumn::Note
                            {
                                Color32::YELLOW
                            } else if cell.event.is_trigger() {
                                Color32::from_rgb(200, 200, 255)
                            } else {
                                Color32::from_rgb(140, 140, 140)
                            };

                            painter.text(
                                row_rect.left_top() + Vec2::new(x + 2.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                note_str,
                                egui::FontId::monospace(13.0),
                                note_color,
                            );

                            // Instrument + Volume
                            let iv_str = edit::cell_instr_vol_string(
                                &cell,
                                module.tracks.get(track_idx as usize).map(|t| t.instrument()),
                            );
                            let iv_color = if row == self.current_row
                                && ch == self.current_channel
                                && matches!(
                                    self.cursor_column,
                                    CursorColumn::Instrument | CursorColumn::Volume
                                )
                            {
                                Color32::YELLOW
                            } else {
                                Color32::from_rgb(160, 160, 160)
                            };

                            painter.text(
                                row_rect.left_top() + Vec2::new(x + 42.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                iv_str,
                                egui::FontId::monospace(12.0),
                                iv_color,
                            );

                            // Effects
                            let fx_str = edit::cell_effects_string(&cell);
                            let fx_color = if row == self.current_row
                                && ch == self.current_channel
                                && matches!(self.cursor_column, CursorColumn::Effect(_))
                            {
                                Color32::YELLOW
                            } else if !cell.effects.is_empty() {
                                Color32::from_rgb(100, 200, 100)
                            } else {
                                Color32::from_rgb(100, 100, 100)
                            };

                            painter.text(
                                row_rect.left_top() + Vec2::new(x + 70.0, 2.0),
                                egui::Align2::LEFT_TOP,
                                fx_str,
                                egui::FontId::monospace(12.0),
                                fx_color,
                            );
                        }
                    }

                    // Highlight cursor row
                    if row == self.current_row {
                        painter.rect_stroke(
                            row_rect,
                            0.0,
                            egui::Stroke::new(1.0, Color32::from_rgb(80, 80, 120)),
                            egui::StrokeKind::Inside,
                        );
                    }
                }

                // Handle click on pattern grid
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let rel = pos - rect.min;
                        let clicked_row = ((rel.y - header_height) / row_height) as usize;
                        let clicked_ch =
                            ((rel.x - 40.0) / channel_width).max(0.0) as usize;

                        if clicked_row < num_rows && clicked_ch < num_channels {
                            self.current_row = clicked_row;
                            self.current_channel = clicked_ch;
                        }
                    }
                }
            });

        commands
    }

    fn handle_keyboard_input(
        &mut self,
        ui: &mut Ui,
        pattern_pos: &crate::module::edit::PatternPosition,
        commands: &mut Vec<EditCommand>,
    ) {
        let ctx = ui.ctx();

        ctx.input(|i| {
            for event in &i.events {
                match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => {
                        self.handle_key(*key, *modifiers, pattern_pos, commands);
                    }
                    _ => {}
                }
            }
        });
    }

    fn handle_key(&mut self, key: Key, modifiers: egui::Modifiers, pattern_pos: &crate::module::edit::PatternPosition, commands: &mut Vec<EditCommand>) {
        // Check if this is a hex digit key (for effect entry)
        let hex_ch = key_to_hex_digit(key);

        match key {
            // --- Navigation ---
            Key::ArrowUp => {
                if self.current_row > 0 { self.current_row -= 1; }
                self.input_buffer.clear();
            }
            Key::ArrowDown => {
                self.current_row += 1;
                self.input_buffer.clear();
            }
            Key::ArrowLeft => { self.navigate_column_left(); self.input_buffer.clear(); }
            Key::ArrowRight => { self.navigate_column_right(); self.input_buffer.clear(); }
            Key::Tab => {
                if modifiers.shift {
                    if self.current_channel > 0 { self.current_channel -= 1; }
                } else { self.current_channel += 1; }
                self.input_buffer.clear();
            }
            Key::Home => {
                if modifiers.ctrl { self.current_order = 0; self.current_row = 0; }
                else { self.current_row = 0; }
                self.input_buffer.clear();
            }
            Key::End => { self.input_buffer.clear(); }
            Key::PageUp => { self.current_row = self.current_row.saturating_sub(16); self.input_buffer.clear(); }
            Key::PageDown => { self.current_row += 16; self.input_buffer.clear(); }

            // --- Note input (QWERTY) — only when cursor is on Note column ---
            Key::Z if self.cursor_column == CursorColumn::Note => self.enter_note(0, modifiers, pattern_pos, commands),
            Key::S if self.cursor_column == CursorColumn::Note => self.enter_note(1, modifiers, pattern_pos, commands),
            Key::X if self.cursor_column == CursorColumn::Note => self.enter_note(2, modifiers, pattern_pos, commands),
            Key::D if self.cursor_column == CursorColumn::Note => self.enter_note(3, modifiers, pattern_pos, commands),
            Key::C if self.cursor_column == CursorColumn::Note => self.enter_note(4, modifiers, pattern_pos, commands),
            Key::V if self.cursor_column == CursorColumn::Note => self.enter_note(5, modifiers, pattern_pos, commands),
            Key::G if self.cursor_column == CursorColumn::Note => self.enter_note(6, modifiers, pattern_pos, commands),
            Key::B if self.cursor_column == CursorColumn::Note => self.enter_note(7, modifiers, pattern_pos, commands),
            Key::H if self.cursor_column == CursorColumn::Note => self.enter_note(8, modifiers, pattern_pos, commands),
            Key::N if self.cursor_column == CursorColumn::Note => self.enter_note(9, modifiers, pattern_pos, commands),
            Key::J if self.cursor_column == CursorColumn::Note => self.enter_note(10, modifiers, pattern_pos, commands),
            Key::M if self.cursor_column == CursorColumn::Note => self.enter_note(11, modifiers, pattern_pos, commands),

            Key::Q if self.cursor_column == CursorColumn::Note => self.enter_note(12, modifiers, pattern_pos, commands),
            Key::Num2 if self.cursor_column == CursorColumn::Note => self.enter_note(13, modifiers, pattern_pos, commands),
            Key::W if self.cursor_column == CursorColumn::Note => self.enter_note(14, modifiers, pattern_pos, commands),
            Key::Num3 if self.cursor_column == CursorColumn::Note => self.enter_note(15, modifiers, pattern_pos, commands),
            Key::E if self.cursor_column == CursorColumn::Note => self.enter_note(16, modifiers, pattern_pos, commands),
            Key::R if self.cursor_column == CursorColumn::Note => self.enter_note(17, modifiers, pattern_pos, commands),
            Key::Num5 if self.cursor_column == CursorColumn::Note => self.enter_note(18, modifiers, pattern_pos, commands),
            Key::T if self.cursor_column == CursorColumn::Note => self.enter_note(19, modifiers, pattern_pos, commands),
            Key::Num6 if self.cursor_column == CursorColumn::Note => self.enter_note(20, modifiers, pattern_pos, commands),
            Key::Y if self.cursor_column == CursorColumn::Note => self.enter_note(21, modifiers, pattern_pos, commands),
            Key::Num7 if self.cursor_column == CursorColumn::Note => self.enter_note(22, modifiers, pattern_pos, commands),
            Key::U if self.cursor_column == CursorColumn::Note => self.enter_note(23, modifiers, pattern_pos, commands),

            Key::Delete | Key::Backspace => { self.delete_note(pattern_pos, commands); }

            Key::Space => { self.edit_mode = !self.edit_mode; }

            Key::Escape => { self.input_buffer.clear(); self.edit_mode = false; }

            // --- Hex digit entry (when on Effect column, or any hex key when not on Note) ---
            _ if hex_ch.is_some() => {
                let ch = hex_ch.unwrap();
                self.input_buffer.push(ch);

                // On 3-digit hex entry when cursor is on Effect column,
                // parse and commit the effect immediately.
                if self.input_buffer.len() >= 3
                    && matches!(self.cursor_column, CursorColumn::Effect(_))
                {
                    let buf = self.input_buffer.clone();
                    self.input_buffer.clear();
                    if let Some(track_idx) = pattern_pos.tracks.get(self.current_channel).and_then(|t| *t) {
                        if let Some(effect) = crate::module::edit::parse_effect(&buf) {
                            commands.push(EditCommand::AddCellEffect {
                                track: track_idx,
                                row_offset: self.current_row as u32,
                                effect,
                            });
                        }
                    }
                }
            }

            _ => {} // Ignore other keys
        }
    }
}

/// Map an egui `Key` to a hex digit character (0-9, A-F), or None if not a hex key.
fn key_to_hex_digit(key: Key) -> Option<char> {
    match key {
        Key::Num0 => Some('0'),
        Key::Num1 => Some('1'),
        Key::Num2 => Some('2'),
        Key::Num3 => Some('3'),
        Key::Num4 => Some('4'),
        Key::Num5 => Some('5'),
        Key::Num6 => Some('6'),
        Key::Num7 => Some('7'),
        Key::Num8 => Some('8'),
        Key::Num9 => Some('9'),
        Key::A => Some('A'),
        Key::B => Some('B'),
        Key::C => Some('C'),
        Key::D => Some('D'),
        Key::E => Some('E'),
        Key::F => Some('F'),
        _ => None,
    }
}

impl PatternEditor {
    fn navigate_column_left(&mut self) {
        match self.cursor_column {
            CursorColumn::Note => {
                if self.current_channel > 0 {
                    self.current_channel -= 1;
                    self.cursor_column = CursorColumn::Effect(1);
                }
            }
            CursorColumn::Instrument => self.cursor_column = CursorColumn::Note,
            CursorColumn::Volume => self.cursor_column = CursorColumn::Instrument,
            CursorColumn::Effect(0) => self.cursor_column = CursorColumn::Volume,
            CursorColumn::Effect(1) => self.cursor_column = CursorColumn::Effect(0),
            CursorColumn::Effect(_) => self.cursor_column = CursorColumn::Effect(1),
        }
    }

    fn navigate_column_right(&mut self) {
        match self.cursor_column {
            CursorColumn::Note => self.cursor_column = CursorColumn::Instrument,
            CursorColumn::Instrument => self.cursor_column = CursorColumn::Volume,
            CursorColumn::Volume => self.cursor_column = CursorColumn::Effect(0),
            CursorColumn::Effect(0) => self.cursor_column = CursorColumn::Effect(1),
            CursorColumn::Effect(1) | CursorColumn::Effect(_) => {
                self.current_channel += 1;
                self.cursor_column = CursorColumn::Note;
            }
        }
    }

    fn enter_note(&mut self, note_offset: u8, modifiers: egui::Modifiers, pattern_pos: &crate::module::edit::PatternPosition, commands: &mut Vec<EditCommand>) {
        let base_octave = if modifiers.ctrl { 5 } else if note_offset >= 12 { 5 } else { 4 };
        let note_value = (note_offset % 12) + base_octave * 12;

        // Create the note-on event
        if let Some(pitch) = Pitch::try_from(note_value).ok() {
            if let Some(track_idx) = pattern_pos.tracks.get(self.current_channel).and_then(|t| *t) {
                let cell = Cell {
                    event: CellEvent::NoteOn {
                        pitch,
                        velocity: Volume::FULL,
                    },
                    effects: vec![],
                };
                commands.push(EditCommand::SetCell {
                    track: track_idx,
                    row_offset: self.current_row as u32,
                    content: cell,
                });
            }
        }

        self.current_row += 1;
        self.edit_mode = true;
    }

    fn delete_note(&mut self, pattern_pos: &crate::module::edit::PatternPosition, commands: &mut Vec<EditCommand>) {
        if let Some(track_idx) = pattern_pos.tracks.get(self.current_channel).and_then(|t| *t) {
            commands.push(EditCommand::SetCell {
                track: track_idx,
                row_offset: self.current_row as u32,
                content: Cell::default(),
            });
        }
    }
}
