use crate::common;

use app::{Session, demo, edit::Edit, meta::TaskMeta};
use common::{clock, demo_session, outline_session};
use engine::{DAY, Rng, Solution, Task, TaskId};
use timeline::{Summaries, Timeline};

type Snapshot = (Vec<Task>, Solution, Vec<TaskMeta>, Summaries);

pub(crate) fn snapshot(session: &Session) -> Snapshot {
    (
        session.schedule().tasks().to_vec(),
        session.solution().clone(),
        session.meta().to_vec(),
        session.timeline().summaries().clone(),
    )
}

fn random_edit(rng: &mut Rng, session: &Session) -> Edit {
    let task = TaskId(rng.range(0, session.schedule().tasks().len() as i64 - 1) as u32);
    match rng.range(0, 2) {
        0 => {
            let start = session.solution().dates[task.0 as usize].es;
            let snet = if rng.chance(0.2) {
                None
            } else {
                Some((start + rng.range(-3 * DAY, 3 * DAY)).max(0))
            };
            Edit::Move { task, snet }
        }
        1 => Edit::SetDuration {
            task,
            minutes: rng.range(0, 4 * DAY),
        },
        _ => {
            assert!(
                (task.0 as usize) < session.meta().len(),
                "random task has metadata"
            );
            Edit::Rename {
                task,
                name: format!("T{}", rng.range(0, 1_000_000)),
            }
        }
    }
}

/// A 1000 x 400 canvas over a generated schedule, zoomed to 100 minutes per
/// pixel with the row of a mid-schedule task at the top and the task's bar
/// starting 500 px in from the left. Returns the session, the
/// task, its start, and the canvas point to press on its bar.
fn setup() -> (Session, u32, i64, (f64, f64)) {
    const MINUTES_PER_PX: f64 = 100.0;

    let (schedule, solution, order, meta) = demo(2_000);
    let mut timeline = Timeline::new(order, 1_000.0, 400.0, 1.0);
    let row = (100..timeline.order().len())
        .find(|&row| {
            let task = timeline.order()[row] as usize;
            let dates = &solution.dates[task];
            dates.ef - dates.es >= 800
        })
        .expect("a generated schedule has a task long enough to press");
    let task = timeline.order()[row];
    let start = solution.dates[task as usize].es;
    let viewport = &mut timeline.viewport;
    viewport.px_per_minute = 1.0 / MINUTES_PER_PX;
    viewport.t0 = start as f64 - 500.0 * MINUTES_PER_PX;
    viewport.scroll_to_row(row);
    let point = (
        viewport.x_of(start as f64) + 2.0,
        viewport.row_top(row) + viewport.row_height / 2.0,
    );
    let session = Session::new(schedule, solution, timeline, meta, clock);
    assert_eq!(
        session
            .timeline()
            .hit_test(&session.solution().dates, point.0, point.1),
        Some(task),
        "the setup point lands on the task bar"
    );
    (session, task, start, point)
}

#[test]
fn undoing_every_edit_restores_the_state_exactly() {
    let (session, _) = outline_session(800);
    let mut session = session.with_history_cap(500);
    let mut rng = Rng::new(0x51_0001);
    let mut before = Vec::with_capacity(500);
    let mut draws = 0;

    while before.len() < 500 {
        draws += 1;
        let edit = random_edit(&mut rng, &session);
        let state = snapshot(&session);
        if session.edit(edit).is_ok() {
            before.push(state);
        }
        assert!(draws < 2_000, "500 successful edits took {draws} draws");
    }
    let after = snapshot(&session);

    for index in (0..500).rev() {
        assert_eq!(session.undo(), Ok(true), "undo edit {index}");
        assert_eq!(
            snapshot(&session),
            before[index],
            "state after undoing edit {index}"
        );
    }
    assert_eq!(
        snapshot(&session),
        before[0],
        "all undos restore the initial state"
    );
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the restored schedule computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "the restored solution is a fresh full solve"
    );
    assert!(!session.can_undo(), "all undo entries were consumed");
    assert_eq!(session.undo(), Ok(false), "undo reports an empty stack");

    for index in 0..500 {
        assert_eq!(session.redo(), Ok(true), "redo edit {index}");
        let expected = if index == 499 {
            &after
        } else {
            &before[index + 1]
        };
        assert_eq!(
            snapshot(&session),
            *expected,
            "state after redoing edit {index}"
        );
    }
    assert_eq!(
        snapshot(&session),
        after,
        "all redos restore the post-sequence state"
    );
    assert!(!session.can_redo(), "all redo entries were consumed");
}

