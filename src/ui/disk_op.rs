//! Disk operations — file browser, load/save module and samples.

use egui::{ScrollArea, Ui};
use std::path::{Path, PathBuf};

/// File browser and disk operations panel.
pub struct DiskOp {
    /// Current directory being browsed.
    current_dir: PathBuf,
    /// Directory entries in the current directory.
    entries: Vec<DirEntry>,
    /// Selected entry index.
    selected: Option<usize>,
    /// Whether the panel is visible.
    pub visible: bool,
    /// Pending file to load (set when user double-clicks a module file).
    pub pending_load: Option<PathBuf>,
    /// Pending file to save to.
    pub pending_save: Option<PathBuf>,
    /// File filter (e.g. "xm", "mod", etc.)
    filter: Vec<String>,
    /// Show only directories and supported files.
    filter_enabled: bool,
}

#[derive(Debug, Clone)]
struct DirEntry {
    name: String,
    path: PathBuf,
    is_dir: bool,
    size: u64,
    ext: String,
}

impl DiskOp {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        let mut disk = Self {
            current_dir,
            entries: Vec::new(),
            selected: None,
            visible: true,
            pending_load: None,
            pending_save: None,
            filter: vec![
                "xm".into(), "mod".into(), "s3m".into(), "it".into(),
                "xi".into(), "wav".into(), "aiff".into(), "flac".into(),
                "ogg".into(), "mp3".into(),
            ],
            filter_enabled: true,
        };
        disk.refresh();
        disk
    }

    /// Refresh the directory listing.
    fn refresh(&mut self) {
        self.entries.clear();
        self.selected = None;

        if let Ok(read_dir) = std::fs::read_dir(&self.current_dir) {
            let mut entries: Vec<DirEntry> = Vec::new();

            // Parent directory
            if let Some(parent) = self.current_dir.parent() {
                entries.push(DirEntry {
                    name: "..".to_string(),
                    path: parent.to_path_buf(),
                    is_dir: true,
                    size: 0,
                    ext: String::new(),
                });
            }

            for entry in read_dir.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();
                let is_dir = path.is_dir();
                let size = if is_dir { 0 } else { entry.metadata().map(|m| m.len()).unwrap_or(0) };
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();

                if self.filter_enabled && !is_dir && !self.filter.contains(&ext) {
                    continue;
                }

                entries.push(DirEntry {
                    name,
                    path,
                    is_dir,
                    size,
                    ext,
                });
            }

            // Sort: directories first, then alphabetical
            entries.sort_by(|a, b| {
                b.is_dir
                    .cmp(&a.is_dir)
                    .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
            });

            self.entries = entries;
        }
    }

    /// Navigate into a directory.
    fn enter_dir(&mut self, path: &Path) {
        self.current_dir = path.to_path_buf();
        self.refresh();
    }

    /// Render the disk operations panel.
    pub fn show(&mut self, ui: &mut Ui) {
        // Current path
        ui.horizontal(|ui| {
            if ui.button("🏠").clicked() {
                if let Some(home) = dirs::home_dir() {
                    self.enter_dir(&home);
                }
            }
            if ui.button("⬆").clicked() {
                if let Some(parent) = self.current_dir.parent() {
                    self.enter_dir(&PathBuf::from(parent));
                }
            }
            ui.label(self.current_dir.display().to_string());
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.filter_enabled, "Filter: tracker/sample files");
        });

        ui.separator();

        // File list
        let available = ui.available_height() - 60.0;
        let mut dir_to_enter: Option<PathBuf> = None;

        ScrollArea::vertical()
            .max_height(available)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for (i, entry) in self.entries.iter().enumerate() {
                    let icon = if entry.is_dir { "📁" } else { "📄" };
                    let label = format!("{} {}", icon, entry.name);

                    let is_selected = self.selected == Some(i);
                    let response = ui.selectable_label(is_selected, &label);

                    if response.clicked() {
                        self.selected = Some(i);
                    }

                    if response.double_clicked() {
                        if entry.is_dir {
                            dir_to_enter = Some(entry.path.clone());
                        } else {
                            let ext = entry.ext.as_str();
                            if ["xm", "mod", "s3m", "it"].contains(&ext) {
                                self.pending_load = Some(entry.path.clone());
                            }
                        }
                    }
                }
            });

        if let Some(dir) = dir_to_enter {
            self.enter_dir(&dir);
        }

        ui.separator();

        // Action buttons
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.selected.is_some(),
                    egui::Button::new("Load Module"),
                )
                .clicked()
            {
                if let Some(idx) = self.selected {
                    if idx < self.entries.len() {
                        let path = self.entries[idx].path.clone();
                        if path.is_file() {
                            self.pending_load = Some(path);
                        }
                    }
                }
            }

            if ui.button("Refresh").clicked() {
                self.refresh();
            }
        });
    }
}
