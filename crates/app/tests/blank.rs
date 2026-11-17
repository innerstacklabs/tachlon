//! A schedule with no tasks (gs#703): `Loaded::blank` and the first insert on a session over it.

use crate::common;

use app::{Loaded, Session, edit::day_minutes};
use common::clock;
use timeline::Timeline;

/// Days since 1970-01-01 of Monday 2026-01-05.
const MONDAY: i64 = 20_458;

/// A session over `Loaded::blank(start_day)` as `App::create` builds it.
fn blank_session(start_day: i64) -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        meta,
        outline,
        data_date,
        origin,
        ..
    } = Loaded::blank(start_day);
    let timeline = Timeline::new(order, 1200.0, 600.0, 1.0);
    Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("an empty outline matches an empty schedule")
        .with_data_date(data_date)
        .with_origin(origin)
}

#[test]
fn the_origin_is_the_monday_on_or_before_the_day() {
    // Monday to Sunday of one week, then the week before 1970 and a Monday before it.
    for offset in 0..7 {
        assert_eq!(Loaded::blank(MONDAY + offset).origin.day(), MONDAY);
    }
    assert_eq!(Loaded::blank(MONDAY + 7).origin.day(), MONDAY + 7);
    assert_eq!(Loaded::blank(0).origin.day(), -3);
    assert_eq!(Loaded::blank(-1).origin.day(), -3);
    assert_eq!(Loaded::blank(MONDAY + 2).origin.day_text(0), "2026-01-05");
}

#[test]
fn a_blank_schedule_has_one_weekday_calendar_and_no_tasks() {
    let loaded = Loaded::blank(MONDAY);
    assert_eq!(loaded.schedule.live_task_count(), 0);
    assert_eq!(loaded.solution.dates, []);
    assert!(loaded.order.is_empty() && loaded.meta.is_empty());
    assert_eq!(loaded.outline.roots(), []);
    let [calendar] = loaded.schedule.calendars() else {
        panic!("one calendar");
    };
    let working: Vec<bool> = calendar.week().iter().map(|day| !day.is_empty()).collect();
    assert_eq!(working, [true, true, true, true, true, false, false]);
    assert_eq!(day_minutes(calendar), 480);
}

#[test]
fn a_blank_schedule_shades_its_weekend() {
    // The first week, Monday 00:00 to Sunday 24:00: one run of non-working days, Saturday and Sunday.
    let mut session = blank_session(MONDAY);
    session.fit(0.0, 7.0 * 1440.0);
    assert_eq!(session.scene().shaded, 1);
}

#[test]
fn an_insert_on_a_blank_session_makes_the_first_row() {
    let mut session = blank_session(MONDAY + 2);
    assert_eq!(session.task_count(), 0);
    let task = session.insert_below_selection().unwrap();
    let row = &session.meta()[task.0 as usize];
    assert_eq!(row.name, "New task");
    assert_eq!(row.code.as_deref(), Some("A1000"));
    assert_eq!(row.outline_level, 1);
    assert_eq!(session.schedule().tasks()[task.0 as usize].duration, 480);
    assert_eq!(session.selection().map(|s| s.task), Some(task.0));
    // The first task starts at 08:00 on the origin Monday and ends at 17:00 the same day.
    let dates = session.solution().get(task);
    assert_eq!((dates.es, dates.ef), (480, 1020));
    assert_eq!(session.solution().project_finish, 1020);
    // The second row takes the ID one above the first, below it.
    let second = session.insert_below_selection().unwrap();
    assert_eq!(
        session.meta()[second.0 as usize].code.as_deref(),
        Some("A1001")
    );
    assert_eq!(session.outline().unwrap().preorder(), [task, second]);
}
