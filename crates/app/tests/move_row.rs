//! Move a row among its siblings (gs#703): `Session::move_selected_up` and `move_selected_down`,
//! on the reparent tests' outline, by task id: 0 holds 1, 2 (with 3), 4 and 5 (with 6); 7 is a
//! second root.

use crate::reparent::{TREE, build, shape, state};
use app::edit::EditError;
use engine::TaskId;

#[test]
fn a_row_moves_up_with_its_subtree_and_links_and_undo_puts_it_back() {
    // 3 -> 4: the moved row 2 holds 3, and the link crosses the move.
    let mut session = build(&TREE, &[(3, 4)]);
    let before = shape(&session);
    let dates = session.solution().dates.clone();
    let links = session.schedule().dependencies().to_vec();
    session.select(2);
    assert_eq!(session.move_selected_up(), Ok(TaskId(2)));
    let outline = session.outline().unwrap();
    assert_eq!(outline.preorder(), [0, 2, 3, 1, 4, 5, 6, 7].map(TaskId));
    assert_eq!(outline.children(TaskId(0)), [2, 1, 4, 5].map(TaskId));
    assert_eq!(session.meta(), before.1, "no parent or level changed");
    assert_eq!(session.solution().dates, dates);
    assert_eq!(session.schedule().dependencies(), links);
    assert_eq!(session.selection().map(|s| s.task), Some(2));
    assert_eq!(session.undo(), Ok(true));
    assert_eq!(shape(&session), before, "undo restores the order");
}

#[test]
fn a_row_moves_down_past_the_next_sibling_and_a_top_row_moves_too() {
    let mut session = build(&TREE, &[]);
    let before = shape(&session);
    session.select(2);
    assert_eq!(session.move_selected_down(), Ok(TaskId(2)));
    let outline = session.outline().unwrap();
    assert_eq!(outline.preorder(), [0, 1, 4, 2, 3, 5, 6, 7].map(TaskId));
    assert_eq!(session.move_selected_up(), Ok(TaskId(2)));
    assert_eq!(shape(&session), before, "up is the exact inverse of down");
    session.select(7);
    assert_eq!(session.move_selected_up(), Ok(TaskId(7)));
    assert_eq!(session.outline().unwrap().roots(), [7, 0].map(TaskId));
}

#[test]
fn a_move_at_the_end_or_with_nothing_selected_is_refused_and_changes_nothing() {
    let mut session = build(&TREE, &[]);
    for (task, up, want) in [
        (Some(1), true, EditError::NoPreviousSibling),
        (Some(0), true, EditError::NoPreviousSibling),
        (Some(5), false, EditError::NoNextSibling),
        (Some(7), false, EditError::NoNextSibling),
        (Some(3), false, EditError::NoNextSibling),
        (None, true, EditError::NoSelection),
    ] {
        match task {
            Some(task) => session.select(task),
            None => session.clear_selection(),
        }
        let before = state(&session);
        let moved = if up {
            session.move_selected_up()
        } else {
            session.move_selected_down()
        };
        assert_eq!(moved, Err(want));
        assert_eq!(state(&session), before);
    }
}
