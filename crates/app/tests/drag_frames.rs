//! The bar-drag path of the app, without a canvas: press on a bar, move,
//! release. `web::App` drives the same `Session`, so what passes here is
//! what the browser runs, minus the renderer. This file holds the frame and
//! release ordering, the cancel paths and the whole-schedule recompute check.

use crate::common;

use common::{MINUTES_PER_PX, TRAVEL_PX, setup, setup_with};

use app::Session;
use engine::{DAY, TaskId};

/// The first working instant of the task's calendar at or after `wanted`
/// (`Calendar::next_working`). For a task with a duration that is the start
/// the engine gives it, unless a predecessor holds it later; the tests that
/// use this assert `held` is false, so the ask is what lands. A milestone
/// can also start at the end of a working period (`engine::Admissible`),
/// which this does not model: the milestone tests assert their dates directly.
fn snapped(session: &Session, task: u32, wanted: i64) -> i64 {
    session
        .schedule()
        .calendar_of(TaskId(task))
        .next_working(wanted)
}

/// The start a drag to `raw` asks for: the first working minute of the working
/// day the pointer's minute falls on (`Calendar::day_start_at`).
fn asked(session: &Session, task: u32, raw: i64) -> i64 {
    session
        .schedule()
        .calendar_of(TaskId(task))
        .day_start_at(raw)
}

#[test]
fn release_between_frames_applies_the_last_start() {
    let (mut s, task, es, (x, y)) = setup();
    s.pointer_down(x, y, false);
    assert_eq!(s.timeline().bar_drag().map(|b| b.task), Some(task));
    // Several moves and then the release, with no frame (no `apply_drag`)
    // in between: the browser can deliver them inside one animation frame.
    for step in [0.25, 0.5, 0.75, 1.0] {
        s.pointer_move(x + TRAVEL_PX * step, y);
    }
    // Nothing is recomputed until a frame or the release asks for it.
    assert_eq!(s.revision(), 0);
    assert_eq!(s.solution().dates[task as usize].es, es);

    s.pointer_up();

    let wanted = asked(&s, task, es + (TRAVEL_PX * MINUTES_PER_PX) as i64);
    assert!(s.timeline().bar_drag().is_none(), "the drag ended");
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(wanted));
    let info = s.drag_info().expect("the release recorded the drag");
    assert!(!info.held, "no predecessor holds this task past the ask");
    assert_eq!(info.task, task);
    let landed = snapped(&s, task, wanted);
    assert_eq!(info.es, landed);
    assert_eq!(s.solution().dates[task as usize].es, landed);
    assert!(landed >= wanted && landed - wanted < 3 * DAY);
    // One recompute for all four moves.
    assert_eq!(s.revision(), 1);
}

#[test]
fn a_frame_mid_drag_does_not_lose_the_moves_after_it() {
    let (mut s, task, es, (x, y)) = setup();
    s.pointer_down(x, y, false);
    s.pointer_move(x + 40.0, y);
    // A frame lands: the first ask is applied.
    s.apply_drag();
    let first = asked(&s, task, es + (40.0 * MINUTES_PER_PX) as i64);
    assert_eq!(s.revision(), 1);
    assert_eq!(
        s.drag_info().map(|d| d.wanted),
        Some(snapped(&s, task, first))
    );
    // More moves, then the release, before the next frame.
    s.pointer_move(x + 120.0, y);
    s.pointer_move(x + TRAVEL_PX, y);
    s.pointer_up();

    let wanted = asked(&s, task, es + (TRAVEL_PX * MINUTES_PER_PX) as i64);
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(wanted));
    assert!(!s.drag_info().expect("recorded").held);
    assert_eq!(
        s.solution().dates[task as usize].es,
        snapped(&s, task, wanted)
    );
    assert_eq!(s.revision(), 2);
}

#[test]
fn a_cancel_before_any_frame_commits_nothing() {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    s.pointer_down(x, y, false);
    s.pointer_move(x + TRAVEL_PX, y);
    s.pointer_cancel();
    assert!(s.timeline().bar_drag().is_none());
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
    assert!(s.drag_info().is_none());
}

