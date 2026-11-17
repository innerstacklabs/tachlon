//! Shared setup for the bar-drag tests (`drag_*.rs`), the collapse tests (`collapse*.rs`, helpers
//! in `collapse.rs`) and the open tests (`open*.rs`, helpers in `xer_text.rs`): the clock, the demo
//! sessions, the generated-schedule canvas and the pointer constants. Each test binary uses
//! part of it, so unused items are expected.
#![allow(dead_code, reason = "each test binary uses part of the shared setup")]

pub mod baseline;
pub mod collapse;
pub mod xer_text;

use app::{
    SEED, Session, demo,
    meta::TaskMeta,
    outline::{Outline, demo_layout},
};
use engine::{CalendarId, Task, TaskId};
use timeline::Timeline;

/// Pixels the pointer travels to the right in the drag.
pub const TRAVEL_PX: f64 = 200.0;
/// Minutes per pixel at the test's zoom, so a travel of `TRAVEL_PX` asks for
/// exactly `TRAVEL_PX * MINUTES_PER_PX` minutes more than the grabbed start.
pub const MINUTES_PER_PX: f64 = 100.0;

pub fn clock() -> f64 {
    0.0
}

/// Zooms `session` to `minutes_per_px` with the instant `t0` at the left edge of the canvas. The
/// tests used to set the viewport's scale and left edge by hand; `Session::fit` leaves a 2%
/// margin on the left and pads the span by 4%, so the span to fit is worked back from those.
pub fn zoom_to(session: &mut Session, minutes_per_px: f64, t0: f64) {
    let span = session.timeline().viewport.width * minutes_per_px;
    let start = t0 + 0.02 * span;
    session.fit(start, start + span / 1.04);
}

/// A 1000 x 400 session over `demo(n)`, with its task meta.
pub fn demo_session(n: usize) -> (Session, Vec<TaskMeta>) {
    demo_session_seeded(n, |_| {})
}

/// [`demo_session`], with `seed` changing the task meta before the session is built, the way an
/// imported file brings its own names. The returned meta is the seeded one.
pub fn demo_session_seeded(
    n: usize,
    seed: impl FnOnce(&mut Vec<TaskMeta>),
) -> (Session, Vec<TaskMeta>) {
    let (schedule, solution, order, mut meta) = demo(n);
    seed(&mut meta);
    (
        Session::new(
            schedule,
            solution,
            Timeline::new(order, 1_000.0, 400.0, 1.0),
            meta.clone(),
            clock,
        ),
        meta,
    )
}

/// A 1000 x 400 session over `demo_layout(n)` with its outline, and the task meta.
pub fn outline_session(n: usize) -> (Session, Vec<TaskMeta>) {
    let layout = demo_layout(n);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
        .expect("demo layout has a valid outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta.clone(),
        clock,
    )
    .with_outline(outline)
    .expect("outline matches the demo schedule");
    (session, layout.meta)
}

/// A 1000 x 400 canvas over a generated schedule, zoomed to 100 minutes per
/// pixel with the row of a mid-schedule task at the top and the task's bar
/// starting 500 px in from the left. Returns the session, the task, its
/// start, and the canvas point to press on its bar.
pub fn setup() -> (Session, u32, i64, (f64, f64)) {
    setup_with(false)
}

/// [`setup`], with the task required to have a successor when
/// `needs_successor` is set, so a drag has a downstream to move.
pub fn setup_with(needs_successor: bool) -> (Session, u32, i64, (f64, f64)) {
    setup_with_clock(needs_successor, clock)
}

/// [`setup_with`], on a session that reads `clock` for its milliseconds.
pub fn setup_with_clock(
    needs_successor: bool,
    clock: fn() -> f64,
) -> (Session, u32, i64, (f64, f64)) {
    let (schedule, solution, order, _) = demo(2_000);
    let mut timeline = Timeline::new(order, 1000.0, 400.0, 1.0);
    let mut has_successor = vec![false; schedule.tasks().len()];
    for d in schedule.dependencies() {
        has_successor[d.pred.0 as usize] = true;
    }
    // A task whose bar is at least 8 px wide, far enough down that the row
    // is not the first.
    let row = (100..timeline.order().len())
        .find(|&r| {
            let t = timeline.order()[r] as usize;
            let d = &solution.dates[t];
            d.ef - d.es >= 800 && (has_successor[t] || !needs_successor)
        })
        .expect("a generated schedule has tasks that long");
    let task = timeline.order()[row];
    let es = solution.dates[task as usize].es;
    let v = &mut timeline.viewport;
    v.px_per_minute = 1.0 / MINUTES_PER_PX;
    v.t0 = es as f64 - 500.0 * MINUTES_PER_PX;
    v.scroll_to_row(row);
    let point = (v.x_of(es as f64) + 2.0, v.row_top(row) + v.row_height / 2.0);
    let session = Session::new(schedule, solution, timeline, Vec::new(), clock);
    // The press must land on that bar, or every test below checks nothing.
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task)
    );
    (session, task, es, point)
}

/// A 1000 x 400 session over `engine::stress(SEED, n)`, solved the way `App::stress` does it, with
/// a flat outline: one extra summary above every stress task, so that selecting it covers the whole
/// schedule. Returns the session and the summary's id.
pub fn rooted_stress_session(n: usize) -> (Session, TaskId) {
    let (mut schedule, _head) = engine::stress(SEED, n);
    let top = schedule
        .add_task(Task {
            duration: 0,
            calendar: CalendarId(0),
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("the stress schedule has calendar 0");
    let solution = schedule
        .compute()
        .expect("generated schedules are acyclic and valid");
    let mut meta = app::names::generate(SEED, n);
    for task in &mut meta {
        task.parent = Some(top);
        task.outline_level = 2;
    }
    meta.push(TaskMeta {
        name: "all tasks".to_string(),
        parent: None,
        outline_level: 1,
        code: None,
        ..TaskMeta::default()
    });
    let wbs: Vec<TaskId> = std::iter::once(top)
        .chain((0..n as u32).map(TaskId))
        .collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let order = timeline::order_by_start(&solution.dates);
    let timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    let session = Session::new(schedule, solution, timeline, meta, clock)
        .with_outline(outline)
        .expect("outline matches the schedule");
    (session, top)
}
