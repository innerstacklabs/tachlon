//! Engine checks that `packages/gantt/tests/links.spec.ts` made through the page (gs#1163, PR 3b): the
//! details of both ends after a link drag, a loop named mid-drag, undo and redo of a selected
//! link's remove, and a plain bar drag. Helpers come from the parent module.

use super::*;

#[test]
fn an_alt_drag_shows_the_link_in_the_details_of_both_ends() {
    let mut s = session();
    let revision = s.revision();
    let to = on(&s, C);
    link(&mut s, A, to);

    let c = s.task_details(C).expect("C has details");
    assert_eq!(c.predecessor_count, 1, "C gained A");
    assert_eq!(c.predecessors, ["A (FS)"]);
    let a = s.task_details(A).expect("A has details");
    assert_eq!(a.successor_count, 2, "A has B and C after it");
    assert_eq!(s.revision(), revision + 1, "one revision");
    let dates = &s.solution().dates;
    assert_eq!(
        dates[C as usize].es, dates[A as usize].ef,
        "C starts at A's finish"
    );
    assert_eq!(
        dates[C as usize].es, dates[B as usize].es,
        "with A's other successor"
    );
    assert_eq!(s.link_refusal(), None);
}

#[test]
fn a_loop_is_named_mid_drag_and_one_undo_after_its_refusal_takes_out_the_earlier_add() {
    let mut s = session();
    let original = s.solution().dates.clone();
    let to = on(&s, C);
    link(&mut s, A, to);
    let linked = state(&s);

    // C back to A would close A -> C -> A.
    let loop_text = "\"C\" to \"A\" would make a loop: A → C → A.";
    let from = on(&s, C);
    let to = on(&s, A);
    drag(&mut s, from, &[to]);
    let dragging = &status(&s)["link"]["drag"];
    assert_eq!(
        (dragging["pred"].as_u64(), dragging["target"].as_u64()),
        (Some(C.into()), Some(A.into()))
    );
    assert_eq!(dragging["refused"], loop_text, "known before the release");
    s.pointer_up();
    assert_eq!(s.link_refusal(), Some(loop_text));
    assert_eq!(state(&s), linked, "the refusal changed nothing");
    assert_eq!(s.task_details(A).expect("A").predecessor_count, 0);

    assert!(s.undo().expect("undoes"));
    assert_eq!(s.task_details(C).expect("C").predecessor_count, 0);
    assert_eq!(
        s.solution().dates,
        original,
        "the one undo took out the add"
    );
}

#[test]
fn redo_of_a_selected_links_remove_takes_it_out_again_and_clears_the_selection() {
    let mut s = session();
    let linked = s.solution().dates.clone();
    assert!(s.select_link(DepId(0)));
    assert_eq!(s.remove_selected_link().expect("removes"), DepId(0));
    let removed = s.solution().dates.clone();
    assert_ne!(removed, linked, "B no longer waits for A");
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());

    assert!(s.undo().expect("undoes"));
    assert_eq!(s.selected_link(), Some(DepId(0)), "the same id, selected");
    assert_eq!(s.solution().dates, linked);
    assert!(s.redo().expect("redoes"));
    assert_eq!(s.selected_link(), None, "redo removes it again");
    assert!(!s.schedule().is_dependency_live(DepId(0)));
    assert_eq!(s.solution().dates, removed);
}

#[test]
fn a_drag_without_alt_moves_the_bar_and_adds_no_link() {
    let mut s = session();
    // Ten minutes a pixel, so 300 px is past the next day start a bar drag snaps to.
    zoom_to(&mut s, 10.0, 0.0);
    let revision = s.revision();
    let start = s.solution().dates[A as usize].es;
    let (x, y) = on(&s, A);
    let x = x - 4.0; // on A's body: its bar is 6 px wide and its last 2 px are the edge
    s.pointer_down(x, y, false);
    s.pointer_move(x + 300.0, y);
    assert!(!s.linking(), "no link drag without Alt");
    assert!(status(&s)["link"]["drag"].is_null());
    assert!(s.drag_started(), "a bar drag");
    s.pointer_up();

    assert!(
        s.solution().dates[A as usize].es > start,
        "the bar moved right"
    );
    assert!(s.revision() > revision, "the move was recorded");
    assert_eq!(live(&s), 2, "no link was added");
    assert_eq!(s.task_details(A).expect("A").successor_count, 1);
}
