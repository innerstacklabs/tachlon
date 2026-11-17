//! The Near critical row filter (gs#703): the table keeps the tasks whose total float is at most
//! ten working days of their own calendar, critical ones included, and the summaries above them.

use crate::common;

use app::{RowFilter, Session, meta::TaskMeta, outline::Outline};
use common::clock;
use engine::{Calendar, Schedule, Task, TaskId};
use timeline::Timeline;

/// A Monday-to-Friday calendar with one working period of `hours` hours a day.
fn calendar(hours: i64) -> Calendar {
    let day = vec![(480_u16, 480 + hours as u16 * 60)];
    let week = std::array::from_fn(|d| if d < 5 { day.clone() } else { vec![] });
    Calendar::new(week, &[]).expect("a valid calendar")
}

fn task(duration: i64, calendar: engine::CalendarId) -> Task {
    Task {
        duration,
        calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    }
}

/// Task 0 is a summary over the leaves 1 to 8, which have no links: the project finish is task 1's
/// (40 days of 8 hours), and every other leaf's float is how much earlier it finishes, in its own
/// calendar. Tasks 1 to 3 are on the 8-hour calendar with 0, 5 and 15 days of float; tasks 4 to 6
/// are on a 4-hour calendar with 8, 10 and 12 days of float (a 4-hour day is 240 minutes, so the
/// same 4800-minute limit would keep them all). Task 7 is on its own. Task 8 is finished (an actual finish), so its float is 0 and only the
/// finished test keeps it out.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let full = schedule.add_calendar(calendar(8));
    let half = schedule.add_calendar(calendar(4));
    let day = 480;
    let mut add = |duration: i64, calendar| {
        schedule
            .add_task(task(duration, calendar))
            .expect("the calendar is in the schedule")
    };
    add(day, full);
    for days in [40, 35, 25] {
        add(days * day, full);
    }
    // Four-hour days: 240 minutes each; these finish 8, 10 and 12 four-hour days before the end.
    for days in [32, 30, 28] {
        add(days * 240, half);
    }
    add(40 * day, full);
    // Task 8: worked and finished on day one.
    schedule
        .add_task(Task {
            actual_start: Some(0),
            actual_finish: Some(day),
            ..task(day, full)
        })
        .expect("the calendar is in the schedule");
    let mut meta = vec![TaskMeta::default(); 9];
    for leaf in &mut meta[1..] {
        leaf.parent = Some(TaskId(0));
    }
    let wbs: Vec<TaskId> = (0..9).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a one-summary outline");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new((0..9).collect(), 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

fn float(s: &Session, task: u32) -> i64 {
    s.solution().dates[task as usize].float
}

fn listed(s: &Session) -> Vec<u32> {
    s.timeline().order().to_vec()
}

#[test]
fn near_critical_keeps_tasks_with_float_up_to_ten_days() {
    let mut s = session();
    // The setup is what the doc says: float in working minutes of each task's own calendar.
    assert_eq!([1, 2, 3].map(|t| float(&s, t)), [0, 5 * 480, 15 * 480]);
    assert_eq!(
        [4, 5, 6].map(|t| float(&s, t)),
        [8 * 240, 10 * 240, 12 * 240]
    );

    // The finished leaf has no float, so only the finished test can keep it out.
    assert_eq!((float(&s, 8), s.solution().dates[8].finished), (0, true));

    assert!(s.set_near_critical(true));
    assert!(s.near_critical());
    assert_eq!(s.row_filter(), RowFilter::NearCritical);
    // 0, 5 days, 8 and 10 four-hour days stay; 15 days and 12 four-hour days go. The summary stays
    // for the leaves under it.
    // The finished task (8) is left out, as Critical only leaves it out, though its float is 0.
    assert_eq!(listed(&s), vec![0, 1, 2, 4, 5, 7]);

    assert!(s.set_near_critical(false));
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert_eq!(listed(&s), (0..9).collect::<Vec<u32>>());
}

#[test]
fn it_is_one_filter_at_a_time() {
    let mut s = session();
    assert!(s.set_critical_only(true));
    assert!(s.set_near_critical(true));
    assert!(!s.critical_only() && s.near_critical());
    // Turning another filter off leaves this one on.
    assert!(s.set_longest_path(false));
    assert!(s.near_critical());
    assert!(s.set_longest_path(true));
    assert!(!s.near_critical());
}

#[test]
fn the_filter_follows_an_edit() {
    let mut s = session();
    assert!(s.set_near_critical(true));
    assert!(!listed(&s).contains(&3));
    // Task 3 grows to the project length: no float left, so it joins.
    s.edit(app::edit::Edit::SetDuration {
        task: TaskId(3),
        minutes: 40 * 480,
    })
    .expect("applies");
    assert!(listed(&s).contains(&3));
}
