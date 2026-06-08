//! Sample editor — waveform display with zoom, scroll, loop point markers,
//! and sample operations (normalize, amplify, reverse, fade, trim).

use crate::module::sample::SampleData;
use egui::{Color32, Rect, Sense, Ui, Vec2};
use xmrs::prelude::LoopType;

/// State for the sample editor.
#[allow(dead_code)]
pub struct SampleEditor {
    /// Currently selected instrument index.
    pub current_instrument: usize,
    /// Currently selected sample index within the instrument.
    pub current_sample: usize,
    /// Zoom level (samples per pixel).
    pub zoom: f32,
    /// Horizontal scroll offset in samples.
    pub scroll_offset: f32,
    /// Whether the editor is visible.
    pub visible: bool,
    /// Currently editing loop points mode.
    pub editing_loop: bool,
    /// Sample data cache for the current sample.
    pub sample_data: Option<SampleData>,
    /// Pending operation name for status display.
    pub status: Option<String>,
}

impl SampleEditor {
    pub fn new() -> Self {
        Self {
            current_instrument: 0,
            current_sample: 0,
            zoom: 100.0,
            scroll_offset: 0.0,
            visible: true,
            editing_loop: false,
            sample_data: None,
            status: None,
        }
    }

    /// Load sample data from a module's instrument.
    pub fn load_sample(&mut self, module: &xmrs::prelude::Module, instr_idx: usize, sample_idx: usize) {
        self.current_instrument = instr_idx;
        self.current_sample = sample_idx;

        if let Some(instrument) = module.instrument.get(instr_idx) {
            if let xmrs::prelude::InstrumentType::Default(ref instr_default) = instrument.instr_type {
                if let Some(Some(ref sample)) = instr_default.sample.get(sample_idx) {
                    self.sample_data = Some(SampleData::from_sample(sample));
                    self.scroll_offset = 0.0;
                    return;
                }
            }
        }
        self.sample_data = None;
    }

    /// Get list of instrument names for the dropdown.
    pub fn instrument_list(&self, module: &xmrs::prelude::Module) -> Vec<String> {
        module
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
            .collect()
    }