#[test]
fn a_cancel_restores_the_snet_the_task_had_at_its_own_press() {
    let (mut s, task, es, (x, y)) = setup_with(true);
    // A first drag commits a constraint.
    s.pointer_down(x, y, false);
    s.pointer_move(x + TRAVEL_PX, y);
    s.pointer_up();
    let committed = s.schedule().tasks()[task as usize].snet;
    assert!(committed.is_some());
    let moved = s.solution().clone();
    let revision = s.revision();
    let stamp = s.stamp();
    // A second drag on the moved bar, cancelled, goes back to that constraint,
    // not to none and not to the original start.
    let (x2, y2) = (
        s.timeline()
            .viewport
            .x_of(moved.dates[task as usize].es as f64)
            + 2.0,
        y,
    );
    assert_eq!(s.timeline().hit_test(&moved.dates, x2, y2), Some(task));
    s.pointer_down(x2, y2, false);
    s.pointer_move(x2 + 100.0, y2);
    s.apply_drag();
    assert_eq!(s.revision(), revision + 1);
    s.pointer_cancel();
    assert_eq!(s.schedule().tasks()[task as usize].snet, committed);
    assert_eq!(*s.solution(), moved);
    assert_ne!(s.solution().dates[task as usize].es, es);
    assert_eq!(s.revision(), revision, "back to the revision of this press");
    assert_eq!(s.stamp(), stamp + 2);
}

#[test]
fn a_cancelled_pan_or_click_is_harmless() {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    // A press on a bar that never passes the threshold.
    s.pointer_down(x, y, false);
    s.pointer_cancel();
    // A press on the axis band: a pan.
    s.pointer_down(x, 5.0, false);
    s.pointer_move(x + 50.0, 5.0);
    s.pointer_cancel();
    assert!(!s.timeline().dragging());
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
    assert_eq!(s.revision(), 0);
}

#[test]
fn the_wheel_does_not_move_the_view_during_a_bar_drag() {
    let (mut s, task, es, (x, y)) = setup();
    s.pointer_down(x, y, false);
    s.pointer_move(x + 40.0, y);
    let t0 = s.timeline().viewport.t0;
    // The app's wheel entry point is `Session::wheel`.
    s.wheel(x, y, 300.0, 0.0, false);
    s.wheel(x, y, 0.0, -400.0, true);
    assert_eq!(s.timeline().viewport.t0, t0);
    s.pointer_up();
    // The release asks for exactly the pointer's travel, not a panned view.
    let wanted = asked(&s, task, es + (40.0 * MINUTES_PER_PX) as i64);
    assert_eq!(s.schedule().tasks()[task as usize].snet, Some(wanted));
}

#[test]
fn the_pressed_bar_has_no_outline_until_the_drag_starts() {
    let outlines = |s: &Session| {
        let outline = s.theme().text;
        s.scene()
            .rects
            .iter()
            .filter(|r| r.color == outline)
            .count()
    };
    let (mut s, _, _, (x, y)) = setup();
    assert_eq!(outlines(&s), 0);
    s.pointer_down(x, y, false);
    assert!(!s.drag_started());
    assert_eq!(outlines(&s), 0, "a press is not yet a drag");
    s.pointer_move(x + 2.0, y);
    assert_eq!(outlines(&s), 0);
    s.pointer_move(x + 4.0, y);
    assert!(s.drag_started());
    assert_eq!(outlines(&s), 1);
}

/// `apply_drag` updates the solution through the engine's cone path. After
/// every frame of a drag, forward, back past the start and forward again,
/// the session's solution is what a full recompute of its schedule gives:
/// every task's dates and float, and the project finish.
#[test]
fn every_frame_of_a_drag_equals_a_full_recompute() {
    let (mut s, task, _, (x, y)) = setup_with(true);
    let start = s.solution().clone();
    s.pointer_down(x, y, false);
    let mut others_moved = 0;
    let mut frames = 0;
    for dx in [30.0, 120.0, 200.0, 700.0, 40.0, -20.0, 0.0, 400.0] {
        s.pointer_move(x + dx, y);
        let stamp = s.stamp();
        let snet = s.schedule().tasks()[task as usize].snet;
        s.apply_drag();
        // A frame that leaves the constraint as it was changed nothing.
        let changed = s.schedule().tasks()[task as usize].snet != snet;
        assert_eq!(
            s.stamp(),
            stamp + u32::from(changed),
            "dx {dx}: the frame recomputed"
        );
        if dx == 0.0 {
            assert_eq!(s.revision(), 0, "dx 0: back at the press, so the revision");
        }
        frames += u32::from(changed);
        let full = s
            .schedule()
            .clone()
            .compute()
            .expect("the edited schedule computes");
        assert_eq!(*s.solution(), full, "dx {dx}");
        others_moved += (0..start.dates.len())
            .filter(|&i| i != task as usize && s.solution().dates[i].es != start.dates[i].es)
            .count();
    }
    s.pointer_up();
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
    assert_eq!(s.stamp(), frames);
    // The last frame moved the task again from the press-time schedule.
    assert_eq!(s.revision(), 1);
    // The drag reached other tasks, so the cone was not just the dragged one.
    assert!(others_moved > 0, "no other task moved");
}
