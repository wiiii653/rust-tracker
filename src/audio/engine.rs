//! Audio playback engine.
//!
//! The UI keeps the authoritative editable `Module`. The audio thread owns a
//! separate playback copy and receives live edit messages. Because
//! `xmrsplayer::XmrsPlayer` borrows its `Module` immutably, so the audio
//! thread keeps a separate playback snapshot and rebuilds the player when the
//! snapshot changes.

use crate::state::VizBuffer;
use crate::audio::runtime::PlaybackSnapshot;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use log::{error, info, warn};
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc,
    Arc, Mutex,
};
use std::thread;
use std::time::Duration;
use xmrs::edit::EditCommand;
use xmrs::prelude::Module;
use xmrsplayer::prelude::XmrsPlayer;

/// Playback state reported from the audio thread.
#[derive(Debug, Clone)]
pub struct PlaybackState {
    pub is_playing: bool,
    pub current_pattern: usize,
    pub current_order: usize,
    pub current_row: usize,
    pub elapsed_samples: u64,
    pub bpm: usize,
    pub tempo: usize,
    pub module_revision: u64,
    /// Sample rate in Hz (set once at start).
    pub sample_rate: u32,
}

impl Default for PlaybackState {
    fn default() -> Self {
        Self {
            is_playing: false,
            current_pattern: 0,
            current_order: 0,
            current_row: 0,
            elapsed_samples: 0,
            bpm: 0,
            tempo: 0,
            module_revision: 0,
            sample_rate: 44100,
        }
    }
}

enum AudioCommand {
    ApplyEdit { revision: u64, command: EditCommand },
    ReplaceModule { revision: u64, module: Module },
}

enum PendingPlaybackUpdate {
    /// Only individual pattern edits (no module replacement).
    ApplyEdits { revision: u64, commands: Vec<EditCommand> },
    /// Full module replacement. Edits that arrived *after* the replacement
    /// was queued are applied on top of the new module.
    ReplaceModule {
        revision: u64,
        module: Module,
        follow_up_edits: Vec<EditCommand>,
    },
}

#[derive(Clone, Copy)]
struct ResumeCursor {
    table_index: usize,
    row: usize,
    tempo: usize,
}

/// Holds the running playback session.
pub struct AudioEngine {
    /// Shared state updated by the audio thread.
    pub state: Arc<Mutex<PlaybackState>>,
    /// Signal the thread to stop.
    stop_flag: Arc<AtomicBool>,
    /// Command sender for live edits.
    command_tx: mpsc::Sender<AudioCommand>,
    /// The cpal audio stream — dropped on stop.
    _stream: Option<cpal::Stream>,
    /// Handle for joining the player thread.
    player_thread: Option<thread::JoinHandle<()>>,
}

