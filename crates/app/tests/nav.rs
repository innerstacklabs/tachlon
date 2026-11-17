//! The cell cursor's rows (gs#416, story S1 of #300): `Session::move_selection` and
//! `Session::select_task` through the native `Session`, over the demo outline.

use crate::common;

use app::{Session, edit::Edit};
use common::collapse::{N, session, summaries};
use engine::TaskId;

/// A summary that has a row after its whole subtree, with that row's task: the pair the skip
/// tests walk across. It is not the first row, so a cursor that restarted from row 0 would be seen.
/// The demo outline has several.
fn summary_with_next(session: &Session) -> (TaskId, TaskId) {
    let outline = session.outline().expect("test session has an outline");
    let preorder = outline.preorder();
    summaries(session)
        .into_iter()
        .find_map(|summary| {
            let at = preorder.iter().position(|&t| t == summary)?;
            let end = at + outline.subtree(summary).len();
            let next = *preorder.get(end)?;
            (at > 0 && outline.subtree(summary).len() > 1).then_some((summary, next))
        })
        .expect("the demo outline has a summary with a row after it")
}

fn selected(session: &Session) -> Option<u32> {
    session.selection().map(|selection| selection.task)
}

#[test]
fn arrow_down_skips_the_children_of_a_collapsed_summary() {
    let mut session = session(N);
    let (summary, next) = summary_with_next(&session);
    assert!(session.toggle(summary), "the summary collapses");
    session.select(summary.0);

    let moved = session.move_selection(1);

    assert_eq!(
        moved,
        Some(next.0),
        "down lands on the row after the subtree"
    );
    assert_eq!(selected(&session), Some(next.0));
}

#[test]
fn arrow_up_skips_the_children_of_a_collapsed_summary() {
    let mut session = session(N);
    let (summary, next) = summary_with_next(&session);
    assert!(session.toggle(summary), "the summary collapses");
    session.select(next.0);

    let moved = session.move_selection(-1);

    assert_eq!(moved, Some(summary.0), "up lands on the collapsed summary");
}

#[test]
fn an_expanded_summary_is_walked_into() {
    let mut session = session(N);
    let (summary, _) = summary_with_next(&session);
    let first_child = session.outline().expect("outline").children(summary)[0];
    session.select(summary.0);

    assert_eq!(session.move_selection(1), Some(first_child.0));
}

#[test]
fn the_cursor_stops_at_the_first_and_last_row() {
    let mut session = session(N);
    let order = session.timeline().order().to_vec();
    let (first, last) = (order[0], *order.last().expect("rows"));

    session.select(last);
    assert_eq!(
        session.move_selection(1),
        Some(last),
        "down at the last row stays"
    );
    assert_eq!(selected(&session), Some(last));

    session.select(first);
    assert_eq!(
        session.move_selection(-1),
        Some(first),
        "up at the first row stays"
    );
    assert_eq!(
        session.move_selection(3),
        Some(order[3]),
        "a larger step moves that many rows"
    );
    assert_eq!(
        session.move_selection(-100_000),
        Some(first),
        "a step past the top clamps"
    );
}

#[test]
fn nothing_selected_starts_at_the_first_row() {
    let mut session = session(N);
    let first = session.timeline().order()[0];
    assert_eq!(selected(&session), None);

    assert_eq!(session.move_selection(1), Some(first));
    assert_eq!(selected(&session), Some(first));
}

#[test]
fn a_selection_hidden_by_a_collapse_moves_from_its_summary() {
    let mut session = session(N);
    let (summary, next) = summary_with_next(&session);
    let child = session.outline().expect("outline").children(summary)[0];
    session.select(child.0);
    assert!(
        session.toggle(summary),
        "the summary collapses over the selection"
    );
    assert_eq!(
        selected(&session),
        Some(child.0),
        "the selection stays on the hidden child"
    );

    assert_eq!(session.move_selection(1), Some(next.0));
}

