//! Transport bar — Play, Stop, Pause controls and playback position display.

use crate::audio::engine::PlaybackState;
use egui::{Align, Color32, Layout, RichText, Ui};

/// Render the transport bar at the top of the screen.
pub struct TransportBar;

impl TransportBar {
    pub fn show(
        ui: &mut Ui,
        is_playing: bool,
        is_module_loaded: bool,
        playback: &PlaybackState,
        on_play: &mut dyn FnMut(),
        on_stop: &mut dyn FnMut(),
    ) {
        ui.horizontal(|ui| {
            ui.set_height(32.0);

            // Play / Stop button
            if is_playing {
                let stop_btn = egui::Button::new(
                    RichText::new("⏹ Stop").color(Color32::WHITE).size(14.0),
                )
                .fill(Color32::from_rgb(180, 40, 40))
                .min_size(egui::vec2(72.0, 26.0));

                if ui.add(stop_btn).clicked() {
                    on_stop();
                }
            } else {
                let play_btn = egui::Button::new(
                    RichText::new("▶ Play").color(Color32::WHITE).size(14.0),
                )
                .fill(Color32::from_rgb(40, 160, 40))
                .min_size(egui::vec2(72.0, 26.0));

                let play_response = ui.add_enabled(is_module_loaded, play_btn);
                if play_response.clicked() {
                    on_play();
                }
            }

            ui.separator();

            // Pattern / Row display
            if is_playing {
                ui.label(
                    RichText::new(format!(
                        "Pat: {:02X}  Row: {:02X}",
                        playback.current_pattern, playback.current_row
                    ))
                    .size(13.0)
                    .color(Color32::LIGHT_GRAY),
                );
            } else {
                ui.label(
                    RichText::new("Pat: --  Row: --")
                        .size(13.0)
                        .color(Color32::DARK_GRAY),
                );
            }

            ui.separator();

            // BPM / Tempo
            if is_playing {
                ui.label(
                    RichText::new(format!("BPM: {}  Spd: {}", playback.bpm, playback.tempo))
                        .size(13.0)
                        .color(Color32::LIGHT_GRAY),
                );
            } else {
                ui.label(
                    RichText::new("BPM: ---  Spd: --")
                        .size(13.0)
                        .color(Color32::DARK_GRAY),
                );
            }

            ui.separator();

            // Time elapsed
            let sr = if playback.sample_rate > 0 { playback.sample_rate as f64 } else { 44100.0 };
            let total_seconds = playback.elapsed_samples as f64 / sr;
            let minutes = total_seconds as u64 / 60;
            let seconds = total_seconds as u64 % 60;
            ui.label(
                RichText::new(format!("{:02}:{:02}", minutes, seconds))
                    .size(13.0)
                    .color(Color32::LIGHT_GRAY),
            );

            // Push everything to the right for future additions
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(
                    RichText::new("rust-tracker v0.1")
                        .size(11.0)
                        .color(Color32::DARK_GRAY),
                );
            });
        });
    }
}
