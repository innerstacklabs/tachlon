//! A host's add with `first` (tl#2087): the row is the first among its siblings, where no `after` is the last.

use super::*;

const N1: &str = r#"{"key":"n1"}"#;

#[test]
fn a_row_added_first_is_its_parents_first_child_and_the_others_keep_their_order() {
    let mut s = keyed();
    let n = s.add_task_by_key(N1, Some("t0"), None, true).unwrap();
    assert_eq!(s.outline().unwrap().children(P), [n, A, B]);
}

#[test]
fn at_the_top_level_a_row_added_first_is_the_first_root() {
    let mut s = keyed();
    let n = s.add_task_by_key(N1, None, None, true).unwrap();
    assert_eq!(s.outline().unwrap().roots(), [n, P, C, D, E]);
}

#[test]
fn first_with_an_after_is_refused_by_name_and_nothing_changes() {
    let mut s = keyed();
    let (count, history, revision) = (s.task_count(), s.history_len(), s.revision());
    let error = (s.add_task_by_key(N1, Some("t0"), Some("t1"), true)).unwrap_err();
    assert_eq!(
        (error.kind, error.member.as_deref()),
        ("badValue", Some("after"))
    );
    assert_eq!(
        (s.task_count(), s.history_len(), s.revision()),
        (count, history, revision)
    );
    assert_eq!(s.task_for_key("n1"), None);
    assert_eq!(s.outline().unwrap().children(P), [A, B]);
}

#[test]
fn a_first_add_leaves_the_undo_list_the_revision_and_the_selection_as_an_add_does() {
    let mut s = keyed();
    rename(&mut s, A, "Mine");
    assert!(s.select_task(E.0));
    let (history, revision) = (s.history_len(), s.revision());
    let n = s.add_task_by_key(N1, None, None, true).unwrap();
    assert_eq!(s.outline().unwrap().roots()[0], n);
    assert_eq!((s.history_len(), s.revision()), (history, revision));
    assert_eq!(s.selection().map(|selected| selected.task), Some(E.0));
    assert_eq!((s.undo(), name(&s, A)), (Ok(true), "A"));
}
