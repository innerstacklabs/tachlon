//! The hover card's float text for a negative float (gantt#1875): one minus sign, then the
//! magnitude.

use crate::common::collapse::{schedule_with_tasks, task_meta};
use app::Session;
use engine::{DAY, Limit, TaskId};
use timeline::Timeline;

#[test]
fn a_late_start_before_the_early_start_prints_one_minus_sign() {
    // Starts no earlier than Thursday (work begins 08:00) and no later than Tuesday 15:00: two
    // hours of Tuesday and a whole 8 h Wednesday short.
    let mut schedule = schedule_with_tasks(1);
    let limit = Limit {
        snlt: Some(DAY + 900),
        ..Limit::default()
    };
    schedule.set_limit(TaskId(0), limit).expect("live task");
    schedule.set_snet(TaskId(0), Some(3 * DAY)).expect("live");
    let solution = schedule.compute().expect("acyclic schedule");
    let timeline = Timeline::new(vec![0], 800.0, 400.0, 1.0);
    let meta = vec![task_meta(None, 1)];
    let session = Session::new(schedule, solution, timeline, meta, || 0.0);
    let details = session.task_details(0).expect("task exists");
    assert_eq!(details.float_minutes, -600);
    assert_eq!(details.float_text, "-1 d 2 h");
}
