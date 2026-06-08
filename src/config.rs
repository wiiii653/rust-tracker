//! Application configuration — stored in $XDG_CONFIG_HOME/rust-tracker/

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// Last opened directory for file dialogs.
    pub last_directory: Option<PathBuf>,
    /// Recently opened files (most recent first).
    pub recent_files: Vec<PathBuf>,
    /// Playback amplification (0.0 - 2.0).
    pub amplification: f32,
    /// Whether to loop the song.
    pub loop_song: bool,
    /// Maximum pattern loop count before escaping.
    pub max_loop_count: usize,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            last_directory: None,
            recent_files: Vec::new(),
            amplification: 1.0,
            loop_song: false,
            max_loop_count: 16,
        }
    }
}

impl AppConfig {
    /// Resolve config path: $XDG_CONFIG_HOME/rust-tracker/config.json
    fn config_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("No XDG config directory")?
            .join("rust-tracker");
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("Failed to create config dir: {}", dir.display()))?;
        Ok(dir.join("config.json"))
    }

    /// Load config from disk, falling back to defaults.
    pub fn load() -> Self {
        let path = match Self::config_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };

        match std::fs::read_to_string(&path) {
            Ok(data) => serde_json::from_str(&data).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, data)?;
        Ok(())
    }

    /// Add a file to recent files list (most recent first, max 10 entries, deduplicated).
    pub fn add_recent(&mut self, path: PathBuf) {
        self.recent_files.retain(|p| p != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(10);
    }
}
