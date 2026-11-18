//! Setting the data date on a [`Session`]: the floor the schedule honors and the date the scene marks.

use crate::Session;
use crate::edit::EditError;
use crate::history::DataDates;
use engine::{Moved, TaskId};

impl Session {
    /// Put the data date at `to`: the schedule's floor and the date the scene marks. Unstarted
    /// work cannot start before the floor and the cones cannot take a change to it, so the whole
    /// schedule recomputes, as for a link edit. Returns the dates it replaced and the milliseconds
    /// the compute took. Refused during a bar press. On `Err` nothing has changed.
    pub(crate) fn run_set_data_date(
        &mut self,
        to: DataDates,
    ) -> Result<(DataDates, f64), EditError> {
        if self.busy() {
            return Err(EditError::Busy);
        }
        let before = DataDates {
            floor: self.schedule.data_date(),
            shown: self.data_date,
        };
        self.schedule.set_data_date(to.floor)?;
        let (old_finish, held) = (self.solution.project_finish, self.reports.held);
        let t0 = (self.clock)();
        let solution = match self.schedule.compute() {
            Ok(solution) => solution,
            Err(error) => {
                let _ = self.schedule.set_data_date(before.floor);
                return Err(error.into());
            }
        };
        let elapsed = (self.clock)() - t0;
        let old = std::mem::replace(&mut self.solution, solution);
        self.data_date = to.shown;
        self.rollup_full_summaries();
        self.rebuild_links();
        self.follow_filter();
        self.moved.clear();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
        )]
        self.moved.extend(
            old.dates
                .iter()
                .zip(&self.solution.dates)
                .enumerate()
                .filter(|(_, (was, now))| was.es != now.es || was.ef != now.ef)
                .map(|(index, (was, _))| Moved {
                    task: TaskId(index as u32),
                    was_es: was.es,
                    was_ef: was.ef,
                }),
        );
        self.changed();
        self.note_data_date(before.shown, to.shown);
        self.note_links_moved(old_finish, held);
        Ok((before, elapsed))
    }
}
