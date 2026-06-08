//! Audio visualization — oscilloscope and VU meter display.
//!
//! Drains shared `VizBuffer`s for mix and channel data.

use egui::{Color32, Sense, Ui, Vec2};

/// Audio visualization state, fed from the audio engine via VizBuffer.
pub struct AudioViz {
    /// Number of channels for VU meters.
    num_channels: usize,
    /// Accumulated waveform samples for display.
    waveform_buffer: Vec<f32>,
    /// Max waveform buffer size.
    max_waveform: usize,
    /// Smoothed channel levels for VU display.
    channel_levels: Vec<f32>,
}

impl AudioViz {
    pub fn new(num_channels: usize) -> Self {
        Self {
            num_channels,
            waveform_buffer: Vec::new(),
            max_waveform: 4096,
            channel_levels: vec![0.0; num_channels.max(1)],
        }
    }

    /// Feed raw mix data and channel data from the audio thread.
    pub fn feed_mix(&mut self, samples: &[i16]) {
        for &s in samples {
            self.waveform_buffer.push(s as f32 / 32768.0);
            if self.waveform_buffer.len() > self.max_waveform {
                self.waveform_buffer.remove(0);
            }
        }
    }

    /// Feed per-channel data and update VU levels.
    pub fn feed_channels(&mut self, samples: &[i16]) {
        let mut peaks = vec![0.0f32; self.num_channels.max(1)];
        let mut ch = 0;
        for &s in samples {
            let val = (s as f32 / 32768.0).abs();
            if ch < peaks.len() {
                peaks[ch] = peaks[ch].max(val);
            }
            ch += 1;
            if ch >= self.num_channels {
                ch = 0;
            }
        }
        for (i, &p) in peaks.iter().enumerate() {
            self.channel_levels[i] = self.channel_levels[i] * 0.7 + p * 0.3;
        }
    }

    /// Render the oscilloscope (waveform) at the given size.
    pub fn render_oscilloscope(&self, ui: &mut Ui, height: f32) {
        let desired = Vec2::new(ui.available_width(), height);
        let (response, painter) = ui.allocate_painter(desired, Sense::hover());
        let rect = response.rect;
        let mid = rect.center().y;

        painter.rect_filled(rect, 0.0, Color32::from_rgb(20, 20, 30));
        painter.line_segment(
            [rect.left_center(), rect.right_center()],
            egui::Stroke::new(1.0, Color32::from_rgb(40, 40, 60)),
        );

        let buf = &self.waveform_buffer;
        if buf.len() > 1 {
            let step = rect.width() / buf.len() as f32;
            let mut last_x = rect.left();
            let mut last_y = mid;
            let mut first = true;

            for (i, &sample) in buf.iter().enumerate() {
                let x = rect.left() + i as f32 * step;
                let y = mid - sample * height * 0.45;
                let y = y.clamp(rect.top(), rect.bottom());

                if !first {
                    painter.line_segment(
                        [egui::pos2(last_x, last_y), egui::pos2(x, y)],
                        egui::Stroke::new(1.0, Color32::from_rgb(100, 255, 100)),
                    );
                }
                last_x = x;
                last_y = y;
                first = false;
            }
        }
    }

    /// Render VU meters for each channel.
    pub fn render_vu_meters(&self, ui: &mut Ui) {
        let bar_height = 14.0;
        let max_width = ui.available_width();

        for (i, &level) in self.channel_levels.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("Ch{:02}", i));
                let desired = Vec2::new(max_width - 40.0, bar_height);
                let (response, painter) = ui.allocate_painter(desired, Sense::hover());
                let rect = response.rect;

                painter.rect_filled(rect, 0.0, Color32::from_rgb(30, 30, 40));

                let level_width = rect.width() * level.clamp(0.0, 1.0);
                let color = if level > 0.8 {
                    Color32::RED
                } else if level > 0.5 {
                    Color32::YELLOW
                } else {
                    Color32::GREEN
                };

                let bar_rect =
                    egui::Rect::from_min_size(rect.min, Vec2::new(level_width, bar_height));
                painter.rect_filled(bar_rect, 0.0, color);

                let peak_x = rect.left() + level_width;
                if level_width > 0.0 {
                    painter.line_segment(
                        [
                            egui::pos2(peak_x, rect.top()),
                            egui::pos2(peak_x, rect.bottom()),
                        ],
                        egui::Stroke::new(1.0, Color32::WHITE),
                    );
                }
            });
        }
    }
}
