//! The one place a history entry is recorded or replayed (decision 0019, rule 1): every undo step
//! is a commit with a `seq` and a source, which a host reads from the outbox; a discard's steps
//! share one commit.

use super::{Entry, History, Step, task_of};
use crate::{
    Session,
    edit::{Edit, EditError},
    host::Pending,
    json::quote,
};
use engine::TaskId;

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

    /// Each latest undo entry becomes the next of `inverses` on the redo stack, in order.
    fn redo_all(&mut self, inverses: Vec<Entry>) {
        for inverse in inverses {
            self.move_to_redo(inverse);
        }
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
        self.stamp = self.stamp.wrapping_add(1);
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

    /// Undo every entry past the first `depth`, newest first. A host entry is its own commit, so the
    /// host reads its token; the entries between share one commit. `Ok(false)` during a press.
    pub(crate) fn undo_to(&mut self, depth: usize) -> Result<bool, EditError> {
        while self.history.undo.len() > depth {
            let plain = |entry: &&Entry| !matches!(entry, Entry::Host(_));
            let run = self.history.past(depth).take_while(plain).count();
            if !(if run == 0 {
                self.undo_step()?
            } else {
                self.undo_run(run)?
            }) {
                return Ok(false);
            }
        }
        Ok(true)
    }

    /// Undo the latest `count` entries as one commit with source `undo`. A refusal puts back what
    /// already ran, so nothing changes on `Err`.
    fn undo_run(&mut self, count: usize) -> Result<bool, EditError> {
        if self.busy() {
            return Ok(false);
        }
        let from = self.history.undo.len() - count;
        let entries: Vec<Entry> = self.history.past(from).cloned().collect();
        let row = entries.iter().filter_map(task_of).next_back();
        let mut done = Vec::new();
        for entry in entries {
            match self.run_entry(entry, true) {
                Ok((inverse, _)) => done.push(inverse),
                Err(error) => {
                    for inverse in done.into_iter().rev() {
                        let _ = self.run_entry(inverse, true);
                    }
                    return Err(error);
                }
            }
        }
        let changes = self.run_changes(&done).join(",");
        self.history.redo_all(done);
        let seq = self.record(Source::Undo, &changes, "");
        for _ in 0..count {
            self.host.pending.push(seq, Source::Undo);
        }
        self.history.end_whatif_past_mark();
        self.reveal_changed(row);
        Ok(true)
    }

    /// The changes of a run of undone entries, `done` being the inverses in the order they ran. They are
    /// read once, after the last ran, as a group's are: each field keeps its first old value, and a
    /// task the run both made and deleted is left out. A data-date step keeps its first old date.
    fn run_changes(&self, done: &[Entry]) -> Vec<String> {
        let mut steps: Vec<Step> = Vec::new();
        for entry in done {
            match entry {
                Entry::Step(step) => steps.push(step.clone()),
                Entry::Group(group) => steps.extend(group.iter().rev().cloned()),
                Entry::DataDates(_) | Entry::Host(_) => {}
            }
        }
        let marks: Vec<(TaskId, bool)> = steps
            .iter()
            .filter_map(|s| match s.edit {
                Edit::Delete { task } => Some((task, true)),
                Edit::Restore { task, .. } => Some((task, false)),
                _ => None,
            })
            .collect();
        let made_then_deleted = |task: TaskId| {
            let mut run = marks.iter().filter(|m| m.0 == task).map(|m| m.1);
            (run.next(), run.next_back()) == (Some(true), Some(false))
        };
        let netted: Vec<TaskId> = marks
            .iter()
            .map(|m| m.0)
            .filter(|&task| made_then_deleted(task))
            .collect();
        let named = |s: &Step| match s.edit {
            Edit::SetLimit { task, .. } => Some(task),
            _ => task_of(&Entry::Step(s.clone())),
        };
        steps.retain(|s| named(s).is_none_or(|task| !netted.contains(&task)));
        steps.reverse();
        let mut changes = self.changes_of(&Entry::Group(steps));
        let dates = done
            .iter()
            .find(|entry| matches!(entry, Entry::DataDates(_)));
        changes.extend(dates.map_or_else(Vec::new, |entry| self.changes_of(entry)));
        changes
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
