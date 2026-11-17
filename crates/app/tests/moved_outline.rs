//! The dashed outlines of the move report (gs#428, story S4 of #309): where they sit, how long
//! they last, what a drag release shows and what the cap leaves the report saying. They draw
//! only: every test ends with the `Solution` equal to a full `compute()`.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use common::setup_with;
use engine::{CalendarId, DAY, Dependency, LinkType, Schedule, Task, TaskId};
use import::ProjectSelection;
use std::cell::Cell;
use timeline::scene::{OUTLINE_CAP, Rect, span_extent};
use timeline::{Theme, Timeline};

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

thread_local! {
    static NOW: Cell<f64> = const { Cell::new(0.0) };
}

/// A clock a test sets: every test runs on its own thread.
fn now() -> f64 {
    NOW.with(Cell::get)
}

fn set_now(ms: f64) {
    NOW.with(|n| n.set(ms));
}

fn assert_equals_compute(s: &Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

/// The 43-row ledger fixture on a canvas tall enough for every row.
fn ledger() -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &now).expect("the file loads");
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 1200.0, 1.0),
        meta,
        now,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin)
}

fn rings(s: &Session) -> Vec<Rect> {
    let mut scene = s.scene();
    s.add_move_outlines(&mut scene);
    scene
        .rects
        .iter()
        .filter(|r| r.stroke > 0.0)
        .copied()
        .collect()
}

#[test]
fn a_duration_edit_outlines_the_old_places_and_the_outlines_fade_after_four_seconds() {
    set_now(0.0);
    let mut s = ledger();
    assert!(rings(&s).is_empty(), "no report, no outline");
    s.edit(Edit::SetDuration {
        task: TaskId(16),
        minutes: 4800,
    })
    .expect("applies");
    let report = s.move_report().expect("others moved").clone();
    let summaries = s.timeline().summaries();
    let v = &s.timeline().viewport;
    let mut want: Vec<f64> = report
        .was
        .iter()
        .filter(|m| summaries.get(m.task.0).is_none())
        .map(|m| span_extent(v, m.was_es, m.was_ef).0.round())
        .collect();
    want.sort_by(f64::total_cmp);
    assert!(!want.is_empty(), "the fixture moves some plain tasks");
    let mut got: Vec<f64> = rings(&s).iter().map(|r| f64::from(r.pos[0])).collect();
    got.sort_by(f64::total_cmp);
    assert_eq!(got, want, "one ring per moved plain task, at its old start");
    let dash = Theme::light().sel_edge[3] * 0.6;
    let alpha_at = |s: &Session, ms: f64| {
        set_now(ms);
        rings(s).first().map(|r| r.color[3])
    };
    assert!((alpha_at(&s, 3999.0).unwrap() - dash).abs() < 1e-6);
    assert!((alpha_at(&s, 4200.0).unwrap() - dash * 0.5).abs() < 1e-5);
    assert_eq!(alpha_at(&s, 4400.0), None);
    assert_equals_compute(&s);
}

#[test]
#[expect(clippy::many_single_char_names, reason = "x and y are the press point")]
fn a_drag_release_outlines_the_press_time_places_while_the_bars_are_at_the_new_ones() {
    set_now(0.0);
    let (mut s, _, _, (x, y)) = setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    s.pointer_up();
    let report = s.move_report().expect("the release reports").clone();
    assert!(!report.slide && report.others > 0, "{report:?}");
    let v = &s.timeline().viewport;
    let dates = &s.solution().dates;
    let mut checked = 0;
    for ring in rings(&s) {
        let found = report.was.iter().find(|m| {
            let d = &dates[m.task.0 as usize];
            span_extent(v, m.was_es, m.was_ef).0.round() == f64::from(ring.pos[0])
                && span_extent(v, d.es, d.ef).0.round() != f64::from(ring.pos[0])
        });
        assert!(
            found.is_some(),
            "a ring at {} is at no old place",
            ring.pos[0]
        );
        checked += 1;
    }
    assert!(checked > 0, "the release drew no outline");
    assert_equals_compute(&s);
}

/// A chain of `n` one-day tasks, on a canvas 12,500 px tall.
fn chain(n: usize) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    for _ in 0..n {
        let task = Task {
            duration: 480,
            calendar: CalendarId(calendar.0),
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("the calendar is there");
    }
    for t in 1..n as u32 {
        schedule
            .add_dependency(Dependency {
                pred: TaskId(t - 1),
                succ: TaskId(t),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("a chain has no cycle");
    }
    let solution = schedule.compute().expect("a chain computes");
    let order: Vec<u32> = (0..n as u32).collect();
    let timeline = Timeline::new(order, 1200.0, 12_500.0, 1.0);
    Session::new(schedule, solution, timeline, Vec::new(), now)
}

#[test]
fn the_cap_draws_four_hundred_outlines_and_the_report_still_counts_every_task() {
    set_now(0.0);
    let mut s = chain(500);
    s.edit(Edit::SetDuration {
        task: TaskId(0),
        minutes: 480 + 10 * DAY,
    })
    .expect("applies");
    let report = s.move_report().expect("the chain moved").clone();
    assert_eq!(rings(&s).len(), OUTLINE_CAP);
    assert_eq!(report.others, 499);
    assert!(report.finish_days > 0, "{report:?}");
    assert_eq!(
        report.text,
        format!("499 tasks moved · finish +{} days", report.finish_days)
    );
    assert_equals_compute(&s);
}
