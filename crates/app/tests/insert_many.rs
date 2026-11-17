//! Insert many rows from a list of names as one undo step (gs#703): the engine half of paste.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::outline_session;
use app::{Session, edit::EditError};
use engine::TaskId;

const N: usize = 300;

fn leaf(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| !outline.is_summary(t) && outline.parent(t).is_some())
        .expect("a nested leaf")
}

#[test]
fn three_names_make_three_rows_and_one_undo_step() {
    let (mut session, _) = outline_session(N);
    let at = leaf(&session);
    session.select(at.0);
    let before = session.outline().unwrap().preorder().to_vec();
    let dates = session.solution().dates.clone();
    let steps = session.history_len().0;

    let new = session.insert_many(&["Footing", " Slab ", "Wall"]).unwrap();

    assert_eq!(new.len(), 3);
    assert_eq!(session.task_count(), N + 3);
    let names: Vec<&str> = new
        .iter()
        .map(|t| session.meta()[t.0 as usize].name.as_str())
        .collect();
    assert_eq!(names, ["Footing", "Slab", "Wall"]);
    // The rows follow the selected leaf in the order given, as siblings.
    let preorder = session.outline().unwrap().preorder().to_vec();
    let from = preorder.iter().position(|&t| t == at).unwrap();
    assert_eq!(preorder[from + 1..from + 4], new[..]);
    for task in &new {
        let meta = &session.meta()[task.0 as usize];
        assert_eq!(meta.parent, session.meta()[at.0 as usize].parent);
        assert_eq!(
            meta.outline_level,
            session.meta()[at.0 as usize].outline_level
        );
    }
    assert_eq!(session.selection().map(|s| s.task), Some(new[2].0));
    assert_eq!(session.history_len().0, steps + 1, "one undo step");

    assert!(session.undo().unwrap());
    assert_eq!(session.history_len().0, steps);
    assert_eq!(session.task_count(), N);
    assert_eq!(session.outline().unwrap().preorder(), &before[..]);
    // The three tombstoned slots stay at the end; every live task reads as before.
    assert_eq!(session.solution().dates[..dates.len()], dates[..]);

    assert!(session.redo().unwrap());
    assert_eq!(session.task_count(), N + 3);
    assert_eq!(session.outline().unwrap().preorder(), &preorder[..]);
    assert_eq!(session.history_len().0, steps + 1);
}

#[test]
fn a_filter_or_a_bad_name_refuses_the_whole_list_and_changes_nothing() {
    let (mut session, _) = outline_session(N);
    let at = leaf(&session);
    session.select(at.0);
    let before = session.outline().unwrap().preorder().to_vec();
    let history = session.history_len();

    assert_eq!(
        session.insert_many(&["A", "B / C"]),
        Err(EditError::InvalidName)
    );
    assert_eq!(session.insert_many(&["A", " "]), Err(EditError::EmptyName));

    assert!(session.set_critical_only(true));
    assert_eq!(
        session.insert_many(&["A", "B"]),
        Err(EditError::FilterActive)
    );
    assert!(session.set_critical_only(false));

    assert_eq!(session.task_count(), N);
    assert_eq!(session.outline().unwrap().preorder(), &before[..]);
    assert_eq!(session.history_len(), history);
}
