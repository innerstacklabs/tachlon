//! The opening fit (gs#821): the view an opened file starts with shows every row of its first screen
//! as the rows draw it (a summary row with its rolled-up span), and it is refitted when the canvas
//! changes width before the first wheel (the ID column appears or goes with the file).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, Session, fitted_timeline, load_bytes};
use common::clock;
use import::ProjectSelection;
use xer::synth::{Spec, write};

fn loaded(spec: &Spec) -> Loaded {
    load_bytes(&write(spec), &ProjectSelection::All, &|| 0.0).expect("a generated XER should load")
}

fn session_of(loaded: Loaded, width: f64, height: f64) -> Session {
    let timeline = fitted_timeline(loaded.order, &loaded.solution, width, height, 1.0);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        clock,
    )
    .with_outline(loaded.outline)
    .expect("a loaded outline matches its schedule")
}

/// The first and last instant the first screen's rows draw: a summary's rolled-up span, a leaf's dates.
fn first_screen(session: &Session) -> (f64, f64) {
    let rows = session.timeline().viewport.visible_rows();
    let spans: Vec<_> = session.timeline().order()[rows]
        .iter()
        .map(|&t| {
            session.summary_span(t).unwrap_or_else(|| {
                let d = &session.solution().dates[t as usize];
                (d.es, d.ef)
            })
        })
        .collect();
    let first = spans.iter().map(|s| s.0).min().unwrap();
    let last = spans.iter().map(|s| s.1).max().unwrap();
    (first as f64, last as f64)
}

fn assert_fills(session: &Session, what: &str) {
    let (first, last) = first_screen(session);
    let (left, right) = session.timeline().viewport.visible_time();
    assert!(
        left <= first && last <= right,
        "{what}: the real finish {last} is inside {left}..{right}"
    );
    assert!(
        (last - first) * 1.15 >= right - left,
        "{what}: the first screen fills the chart"
    );
}

#[test]
fn the_fit_includes_the_rolled_up_span_of_a_summary_row() {
    let mut spec = Spec::small(7);
    spec.activities = 120;
    let session = session_of(loaded(&spec), 800.0, 400.0);
    let summary = session.timeline().order()[..session.timeline().viewport.visible_rows().end]
        .iter()
        .find(|&&t| {
            session
                .summary_span(t)
                .is_some_and(|s| s.1 > session.solution().dates[t as usize].ef)
        })
        .copied();
    assert!(
        summary.is_some(),
        "a summary on the first screen rolls up past its own dates"
    );
    assert_fills(&session, "summary rows");
}

#[test]
fn a_canvas_that_changes_width_before_the_first_wheel_is_refitted() {
    let mut session = session_of(loaded(&Spec::with_activities(3, 120)), 542.0, 400.0);
    assert_fills(&session, "narrow");
    session.resize(640.0, 400.0, 1.0);
    assert_fills(&session, "after widening (XER to a file without IDs)");
    session.resize(542.0, 400.0, 1.0);
    assert_fills(&session, "after narrowing (a file without IDs to XER)");
}

#[test]
fn a_wheel_ends_the_opening_fit() {
    let mut session = session_of(loaded(&Spec::with_activities(3, 120)), 542.0, 400.0);
    session.wheel(100.0, 100.0, 0.0, 0.0, true);
    let scale = session.timeline().viewport.px_per_minute;
    session.resize(640.0, 400.0, 1.0);
    assert_eq!(
        session.timeline().viewport.px_per_minute,
        scale,
        "a resize keeps the scale once the person has zoomed"
    );
}

#[test]
fn scrolling_the_rows_ends_the_opening_fit() {
    // The task table scrolls through `scroll_by` (the page's `set_scroll_y`); a resize after it keeps the view.
    let mut session = session_of(loaded(&Spec::with_activities(3, 400)), 542.0, 400.0);
    session.scroll_by(0.0);
    session.scroll_by(240.0);
    let (origin, scale) = (
        session.timeline().viewport.t0,
        session.timeline().viewport.px_per_minute,
    );
    session.resize(640.0, 400.0, 1.0);
    assert_eq!(
        session.timeline().viewport.t0,
        origin,
        "the time origin stays"
    );
    assert_eq!(
        session.timeline().viewport.px_per_minute,
        scale,
        "the zoom stays"
    );
}

#[test]
fn a_zero_scroll_leaves_the_opening_fit_pending() {
    let mut session = session_of(loaded(&Spec::with_activities(3, 120)), 542.0, 400.0);
    session.scroll_by(0.0);
    session.resize(640.0, 400.0, 1.0);
    assert_fills(&session, "after a zero scroll");
}

/// A task below the first screen whose bar starts inside the chart's time range, so revealing it
/// scrolls the rows and has no reason to pan.
fn lower_task_in_view(session: &Session) -> (u32, usize) {
    let viewport = &session.timeline().viewport;
    let (left, right) = viewport.visible_time();
    let below = viewport.visible_rows().end..session.timeline().order().len();
    below
        .map(|row| (session.timeline().order()[row], row))
        .find(|&(t, _)| {
            let start = session.solution().dates[t as usize].es as f64;
            session.summary_span(t).is_none() && (left..=right).contains(&start)
        })
        .expect("a task below the first screen starts inside the chart's time range")
}

fn assert_view_kept(session: &mut Session, view: (f64, f64), what: &str) {
    session.resize(640.0, 400.0, 1.0);
    let viewport = &session.timeline().viewport;
    assert_eq!(
        (viewport.t0, viewport.px_per_minute),
        view,
        "{what}: the time view stays"
    );
}

#[test]
fn finding_a_lower_task_ends_the_opening_fit() {
    let mut session = session_of(loaded(&Spec::with_activities(3, 400)), 542.0, 400.0);
    let (task, _) = lower_task_in_view(&session);
    let before = session.timeline().viewport.visible_time();
    let code = session.task_code(task).to_owned();
    session.find(&code);
    assert!(session.find_go(0).is_some(), "the task is found");
    session.land_fly();
    assert!(
        session.timeline().viewport.scroll_y > 0.0,
        "the rows scrolled"
    );
    assert_eq!(
        session.timeline().viewport.visible_time(),
        before,
        "Find did not pan"
    );
    let viewport = &session.timeline().viewport;
    let view = (viewport.t0, viewport.px_per_minute);
    assert_view_kept(&mut session, view, "after Find");
}

#[test]
fn scrolling_to_a_row_ends_the_opening_fit() {
    let mut session = session_of(loaded(&Spec::with_activities(3, 400)), 542.0, 400.0);
    let (_, row) = lower_task_in_view(&session);
    session.scroll_to_row(row);
    assert!(
        session.timeline().viewport.scroll_y > 0.0,
        "the rows scrolled"
    );
    let viewport = &session.timeline().viewport;
    let view = (viewport.t0, viewport.px_per_minute);
    assert_view_kept(&mut session, view, "after scroll_to_row");
}
