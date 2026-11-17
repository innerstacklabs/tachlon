//! The critical-path half of the move report: which tasks were critical before a discrete edit,
//! and how many joined and left the path by the time the report is made.

use crate::Session;
use engine::TaskId;

/// One flag (a byte) per task: was it critical before the edit that is running. Filled once per
/// discrete edit by [`Session::remember_critical`], reused, never filled per frame.
#[derive(Default)]
pub(super) struct Before {
    flags: Vec<bool>,
    /// Whether `flags` belongs to the edit that is running: set by `remember_critical`, consumed
    /// by the report and cleared by an edit that fails. A change that remembers nothing (a
    /// data-date edit) leaves it false and says nothing about the critical path.
    taken: bool,
}

impl Session {
    /// Whether `edited` is the task of the drag in flight: the edit is one frame of it, which the
    /// journal merges and reports at the release.
    pub(super) fn in_drag(&self, edited: TaskId) -> bool {
        self.reports.journal.on && self.reports.journal.task == edited.0
    }

    /// The (joined, left) of the edit being reported, and the before-state spent. An edit that
    /// moves no other task (`others` false) makes no note: skip the walk over every task. A change
    /// that took no before-state (a data-date edit) has nothing to compare: no clause.
    pub(super) fn take_critical_change(&mut self, others: bool) -> (u32, u32) {
        let taken = std::mem::take(&mut self.reports.critical.taken);
        if others && taken {
            self.critical_change()
        } else {
            (0, 0)
        }
    }

    /// Keep which tasks are critical now, before a discrete edit of `edited` (`None` for a link
    /// edit) changes the solution: one flag per task, in a buffer the session reuses. A drag
    /// frame does not call it; the same test as [`Session::note_moved`]'s sends it to the journal.
    pub(crate) fn remember_critical(&mut self, edited: Option<TaskId>) {
        if edited.is_some_and(|task| self.in_drag(task)) {
            return;
        }
        let before = &mut self.reports.critical.flags;
        before.clear();
        before.extend(
            self.solution
                .dates
                .iter()
                .map(engine::TaskDates::is_critical),
        );
        self.reports.critical.taken = true;
    }

    /// Keep which tasks are critical now, at the pointer's threshold, where the drag's journal
    /// begins: the same flags as [`Session::remember_critical`], taken once per drag and never
    /// per frame. They are not marked `taken`: only the release reads them.
    pub(crate) fn remember_critical_at_press(&mut self) {
        let before = &mut self.reports.critical.flags;
        before.clear();
        before.extend(
            self.solution
                .dates
                .iter()
                .map(engine::TaskDates::is_critical),
        );
    }

    /// The edit failed and changed nothing: its before-state is not for the next change.
    pub(crate) fn forget_critical(&mut self) {
        self.reports.critical.taken = false;
    }

    /// How many tasks joined and left the critical path between the remembered before-state and
    /// the solution now. A task past the before-state's length counts as not critical before.
    pub(super) fn critical_change(&self) -> (u32, u32) {
        let (mut joined, mut left) = (0u32, 0u32);
        for (i, now) in self.solution.dates.iter().enumerate() {
            let was = self.reports.critical.flags.get(i).copied().unwrap_or(false);
            match (was, now.is_critical()) {
                (false, true) => joined += 1,
                (true, false) => left += 1,
                _ => {}
            }
        }
        (joined, left)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::{Schedule, Task};
    use timeline::Timeline;

    fn session() -> Session {
        let mut schedule = Schedule::new();
        let calendar = schedule.add_calendar(
            engine::Calendar::new(std::array::from_fn(|_| vec![(480, 720), (780, 1020)]), &[])
                .expect("a valid calendar"),
        );
        schedule
            .add_task(Task {
                duration: 480,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
        let solution = schedule.compute().expect("acyclic");
        let order = timeline::order_by_start(&solution.dates);
        let meta = vec![crate::meta::TaskMeta::default()];
        Session::new(
            schedule,
            solution,
            Timeline::new(order, 1200.0, 600.0, 1.0),
            meta,
            || 0.0,
        )
    }

    #[test]
    fn an_edit_of_the_dragged_task_keeps_no_before_state() {
        let mut s = session();
        s.begin_journal(0);
        s.remember_critical(Some(TaskId(0)));
        assert!(
            !s.reports.critical.taken,
            "a drag frame took a before-state"
        );
        s.remember_critical(None);
        assert!(s.reports.critical.taken);
    }
}
