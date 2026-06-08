//! Audio playback engine — simple approach for Phase 1.
//!
//! The Module is moved into a dedicated playback thread that owns the
//! XmrsPlayer. Audio samples are sent to cpal via a lock-free ring buffer.
//! On stop, the Module is returned to the caller.

use crate::state::VizBuffer;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;
use xmrs::prelude::Module;
use xmrsplayer::prelude::XmrsPlayer;

/// Playback state reported from the audio thread.
#[derive(Debug, Clone, Default)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_pattern: usize,
    pub current_row: usize,
    pub elapsed_samples: u64,
    pub bpm: usize,
    pub tempo: usize,
}

/// Holds the running playback session.
pub struct AudioEngine {
    /// Shared state updated by the audio thread.
    pub state: Arc<Mutex<PlaybackState>>,
    /// Signal the thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// The cpal audio stream — dropped on stop.
    _stream: Option<cpal::Stream>,
    /// Handle for joining the player thread.
    player_thread: Option<thread::JoinHandle<Option<Module>>>,
    /// The raw module bytes, kept so we can re-load.
    #[allow(dead_code)]
    pub module_data: Option<Vec<u8>>,
}

impl AudioEngine {
    /// Start playback. Takes ownership of the Module; returns it on stop.
    pub fn start(
        module: Module,
        module_data: Vec<u8>,
        song: usize,
        viz_mix: VizBuffer,
        viz_chan: VizBuffer,
    ) -> Result<Self> {
        // --- Set up cpal ---
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .context("No audio output device found")?;

        let supported_config = device.default_output_config()?;
        let sample_rate: u32 = supported_config.sample_rate();

        info!(
            "Audio: {} Hz, {:?} channels",
            sample_rate,
            supported_config.channels()
        );

        // --- Create ring buffer for samples ---
        // ~100ms buffer at 48kHz stereo i16 = 9600 samples
        let ring = HeapRb::<i16>::new(16384);
        let (mut producer, mut consumer) = ring.split();

        // --- Shared state ---
        let state = Arc::new(Mutex::new(PlaybackState {
            is_playing: true,
            ..Default::default()
        }));
        let stop_flag = Arc::new(AtomicBool::new(false));

        let state_for_player = state.clone();
        let stop_for_player = stop_flag.clone();

        // --- Spawn player thread ---
        let viz_mix_clone = viz_mix.clone();
        let _viz_chan_clone = viz_chan.clone();
        let player_thread = thread::spawn(move || -> Option<Module> {
            let mut player = XmrsPlayer::new(&module, sample_rate, song);
            info!("Playback started: {} Hz, song {}", sample_rate, song);

            loop {
                if stop_for_player.load(Ordering::Acquire) {
                    info!("Playback stopped by user");
                    break;
                }

                let left = match player.next() {
                    Some(s) => s,
                    None => {
                        info!("Playback finished (end of module)");
                        break;
                    }
                };
                let right = player.next().unwrap_or(0);

                while producer.try_push(left).is_err() {
                    if stop_for_player.load(Ordering::Acquire) {
                        return Some(module);
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                while producer.try_push(right).is_err() {
                    if stop_for_player.load(Ordering::Acquire) {
                        return Some(module);
                    }
                    thread::sleep(Duration::from_millis(1));
                }

                // Push to viz buffer
                viz_mix_clone.push(left);
                viz_mix_clone.push(right);

                let elapsed = player.generated_samples();
                if elapsed % (sample_rate as u64 / 10) == 0 {
                    if let Ok(mut s) = state_for_player.lock() {
                        s.current_pattern = player.playing_pattern();
                        s.current_row = player.playing_row();
                        s.elapsed_samples = elapsed;
                        s.bpm = player.get_bpm();
                        s.tempo = player.get_tempo();
                    }
                }
            }

            Some(module)
        });

        // --- Build cpal output stream ---
        let stream: cpal::Stream = {
            let stream_config = cpal::StreamConfig {
                channels: supported_config.channels(),
                sample_rate: supported_config.sample_rate(),
                buffer_size: cpal::BufferSize::Default,
            };

            let error_callback = |err| error!("Audio stream error: {}", err);

            match supported_config.sample_format() {
                cpal::SampleFormat::F32 => device.build_output_stream(
                    stream_config,
                    move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                        for frame in data.chunks_mut(2) {
                            let l = consumer.try_pop().unwrap_or(0) as f32 / 32768.0;
                            let r = if frame.len() > 1 {
                                consumer.try_pop().unwrap_or(0) as f32 / 32768.0
                            } else {
                                0.0
                            };
                            frame[0] = l;
                            if frame.len() > 1 {
                                frame[1] = r;
                            }
                        }
                    },
                    error_callback,
                    None,
                )?,
                _ => device.build_output_stream(
                    stream_config,
                    move |data: &mut [i16], _: &cpal::OutputCallbackInfo| {
                        for frame in data.chunks_mut(2) {
                            frame[0] = consumer.try_pop().unwrap_or(0);
                            if frame.len() > 1 {
                                frame[1] = consumer.try_pop().unwrap_or(0);
                            }
                        }
                    },
                    error_callback,
                    None,
                )?,
            }
        };

        stream.play()?;
        info!("Audio stream started");

        Ok(AudioEngine {
            state,
            stop_flag,
            _stream: Some(stream),
            player_thread: Some(player_thread),
            module_data: Some(module_data),
        })
    }

    /// Stop playback. Returns the Module if the thread has finished.
    pub fn stop(&mut self) -> Option<Module> {
        self.stop_flag.store(true, Ordering::Release);

        // Drop the stream first to stop audio callbacks
        self._stream = None;

        let module = if let Some(handle) = self.player_thread.take() {
            match handle.join() {
                Ok(module_opt) => module_opt,
                Err(e) => {
                    warn!("Player thread panicked: {:?}", e);
                    None
                }
            }
        } else {
            None
        };

        if let Ok(mut s) = self.state.lock() {
            s.is_playing = false;
        }

        module
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop();
    }
}
