//! Rollback (decision 0019, rule 11): put the session back to the state before a commit the host did
//! not save, by walking the unacknowledged commits newest first and taking each one back.

use super::{Entry, History, Source};
use crate::{Session, edit::EditError, host::PendingKind};
use std::collections::VecDeque;

type Stacks = (VecDeque<Entry>, Vec<Entry>);

impl History {
    /// Whether every step of `walk` finds its entry: a step needs an undo entry (a commit that pushed or
    /// redid) or a redo entry (one that undid), and the stacks only lose the oldest undo entries to the cap.
    fn holds(&self, walk: &[PendingKind]) -> bool {
        let (mut undo, mut redo) = (self.undo.len(), self.redo.len());
        walk.iter().all(|kind| {
            let found = if *kind == PendingKind::Undid {
                redo > 0
            } else {
                undo > 0
            };
            match kind {
                PendingKind::Pushed => undo = undo.saturating_sub(1),
                PendingKind::Undid => (redo, undo) = (redo.saturating_sub(1), undo + 1),
                PendingKind::Redid => (undo, redo) = (undo.saturating_sub(1), redo + 1),
            }
            found
        })
    }

    /// The entry the step that takes back a `kind` commit runs.
    fn top(&self, kind: PendingKind) -> Entry {
        match kind {
            PendingKind::Undid => self.redo.last(),
            _ => self.undo.back(),
        }
        .cloned()
        .expect("the walk was checked")
    }

    /// Settle the stacks after the step for a `kind` commit ran and returned `inverse`: a push is
    /// dropped with its redo entry, an undo becomes a redo and the reverse.
    fn settle(&mut self, kind: PendingKind, inverse: Entry) {
        match kind {
            PendingKind::Pushed => drop(self.undo.pop_back()),
            PendingKind::Undid => self.move_to_undo(inverse),
            PendingKind::Redid => self.move_to_redo(inverse),
        }
    }

    fn stacks(&self) -> Stacks {
        (self.undo.clone(), self.redo.clone())
    }

    fn restore(&mut self, (undo, redo): Stacks) {
        (self.undo, self.redo) = (undo, redo);
    }
}

impl Session {
    /// The host saved every commit up to `seq`: a rollback to one of them is refused from now on.
    pub fn acknowledge(&mut self, seq: u64) {
        self.host.pending.acknowledge(seq);
    }

    /// Put the schedule back to the state before the commit `seq`, taking the commits after it back
    /// too, as one commit with source `rollback` whose `seq` this returns. Not gated: the host asked.
    ///
    /// # Errors
    /// [`EditError::Busy`] during a bar press, and [`EditError::RollbackGone`] when `seq` is
    /// acknowledged, dropped by a cap or not a commit that can be taken back; the step the engine
    /// refuses is returned as it is. Nothing changes on `Err`.
    pub fn rollback_to(&mut self, seq: u64) -> Result<u64, EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        let walk = self
            .host
            .pending
            .walk(seq)
            .filter(|walk| self.history.holds(walk))
            .ok_or(EditError::RollbackGone)?;
        let saved = self.history.stacks();
        let (mut done, mut changes) = (Vec::new(), Vec::new());
        for kind in walk {
            let entry = self.history.top(kind);
            match self.run_entry(entry, true) {
                Ok((inverse, _)) => {
                    changes.extend(self.changes_of(&inverse));
                    self.history.settle(kind, inverse.clone());
                    done.push(inverse);
                }
                Err(error) => {
                    for inverse in done.into_iter().rev() {
                        let _ = self.run_entry(inverse, true);
                    }
                    self.history.restore(saved);
                    return Err(error);
                }
            }
        }
        self.host.pending.truncate(seq);
        self.history.end_whatif_past_mark();
        Ok(self.record(Source::Rollback, &changes.join(",")))
    }
}
