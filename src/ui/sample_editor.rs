//! Sample editor — waveform display with zoom, scroll, loop point markers,
//! and sample operations (normalize, amplify, reverse, fade, trim).

use crate::module::sample::SampleData;
use egui::{Color32, Rect, Sense, Ui, Vec2};
use xmrs::prelude::{LoopType, Module};

/// Which loop marker is being dragged on the waveform.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragMarker {
    LoopStart,
    LoopEnd,
    SustainStart,
    SustainEnd,
}

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
    /// Which loop marker is currently being dragged.
    drag_marker: Option<DragMarker>,
    /// Trim range start (sample frames).
    trim_start: usize,
    /// Trim range end (sample frames).
    trim_end: usize,
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
            drag_marker: None,
            trim_start: 0,
            trim_end: 0,
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
    pub fn show(&mut self, ui: &mut Ui, module: &mut Module) -> bool {
        let mut module_changed = false;

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

            // Slot management buttons
            if ui.button("+ Slot").clicked() {
                self.add_sample_slot(module);
                module_changed = true;
            }
            if ui.button("− Slot").clicked() {
                self.remove_sample_slot(module);
                module_changed = true;
            }
        });

        ui.separator();

        // Load sample if not loaded
        if self.sample_data.is_none() && !module.instrument.is_empty() {
            self.load_sample(module, self.current_instrument, self.current_sample);
        }

        // Take ownership of sample_data to avoid borrow conflicts
        let mut data = self.sample_data.take();
        let mut pending_op: Option<fn(&mut SampleData)> = None;
        let mut pending_wav_data: Option<SampleData> = None;

        if let Some(ref mut sample) = data {
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

            // WAV import button
            ui.horizontal(|ui| {
                if ui.button("📂 Load WAV…").clicked() {
                    if let Some(wav_path) = rfd::FileDialog::new()
                        .add_filter("WAV Files", &["wav", "wave"])
                        .pick_file()
                    {
                        match SampleData::from_wav_path(&wav_path) {
                            Ok(wav_data) => {
                                self.status = Some(format!(
                                    "Loaded: {}",
                                    wav_path.file_name().unwrap_or_default().to_string_lossy()
                                ));
                                pending_wav_data = Some(wav_data);
                            }
                            Err(e) => {
                                self.status = Some(format!("WAV error: {}", e));
                            }
                        }
                    }
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
                &mut self.drag_marker,
            );
            self.zoom = zoom_changed.0;
            self.scroll_offset = zoom_changed.1;

            // Loop point display
            ui.separator();
            draw_loop_info(ui, sample);

            // Loop type selectors
            ui.horizontal(|ui| {
                ui.label("Loop:");
                egui::ComboBox::from_id_salt("loop_type_combo")
                    .selected_text(format!("{:?}", sample.loop_type))
                    .show_ui(ui, |ui| {
                        for lt in &[LoopType::No, LoopType::Forward, LoopType::PingPong] {
                            if ui.selectable_label(sample.loop_type == *lt, format!("{:?}", lt)).clicked() {
                                sample.loop_type = *lt;
                            }
                        }
                    });
                ui.label("Sustain:");
                egui::ComboBox::from_id_salt("sustain_loop_type_combo")
                    .selected_text(format!("{:?}", sample.sustain_loop_type))
                    .show_ui(ui, |ui| {
                        for lt in &[LoopType::No, LoopType::Forward, LoopType::PingPong] {
                            if ui.selectable_label(sample.sustain_loop_type == *lt, format!("{:?}", lt)).clicked() {
                                sample.sustain_loop_type = *lt;
                            }
                        }
                    });
            });

            // Trim controls
            ui.horizontal(|ui| {
                ui.label("Trim:");
                if self.trim_end == 0 || self.trim_end > sample.length {
                    self.trim_end = sample.length;
                }
                if ui.add(egui::DragValue::new(&mut self.trim_start)
                    .range(0..=sample.length.saturating_sub(1))
                    .speed(100.0))
                    .changed() {}
                ui.label("–");
                if ui.add(egui::DragValue::new(&mut self.trim_end)
                    .range(1..=sample.length)
                    .speed(100.0))
                    .changed() {}
                if ui.button("✂ Trim").clicked()
                    && self.trim_start < self.trim_end
                {
                    sample.trim(self.trim_start, self.trim_end);
                    if self.write_back_sample(module, sample) {
                        module_changed = true;
                    }
                    self.status = Some("Trim applied".to_string());
                    self.trim_end = sample.length;
                }
                if ui.button("↺ Reset").clicked() {
                    self.trim_start = 0;
                    self.trim_end = sample.length;
                }
            });

            // Apply pending operation
            if let Some(op) = pending_op {
                if let Some(ref mut d) = data {
                    op(d);
                    if self.write_back_sample(module, d) {
                        module_changed = true;
                    }
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

        // Process pending WAV import (handled outside if-let to avoid borrow conflict)
        if let Some(wav_data) = pending_wav_data.take() {
            if self.write_back_sample(module, &wav_data) {
                module_changed = true;
            }
            self.zoom = 100.0;
            self.scroll_offset = 0.0;
            data = Some(wav_data);
        }

        // Sync sample metadata changes (loop type, markers, etc.) back to module.
        // This is cheap metadata-only when PCM data hasn't changed.
        if let Some(ref d) = data {
            if self.write_back_sample(module, d) {
                module_changed = true;
            }
        }

        // Put data back
        self.sample_data = data;
        module_changed
    }

    fn write_back_sample(&self, module: &mut Module, data: &SampleData) -> bool {
        if let Some(sample) = self.current_sample_mut(module) {
            data.apply_to_sample(sample);
            return true;
        }
        false
    }

    /// Get mutable reference to the current sample, auto-creating the slot if needed.
    fn current_sample_mut<'a>(&self, module: &'a mut Module) -> Option<&'a mut xmrs::prelude::Sample> {
        let instrument = module.instrument.get_mut(self.current_instrument)?;
        let xmrs::prelude::InstrumentType::Default(ref mut instr_default) = instrument.instr_type else {
            return None;
        };
        // Ensure the sample vec has enough slots
        while instr_default.sample.len() <= self.current_sample {
            instr_default.sample.push(None);
        }
        // Create a default sample if the slot is empty
        if instr_default.sample[self.current_sample].is_none() {
            use xmrs::prelude::*;
            instr_default.sample[self.current_sample] = Some(Sample {
                name: String::new(),
                relative_pitch: 0,
                finetune: Finetune::ZERO,
                volume: ChannelVolume::from_byte_64(64),
                default_note_volume: Volume::FULL,
                panning: Panning::CENTER,
                loop_flag: LoopType::No,
                loop_start: 0,
                loop_length: 0,
                sustain_loop_flag: LoopType::No,
                sustain_loop_start: 0,
                sustain_loop_length: 0,
                data: None,
            });
        }
        instr_default.sample[self.current_sample].as_mut()
    }

    /// Add a new empty sample slot to the current instrument and select it.
    fn add_sample_slot(&mut self, module: &mut Module) {
        use xmrs::prelude::*;
        if let Some(instrument) = module.instrument.get_mut(self.current_instrument) {
            if let InstrumentType::Default(ref mut instr) = instrument.instr_type {
                instr.sample.push(Some(Sample {
                    name: String::new(),
                    relative_pitch: 0,
                    finetune: Finetune::ZERO,
                    volume: ChannelVolume::from_byte_64(64),
                    default_note_volume: Volume::FULL,
                    panning: Panning::CENTER,
                    loop_flag: LoopType::No,
                    loop_start: 0,
                    loop_length: 0,
                    sustain_loop_flag: LoopType::No,
                    sustain_loop_start: 0,
                    sustain_loop_length: 0,
                    data: None,
                }));
                self.current_sample = instr.sample.len() - 1;
                self.load_sample(module, self.current_instrument, self.current_sample);
            }
        }
    }

    /// Remove the current sample slot from the instrument.
    fn remove_sample_slot(&mut self, module: &mut Module) {
        if let Some(instrument) = module.instrument.get_mut(self.current_instrument) {
            if let xmrs::prelude::InstrumentType::Default(ref mut instr) = instrument.instr_type {
                if instr.sample.is_empty() || self.current_sample >= instr.sample.len() {
                    return;
                }
                instr.sample.remove(self.current_sample);
                if self.current_sample >= instr.sample.len() {
                    self.current_sample = instr.sample.len().saturating_sub(1);
                }
                self.load_sample(module, self.current_instrument, self.current_sample);
            }
        }
    }
}

/// Draw the waveform visualization. Returns (new_zoom, new_scroll_offset).
fn draw_waveform(
    ui: &mut Ui,
    data: &mut SampleData,
    mut zoom: f32,
    mut scroll_offset: f32,
    drag_marker: &mut Option<DragMarker>,
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

    // Helper: convert screen x to sample position
    let x_to_sample = |x: f32| -> u32 {
        let sample = (start_sample as f32 + (x - rect.left()) * samples_per_pixel) as i64;
        sample.clamp(0, data.length.saturating_sub(1) as i64) as u32
    };

    // Collect visible marker positions for hit-testing
    let mut marker_positions: Vec<(f32, DragMarker)> = Vec::new();

    // Draw loop points
    if data.loop_type != LoopType::No {
        let loop_start_x = rect.left()
            + data.loop_start as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;
        let loop_end_x = rect.left()
            + (data.loop_start + data.loop_length) as f32 / samples_per_pixel
            - start_sample as f32 / samples_per_pixel;

        if loop_start_x >= rect.left() && loop_start_x <= rect.right() {
            marker_positions.push((loop_start_x, DragMarker::LoopStart));
            painter.line_segment(
                [
                    egui::pos2(loop_start_x, rect.top()),
                    egui::pos2(loop_start_x, rect.bottom()),
                ],
                egui::Stroke::new(2.0, Color32::YELLOW),
            );
        }
        if loop_end_x >= rect.left() && loop_end_x <= rect.right() {
            marker_positions.push((loop_end_x, DragMarker::LoopEnd));
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
            marker_positions.push((sus_start_x, DragMarker::SustainStart));
            painter.line_segment(
                [
                    egui::pos2(sus_start_x, rect.top()),
                    egui::pos2(sus_start_x, rect.bottom()),
                ],
                egui::Stroke::new(1.5, Color32::from_rgb(100, 200, 255)),
            );
        }
        if sus_end_x >= rect.left() && sus_end_x <= rect.right() {
            marker_positions.push((sus_end_x, DragMarker::SustainEnd));
            painter.line_segment(
                [
                    egui::pos2(sus_end_x, rect.top()),
                    egui::pos2(sus_end_x, rect.bottom()),
                ],
                egui::Stroke::new(1.5, Color32::from_rgb(100, 200, 255)),
            );
        }
    }

    // Handle drag: marker dragging vs waveform scrolling
    let hit_radius = 10.0;
    if response.dragged() {
        if let Some(pointer) = response.hover_pos() {
            if let Some(drag) = *drag_marker {
                // Update the marker being dragged
                let sample_pos = x_to_sample(pointer.x);
                match drag {
                    DragMarker::LoopStart => {
                        let max_start = data.loop_start + data.loop_length.saturating_sub(1);
                        data.loop_start = sample_pos.min(max_start);
                    }
                    DragMarker::LoopEnd => {
                        let min_end = data.loop_start.saturating_add(1);
                        data.loop_length = sample_pos.max(min_end) - data.loop_start;
                    }
                    DragMarker::SustainStart => {
                        let max_start = data.sustain_loop_start + data.sustain_loop_length.saturating_sub(1);
                        data.sustain_loop_start = sample_pos.min(max_start);
                    }
                    DragMarker::SustainEnd => {
                        let min_end = data.sustain_loop_start.saturating_add(1);
                        data.sustain_loop_length = sample_pos.max(min_end) - data.sustain_loop_start;
                    }
                }
            } else {
                // Check if dragging started near a marker
                let nearest = marker_positions.iter()
                    .filter(|(mx, _)| (pointer.x - mx).abs() < hit_radius)
                    .min_by(|(a, _), (b, _)| (pointer.x - a).abs().total_cmp(&(pointer.x - b).abs()));

                if let Some((_, marker)) = nearest {
                    *drag_marker = Some(*marker);
                } else {
                    // Scroll waveform
                    scroll_offset -= response.drag_delta().x * zoom;
                    if scroll_offset < 0.0 {
                        scroll_offset = 0.0;
                    }
                }
            }
        }
    } else {
        // Not dragging — clear drag state
        *drag_marker = None;
    }

    // Highlight hovered marker with a slightly brighter stroke
    if let Some(pointer) = response.hover_pos() {
        let nearest = marker_positions.iter()
            .filter(|(mx, _)| (pointer.x - mx).abs() < hit_radius)
            .min_by(|(a, _), (b, _)| (pointer.x - a).abs().total_cmp(&(pointer.x - b).abs()));
        if let Some((mx, _)) = nearest {
            painter.line_segment(
                [egui::pos2(*mx, rect.top()), egui::pos2(*mx, rect.bottom())],
                egui::Stroke::new(3.0, egui::Color32::WHITE),
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
