//! A host moves a task by key (tl#2087): the row and its subtree change place, no undo step, commit or revision,
//! the selection and the scroll stay, and the person's steps that name the moved rows are forgotten.

use super::*;

fn roots(s: &Session) -> Vec<TaskId> {
    s.outline().unwrap().roots().to_vec()
}

#[test]
fn a_move_takes_the_subtree_along_and_leaves_history_revision_commits_selection_and_scroll() {
    let mut s = keyed();
    rename(&mut s, E, "Mine");
    s.take_commits();
    s.set_scroll_slack(2000.0);
    s.scroll_by(60.0);
    assert!(s.select_task(E.0));
    let (history, revision) = (s.history_len(), s.revision());
    let top = |s: &Session| {
        let view = &s.timeline().viewport;
        let row = view.visible_rows().start;
        (
            s.timeline().task_at_row(row),
            view.scroll_y % view.row_height,
        )
    };
    let view = top(&s);
    assert!(view.1 > 0.0 && view.0.is_some(), "the fixture scrolls");
    s.move_task_by_key("t0", None, Some("t4"), false).unwrap();
    assert_eq!(roots(&s), [C, D, P, E]);
    assert_eq!(s.outline().unwrap().children(P), [A, B]);
    s.move_task_by_key("t3", Some("t0"), None, true).unwrap();
    assert_eq!(s.outline().unwrap().children(P), [C, A, B]);
    assert_eq!(s.meta()[C.0 as usize].outline_level, 2);
    assert_eq!((s.history_len(), s.revision()), (history, revision));
    assert_eq!(s.selection().map(|selected| selected.task), Some(E.0));
    assert_eq!(top(&s), view, "the same row is at the top");
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 0, "no commit");
}

#[test]
fn a_move_forgets_the_steps_that_name_the_moved_row_and_reaches_the_older_ones() {
    let mut s = keyed();
    rename(&mut s, E, "Older");
    rename(&mut s, C, "Gone");
    s.move_task_by_key("t3", Some("t0"), None, false).unwrap();
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, E), s.history_len()), ("E", (0, 1)));
}

#[test]
fn a_move_forgets_the_steps_of_a_row_in_the_moved_subtree() {
    let mut s = keyed();
    rename(&mut s, E, "Older");
    rename(&mut s, B, "Gone");
    s.move_task_by_key("t0", None, Some("t5"), false).unwrap();
    assert_eq!(s.undo(), Ok(true));
    assert_eq!((name(&s, E), s.history_len()), ("E", (0, 1)));
}

#[test]
fn a_move_forgets_a_counted_reparent_and_keeps_a_plain_one() {
    let mut s = keyed();
    assert!(s.select_task(A.0));
    assert_eq!(s.outdent_selected(), Ok(A));
    let under = Place {
        parent: Some(P),
        after: None,
    };
    let siblings = Siblings::Keep;
    let task = E;
    s.edit(Edit::Reparent {
        task,
        place: under,
        siblings,
    })
    .unwrap();
    s.move_task_by_key("t3", None, Some("t0"), false).unwrap();
    assert_eq!(s.history_len(), (1, 0), "only the plain reparent is kept");
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.outline().unwrap().parent(E), None);
}

#[test]
fn a_move_under_a_leaf_forgets_its_progress_edit() {
    let mut s = keyed();
    rename(&mut s, D, "Older");
    s.edit(Edit::SetProgress {
        task: A,
        actual_start: Some(0),
        actual_finish: None,
        remaining: Some(30),
    })
    .unwrap();
    s.move_task_by_key("t5", Some("t1"), None, false).unwrap();
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, D), "D");
}

#[test]
fn a_refused_move_changes_nothing() {
    let mut s = keyed();
    rename(&mut s, A, "Mine");
    let (history, order) = (s.history_len(), s.outline().unwrap().preorder().to_vec());
    let mut kind = |key, parent, after, first| {
        let error = s.move_task_by_key(key, parent, after, first).unwrap_err();
        (error.kind, error.member)
    };
    assert_eq!(kind("nobody", None, None, false), ("unknownKey", None));
    let (gone, named) = (("unknownKey", Some("parent".to_string())), Some("nobody"));
    assert_eq!(kind("t3", named, None, false), gone);
    assert_eq!(
        kind("t3", None, Some("t4"), true),
        ("badValue", Some("after".to_string()))
    );
    assert_eq!(kind("t0", Some("t1"), None, false).0, "badPlace");
    assert_eq!(kind("t3", Some("t4"), None, false).0, "outline");
    assert_eq!(
        (s.history_len(), s.outline().unwrap().preorder().to_vec()),
        (history, order)
    );
    let (x, y) = on(&s, C.0);
    s.pointer_down(x, y, false);
    let busy = s.move_task_by_key("t3", None, None, true).unwrap_err();
    assert_eq!(busy.kind, "busy");
}

#[test]
fn a_move_while_a_filter_or_grouping_is_on_is_taken_and_a_typed_reparent_is_still_refused() {
    let mut s = keyed();
    assert!(s.select_task(E.0));
    assert!(s.set_critical_only(true));
    s.move_task_by_key("t5", None, None, true).unwrap();
    assert_eq!(s.indent_selected(), Err(EditError::ReparentFiltered));
    assert!(s.set_critical_only(false) && s.set_group_by(Some("Zone")));
    s.move_task_by_key("t4", None, None, true).unwrap();
    assert_eq!(s.move_selected_up(), Err(EditError::ReparentGrouped));
    assert_eq!(roots(&s), [D, E, P, C]);
}

#[test]
fn a_move_to_last_under_a_parent_leaves_a_row_already_last_there_and_an_only_child_where_it_is() {
    let mut s = keyed();
    s.move_task_by_key("t2", Some("t0"), None, false).unwrap();
    s.move_task_by_key("t5", None, None, false).unwrap();
    assert_eq!(s.outline().unwrap().children(P), [A, B]);
    assert_eq!(roots(&s), [P, C, D, E]);
    s.move_task_by_key("t2", None, Some("t3"), false).unwrap();
    s.move_task_by_key("t1", Some("t0"), None, false).unwrap();
    assert_eq!(s.outline().unwrap().children(P), [A]);
}