#[test]
fn select_task_refuses_a_hidden_or_unknown_task() {
    let mut session = session(N);
    let (summary, next) = summary_with_next(&session);
    let child = session.outline().expect("outline").children(summary)[0];
    assert!(session.toggle(summary), "the summary collapses");

    assert!(!session.select_task(child.0), "a hidden child has no row");
    assert!(!session.select_task(u32::MAX), "an unknown task");
    assert_eq!(selected(&session), None, "a refusal selects nothing");
    assert!(session.select_task(next.0));
    assert_eq!(selected(&session), Some(next.0));
}

#[test]
fn the_cursor_survives_an_edit_and_its_undo() {
    let mut session = session(N);
    let task = session.timeline().order()[2];
    assert!(session.select_task(task));

    session
        .edit(Edit::Rename {
            task: TaskId(task),
            name: "Renamed".to_string(),
        })
        .expect("rename");
    assert_eq!(selected(&session), Some(task), "selected after the edit");
    assert!(session.undo().expect("undo"));
    assert_eq!(selected(&session), Some(task), "selected after the undo");

    let order = session.timeline().order().to_vec();
    assert_eq!(
        session.move_selection(1),
        Some(order[3]),
        "the cursor still moves from its row"
    );
}

/// Zoom the view onto a short span at the project's start, and return a task whose bar starts
/// well to the right of it, with that start.
fn session_viewing_the_start() -> (Session, u32, f64) {
    let mut session = session(N);
    session.fit(0.0, 1_000.0);
    let task = session
        .timeline()
        .order()
        .iter()
        .copied()
        .find(|&t| session.solution().dates[t as usize].es as f64 > 50_000.0)
        .expect("the demo has a task starting far to the right");
    let start = session.solution().dates[task as usize].es as f64;
    (session, task, start)
}

fn start_x(session: &Session, start: f64) -> f64 {
    session.timeline().viewport.x_of(start)
}

#[test]
fn selecting_a_task_pans_to_a_bar_that_starts_off_screen() {
    let (mut session, task, start) = session_viewing_the_start();
    let zoom = session.timeline().viewport.px_per_minute;
    let width = session.timeline().viewport.width;
    assert!(
        start_x(&session, start) > width,
        "the bar starts off to the right"
    );

    assert!(session.select_task(task));

    let x = start_x(&session, start);
    assert!(
        (x - width * 0.2).abs() < 1e-6,
        "the start sits a fifth in, at {x}"
    );
    assert_eq!(
        session.timeline().viewport.px_per_minute,
        zoom,
        "the zoom stays"
    );
}

#[test]
fn selecting_a_task_pans_back_to_a_bar_that_starts_off_to_the_left() {
    let (mut session, task, start) = session_viewing_the_start();
    assert!(session.select_task(task));
    // A bar well before the far one but not at the project's start, where the pan clamps.
    let first = session
        .timeline()
        .order()
        .iter()
        .copied()
        .find(|&t| {
            let es = session.solution().dates[t as usize].es as f64;
            es > 10_000.0 && es < start - 20_000.0
        })
        .expect("the demo has a task starting before the far one");
    let first_start = session.solution().dates[first as usize].es as f64;
    assert!(
        start_x(&session, first_start) < 0.0,
        "that bar is now off to the left"
    );

    assert!(session.select_task(first));

    let width = session.timeline().viewport.width;
    let x = start_x(&session, first_start);
    assert!(
        (x - width * 0.2).abs() < 1e-6,
        "the start sits a fifth in, at {x}"
    );
    assert!(
        start > first_start,
        "the first bar starts before the far one"
    );
}

#[test]
fn selecting_a_task_whose_bar_is_on_screen_does_not_pan() {
    let mut session = session(N);
    let task = session.timeline().order()[0];
    session.fit(0.0, 1_000.0);
    let start = session.solution().dates[task as usize].es as f64;
    let x = start_x(&session, start);
    assert!(
        (0.0..=session.timeline().viewport.width).contains(&x),
        "the bar is on screen"
    );
    let t0 = session.timeline().viewport.t0;

    assert!(session.select_task(task));

    assert_eq!(
        session.timeline().viewport.t0,
        t0,
        "an on-screen bar does not move"
    );
}

