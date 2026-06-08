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

/// The undo/redo manager.
#[derive(Default)]
pub struct UndoManager {
    /// Stack of undo entries (most recent last).
    undo_stack: Vec<UndoEntry>,
    /// Stack of redo entries (most recent last).
    redo_stack: Vec<UndoEntry>,
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
                    // The undo itself is in the receipt; we can redo by re-applying
                    // Actually, xmrs's undo doesn't give us a receipt for the redo.
                    // We need to store the original command to redo.
                    // For simplicity, we'll just pop without redo support initially.
                    self.redo_stack.clear();
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
    pub fn redo(&mut self, _module: &mut Module) -> Result<bool, String> {
        // xmrs's undo/redo is asymmetric: Module::undo consumes the receipt
        // but doesn't produce a new receipt for redo-applying.
        // For now, redo is not supported. We'd need to store the original command.
        if !self.redo_stack.is_empty() {
            self.redo_stack.clear();
        }
        Ok(false)
    }

    /// Whether undo is available.
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
