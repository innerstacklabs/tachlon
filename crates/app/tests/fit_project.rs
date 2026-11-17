//! The Fit button (gs#703): one call shows the earliest start of any live task to the project finish,
//! whatever the view was and whatever the row filter hides.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::Session;
use common::demo_session;
use engine::TaskId;

/// The first start and the last finish over every live task, in minutes.
fn project_span(session: &Session) -> (f64, f64) {
    let live = (0..session.schedule().tasks().len() as u32)
        .filter(|&t| !session.schedule().is_deleted(TaskId(t)));
    let first = live
        .clone()
        .map(|t| session.solution().dates[t as usize].es)
        .min();
    (
        first.unwrap() as f64,
        session.solution().project_finish as f64,
    )
}

fn zoom_in_twice(session: &mut Session) {
    for _ in 0..2 {
        session.wheel(500.0, 0.0, 0.0, -350.0, true);
    }
}

fn assert_shows_project(session: &Session, what: &str) {
    let (first, last) = project_span(session);
    let (left, right) = session.timeline().viewport.visible_time();
    assert!(
        left <= first && last <= right,
        "{what}: {first}..{last} is inside {left}..{right}"
    );
    assert!(
        (last - first) * 1.15 >= right - left,
        "{what}: the project fills the chart, not much more ({} of {} minutes)",
        last - first,
        right - left
    );
}

#[test]
fn fit_project_shows_every_bar_after_a_zoom_in() {
    let (mut session, _) = demo_session(2_000);
    zoom_in_twice(&mut session);
    let (first, last) = project_span(&session);
    let (left, right) = session.timeline().viewport.visible_time();
    assert!(
        !(left <= first && last <= right),
        "the zoom-ins left the project out of view"
    );
    assert!(session.fit_project(), "a schedule with tasks fits");
    session.land_fly();
    assert_shows_project(&session, "after two zoom-ins");
}

#[test]
fn fit_project_ignores_the_row_filter() {
    let (mut session, _) = demo_session(2_000);
    session.set_critical_only(true);
    zoom_in_twice(&mut session);
    assert!(session.fit_project());
    session.land_fly();
    assert_shows_project(&session, "with Critical only on");
}

#[test]
fn fit_project_ends_the_opening_fit() {
    let (mut session, _) = demo_session(2_000);
    assert!(session.fit_project());
    session.land_fly();
    let view = {
        let v = &session.timeline().viewport;
        (v.t0, v.px_per_minute)
    };
    session.resize(1_200.0, 400.0, 1.0);
    let v = &session.timeline().viewport;
    assert_eq!(
        (v.t0, v.px_per_minute),
        view,
        "a resize keeps the fitted project"
    );
}