#[test]
fn arrow_down_pans_to_a_bar_that_starts_off_screen() {
    let (mut session, task, start) = session_viewing_the_start();
    let order = session.timeline().order().to_vec();
    let at = order.iter().position(|&t| t == task).expect("row");
    session.select(order[at - 1]);
    session.fit(0.0, 1_000.0);
    let zoom = session.timeline().viewport.px_per_minute;
    let width = session.timeline().viewport.width;
    assert!(
        start_x(&session, start) > width,
        "the next bar starts off to the right"
    );

    assert_eq!(session.move_selection(1), Some(task));

    let x = start_x(&session, start);
    assert!(
        (x - width * 0.2).abs() < 1e-6,
        "the start sits a fifth in, at {x}"
    );
    assert_eq!(
        session.timeline().viewport.px_per_minute,
        zoom,
        "the zoom stays"
    );
}

/// An edit on the far task, then another task selected and the view back at the start: the state
/// an undo or redo of that edit has to bring the person back from.
fn far_edit_then_elsewhere() -> (Session, u32, f64) {
    let (mut session, far, start) = session_viewing_the_start();
    let was = session.schedule().tasks()[far as usize].duration;
    session
        .edit(Edit::SetDuration {
            task: TaskId(far),
            minutes: was + 480,
        })
        .expect("edit");
    let other = session.timeline().order()[0];
    assert_ne!(other, far);
    session.select(other);
    session.fit(0.0, 1_000.0);
    (session, far, start)
}

fn assert_revealed(session: &Session, far: u32, start: f64) {
    assert_eq!(selected(session), Some(far), "the changed row is selected");
    let x = start_x(session, start);
    let width = session.timeline().viewport.width;
    assert!(
        (0.0..=width).contains(&x),
        "its bar start is on screen at {x}"
    );
}

#[test]
fn undo_selects_and_reveals_the_task_it_changed() {
    let (mut session, far, start) = far_edit_then_elsewhere();
    assert!(session.undo().expect("undo"));
    assert_revealed(&session, far, start);
}

#[test]
fn redo_selects_and_reveals_the_task_it_changed() {
    let (mut session, far, start) = far_edit_then_elsewhere();
    assert!(session.undo().expect("undo"));
    session.select(session.timeline().order()[0]);
    session.fit(0.0, 1_000.0);
    assert!(session.redo().expect("redo"));
    assert_revealed(&session, far, start);
}

#[test]
fn undo_at_the_selected_row_keeps_its_selection() {
    let (mut session, far, _) = far_edit_then_elsewhere();
    session.select(far);
    let before = session.selection().map(|s| (s.task, s.row));
    assert!(session.undo().expect("undo"));
    assert_eq!(session.selection().map(|s| (s.task, s.row)), before);
}

#[test]
fn undoing_a_baseline_step_selects_nothing() {
    let (mut session, _, _) = session_viewing_the_start();
    session
        .edit(Edit::SetBaseline { baseline: None })
        .expect("baseline");
    session.clear_selection();
    assert!(session.undo().expect("undo"));
    assert_eq!(selected(&session), None);
}

#[test]
fn undo_that_leaves_the_selected_link_in_place_keeps_it_selected() {
    let (mut session, far, _) = session_viewing_the_start();
    let was = session.schedule().tasks()[far as usize].duration;
    session
        .edit(Edit::SetDuration {
            task: TaskId(far),
            minutes: was + 480,
        })
        .expect("edit");
    let linked = session
        .schedule()
        .dependencies()
        .iter()
        .find(|d| d.pred.0 != far && d.succ.0 != far)
        .expect("a link away from the far task")
        .pred
        .0;
    session.select(linked);
    assert!(session.step_link(true), "a link of the task is selected");
    let link = session.selected_link();
    assert!(link.is_some());

    assert!(session.undo().expect("undo"));

    assert_eq!(session.selected_link(), link, "the link stays selected");
    assert_eq!(selected(&session), Some(linked), "and so does its task");
}
