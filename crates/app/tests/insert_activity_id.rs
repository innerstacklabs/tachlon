//! The activity ID an inserted row takes (decision 0009, item 4; gs#703).

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{
    Place, Session,
    document::{Extras, read, write},
    edit::Edit,
    outline::{Outline, demo_layout},
};
use common::outline_session;
use engine::TaskId;
use timeline::Timeline;

const N: usize = 300;

/// The first nested leaf, the row the tests insert after.
fn leaf(session: &Session) -> TaskId {
    let outline = session.outline().unwrap();
    *outline
        .preorder()
        .iter()
        .find(|&&t| !outline.is_summary(t) && outline.parent(t).is_some())
        .expect("a nested leaf")
}

/// An outline session over the demo layout where each `(task, code)` pair has its activity ID.
fn coded_session(codes: &[(u32, &str)]) -> Session {
    let mut layout = demo_layout(N);
    for &(task, code) in codes {
        layout.meta[task as usize].code = Some(code.to_string());
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        common::clock,
    )
    .with_outline(outline)
    .expect("outline matches the demo schedule")
}

fn code_of(session: &Session, task: TaskId) -> Option<&str> {
    session.meta()[task.0 as usize].code.as_deref()
}

/// A session where the first row in preorder has `B-40`, the first nested leaf `A1-2` and the last
/// row `A1-9`, and that leaf. The nearest coded row above an insert after the leaf is the leaf, so
/// only a lookup that follows the neighbour gives the `A1-` prefix.
fn coded_leaf_session() -> (Session, TaskId) {
    let plain = outline_session(N).0;
    let leaf = leaf(&plain);
    let preorder = plain.outline().unwrap().preorder();
    let (first, last) = (preorder[0], *preorder.last().unwrap());
    let codes = [(first.0, "B-40"), (leaf.0, "A1-2"), (last.0, "A1-9")];
    (coded_session(&codes), leaf)
}

#[test]
fn an_inserted_row_takes_the_next_activity_id_after_its_neighbour() {
    let (mut session, leaf) = coded_leaf_session();
    session.select(leaf.0);
    let new = session.insert_below_selection().unwrap();
    // The neighbour's prefix `A1-`, not the first row's `B-`, one above the largest `A1-` number.
    assert_eq!(code_of(&session, new), Some("A1-10"));
}

#[test]
fn a_second_insert_does_not_repeat_it() {
    let (mut session, leaf) = coded_leaf_session();
    session.select(leaf.0);
    let first = session.insert_below_selection().unwrap();
    session.select(leaf.0);
    let second = session.insert_below_selection().unwrap();
    assert_eq!(code_of(&session, first), Some("A1-10"));
    assert_eq!(code_of(&session, second), Some("A1-11"));
}

#[test]
fn undo_and_redo_keep_the_id() {
    let (mut session, leaf) = coded_leaf_session();
    session.select(leaf.0);
    let new = session.insert_below_selection().unwrap();
    assert!(session.undo().unwrap());
    assert_eq!(code_of(&session, new), Some("A1-10"));
    assert!(session.redo().unwrap());
    assert_eq!(code_of(&session, new), Some("A1-10"));
}

#[test]
fn the_id_survives_a_save_and_reopen() {
    let (mut session, leaf) = coded_leaf_session();
    session.select(leaf.0);
    let new = session.insert_below_selection().unwrap();
    let extras = Extras {
        name: "ids",
        notes: &[],
        source: None,
    };
    let text = write(&session, &extras);
    let loaded = read(&text, &common::clock).expect("reads").loaded;
    // Ids are renumbered when a document is written; the new row is found by its place.
    let at = session
        .outline()
        .unwrap()
        .preorder()
        .iter()
        .position(|&t| t == new)
        .unwrap();
    let reopened = loaded.outline.preorder()[at];
    assert_eq!(loaded.meta[reopened.0 as usize].name, "New task");
    assert_eq!(
        loaded.meta[reopened.0 as usize].code.as_deref(),
        Some("A1-10")
    );
}

#[test]
fn with_no_coded_row_the_first_id_is_a1000() {
    let (mut session, _) = outline_session(N);
    session.select(leaf(&session).0);
    let new = session.insert_below_selection().unwrap();
    assert_eq!(code_of(&session, new), Some("A1000"));
}

#[test]
fn a_row_above_every_coded_row_follows_the_first_one_below_it() {
    let plain = outline_session(N).0;
    let last = *plain.outline().unwrap().preorder().last().unwrap();
    let mut session = coded_session(&[(last.0, "C-5")]);
    session.clear_selection();
    let place = Place {
        parent: None,
        after: None,
    };
    let name = "New task".to_string();
    session.edit(Edit::Insert { place, name }).unwrap();
    let new = TaskId(session.schedule().tasks().len() as u32 - 1);
    assert_eq!(session.outline().unwrap().preorder()[0], new);
    assert_eq!(code_of(&session, new), Some("C-6"));
}