    /// Get list of sample indices for the current instrument.
    pub fn sample_list(&self, module: &xmrs::prelude::Module) -> Vec<(usize, String)> {
        if let Some(instrument) = module.instrument.get(self.current_instrument) {
            if let xmrs::prelude::InstrumentType::Default(ref instr_default) = instrument.instr_type {
                return instr_default
                    .sample
                    .iter()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        s.as_ref().map(|sample| {
                            let name = if sample.name.is_empty() {
                                format!("Sample {:02X}", i)
                            } else {
                                format!("{:02X}: {}", i, sample.name)
                            };
                            (i, name)
                        })
                    })
                    .collect();
            }
        }
        Vec::new()
    }

    /// Render the sample editor.
    pub fn show(&mut self, ui: &mut Ui, module: &xmrs::prelude::Module) {
        // Instrument selector
        ui.horizontal(|ui| {
            ui.label("Instrument:");
            let instruments = self.instrument_list(module);
            let current_name = instruments
                .get(self.current_instrument)
                .cloned()
                .unwrap_or_else(|| "None".to_string());

            egui::ComboBox::from_id_salt("instr_select")
                .selected_text(&current_name)
                .show_ui(ui, |ui| {
                    for (i, name) in instruments.iter().enumerate() {
                        if ui.selectable_label(i == self.current_instrument, name).clicked() {
                            self.current_instrument = i;
                            self.current_sample = 0;
                            self.load_sample(module, i, 0);
                        }
                    }
                });
        });

        // Sample selector
        ui.horizontal(|ui| {
            ui.label("Sample:");
            let samples = self.sample_list(module);
            let current_name = samples
                .iter()
                .find(|(i, _)| *i == self.current_sample)
                .map(|(_, n)| n.clone())
                .unwrap_or_else(|| "None".to_string());

            egui::ComboBox::from_id_salt("sample_select")
                .selected_text(&current_name)
                .show_ui(ui, |ui| {
                    for (i, name) in &samples {
                        if ui.selectable_label(*i == self.current_sample, name).clicked() {
                            self.current_sample = *i;
                            self.load_sample(module, self.current_instrument, *i);
                        }
                    }
                });
        });

        ui.separator();

        // Load sample if not loaded
        if self.sample_data.is_none() && !module.instrument.is_empty() {
            self.load_sample(module, self.current_instrument, self.current_sample);
        }

        // Take ownership of sample_data to avoid borrow conflicts
        let mut data = self.sample_data.take();
        let mut pending_op: Option<fn(&mut SampleData)> = None;

        if let Some(ref sample) = data {
            // Sample info
            ui.horizontal(|ui| {
                ui.label(format!(
                    "Length: {} samples | Rate: {} Hz | Bits: {} | {}",
                    sample.length,
                    sample.sample_rate,
                    sample.bits,
                    if sample.is_stereo { "Stereo" } else { "Mono" }
                ));
            });

            // Zoom controls
            ui.horizontal(|ui| {
                if ui.button("−").clicked() {
                    self.zoom = (self.zoom * 0.5).max(1.0);
                }
                ui.label(format!("Zoom: {:.0}%", 100.0 / self.zoom.max(1.0)));
                if ui.button("+").clicked() {
                    self.zoom = (self.zoom * 2.0).min(10000.0);
                }
                if ui.button("Fit").clicked() {
                    let available = ui.available_width();
                    if sample.length > 0 {
                        self.zoom = sample.length as f32 / available.max(1.0);
                    }
                }
                if ui.button("1:1").clicked() {
                    self.zoom = 1.0;
                }
            });

            // Sample operation buttons — collect operation, apply after UI
            ui.horizontal(|ui| {
                if ui.button("Normalize").clicked() {
                    pending_op = Some(|d: &mut SampleData| d.normalize());
                }
                if ui.button("Reverse").clicked() {
                    pending_op = Some(|d: &mut SampleData| d.reverse());
                }
                if ui.button("×2").clicked() {
                    pending_op = Some(|d: &mut SampleData| d.amplify(2.0));
                }
                if ui.button("×½").clicked() {
                    pending_op = Some(|d: &mut SampleData| d.amplify(0.5));
                }
                if ui.button("Fade In").clicked() {
                    pending_op = Some(|d: &mut SampleData| {
                        let dur = (d.length as f32 * 0.1) as usize;
                        d.fade_in(dur);
                    });
                }
                if ui.button("Fade Out").clicked() {
                    pending_op = Some(|d: &mut SampleData| {
                        let dur = (d.length as f32 * 0.1) as usize;
                        d.fade_out(dur);
                    });
                }
            });

            ui.separator();

            // Waveform display
            let zoom_changed = draw_waveform(
                ui,
                sample,
                self.zoom,
                self.scroll_offset,
            );
            self.zoom = zoom_changed.0;
            self.scroll_offset = zoom_changed.1;

            // Loop point display
            ui.separator();
            draw_loop_info(ui, sample);

            // Apply pending operation
            if let Some(op) = pending_op {
                if let Some(ref mut d) = data {
                    op(d);
                    self.status = Some("Operation applied".to_string());
                }
            }

            // Status
            if let Some(ref status) = self.status {
                ui.label(
                    egui::RichText::new(status)
                        .color(Color32::LIGHT_GREEN)
                        .size(12.0),
                );
            }
        } else {
            ui.label("No sample selected or instrument has no samples.");
        }

        // Put data back
        self.sample_data = data;
    }
}

