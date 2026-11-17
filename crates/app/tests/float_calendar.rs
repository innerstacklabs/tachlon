//! The hover card's Float row in calendar days (gs#703): the whole days from a task's early
//! finish to its late finish, beside the float in working days.

use crate::common::collapse::{schedule_with_tasks, task_meta};
use app::Session;
use engine::{Dependency, LinkType, TaskId};
use timeline::Timeline;

/// `A` (one working day) and `B` (`long_days` working days) both lead into `C` (one working
/// day), Monday to Friday, 8 h a day. `A` has `long_days - 1` working days of float.
fn session(long_days: i64) -> Session {
    let mut schedule = schedule_with_tasks(3);
    for (task, days) in [(0, 1), (1, long_days), (2, 1)] {
        schedule
            .set_duration(TaskId(task), days * 8 * 60)
            .expect("valid duration");
    }
    for pred in [0, 1] {
        let kind = LinkType::FS;
        let dependency = Dependency {
            pred: TaskId(pred),
            succ: TaskId(2),
            kind,
            lag: 0,
        };
        schedule.add_dependency(dependency).expect("valid link");
    }
    let solution = schedule.compute().expect("acyclic schedule");
    let metadata = (0..3).map(|_| task_meta(None, 1)).collect();
    let timeline = Timeline::new((0..3).collect(), 800.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, metadata, || 0.0)
}

#[test]
fn five_working_days_of_float_over_a_weekend_read_seven_calendar_days() {
    // A ends Monday 17:00 and B takes six working days, so A can end as late as the Monday after
    // at 17:00: five working days, seven calendar days.
    let session = session(6);
    let details = session.task_details(0).expect("A exists");
    assert_eq!(details.float_text, "5 d 0 h");
    assert_eq!(session.task_float_calendar_text(0), "7 calendar days");
}

#[test]
fn one_calendar_day_is_singular_and_a_task_without_float_has_no_text() {
    let session = session(2);
    assert_eq!(session.task_float_calendar_text(0), "1 calendar day");
    assert_eq!(session.task_float_calendar_text(1), "", "B is critical");
    assert_eq!(session.task_float_calendar_text(99), "", "no such task");
}
