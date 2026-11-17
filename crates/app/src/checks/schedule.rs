//! The DCMA float, duration, date, resource and constraint checks as counts: high float, high
//! duration, invalid dates, missing resources, hard constraints and negative float. Like the link checks, they live here because only the
//! session's outline knows which tasks are summaries.

use crate::{Session, edit::day_minutes};
use engine::{Pin, Task, TaskId};

/// A leaf is "high" past this many working days of its own calendar.
pub const HIGH_DAYS: i64 = 44;

/// Whether a fixed date holds this unfinished leaf: a pin, a finish no later than, or a start no later than before it starts.
pub(crate) fn holds_a_date(task: &Task) -> bool {
    let (limit, open) = (&task.limit, task.actual_start.is_none());
    matches!(limit.pin, Some(Pin::Finish(_)))
        || (open && matches!(limit.pin, Some(Pin::Start(_))))
        || limit.fnlt.is_some()
        || (open && limit.snlt.is_some())
}

/// Counts over the live tasks of one session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScheduleChecks {
    /// Unfinished leaves with more than 44 working days of total float.
    pub high_float: usize,
    /// Leaves longer than 44 working days of their own calendar.
    pub high_duration: usize,
    /// Tasks with an actual start or finish after the data date; 0 without a data date.
    pub invalid_dates: usize,
    /// Unfinished leaves with a duration and no resource; `None` when the file names no resource
    /// at all, because a 0 there would read as a clean check that never ran.
    pub missing_resources: Option<usize>,
    /// Unfinished leaves a fixed date holds: a start-no-later-than, a finish-no-later-than or a
    /// pin that the task still obeys. A start-no-earlier-than and a finish-no-earlier-than are
    /// floors, not holds, and are not counted.
    pub hard_constraints: usize,
    /// Unfinished leaves with a total float below 0: a late limit no date can meet.
    pub negative_float: usize,
}

impl ScheduleChecks {
    /// Counts the checks for `session`. Deleted tasks are left out; without an outline every
    /// task is a leaf. A day is the working minutes of one day of the task's calendar.
    pub fn of(session: &Session) -> ScheduleChecks {
        let schedule = session.schedule();
        let solution = session.solution();
        let data_date = session.data_date();
        let mut checks = ScheduleChecks {
            high_float: 0,
            high_duration: 0,
            invalid_dates: 0,
            missing_resources: (!session.resource_names().is_empty()).then_some(0),
            hard_constraints: 0,
            negative_float: 0,
        };

        for (index, task) in schedule.tasks().iter().enumerate() {
            if task.deleted {
                continue;
            }
            #[expect(
                clippy::cast_possible_truncation,
                reason = "a task slot index; Schedule::add_task casts the slot count to u32 with no check against u32::MAX, so past 2^32 slots it wraps, as that id does"
            )]
            let id = TaskId(index as u32);
            if let Some(data_date) = data_date {
                checks.invalid_dates += usize::from(
                    task.actual_start.is_some_and(|at| at > data_date)
                        || task.actual_finish.is_some_and(|at| at > data_date),
                );
            }
            let summary = session
                .outline()
                .is_some_and(|outline| outline.is_summary(id));
            if summary {
                continue;
            }
            let limit = HIGH_DAYS * day_minutes(schedule.calendar_of(id));
            checks.high_duration += usize::from(task.duration > limit);
            checks.high_float +=
                usize::from(task.actual_finish.is_none() && solution.get(id).float > limit);
            if task.actual_finish.is_none() {
                checks.hard_constraints += usize::from(holds_a_date(task));
                checks.negative_float += usize::from(solution.get(id).float < 0);
            }
            if let Some(missing) = &mut checks.missing_resources {
                *missing += usize::from(
                    task.actual_finish.is_none()
                        && task.duration > 0
                        && session.resources(id).iter().all(|a| a.name.is_empty()),
                );
            }
        }
        checks
    }
}
