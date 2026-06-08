//! Color themes for the tracker UI.
//!
//! Provides:
//! - `ft2_classic` — faithful Fast Tracker 2 DOS aesthetic
//! - `modern_dark` — sleek dark theme with improved contrast

use egui::{Color32, Visuals};

/// Apply the FT2 Classic theme — dark blue background, cyan/white text.
pub fn apply_ft2_classic(ctx: &egui::Context) {
    let visuals = Visuals {
        dark_mode: true,
        override_text_color: Some(Color32::from_rgb(180, 200, 220)),
        panel_fill: Color32::from_rgb(10, 20, 50),
        window_fill: Color32::from_rgb(12, 22, 52),
        faint_bg_color: Color32::from_rgb(15, 28, 60),
        extreme_bg_color: Color32::from_rgb(5, 12, 30),
        code_bg_color: Color32::from_rgb(12, 22, 52),
        warn_fg_color: Color32::YELLOW,
        error_fg_color: Color32::RED,
        window_corner_radius: 0.0.into(),
        window_stroke: egui::Stroke::new(1.0, Color32::from_rgb(40, 60, 100)),
        selection: egui::style::Selection {
            bg_fill: Color32::from_rgb(60, 80, 160),
            stroke: egui::Stroke::new(1.0, Color32::from_rgb(100, 140, 255)),
        },
        hyperlink_color: Color32::from_rgb(100, 180, 255),
        ..Visuals::dark()
    };

    ctx.set_visuals(visuals);

    // Set monospace as default font
    let mut style = (*ctx.style()).clone();
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Monospace,
        egui::FontId::new(12.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(13.0, egui::FontFamily::Monospace),
    );
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(16.0, egui::FontFamily::Monospace),
    );
    ctx.set_style(style);
}

/// Apply the Modern Dark theme — clean, high contrast, rounded corners.
pub fn apply_modern_dark(ctx: &egui::Context) {
    let visuals = Visuals {
        dark_mode: true,
        override_text_color: Some(Color32::from_rgb(220, 220, 230)),
        panel_fill: Color32::from_rgb(27, 27, 37),
        window_fill: Color32::from_rgb(30, 30, 42),
        faint_bg_color: Color32::from_rgb(35, 35, 48),
        extreme_bg_color: Color32::from_rgb(18, 18, 26),
        code_bg_color: Color32::from_rgb(30, 30, 42),
        warn_fg_color: Color32::from_rgb(255, 200, 50),
        error_fg_color: Color32::from_rgb(255, 80, 80),
        window_corner_radius: 6.0.into(),
        window_stroke: egui::Stroke::new(1.0, Color32::from_rgb(50, 50, 65)),
        selection: egui::style::Selection {
            bg_fill: Color32::from_rgb(80, 100, 200),
            stroke: egui::Stroke::new(1.0, Color32::from_rgb(140, 160, 255)),
        },
        hyperlink_color: Color32::from_rgb(120, 180, 255),
        ..Visuals::dark()
    };

    ctx.set_visuals(visuals);
}
