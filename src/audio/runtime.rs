//! Playback runtime snapshot.
//!
//! The editor owns the authoritative song data. The audio thread keeps a
//! separate playback snapshot so runtime state can evolve independently from
//! the UI-owned module.

use anyhow::{anyhow, Result};
use xmrs::edit::EditCommand;
use xmrs::prelude::Module;

/// Audio-thread-owned playback snapshot.
#[derive(Debug)]
pub struct PlaybackSnapshot {
    module: Module,
    revision: u64,
    song: usize,
}

impl PlaybackSnapshot {
    /// Create a new snapshot from an authored module.
    pub fn new(module: Module, revision: u64, song: usize) -> Self {
        Self {
            module,
            revision,
            song,
        }
    }

    /// Current revision of the playback copy.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Current song index inside the module.
    pub fn song(&self) -> usize {
        self.song
    }

    /// Borrow the playback module.
    pub fn module(&self) -> &Module {
        &self.module
    }

    /// Replace the playback snapshot wholesale.
    pub fn replace_module(&mut self, module: Module, revision: u64) {
        self.module = module;
        self.revision = revision;
    }

    /// Apply a live edit to the playback snapshot.
    pub fn apply_edit(&mut self, command: EditCommand) -> Result<()> {
        self.module
            .apply(command)
            .map_err(|e| anyhow!("Failed to apply live edit to playback snapshot: {:?}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_module_updates_revision() {
        let snapshot = PlaybackSnapshot::new(Module::default(), 2, 1);
        assert_eq!(snapshot.revision(), 2);
        assert_eq!(snapshot.song(), 1);
    }

    #[test]
    fn module_borrow_is_stable() {
        let snapshot = PlaybackSnapshot::new(Module::default(), 7, 0);
        let _module = snapshot.module();
        assert_eq!(snapshot.revision(), 7);
    }
}
