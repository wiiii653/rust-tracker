//! Order list — displays and navigates the pattern sequence order.

#![allow(dead_code)]

use egui::{Color32, Key, ScrollArea, Sense, Ui, Vec2};
use xmrs::prelude::*;

/// The order list widget.
pub struct OrderList {
    pub current_song: usize,
    pub current_order: usize,
    pub visible: bool,
}

impl OrderList {
    pub fn new() -> Self {
        Self {
            current_song: 0,
            current_order: 0,
            visible: true,
        }
    }

    /// Render the order list for the given module.
    /// Returns the newly selected order index if changed.
    pub fn show(&mut self, ui: &mut Ui, module: &Module) -> Option<usize> {
        let mut new_order = None;

        // Get the order count for the current song
        let order_count = module.timeline_map.order_count(self.current_song);

        // Handle keyboard
        ui.ctx().input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed: true,
                    modifiers,
                    ..
                } = event
                {
                    match key {
                        Key::F11 => {
                            if self.current_order > 0 {
                                new_order = Some(self.current_order - 1);
                            }
                        }
                        Key::F12 => {
                            if self.current_order + 1 < order_count {
                                new_order = Some(self.current_order + 1);
                            }
                        }
                        Key::Home if modifiers.ctrl => {
                            new_order = Some(0);
                        }
                        Key::End if modifiers.ctrl => {
                            new_order = Some(order_count.saturating_sub(1));
                        }
                        _ => {}
                    }
                }
            }
        });

        // Draw order list
        let cell_size = 24.0;
        let cols = 16;
        let rows = (order_count + cols - 1) / cols;
        let total_width = cols as f32 * cell_size;
        let total_height = rows.max(1) as f32 * cell_size;

        ScrollArea::vertical()
            .id_salt("order_list_scroll")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let (response, painter) = ui.allocate_painter(
                    Vec2::new(total_width.max(ui.available_width()), total_height),
                    Sense::click(),
                );

                let rect = response.rect;

                // Header
                painter.text(
                    rect.left_top() + Vec2::new(2.0, 2.0),
                    egui::Align2::LEFT_TOP,
                    format!("Order List ({} entries)", order_count),
                    egui::FontId::monospace(11.0),
                    Color32::LIGHT_GRAY,
                );

                for (i, entry) in module
                    .timeline_map
                    .entries
                    .iter()
                    .filter(|e| e.song as usize == self.current_song && e.loop_iter == 0)
                    .enumerate()
                {
                    let col = i % cols;
                    let row = i / cols + 1; // +1 for header

                    let x = col as f32 * cell_size + rect.min.x;
                    let y = row as f32 * cell_size + rect.min.y;

                    let cell_rect = egui::Rect::from_min_size(
                        egui::pos2(x, y),
                        Vec2::new(cell_size, cell_size),
                    );

                    // Background
                    let bg = if i == self.current_order {
                        Color32::from_rgb(60, 60, 100)
                    } else if i % 8 < 4 {
                        Color32::from_rgb(35, 35, 45)
                    } else {
                        Color32::from_rgb(30, 30, 40)
                    };
                    painter.rect_filled(cell_rect, 0.0, bg);

                    // Pattern number
                    painter.text(
                        cell_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{:02X}", entry.pattern_idx),
                        egui::FontId::monospace(13.0),
                        if i == self.current_order {
                            Color32::YELLOW
                        } else {
                            Color32::from_rgb(180, 180, 180)
                        },
                    );

                    // Border for current
                    if i == self.current_order {
                        painter.rect_stroke(
                            cell_rect,
                            0.0,
                            egui::Stroke::new(2.0, Color32::YELLOW),
                            egui::StrokeKind::Inside,
                        );
                    }
                }

                // Handle click
                if response.clicked() {
                    if let Some(pos) = response.interact_pointer_pos() {
                        let rel = pos - rect.min;
                        let col = (rel.x / cell_size) as usize;
                        let row = (rel.y / cell_size) as usize;
                        if row > 0 {
                            let idx = (row - 1) * cols + col;
                            if idx < order_count {
                                new_order = Some(idx);
                            }
                        }
                    }
                }
            });

        if let Some(order) = new_order {
            if order != self.current_order {
                self.current_order = order;
                return Some(order);
            }
        }

        None
    }
}
