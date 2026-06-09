//! rust-tracker — A modern Fast Tracker 2 clone for Linux.
//!
//! Usage:
//!   rust-tracker [FILE]           Open and play a tracker module
//!   rust-tracker --render FILE    Render module to WAV (headless)

#![allow(deprecated)]

mod app;
mod audio;
mod config;
mod midi;
mod module;
mod state;
mod ui;
mod undo;

use app::RustTracker;
use clap::Parser;
use log::info;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    name = "rust-tracker",
    version = "0.1.0",
    about = "A modern Fast Tracker 2 clone for Linux"
)]
struct Cli {
    /// Tracker module file to open (.xm, .mod, .s3m, .it)
    file: Option<PathBuf>,

    /// Render module to WAV and exit (headless mode)
    #[arg(long)]
    render: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    let cli = Cli::parse();

    // Headless render mode
    if let Some(output) = cli.render {
        let input = cli.file.as_ref().expect("Input file required for --render");
        return render_to_wav(input, &output);
    }

    // --- GUI mode ---
    info!("Starting rust-tracker GUI...");

    let event_loop = winit::event_loop::EventLoop::new()?;

    // Use Arc<Window> so both the surface and the event loop can hold a reference
    let window_attrs = winit::window::WindowAttributes::default()
        .with_title("rust-tracker")
        .with_inner_size(winit::dpi::LogicalSize::new(1024, 768));
    let window = Arc::new(event_loop.create_window(window_attrs)?);

