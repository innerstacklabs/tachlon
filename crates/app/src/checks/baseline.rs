//! The baseline checks as counts: missed tasks and the baseline execution index. Both read the
//! baseline's early finish against the data date, so with no baseline or no data date there is
//! nothing to count. Like the other checks they live here because only the session's outline
//! knows which tasks are summaries.

use crate::Session;
use engine::TaskId;

/// Counts over the live leaf tasks the baseline holds, for one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineChecks {
    /// Tasks baselined to finish by the data date that finished later than their baseline, or have
    /// not finished.
    pub missed_tasks: usize,
    /// Tasks whose baseline early finish is on or before the data date.
    pub baselined_to_finish: usize,
    /// Of those, the tasks with an actual finish.
    pub finished: usize,
}

impl BaselineChecks {
    /// Counts the checks for `session`. Deleted tasks, summaries and tasks the baseline has no
    /// entry for (added since) are left out; without an outline every task is a leaf. With no
    /// baseline or no data date every count is 0.
    pub fn of(session: &Session) -> BaselineChecks {
        let mut checks = BaselineChecks {
            missed_tasks: 0,
            baselined_to_finish: 0,
            finished: 0,
        };
        let (Some(baseline), Some(data_date)) = (session.baseline(), session.data_date()) else {
            return checks;
        };

        for (index, task) in session.schedule().tasks().iter().enumerate() {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let id = TaskId(index as u32);
            let summary = session
                .outline()
                .is_some_and(|outline| outline.is_summary(id));
            if task.deleted || summary {
                continue;
            }
            let Some(planned) = baseline.task(id) else {
                continue;
            };
            if planned.ef > data_date {
                continue;
            }
            checks.baselined_to_finish += 1;
            checks.finished += usize::from(task.actual_finish.is_some());
            checks.missed_tasks += usize::from(task.actual_finish.is_none_or(|at| at > planned.ef));
        }
        checks
    }

    /// The baseline execution index: tasks finished over tasks baselined to finish by the data
    /// date. `None` when none was baselined to finish by then, which is also the case with no
    /// baseline or no data date.
    #[expect(
        clippy::cast_precision_loss,
        reason = "task counts, far under 2^53 (that many tasks would not fit in memory), so f64 holds both exactly"
    )]
    #[must_use]
    pub fn execution_index(&self) -> Option<f64> {
        (self.baselined_to_finish > 0)
            .then(|| self.finished as f64 / self.baselined_to_finish as f64)
    }
}