impl AudioEngine {
    /// Start playback. The audio thread owns its own playback copy of the module.
    /// `amplification` is a volume multiplier applied to all output samples (0.0–2.0).
    pub fn start(
        module: Module,
        revision: u64,
        song: usize,
        viz_mix: VizBuffer,
        viz_chan: VizBuffer,
        amplification: f32,
    ) -> Result<Self> {
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

        let ring = HeapRb::<i16>::new(16384);
        let (mut producer, mut consumer) = ring.split();

        let state = Arc::new(Mutex::new(PlaybackState {
            is_playing: true,
            module_revision: revision,
            sample_rate,
            ..Default::default()
        }));
        let stop_flag = Arc::new(AtomicBool::new(false));
        let (command_tx, command_rx) = mpsc::channel();

        let state_for_player = state.clone();
        let stop_for_player = stop_flag.clone();
        let viz_mix_clone = viz_mix.clone();
        let _viz_chan_clone = viz_chan.clone();
        let amp = amplification.clamp(0.0, 2.0);

        let player_thread = thread::spawn(move || {
            let mut playback = PlaybackSnapshot::new(module, revision, song);
            let mut player = build_player(playback.module(), sample_rate, playback.song(), None);
            let mut last_row = player.playing_row();

            info!("Playback started: {} Hz, song {}", sample_rate, song);

            loop {
                if stop_for_player.load(Ordering::Acquire) {
                    info!("Playback stopped by user");
                    break;
                }

                if player.playing_row() != last_row {
                    last_row = player.playing_row();
                    if let Some(update) = drain_pending_commands(&command_rx) {
                        let resume = ResumeCursor {
                            table_index: player.get_current_table_index(),
                            row: player.get_current_row(),
                            tempo: player.get_tempo(),
                        };
                        drop(player);

                        let apply_result: Result<()> = match update {
                            PendingPlaybackUpdate::ReplaceModule {
                                revision,
                                module,
                                follow_up_edits,
                            } => {
                                playback.replace_module(module, revision);
                                for cmd in follow_up_edits {
                                    if let Err(e) = playback.apply_edit(cmd) {
                                        warn!(
                                            "Follow-up live edit on replaced module failed: {:?}",
                                            e
                                        );
                                        break;
                                    }
                                }
                                if let Ok(mut s) = state_for_player.lock() {
                                    s.module_revision = playback.revision();
                                }
                                Ok(())
                            }
                            PendingPlaybackUpdate::ApplyEdits { revision, commands } => {
                                let mut apply_error = None;
                                for cmd in commands {
                                    if let Err(e) = playback.apply_edit(cmd) {
                                        apply_error = Some(anyhow::anyhow!(
                                            "Playback module edit failed: {:?}",
                                            e
                                        ));
                                        break;
                                    }
                                }
                                if apply_error.is_none() {
                                    if let Ok(mut s) = state_for_player.lock() {
                                        s.module_revision = revision;
                                    }
                                }
                                apply_error.map_or(Ok(()), Err)
                            }
                        };

                        if let Err(e) = apply_result {
                            warn!("Failed to apply live playback edit: {}", e);
                            player = build_player(playback.module(), sample_rate, playback.song(), Some(resume));
                        } else {
                            player = build_player(playback.module(), sample_rate, playback.song(), Some(resume));
                            last_row = player.playing_row();
                        }
                    }
                }

                // Apply amplification while we still own the sample values
                let left = match player.next() {
                    Some(s) => (s as f32 * amp).clamp(-32768.0, 32767.0) as i16,
                    None => {
                        info!("Playback finished (end of module)");
                        break;
                    }
                };
                let right = (player.next().unwrap_or(0) as f32 * amp)
                    .clamp(-32768.0, 32767.0) as i16;

                // Push samples to ring buffer with backpressure timeout.
                // If the audio stream stalls (e.g. device unplugged), we
                // break out after ~200ms to avoid hanging forever.
                let mut retries = 0u32;
                while producer.try_push(left).is_err() {
                    if stop_for_player.load(Ordering::Acquire) {
                        return;
                    }
                    retries += 1;
                    if retries > 200 {
                        warn!("Audio backpressure timeout — dropping sample");
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                retries = 0u32;
                while producer.try_push(right).is_err() {
                    if stop_for_player.load(Ordering::Acquire) {
                        return;
                    }
                    retries += 1;
                    if retries > 200 {
                        warn!("Audio backpressure timeout — dropping sample");
                        break;
                    }
                    thread::sleep(Duration::from_millis(1));
                }

                viz_mix_clone.push(left);
                viz_mix_clone.push(right);

                let elapsed = player.generated_samples();
                if elapsed % (sample_rate as u64 / 10) == 0 {
                    if let Ok(mut s) = state_for_player.lock() {
                        let pat_idx = player.playing_pattern();
                        s.current_pattern = pat_idx;
                        // Map pattern index → order index via timeline.
                        s.current_order = playback
                            .module()
                            .timeline_map
                            .entries
                            .iter()
                            .find(|e| {
                                e.pattern_idx as usize == pat_idx
                                    && e.loop_iter == 0
                                    && e.row_idx == 0
                            })
                            .map(|e| e.order_idx as usize)
                            .unwrap_or(s.current_order);
                        s.current_row = player.playing_row();
                        s.elapsed_samples = elapsed;
                        s.bpm = player.get_bpm();
                        s.tempo = player.get_tempo();
                    }
                }
            }
        });

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

        Ok(Self {
            state,
            stop_flag,
            command_tx,
            _stream: Some(stream),
            player_thread: Some(player_thread),
        })
    }

    pub fn apply_edit(&self, revision: u64, cmd: EditCommand) -> Result<()> {
        self.command_tx
            .send(AudioCommand::ApplyEdit {
                revision,
                command: cmd,
            })
            .map_err(|e| anyhow::anyhow!("Failed to queue live edit: {}", e))
    }

    pub fn replace_module(&self, revision: u64, module: Module) -> Result<()> {
        self.command_tx
            .send(AudioCommand::ReplaceModule {
                revision,
                module,
            })
            .map_err(|e| anyhow::anyhow!("Failed to queue playback resync: {}", e))
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.stop_flag.store(true, Ordering::Release);
        self._stream = None;

        if let Some(handle) = self.player_thread.take() {
            if let Err(e) = handle.join() {
                warn!("Player thread panicked: {:?}", e);
            }
        }

        if let Ok(mut s) = self.state.lock() {
            s.is_playing = false;
        }
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        self.stop();
    }
}

fn build_player<'a>(
    module: &'a Module,
    sample_rate: u32,
    song: usize,
    resume: Option<ResumeCursor>,
) -> XmrsPlayer<'a> {
    let mut player = XmrsPlayer::new(module, sample_rate, song);
    if let Some(resume) = resume {
        let _ = player.goto(resume.table_index, resume.row, resume.tempo);
    }
    player
}

fn drain_pending_commands(
    command_rx: &mpsc::Receiver<AudioCommand>,
) -> Option<PendingPlaybackUpdate> {
    let mut latest_revision = 0u64;
    let mut replace_module: Option<Module> = None;
    // Edits that arrived *before* the last ReplaceModule are already
    // included in that module snapshot (undo.execute ran first), so
    // they are discarded. Edits after the last ReplaceModule must be
    // applied on top.
    let mut edits_after_replace: Vec<EditCommand> = Vec::new();
    let mut seen_replace = false;

    while let Ok(command) = command_rx.try_recv() {
        match command {
            AudioCommand::ApplyEdit { revision, command } => {
                latest_revision = latest_revision.max(revision);
                if seen_replace {
                    edits_after_replace.push(command);
                }
                // Before any ReplaceModule: edits are already baked into
                // the snapshot that the next ReplaceModule will carry.
            }
            AudioCommand::ReplaceModule { revision, module } => {
                latest_revision = latest_revision.max(revision);
                replace_module = Some(module);
                seen_replace = true;
                // A later ReplaceModule supersedes an earlier one and
                // its preceding edits are already included.
                edits_after_replace.clear();
            }
        }
    }

    if !seen_replace && edits_after_replace.is_empty() {
        // No ReplaceModule and no edits → nothing to do.
        return None;
    }

    if let Some(module) = replace_module {
        Some(PendingPlaybackUpdate::ReplaceModule {
            revision: latest_revision,
            module,
            follow_up_edits: edits_after_replace,
        })
    } else {
        // Only edits, no replace.
        Some(PendingPlaybackUpdate::ApplyEdits {
            revision: latest_revision,
            commands: edits_after_replace,
        })
    }
}
