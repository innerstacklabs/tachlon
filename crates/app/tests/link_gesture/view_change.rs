//! The link drag when the view changes under it or the pointer leaves the canvas, and Escape
//! (gs#346 fix rounds 1 and 2). Helpers come from the parent module.

use super::*;

/// `session()` on a canvas short enough that the rows scroll by one row.
fn short_session() -> Session {
    let mut s = session();
    s.resize(1_000.0, 168.0, 1.0);
    zoom_to(&mut s, 1.0, 0.0);
    s
}

/// A point on `task`'s row at pixel `x`.
fn at_x(s: &Session, task: u32, x: f64) -> (f64, f64) {
    (x, on(s, task).1)
}

#[test]
fn escape_cancels_the_drag_and_the_release_adds_nothing() {
    let mut s = session();
    let before = state(&s);
    let from = on(&s, A);
    let to = on(&s, C);
    drag(&mut s, from, &[to]);
    assert!(s.linking());
    assert_eq!(s.edit(add(B, D)), Err(EditError::Busy), "busy mid-drag");

    // The keys.ts handler calls `App.pointer_cancel` for Escape, which is this.
    s.pointer_cancel();
    assert!(!s.linking());
    assert!(status(&s)["link"]["drag"].is_null());
    s.pointer_move(to.0 + 3.0, to.1);
    s.pointer_up();
    assert_eq!(state(&s), before, "the release after Escape adds nothing");
    assert_eq!(s.link_refusal(), None);
    s.edit(add(B, D))
        .expect("the session is not busy after Escape");
}

#[test]
fn wheeling_the_target_away_leaves_nothing_under_the_pointer() {
    let mut s = short_session();
    let before = state(&s);
    let from = on(&s, D);
    let onto = at_x(&s, B, 65.0);
    drag(&mut s, from, &[onto]);
    assert_eq!(status(&s)["link"]["drag"]["target"], B);

    // The wheel pans time: B leaves the pointer and the pointer does not move.
    s.wheel(onto.0, onto.1, 300.0, 0.0, false);
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        serde_json::Value::Null
    );
    s.pointer_up();
    assert_eq!(state(&s), before, "no link to a bar that is not there now");
    assert_eq!(s.link_refusal(), None);
}

#[test]
fn wheeling_another_task_under_the_pointer_targets_that_one() {
    let mut s = short_session();
    let before = state(&s);
    let from = on(&s, D);
    let onto = at_x(&s, B, 65.0);
    drag(&mut s, from, &[onto]);
    assert_eq!(status(&s)["link"]["drag"]["target"], B);
    assert_eq!(
        status(&s)["link"]["drag"]["refused"],
        serde_json::Value::Null
    );
    let row_height = s.timeline().viewport.row_height;
    let checks = s.link_checks();

    // One row down the schedule: C, which D follows, is under the pointer now.
    s.wheel(onto.0, onto.1, 0.0, row_height, false);
    assert_eq!(s.timeline().viewport.scroll_y, row_height);
    assert_eq!(status(&s)["link"]["drag"]["target"], C);
    assert!(status(&s)["link"]["drag"]["refused"].is_string());
    assert_eq!(s.link_checks(), checks + 1, "one search for the new target");
    s.pointer_up();
    assert_eq!(state(&s), before, "D -> B is not added; D -> C is a loop");
    assert!(s.link_refusal().is_some());
}