/// Draw the waveform visualization. Returns (new_zoom, new_scroll_offset).
fn draw_waveform(
    ui: &mut Ui,
    data: &SampleData,
    mut zoom: f32,
    mut scroll_offset: f32,
) -> (f32, f32) {
    if data.mono_data.is_empty() {
        ui.label("Empty sample.");
        return (zoom, scroll_offset);
    }

    let desired_size = Vec2::new(ui.available_width(), 200.0);
    let (response, painter) = ui.allocate_painter(desired_size, Sense::click_and_drag());

    let rect = response.rect;
    let width = rect.width();
    let height = rect.height();
    let mid_y = rect.center().y;

    // Background
    painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 30));

    // Center line
    painter.line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0, Color32::from_rgb(40, 40, 60)),
    );

    // Handle scroll with drag
    if response.dragged() {
        scroll_offset -= response.drag_delta().x * zoom;
        if scroll_offset < 0.0 {
            scroll_offset = 0.0;
        }
    }

    // Zoom with mouse wheel
    if let Some(hover_pos) = response.hover_pos() {
        if rect.contains(hover_pos) {
            let scroll = ui.ctx().input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = zoom;
                zoom = (zoom * (1.0 - scroll * 0.001)).clamp(0.1, 10000.0);
                let hover_x = hover_pos.x - rect.left();
                scroll_offset += hover_x * (zoom - old_zoom);
                if scroll_offset < 0.0 {
                    scroll_offset = 0.0;
                }
            }
        }
    }

    let start_sample = scroll_offset as usize;
    let samples_per_pixel = zoom.max(0.1);
    let visible_samples = (width * samples_per_pixel) as usize;

    // Draw waveform
    if samples_per_pixel < 2.0 {
        // Zoomed in: draw individual sample lines
        let mut last_x = rect.left();
        let mut last_y = mid_y;
        let mut first = true;

        let end = (start_sample + visible_samples).min(data.mono_data.len());
        for i in start_sample..end {
            let sample = data.mono_data[i];
            let x = rect.left() + (i - start_sample) as f32 / samples_per_pixel;
            let y = mid_y - sample * height * 0.45;

            if !first {
                painter.line_segment(
                    [egui::pos2(last_x, last_y), egui::pos2(x, y)],
                    egui::Stroke::new(1.0, Color32::from_rgb(100, 200, 100)),
                );
            }
            last_x = x;
            last_y = y;
            first = false;
        }
    } else {
        // Zoomed out: draw min/max overview
        let num_buckets = width as usize;
        let (mins, maxs) = data.overview(num_buckets);

        for (i, (&min, &max)) in mins.iter().zip(maxs.iter()).enumerate() {
            let x = rect.left() + i as f32;
            let y_min = mid_y - min * height * 0.45;
            let y_max = mid_y - max * height * 0.45;

            if max - min > 0.001 {
                painter.line_segment(
                    [egui::pos2(x, y_min), egui::pos2(x, y_max)],
                    egui::Stroke::new(1.0, Color32::from_rgb(100, 200, 100)),
                );
            } else {
                painter.rect_filled(
                    Rect::from_min_size(
                        egui::pos2(x, mid_y - min * height * 0.45),
                        Vec2::new(1.0, 1.0),
                    ),
                    0.0,
                    Color32::from_rgb(100, 200, 100),
                );
            }
        }
    }

    // Draw loop points
    if data.loop_type != LoopType::No {
        let loop_start_x = rect.left()
            + data.loop_start as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;
        let loop_end_x = rect.left()
            + (data.loop_start + data.loop_length) as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;

        if loop_start_x >= rect.left() && loop_start_x <= rect.right() {
            painter.line_segment(
                [
                    egui::pos2(loop_start_x, rect.top()),
                    egui::pos2(loop_start_x, rect.bottom()),
                ],
                egui::Stroke::new(2.0, Color32::YELLOW),
            );
        }
        if loop_end_x >= rect.left() && loop_end_x <= rect.right() {
            painter.line_segment(
                [
                    egui::pos2(loop_end_x, rect.top()),
                    egui::pos2(loop_end_x, rect.bottom()),
                ],
                egui::Stroke::new(2.0, Color32::from_rgb(255, 100, 100)),
            );
        }
    }

    // Draw sustain loop
    if data.sustain_loop_type != LoopType::No {
        let sus_start_x = rect.left()
            + data.sustain_loop_start as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;
        let sus_end_x = rect.left()
            + (data.sustain_loop_start + data.sustain_loop_length) as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;

        if sus_start_x >= rect.left() && sus_start_x <= rect.right() {
            painter.line_segment(
                [
                    egui::pos2(sus_start_x, rect.top()),
                    egui::pos2(sus_start_x, rect.bottom()),
                ],
                egui::Stroke::new(1.5, Color32::from_rgb(100, 200, 255)),
            );
        }
        if sus_end_x >= rect.left() && sus_end_x <= rect.right() {
            painter.line_segment(
                [
                    egui::pos2(sus_end_x, rect.top()),
                    egui::pos2(sus_end_x, rect.bottom()),
                ],
                egui::Stroke::new(1.5, Color32::from_rgb(100, 200, 255)),
            );
        }
    }

    // Scrollbar hint
    if data.length > 0 {
        let total_width = data.length as f32 / samples_per_pixel;
        if total_width > width {
            let thumb_start = scroll_offset / data.length as f32 * width;
            let thumb_width = (width / total_width * width).max(4.0);
            let thumb_rect = Rect::from_min_size(
                egui::pos2(rect.left() + thumb_start, rect.bottom() - 8.0),
                Vec2::new(thumb_width, 6.0),
            );
            painter.rect_filled(thumb_rect, 2.0, Color32::from_rgb(80, 80, 100));
        }
    }

    (zoom, scroll_offset)
}

/// Display loop point information.
fn draw_loop_info(ui: &mut Ui, data: &SampleData) {
    ui.horizontal(|ui| {
        ui.label("Loop:");
        ui.label(format!(
            "Type: {:?} | Start: {} | Length: {}",
            data.loop_type, data.loop_start, data.loop_length
        ));
    });
    ui.horizontal(|ui| {
        ui.label("Sustain:");
        ui.label(format!(
            "Type: {:?} | Start: {} | Length: {}",
            data.sustain_loop_type, data.sustain_loop_start, data.sustain_loop_length
        ));
    });
}