    // Set up wgpu for rendering
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
    let surface = instance.create_surface(window.clone())?;

    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
    }))
    .expect("Failed to find suitable GPU adapter");

    let (device, queue) = pollster::block_on(adapter.request_device(
        &wgpu::DeviceDescriptor {
            required_features: wgpu::Features::default(),
            required_limits: wgpu::Limits::default(),
            label: Some("main_device"),
            memory_hints: Default::default(),
        },
        None,
    ))?;

    let device = Arc::new(device);
    let queue = Arc::new(queue);

    // Configure surface
    let window_size = window.inner_size();
    let mut surface_config = surface
        .get_default_config(&adapter, window_size.width, window_size.height)
        .expect("Failed to get default surface config");
    surface.configure(&device, &surface_config);

    // Set up egui
    let egui_ctx = egui::Context::default();
    let mut egui_winit_state = egui_winit::State::new(
        egui_ctx.clone(),
        egui::ViewportId::ROOT,
        &*window,
        Some(window.scale_factor() as f32),
        None,
        None,
    );

    let mut egui_renderer =
        egui_wgpu::Renderer::new(&device, surface_config.format, None, 1, false);

    // Create the application
    let mut app = RustTracker::new(cli.file);

    event_loop.run(move |event, elwt| {
        match event {
            winit::event::Event::WindowEvent { event, .. } => {
                let response = egui_winit_state.on_window_event(&window, &event);
                if response.repaint {
                    window.request_redraw();
                }

                match event {
                    winit::event::WindowEvent::CloseRequested => {
                        // Save config before exit
                        if let Err(e) = app.state.config.save() {
                            log::error!("Failed to save config: {}", e);
                        }
                        elwt.exit();
                    }
                    winit::event::WindowEvent::Resized(new_size) => {
                        surface_config.width = new_size.width;
                        surface_config.height = new_size.height;
                        surface.configure(&device, &surface_config);
                    }
                    winit::event::WindowEvent::RedrawRequested => {
                        let raw_input = egui_winit_state.take_egui_input(&window);
                        let full_output = egui_ctx.run(raw_input, |ctx| {
                            app.update(ctx);
                        });

                        if app.quit_requested {
                            if let Err(e) = app.state.config.save() {
                                log::error!("Failed to save config: {}", e);
                            }
                            elwt.exit();
                            return;
                        }

                        egui_winit_state.handle_platform_output(
                            &window,
                            full_output.platform_output,
                        );

                        let paint_jobs =
                            egui_ctx.tessellate(full_output.shapes, egui_ctx.pixels_per_point());

                        for (id, image_delta) in &full_output.textures_delta.set {
                            egui_renderer.update_texture(&device, &queue, *id, image_delta);
                        }

                        match surface.get_current_texture() {
                            Ok(output_frame) => {
                                let view = output_frame
                                    .texture
                                    .create_view(&wgpu::TextureViewDescriptor::default());

                                let mut encoder = device.create_command_encoder(
                                    &wgpu::CommandEncoderDescriptor {
                                        label: Some("egui_encoder"),
                                    },
                                );

                                let screen_descriptor = egui_wgpu::ScreenDescriptor {
                                    size_in_pixels: [
                                        surface_config.width,
                                        surface_config.height,
                                    ],
                                    pixels_per_point: window.scale_factor() as f32,
                                };

                                egui_renderer.update_buffers(
                                    &device,
                                    &queue,
                                    &mut encoder,
                                    &paint_jobs,
                                    &screen_descriptor,
                                );

                                // Scoped render pass — use forget_lifetime for egui_wgpu
                                {
                                    let render_pass = encoder.begin_render_pass(
                                        &wgpu::RenderPassDescriptor {
                                            label: Some("egui_render_pass"),
                                            color_attachments: &[Some(
                                                wgpu::RenderPassColorAttachment {
                                                    view: &view,
                                                    resolve_target: None,
                                                    ops: wgpu::Operations {
                                                        load: wgpu::LoadOp::Clear(
                                                            wgpu::Color::BLACK,
                                                        ),
                                                        store: wgpu::StoreOp::Store,
                                                    },
                                                },
                                            )],
                                            depth_stencil_attachment: None,
                                            timestamp_writes: None,
                                            occlusion_query_set: None,
                                        },
                                    );

                                    // egui_wgpu requires RenderPass<'static>,
                                    // so we forget the lifetime here.
                                    let mut render_pass = render_pass.forget_lifetime();

                                    egui_renderer.render(
                                        &mut render_pass,
                                        &paint_jobs,
                                        &screen_descriptor,
                                    );
                                    // render_pass is dropped here
                                }

                                // Safe to call finish() because render_pass has been dropped
                                queue.submit(std::iter::once(encoder.finish()));
                                output_frame.present();
                            }
                            Err(wgpu::SurfaceError::Outdated) => {
                                surface.configure(&device, &surface_config);
                            }
                            Err(e) => {
                                log::error!("Surface error: {:?}", e);
                            }
                        }

                        for id in &full_output.textures_delta.free {
                            egui_renderer.free_texture(id);
                        }
                    }
                    _ => {}
                }
            }
            winit::event::Event::AboutToWait => {
                // Only request redraw if playing (for viz updates)
                if app.state.is_playing() {
                    window.request_redraw();
                }
            }
            _ => {}
        }
    })?;

    // Save config on exit
    let _ = app;

    Ok(())
}

/// Headless mode: render a module to WAV file.
fn render_to_wav(input: &std::path::Path, output: &std::path::Path) -> anyhow::Result<()> {
    use xmrsplayer::prelude::*;

    let data = std::fs::read(input)?;
    let module = xmrs::prelude::Module::load(&data)
        .map_err(|e| anyhow::anyhow!("Failed to load module: {:?}", e))?;

    let sample_rate = 44100;
    let mut player = XmrsPlayer::new(&module, sample_rate, 0);

    let mut samples: Vec<i16> = Vec::new();

    while let Some(s) = player.next() {
        samples.push(s);
    }

    // Save to WAV
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let mut writer = hound::WavWriter::create(output, spec)?;
    for chunk in samples.chunks(2) {
        if chunk.len() == 2 {
            writer.write_sample(chunk[0])?;
            writer.write_sample(chunk[1])?;
        }
    }

    println!(
        "Rendered {} samples ({:.1}s) to {}",
        samples.len(),
        samples.len() as f64 / sample_rate as f64 / 2.0,
        output.display()
    );

    Ok(())
}
