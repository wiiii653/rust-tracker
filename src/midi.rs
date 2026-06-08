//! MIDI input — connect to MIDI devices and convert events to note input.

use anyhow::Result;
use log::{info, warn};
use std::sync::mpsc;

/// A MIDI note event ready for the pattern editor.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct MidiNoteEvent {
    /// MIDI note number (0-127).
    pub note: u8,
    /// Velocity (0-127).
    pub velocity: u8,
    /// True if note on, false if note off.
    pub on: bool,
    /// MIDI channel (0-15).
    pub channel: u8,
}

/// MIDI input manager.
pub struct MidiInput {
    /// Receiver for MIDI events.
    receiver: Option<mpsc::Receiver<MidiNoteEvent>>,
    /// Whether MIDI is enabled.
    pub enabled: bool,
    /// Name of connected device (if any).
    pub device_name: Option<String>,
}

impl MidiInput {
    pub fn new() -> Self {
        Self {
            receiver: None,
            enabled: false,
            device_name: None,
        }
    }

    /// Try to connect to the first available MIDI input device.
    pub fn connect(&mut self, port_name: Option<&str>) -> Result<()> {
        let midi_in = midir::MidiInput::new("rust-tracker")?;

        let ports = midi_in.ports();
        if ports.is_empty() {
            return Err(anyhow::anyhow!("No MIDI input devices found"));
        }

        let port = if let Some(name) = port_name {
            ports
                .iter()
                .find(|p| midi_in.port_name(p).map(|n| n.contains(name)).unwrap_or(false))
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("MIDI port '{}' not found", name))?
        } else {
            ports[0].clone()
        };

        let port_name = midi_in.port_name(&port).unwrap_or_else(|_| "Unknown".to_string());
        let (tx, rx) = mpsc::channel();

        // Keep connection alive (leak is intentional — MIDI runs for app lifetime)
        // We must use map_err because the ConnectError doesn't implement Sync
        let _conn = midi_in.connect(
            &port,
            "rust-tracker-midi",
            move |_timestamp, message, _| {
                if message.len() >= 3 {
                    let status = message[0];
                    let data1 = message[1];
                    let data2 = message[2];
                    let channel = status & 0x0F;
                    let msg_type = status & 0xF0;
                    match msg_type {
                        0x90 => {
                            if data2 > 0 {
                                let _ = tx.send(MidiNoteEvent { note: data1, velocity: data2, on: true, channel });
                            } else {
                                let _ = tx.send(MidiNoteEvent { note: data1, velocity: 0, on: false, channel });
                            }
                        }
                        0x80 => {
                            let _ = tx.send(MidiNoteEvent { note: data1, velocity: data2, on: false, channel });
                        }
                        _ => {}
                    }
                }
            },
            |err: &midir::ConnectError<midir::MidiInput>| { warn!("MIDI error: {}", err); },
        ).map_err(|e| anyhow::anyhow!("MIDI connect error: {:?}", e))?;

        std::mem::forget(_conn);

        self.receiver = Some(rx);
        self.device_name = Some(port_name);
        self.enabled = true;

        info!("Connected to MIDI device: {}", self.device_name.as_ref().unwrap());
        Ok(())
    }

    /// Poll for pending MIDI events.
    pub fn poll(&self) -> Vec<MidiNoteEvent> {
        let mut events = Vec::new();
        if let Some(ref rx) = self.receiver {
            while let Ok(event) = rx.try_recv() {
                events.push(event);
            }
        }
        events
    }
}