#[test]
fn the_history_keeps_the_last_200_steps() {
    let (mut session, _) = demo_session(300);
    let mut rng = Rng::new(0x51_0002);
    let mut before = Vec::with_capacity(500);
    let mut draws = 0;

    while before.len() < 500 {
        draws += 1;
        let edit = random_edit(&mut rng, &session);
        let state = snapshot(&session);
        if session.edit(edit).is_ok() {
            before.push(state);
        }
        assert!(draws < 2_000, "500 successful edits took {draws} draws");
    }

    let mut undone = 0;
    while session.undo().expect("undo does not fail") {
        undone += 1;
    }
    assert_eq!(undone, 200, "the default cap retains 200 undo steps");
    assert_eq!(
        snapshot(&session),
        before[300],
        "undoing the retained steps restores the state before edit 300"
    );
    assert_eq!(
        session.history_len(),
        (0, 200),
        "the retained entries moved to redo"
    );
}

#[test]
fn a_new_edit_clears_the_redo_stack() {
    let (mut session, _) = demo_session(30);
    let task = TaskId(0);

    for name in ["First", "Second", "Third"] {
        session
            .edit(Edit::Rename {
                task,
                name: name.to_string(),
            })
            .expect("setup rename applies");
    }
    assert_eq!(session.undo(), Ok(true), "first setup undo");
    assert_eq!(session.undo(), Ok(true), "second setup undo");
    assert_eq!(
        session.history_len(),
        (1, 2),
        "two entries are available to redo"
    );

    session
        .edit(Edit::Rename {
            task,
            name: "Different".to_string(),
        })
        .expect("new branch edit applies");
    assert!(!session.can_redo(), "the branch edit clears redo");
    assert_eq!(session.redo(), Ok(false), "redo reports the cleared stack");
    assert_eq!(
        session.history_len(),
        (2, 0),
        "the branch edit is the second undo entry"
    );

    let history_before_raw_apply = session.history_len();
    session
        .apply(Edit::Rename {
            task,
            name: "Raw apply".to_string(),
        })
        .expect("raw apply succeeds");
    assert_eq!(
        session.history_len(),
        history_before_raw_apply,
        "raw apply records no history"
    );
}

#[test]
fn a_refused_edit_is_not_recorded() {
    let (mut session, _) = demo_session(30);
    let before = snapshot(&session);

    assert!(
        session
            .edit(Edit::Rename {
                task: TaskId(0),
                name: String::new(),
            },)
            .is_err(),
        "an empty name is refused"
    );
    assert_eq!(
        session.history_len(),
        (0, 0),
        "the refused edit records nothing"
    );
    assert_eq!(
        snapshot(&session),
        before,
        "the refused edit changes no state"
    );
}

#[test]
fn a_drag_with_fifty_pointer_moves_is_one_history_entry() {
    let (mut session, task, start, (x, y)) = setup();
    let before = snapshot(&session);
    session.pointer_down(x, y, false);
    for step in 1..=50 {
        session.pointer_move(x + 4.0 * f64::from(step), y);
        session.apply_drag();
    }
    session.pointer_up();
    let dragged_start = session.solution().dates[task as usize].es;
    let after = snapshot(&session);

    assert_ne!(dragged_start, start, "the fifty-move drag moves the task");
    assert_eq!(
        session.history_len(),
        (1, 0),
        "the fifty-move drag records one entry"
    );
    assert_eq!(session.undo(), Ok(true), "the completed drag can be undone");
    assert_eq!(
        snapshot(&session),
        before,
        "undo restores tasks, solution, and the original snet"
    );
    assert_eq!(session.redo(), Ok(true), "the completed drag can be redone");
    assert_eq!(snapshot(&session), after, "redo restores the dragged state");
    assert_eq!(
        session.solution().dates[task as usize].es,
        dragged_start,
        "redo puts the task at the dragged start"
    );

    let (mut session, _task, _start, (x, y)) = setup();
    session.pointer_down(x, y, false);
    for step in 1..=2 {
        session.pointer_move(x + 4.0 * f64::from(step), y);
        session.apply_drag();
    }
    session.pointer_up();
    assert_eq!(
        session.history_len(),
        (1, 0),
        "a two-move drag also records one entry"
    );
}

