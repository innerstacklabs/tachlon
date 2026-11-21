use super::*;

#[test]
fn an_add_under_a_row_a_person_inserted_forgets_that_insert() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    assert!(s.select_task(C.0));
    let t = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(t), "x").unwrap();
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("x"), None);
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(t), [n]);
    assert_eq!(s.task_for_key("n1"), Some(n));
}

#[test]
fn an_add_as_a_last_child_forgets_an_outdents_release() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    rename(&mut s, A, "a1");
    assert!(s.select_task(A.0));
    assert_eq!(s.outdent_selected(), Ok(A));
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t1"), None);
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(name(&s, A), "a1");
    assert_eq!(s.outline().unwrap().parent(A), None);
    assert_eq!(s.outline().unwrap().children(A), [B, n]);
}

#[test]
fn an_add_after_an_undone_outdent_forgets_its_adopt_redo() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    assert!(s.select_task(A.0));
    assert_eq!(s.outdent_selected(), Ok(A));
    assert_eq!(s.undo(), Ok(true));
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t0"), Some("t1"));
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.redo(), Ok(false));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(P), [A, n, B]);
}

#[test]
fn an_add_under_a_leaf_forgets_its_progress_edit() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    s.edit(Edit::SetProgress {
        task: A,
        actual_start: Some(0),
        actual_finish: None,
        remaining: Some(30),
    })
    .unwrap();
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t1"), None);
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(A), [n]);
}

#[test]
fn an_add_under_a_link_end_forgets_its_link_steps() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    s.edit(Edit::AddLink {
        pred: A,
        succ: E,
        kind: LinkType::FS,
        lag: 0,
    })
    .unwrap();
    s.edit(Edit::RemoveLink { dep: DepId(3) }).unwrap();
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t1"), None);
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(A), [n]);
}

#[test]
fn an_add_under_a_row_whose_deleted_link_ends_would_return_forgets_the_restores() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    assert!(s.select_task(C.0));
    s.delete_selected().unwrap();
    assert!(s.select_task(D.0));
    s.delete_selected().unwrap();
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t2"), None);
    assert_eq!(s.history_len(), (1, 0));
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(B), [n]);
}

#[test]
fn an_add_under_a_row_forgets_the_delete_of_its_parent_too() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    assert!(s.select_task(E.0));
    let g = s.insert_below_selection().unwrap();
    assert!(s.select_task(g.0));
    let x = s.insert_below_selection().unwrap();
    assert!(s.select_task(x.0));
    s.indent_selected().unwrap();
    assert!(s.select_task(x.0));
    let p = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(p), "x").unwrap();
    add(&mut s, r#"{"key":"n1"}"#, Some("x"), None);
    assert_eq!(s.history_len().0, 3);
    for _ in 0..3 {
        assert_eq!(s.undo(), Ok(true));
    }
    assert_eq!(s.undo(), Ok(false));
    assert_eq!(name(&s, E), "E");
}

#[test]
fn a_plain_add_preserves_an_unrelated_rename_and_keep_reparent() {
    let mut s = keyed();
    rename(&mut s, E, "e1");
    s.edit(Edit::Reparent {
        task: C,
        place: Place {
            parent: Some(P),
            after: Some(B),
        },
        siblings: Siblings::Keep,
    })
    .unwrap();
    let history = s.history_len();
    let n = add(&mut s, r#"{"key":"n1"}"#, Some("t0"), None);
    assert_eq!(s.history_len(), history);
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(s.outline().unwrap().parent(C), None);
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, E), "E");
    assert_eq!(s.outline().unwrap().children(P), [A, B, n]);
}
