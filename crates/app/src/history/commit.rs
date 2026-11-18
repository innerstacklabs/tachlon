//! The one place a history entry is recorded or replayed (decision 0019, rule 1): every undo step
//! is a commit with a `seq` and a source, which a host reads from the outbox.

use super::Entry;
use crate::Session;

/// What made a commit. A host reads the name of the variant, in lower case.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Typed,
    Drag,
    Dialog,
    Paste,
    Proposal,
    Undo,
    Redo,
    Rollback,
}

impl Session {
    /// Record `inverse`, the entry that puts back the step that just ran, as the next commit: an undo
    /// moves the latest undo entry to the redo stack, a redo the reverse, any other source pushes one.
    pub(super) fn commit(&mut self, inverse: Entry, source: Source) {
        let changes = self.changes_of(&inverse).join(",");
        match source {
            Source::Undo => self.history.move_to_redo(inverse),
            Source::Redo => self.history.move_to_undo(inverse),
            _ => self.history.push(inverse),
        }
        let seq = self.record(source, &changes);
        self.host.pending.push(seq, source);
    }

    /// Number a commit and put it in the outbox; `changes` is its changes, comma-joined.
    pub(super) fn record(&mut self, source: Source, changes: &str) -> u64 {
        let outbox = &mut self.host.outbox;
        outbox.seq += 1;
        let (seq, source) = (outbox.seq, format!("{source:?}").to_lowercase());
        outbox.push(format!(
            r#"{{"seq":{seq},"source":"{source}","changes":[{changes}]}}"#
        ));
        seq
    }
}
