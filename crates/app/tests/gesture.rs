//! `Status.gesture` and the second press (gantt#1152): the status names what the pointer is
//! doing, and a press while another is down (a second finger) is ignored until the first ends.

use crate::common;

use app::{RenderFacts, Session, edit::Edit};
use common::{TRAVEL_PX, setup};
use engine::TaskId;

fn gesture(s: &Session) -> String {
    let status: serde_json::Value =
        serde_json::from_str(&s.status_json(&RenderFacts::default())).expect("status is JSON");
    status["gesture"].as_str().unwrap_or("missing").to_string()
}

#[test]
fn the_status_names_the_gesture() {
    let (mut s, task, es, (x, y)) = setup();
    assert_eq!(gesture(&s), "idle");
    s.pointer_down(x, y, false);
    assert_eq!(gesture(&s), "bar-pressed");
    s.pointer_move(x + TRAVEL_PX, y);
    assert_eq!(gesture(&s), "bar-dragging");
    s.pointer_up();
    s.pointer_down(x + TRAVEL_PX, y, true);
    assert_eq!(gesture(&s), "link-pressed");
    s.pointer_move(x + TRAVEL_PX + 40.0, y + 40.0);
    assert_eq!(gesture(&s), "link-dragging");
    s.pointer_cancel();
    assert_eq!(gesture(&s), "idle");
    let finish = s.solution().dates[task as usize].ef;
    s.pointer_down(s.timeline().viewport.x_of(finish as f64) - 1.0, y, false);
    assert_eq!(gesture(&s), "resize-pressed");
    s.pointer_move(x + TRAVEL_PX, y);
    assert_eq!(gesture(&s), "resize-dragging");
    s.pointer_cancel();
    s.edit(Edit::SetProgress {
        task: TaskId(task),
        actual_start: Some(es),
        actual_finish: Some(finish),
        remaining: None,
    })
    .expect("finishes the task");
    s.pointer_down(x, y, false);
    assert_eq!(gesture(&s), "finished-pressed");
}

#[test]
fn a_second_press_does_not_replace_a_bar_press() {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    s.pointer_down(x, y, false);
    s.pointer_down(x, y + 150.0, false); // on no bar
    s.pointer_move(x + TRAVEL_PX, y);
    s.pointer_up();
    assert_ne!(s.schedule().tasks()[task as usize].snet, before);
}

#[test]
fn a_second_press_mid_drag_leaves_one_drag_with_one_undo() {
    let (mut s, task, _, (x, y)) = setup();
    let before = s.schedule().tasks()[task as usize].snet;
    s.pointer_down(x, y, false);
    s.pointer_move(x + 100.0, y);
    s.apply_drag();
    s.pointer_down(x + 100.0, y, false); // where the dragged bar is now
    s.pointer_up();
    assert_eq!(s.history_len(), (1, 0), "the drag is one history step");
    assert!(s.undo().expect("undoes"));
    assert_eq!(s.schedule().tasks()[task as usize].snet, before);
}
