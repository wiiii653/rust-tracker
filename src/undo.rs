//! Undo/Redo stack built on xmrs's `EditCommand` / `EditReceipt` system.
//!
//! Every mutation to the Module goes through `AppState::execute()`, which
//! pushes the receipt onto the undo stack. Undo pops and calls
//! `Module::undo()`, producing a new receipt that goes onto the redo stack.

use xmrs::prelude::*;
use xmrs::edit::EditCommand;

/// Maximum number of undo steps to keep.
const MAX_UNDO_STEPS: usize = 256;

/// An edit command with its receipt for undoing.
struct UndoEntry {
    receipt: xmrs::edit::EditReceipt,
}

struct RedoEntry {
    command: EditCommand,
}

/// The undo/redo manager.
#[derive(Default)]
pub struct UndoManager {
    /// Stack of undo entries (most recent last).
    undo_stack: Vec<UndoEntry>,
    /// Stack of redo entries (most recent last).
    redo_stack: Vec<RedoEntry>,
}

impl UndoManager {
    pub fn new() -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Execute a command and push it onto the undo stack.
    /// Clears the redo stack (new action after undo invalidates redo).
    pub fn execute(
        &mut self,
        module: &mut Module,
        cmd: EditCommand,
    ) -> Result<(), String> {
        let receipt = module.apply(cmd).map_err(|e| format!("{:?}", e))?;
        self.undo_stack.push(UndoEntry { receipt });
        self.redo_stack.clear();

        // Trim undo stack if too large
        while self.undo_stack.len() > MAX_UNDO_STEPS {
            self.undo_stack.remove(0);
        }

        Ok(())
    }

    /// Undo the last action. Returns true if something was undone.
    pub fn undo(&mut self, module: &mut Module) -> Result<bool, String> {
        if let Some(entry) = self.undo_stack.pop() {
            // Undo produces a new receipt; push it to redo
            match module.undo(&entry.receipt) {
                Ok(()) => {
                    self.redo_stack.push(RedoEntry {
                        command: entry.receipt.command.clone(),
                    });
                    Ok(true)
                }
                Err(e) => {
                    // Put back on stack on failure
                    self.undo_stack.push(entry);
                    Err(format!("Undo failed: {:?}", e))
                }
            }
        } else {
            Ok(false)
        }
    }

    /// Redo the last undone action. Returns true if something was redone.
    pub fn redo(&mut self, module: &mut Module) -> Result<bool, String> {
        if let Some(entry) = self.redo_stack.pop() {
            match module.apply(entry.command) {
                Ok(receipt) => {
                    self.undo_stack.push(UndoEntry { receipt });
                    Ok(true)
                }
                Err(e) => {
                    Err(format!("Redo failed: {:?}", e))
                }
            }
        } else {
            Ok(false)
        }
    }

    /// Whether undo is available.
    #[expect(dead_code, reason = "useful for UI feedback like menu item enablement")]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    /// Whether redo is available.
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Clear both stacks.
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Number of undo steps available.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xmrs::prelude::{Cell, CellEvent, Pitch, Volume};

    #[test]
    fn redo_reapplies_last_undone_command() {
        let mut module = Module::default();
        module.tracks.push(Track::Notes {
            name: "Track 1".to_string(),
            instrument: 0,
            rows: vec![Cell::default()],
            muted: false,
        });

        let command = EditCommand::SetCell {
            track: 0,
            row_offset: 0,
            content: Cell {
                event: CellEvent::NoteOn {
                    pitch: Pitch::C4,
                    velocity: Volume::FULL,
                },
                effects: vec![],
            },
        };

        let mut undo = UndoManager::new();
        undo.execute(&mut module, command).expect("apply edit");
        undo.undo(&mut module).expect("undo edit");
        assert!(undo.can_redo(), "redo should be available after undo");

        let redid = undo.redo(&mut module).expect("redo edit");
        assert!(redid, "redo should report a successful replay");

        let cell = module.tracks[0].cell_at(0);
        assert!(matches!(cell.event, CellEvent::NoteOn { pitch: Pitch::C4, .. }));
    }
}
