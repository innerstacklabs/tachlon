//! A press of Undo or Redo drops a step the engine refuses and goes on to the next (tl#2087).

use super::{History, Source, task_of};
use crate::{Session, edit::EditError};

impl History {
    /// Drop the entry a press of `source` would replay.
    fn drop_top(&mut self, source: Source) {
        if source == Source::Undo {
            self.undo.pop_back();
            let len = self.undo.len();
            if let Some(mark) = &mut self.whatif {
                mark.depth = mark.depth.map(|depth| depth.min(len));
            }
        } else {
            self.redo.pop();
        }
    }
}

impl Session {
    /// A press of `source`: `gate` runs before each step tried. The engine's refusal of a step drops
    /// it and the press goes on; the gate's refusal, a host's save refusal included, drops nothing.
    pub(super) fn press(
        &mut self,
        source: Source,
        gate: impl Fn(&Self) -> Result<(), EditError>,
    ) -> Result<bool, EditError> {
        let mut dropped = false;
        loop {
            gate(self)?;
            let step = match source {
                Source::Undo => self.undo_step(),
                _ => self.redo_step(),
            };
            match step {
                Err(_) => {
                    self.history.drop_top(source);
                    dropped = true;
                }
                Ok(false) if dropped => return Err(EditError::StepGone),
                done => return done,
            }
        }
    }

    fn redo_step(&mut self) -> Result<bool, EditError> {
        if self.busy() {
            return Ok(false);
        }
        let Some(entry) = self.history.redo.last().cloned() else {
            return Ok(false);
        };
        let row = task_of(&entry);
        let (inverse, _) = self.run_entry(entry, true)?;
        self.commit(inverse, Source::Redo);
        self.reveal_changed(row);
        Ok(true)
    }
}