#[test]
fn a_click_or_a_cancelled_drag_records_nothing() {
    let (mut session, _task, _start, (x, y)) = setup();
    let before = snapshot(&session);

    session.pointer_down(x, y, false);
    session.pointer_up();
    assert_eq!(session.history_len(), (0, 0), "a click records no history");
    assert_eq!(
        snapshot(&session),
        before,
        "a click changes no schedule state"
    );

    session.pointer_down(x, y, false);
    session.pointer_move(x + 20.0, y);
    session.apply_drag();
    session.pointer_cancel();
    assert_eq!(
        session.history_len(),
        (0, 0),
        "a cancelled drag records no history"
    );
    assert_eq!(
        snapshot(&session),
        before,
        "a cancelled drag restores the state"
    );
}

#[test]
fn undo_is_refused_while_a_bar_press_is_in_flight() {
    let (mut session, _task, _start, (x, y)) = setup();
    let edited_task = TaskId(0);
    let duration = session.schedule().tasks()[0].duration;
    session
        .edit(Edit::SetDuration {
            task: edited_task,
            minutes: duration + 60,
        })
        .expect("the earlier duration edit applies");
    assert!(session.can_undo(), "the earlier edit is on the undo stack");

    session.pointer_down(x, y, false);
    let pressed = snapshot(&session);
    assert_eq!(
        session.undo(),
        Ok(false),
        "undo is refused after pointer down"
    );
    assert_eq!(
        session.redo(),
        Ok(false),
        "redo is refused after pointer down"
    );
    assert_eq!(
        snapshot(&session),
        pressed,
        "history operations change nothing after pointer down"
    );

    session.pointer_move(x + 20.0, y);
    session.apply_drag();
    let dragging = snapshot(&session);
    assert_eq!(session.undo(), Ok(false), "undo is refused while dragging");
    assert_eq!(session.redo(), Ok(false), "redo is refused while dragging");
    assert_eq!(
        snapshot(&session),
        dragging,
        "history operations change nothing while dragging"
    );

    session.pointer_up();
    assert_eq!(session.undo(), Ok(true), "undo works after pointer up");
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one test replays a drag and its undo step by step"
)]
fn undoing_a_drag_after_a_duration_edit_keeps_the_solution_exact() {
    let (mut session, task, _start, (x, y)) = setup();
    let initial = snapshot(&session);

    session.pointer_down(x, y, false);
    session.pointer_move(x + 20.0, y);
    session.apply_drag();
    session.pointer_up();
    let after_drag = snapshot(&session);
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the dragged schedule computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after the drag is exact"
    );

    let duration = session.schedule().tasks()[task as usize].duration;
    session
        .edit(Edit::SetDuration {
            task: TaskId(task),
            minutes: duration + 137,
        })
        .expect("duration edit after the drag applies");
    let after_duration = snapshot(&session);
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the duration-edited schedule computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after the duration edit is exact"
    );

    assert_eq!(session.undo(), Ok(true), "undo duration edit");
    assert_eq!(
        snapshot(&session),
        after_drag,
        "undoing duration restores the dragged state"
    );
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the schedule after undoing duration computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after undoing duration is exact"
    );

    assert_eq!(session.undo(), Ok(true), "undo drag");
    assert_eq!(
        snapshot(&session),
        initial,
        "undoing the drag restores the initial state"
    );
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the schedule after undoing the drag computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after undoing the drag is exact"
    );

    assert_eq!(session.redo(), Ok(true), "redo drag");
    assert_eq!(
        snapshot(&session),
        after_drag,
        "redoing the drag restores the dragged state"
    );
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the schedule after redoing the drag computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after redoing the drag is exact"
    );

    assert_eq!(session.redo(), Ok(true), "redo duration edit");
    assert_eq!(
        snapshot(&session),
        after_duration,
        "redoing duration restores the final state"
    );
    let fresh = session
        .schedule()
        .clone()
        .compute()
        .expect("the schedule after redoing duration computes");
    assert_eq!(
        session.solution(),
        &fresh,
        "solution after redoing duration is exact"
    );
}
