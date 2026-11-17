//! A printout limited to a date window (gs#703): the rows active in `[from, to)`, on a scale that
//! is the window, with the dates in the header.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::{self, collapse::schedule_with_tasks};
use app::{PageSize, Session, meta::TaskMeta};
use engine::{DAY, TaskId, WEEK};
use timeline::Timeline;

/// Tuesday 2026-01-13 and Friday 2026-01-16, 00:00 each, as instants: the demo origin is Monday 2026-01-05.
const FROM: i64 = WEEK + DAY;
const TO: i64 = WEEK + 4 * DAY;

/// Tasks `(name, start no earlier than, duration)`, one per row, on the standard calendar.
fn session(tasks: &[(&str, i64, i64)]) -> Session {
    let mut schedule = schedule_with_tasks(tasks.len());
    for (i, &(_, snet, duration)) in tasks.iter().enumerate() {
        let id = TaskId(u32::try_from(i).unwrap());
        schedule.set_snet(id, Some(snet)).unwrap();
        schedule.set_duration(id, duration).unwrap();
    }
    let solution = schedule.clone().compute().unwrap();
    let meta = tasks
        .iter()
        .map(|&(name, ..)| TaskMeta {
            name: name.to_string(),
            ..TaskMeta::default()
        })
        .collect();
    let order = (0..u32::try_from(tasks.len()).unwrap()).collect();
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, common::clock)
}

/// A milestone on each of Monday, Tuesday, Thursday and Friday of the window's week, a task that spans the window, and one after it.
fn week() -> Session {
    session(&[
        ("Mon", WEEK, 0),
        ("Tue", FROM, 0),
        ("Thu", WEEK + 3 * DAY, 0),
        ("Fri", TO, 0),
        ("Span", 0, 40 * DAY),
        ("Later", 4 * WEEK, DAY),
    ])
}

fn windowed(session: &Session) -> String {
    session.print_pages_on("Plan", "", PageSize::Letter, None, Some((FROM, TO)), None)[0].clone()
}

#[test]
fn a_window_keeps_the_rows_active_in_it() {
    let session = week();
    let plan = session.print_plan_on(PageSize::Letter, None, Some((FROM, TO)));
    // The milestone on the first day is in, the one on the last day before `to` is in, the one
    // on the `to` day is out, and so is the one the day before `from`.
    assert_eq!(plan.rows, [1, 2, 4], "Tue, Thu and the spanning task");
    let page = windowed(&session);
    assert!(page.contains(">Tue</text>") && page.contains(">Span</text>"));
    assert!(!page.contains(">Mon</text>") && !page.contains(">Fri</text>"));
}

#[test]
fn the_scale_is_the_window() {
    let session = week();
    let plan = session.print_plan_on(PageSize::Letter, None, Some((FROM, TO)));
    let (from, minutes) = (f64::from(u16::try_from(FROM).unwrap()), 3.0 * 1440.0);
    assert!((plan.t0 - from).abs() < 1e-9, "the left edge is `from`");
    assert!((plan.px_per_minute * minutes - plan.panel_w).abs() < 1e-6);
    // Without a window the same rows scale to their own span, the spanning task's.
    let own = session.print_plan_on(PageSize::Letter, None, None);
    assert!(own.px_per_minute < plan.px_per_minute / 2.0);
}

#[test]
fn the_page_names_the_dates_and_clips_the_bars() {
    let page = windowed(&week());
    assert!(page.contains("Dates: 2026-01-13 to 2026-01-16"), "{page}");
    assert!(page.contains("<svg x="), "the chart panel clips its bars");
    assert!(!week().print_pages("Plan", "")[0].contains("Dates:"));
}
