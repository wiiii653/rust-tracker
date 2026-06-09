//! Live-edit policy for playback synchronization.
//!
//! The UI owns the authoritative module. This module classifies edits so the
//! audio layer can decide whether to queue a live mutation or replace its
//! playback snapshot.

use xmrs::edit::EditCommand;

/// How a live edit should be delivered to playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveEditRoute {
    /// Queue the command for the playback thread.
    Queue,
    /// Replace the playback snapshot with a fresh module clone.
    Resync,
}

/// When the edit should become audible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveEditTiming {
    /// Can be reflected immediately without waiting for a row boundary.
    #[expect(dead_code, reason = "reserved for future immediate-edit optimizations")]
    Immediate,
    /// Apply at the next row boundary.
    NextRow,
    /// Apply on the next trigger of the edited voice.
    #[expect(dead_code, reason = "reserved for future immediate-edit optimizations")]
    NextTrigger,
}

/// Classified policy for a single edit command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiveEditPolicy {
    pub route: LiveEditRoute,
    pub timing: LiveEditTiming,
    pub description: &'static str,
}

pub fn classify_edit_command(cmd: &EditCommand) -> LiveEditPolicy {
    use EditCommand::*;

    match cmd {
        SetCell { .. }
        | AddCellEffect { .. }
        | RemoveCellEffect { .. }
        | SetCellEffects { .. }
        | InsertRow { .. }
        | DeleteRow { .. } => LiveEditPolicy {
            route: LiveEditRoute::Queue,
            timing: LiveEditTiming::NextRow,
            description: "pattern cell edit",
        },

        RenameTrack { .. }
        | SetTrackInstrument { .. }
        | SetTrackMuted { .. }
        | ResizeTrack { .. }
        | CreateTrack { .. }
        | DeleteTrack { .. }
        | CloneTrack { .. } => LiveEditPolicy {
            route: LiveEditRoute::Queue,
            timing: LiveEditTiming::NextRow,
            description: "track edit",
        },

        CreateClip { .. }
        | DeleteClip { .. }
        | MoveClip { .. }
        | SetClipTrack { .. }
        | SetClipTargetChannel { .. }
        | DuplicateClip { .. }
        | SplitClip { .. }
        | ResizeClip { .. }
        | SlipClip { .. }
        | PasteBlock { .. }
        | TransposeBlock { .. }
        | InterpolateEffect { .. } => LiveEditPolicy {
            route: LiveEditRoute::Resync,
            timing: LiveEditTiming::NextRow,
            description: "timeline edit",
        },

        CreateAutomationLane { .. }
        | DeleteAutomationLane { .. }
        | SetAutomationLaneEnabled { .. }
        | AddAutomationPoint { .. }
        | DeleteAutomationPoint { .. }
        | MoveAutomationPoint { .. }
        | AddLfoEvent { .. }
        | AddSlideEvent { .. }
        | AddGlideEvent { .. }
        | SetDefaultBpm { .. }
        | SetDefaultSpeed { .. }
        | SetPlaybackQuirks { .. }
        | SetModuleName { .. }
        | SetPatternHighlight { .. }
        | SetEuclideanHumanize { .. } => LiveEditPolicy {
            route: LiveEditRoute::Resync,
            timing: LiveEditTiming::NextRow,
            description: "global or automation edit",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use xmrs::prelude::{Cell, CellEvent, Pitch, Volume};

    #[test]
    fn pattern_cell_edits_queue_for_next_row() {
        let policy = classify_edit_command(&EditCommand::SetCell {
            track: 0,
            row_offset: 0,
            content: Cell {
                event: CellEvent::NoteOn {
                    pitch: Pitch::C4,
                    velocity: Volume::FULL,
                },
                effects: vec![],
            },
        });

        assert_eq!(policy.route, LiveEditRoute::Queue);
        assert_eq!(policy.timing, LiveEditTiming::NextRow);
    }

    #[test]
    fn timeline_edits_force_resync() {
        let policy = classify_edit_command(&EditCommand::MoveClip {
            clip: 0,
            new_position_tick: 64,
        });

        assert_eq!(policy.route, LiveEditRoute::Resync);
    }
}