#[test]
fn a_release_past_the_left_or_right_edge_adds_nothing() {
    // The left edge: C runs from before the canvas, B sits on it.
    let mut s = session();
    s.wheel(0.0, 0.0, 50.0, 0.0, false);
    let width = s.timeline().viewport.width;
    assert!(s.timeline().viewport.x_of(0.0) < 0.0, "C starts off-canvas");
    let before = state(&s);
    let from = at_x(&s, B, 20.0);
    let onto = at_x(&s, C, 5.0);
    drag(&mut s, from, &[onto]);
    assert_eq!(status(&s)["link"]["drag"]["target"], C);
    s.pointer_move(-10.0, on(&s, C).1);
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        serde_json::Value::Null
    );
    s.pointer_up();
    assert_eq!(state(&s), before, "released past the left edge");

    // The right edge: C runs past the canvas, B sits near it.
    let mut s = session();
    s.wheel(0.0, 0.0, -850.0, 0.0, false);
    assert!(
        s.timeline().viewport.x_of(180.0) > width,
        "C ends off-canvas"
    );
    let before = state(&s);
    let from = at_x(&s, B, 920.0);
    let onto = at_x(&s, C, 900.0);
    drag(&mut s, from, &[onto]);
    assert_eq!(status(&s)["link"]["drag"]["target"], C);
    s.pointer_move(width + 10.0, on(&s, C).1);
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        serde_json::Value::Null
    );
    s.pointer_up();
    assert_eq!(state(&s), before, "released past the right edge");

    // Inside the canvas the same bar still takes the link.
    let mut s = session();
    s.wheel(0.0, 0.0, 50.0, 0.0, false);
    let onto = at_x(&s, C, 5.0);
    link(&mut s, B, onto);
    assert_eq!(live(&s), 3);
}

/// Task `A` (row 0) and a milestone `M` (row 1) at minute 600, 24/7 at one minute per pixel.
fn milestone_session() -> Session {
    let mut schedule = Schedule::new();
    let day = vec![(0, 24 * 60)];
    let calendar = Calendar::new(std::array::from_fn(|_| day.clone()), &[]).expect("24/7");
    let calendar = schedule.add_calendar(calendar);
    let mut meta: Vec<TaskMeta> = Vec::new();
    for (name, duration, snet) in [("A", HOUR, None), ("M", 0, Some(600))] {
        let task = Task {
            duration,
            calendar,
            snet,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("a task");
        meta.push(TaskMeta {
            name: name.to_string(),
            parent: None,
            outline_level: 1,
            code: None,
            ..TaskMeta::default()
        });
    }
    let solution = schedule.compute().expect("acyclic");
    let timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    let mut s = Session::new(schedule, solution, timeline, meta, clock);
    zoom_to(&mut s, 1.0, 0.0);
    s
}

/// The x of the milestone's centre and its half width, the bar height over two.
fn milestone_span(s: &Session) -> (f64, f64) {
    let v = &s.timeline().viewport;
    (v.x_of(600.0), (v.row_height * v.bar_ratio).round() / 2.0)
}

#[test]
fn changing_the_bar_look_retargets_a_milestone_under_the_pointer() {
    // Shrinking: the pointer was inside the milestone's square and is outside it now.
    let mut s = milestone_session();
    let before = state(&s);
    let (centre, half) = milestone_span(&s);
    let pointer = (centre + half - 1.0, on(&s, 1).1);
    let from = on(&s, A);
    drag(&mut s, from, &[pointer]);
    assert_eq!(status(&s)["link"]["drag"]["target"], 1);
    assert_eq!(s.set_bar_look(0.0, 0.3), Ok(()));
    assert!(milestone_span(&s).1 < half - 1.0, "the milestone shrank");
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        serde_json::Value::Null,
        "the preview does not stay on a square the pointer left"
    );
    s.pointer_up();
    assert_eq!(
        state(&s),
        before,
        "no link to a milestone that is not there"
    );

    // Growing: the pointer was outside the square and is inside it now.
    let mut s = milestone_session();
    let (centre, half) = milestone_span(&s);
    let pointer = (centre + half + 2.0, on(&s, 1).1);
    let from = on(&s, A);
    drag(&mut s, from, &[pointer]);
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        serde_json::Value::Null
    );
    assert_eq!(s.set_bar_look(0.0, 1.0), Ok(()));
    assert_eq!(
        status(&s)["link"]["drag"]["target"],
        1,
        "the preview follows the milestone that grew under the pointer"
    );
    s.pointer_up();
    assert_eq!(live(&s), 1, "the release adds the link the preview showed");
}
