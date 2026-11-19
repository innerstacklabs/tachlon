//! The one place a history entry is recorded or replayed (decision 0019, rule 1): every undo step
//! is a commit with a `seq` and a source, which a host reads from the outbox.

use super::{Entry, History};
use crate::{Session, edit::EditError, host::Pending, json::quote};

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
    Host,
}

impl History {
    fn clear(&mut self) {
        *self = Self::new(self.cap);
    }
}

impl Session {
    /// Replace the history with an empty one having `cap` entries.
    pub fn with_history_cap(mut self, cap: usize) -> Session {
        self.history = History::new(cap);
        self
    }

    /// Put an entry for something the host did outside the chart on the undo list (decision 0019, rule 12): undo
    /// and redo change no value, raise `revision` and write a commit whose `host` member is `token`.
    ///
    /// # Errors
    /// [`EditError::BadToken`] for an empty token.
    pub fn push_host_entry(&mut self, token: &str) -> Result<(), EditError> {
        if token.is_empty() {
            return Err(EditError::BadToken);
        }
        self.commit(Entry::Host(token.into()), Source::Host);
        Ok(())
    }

    /// Empty both stacks, the unsaved list `rollback_to` walks and any what-if; the next `seq` is unchanged.
    pub fn clear_history(&mut self) {
        self.history.clear();
        self.host.pending = Pending::default();
        self.stamp = self.stamp.wrapping_add(1);
    }

    /// Run a host entry: no value changes, and the revision rises so the `edit` event carries the commit.
    pub(super) fn run_host(&mut self, token: String) -> (Entry, f64) {
        self.changed();
        (Entry::Host(token), 0.0)
    }

    /// Record `inverse`, the entry that puts back the step that just ran, as the next commit: an undo
    /// moves the latest undo entry to the redo stack, a redo the reverse, any other source pushes one.
    pub(super) fn commit(&mut self, inverse: Entry, source: Source) {
        let changes = self.changes_of(&inverse).join(",");
        let host = match &inverse {
            Entry::Host(token) => format!(r#","host":{}"#, quote(token)),
            _ => String::new(),
        };
        match source {
            Source::Undo => self.history.move_to_redo(inverse),
            Source::Redo => self.history.move_to_undo(inverse),
            _ => self.history.push(inverse),
        }
        let seq = self.record(source, &changes, &host);
        self.host.pending.push(seq, source);
    }

    /// Number a commit and put it in the outbox; `changes` is its changes, comma-joined, and `host`
    /// the `,"host":token` member of a host entry, else empty.
    pub(super) fn record(&mut self, source: Source, changes: &str, host: &str) -> u64 {
        let outbox = &mut self.host.outbox;
        outbox.seq += 1;
        let (seq, source) = (outbox.seq, format!("{source:?}").to_lowercase());
        outbox.push(format!(
            r#"{{"seq":{seq},"source":"{source}","changes":[{changes}]{host}}}"#
        ));
        seq
    }
}
