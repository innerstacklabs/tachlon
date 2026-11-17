//! The release of a drag: its journal becomes one move report, and the helpers that tell which
//! tasks moved against the dates a report holds.

use super::Session;
use engine::{Moved, TaskId};

impl Session {
    /// The drag ended: turn its journal into one report with the press-time dates.
    pub(crate) fn commit_journal(&mut self) {
        if !self.reports.journal.on || !self.drag_started {
            return;
        }
        let edited = TaskId(self.reports.journal.task);
        let entries = std::mem::take(&mut self.reports.journal.entries);
        let was = self.moved_others(entries.iter(), edited);
        self.reports.journal.entries = entries;
        let old_finish = self.reports.journal.finish;
        let edited = self.reports.journal.edited.filter(|m| self.moved_now(m));
        // A release that moved no other task makes no note: skip the walk over every task.
        let critical = if was.is_empty() {
            (0, 0)
        } else {
            self.critical_change()
        };
        self.make_report(was, edited, old_finish, false, critical);
        self.settle_moved_filter();
    }

    /// The entries of `list` that are not `edited` and whose early dates differ now.
    pub(super) fn moved_others<'a>(
        &self,
        list: impl Iterator<Item = &'a Moved>,
        edited: TaskId,
    ) -> Vec<Moved> {
        list.filter(|m| m.task != edited && self.moved_now(m))
            .copied()
            .collect()
    }

    /// The entry of `list` for `edited`, if its early dates differ now.
    pub(super) fn moved_edited(&self, list: &[Moved], edited: TaskId) -> Option<Moved> {
        list.iter()
            .find(|m| m.task == edited)
            .filter(|m| self.moved_now(m))
            .copied()
    }

    /// Whether `m.task` has other early dates now than `m` holds.
    pub(super) fn moved_now(&self, m: &Moved) -> bool {
        let now = &self.solution.dates[m.task.0 as usize];
        m.was_es != now.es || m.was_ef != now.ef
    }
}
