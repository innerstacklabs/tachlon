use super::*;

fn keyed_links() -> Session {
    let mut s = keyed();
    s.assign_key(KeyTarget::Link(DepId(0)), "l0").unwrap();
    s
}

/// A person's rename of A, then an insert keyed `x` with a link from E to it; the link.
fn person_links_x(s: &mut Session) -> DepId {
    rename(s, A, "Mine");
    assert!(s.select_task(C.0));
    let x = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(x), "x").unwrap();
    #[rustfmt::skip]
    s.edit(Edit::AddLink { pred: E, succ: x, kind: LinkType::FS, lag: 0 }).unwrap();
    s.selected_link().unwrap()
}

/// Undo and then Redo until each answers `false`: a press that sticks never gets there.
fn drain(s: &mut Session) {
    for redo in [false, true] {
        let press = |s: &mut Session| if redo { s.redo() } else { s.undo() };
        assert!((0..10).any(|_| !press(s).unwrap()), "a press sticks");
        assert_eq!(name(s, A), if redo { "Mine" } else { "A" });
    }
}

#[test]
fn host_links_keep_history_revision_commits_and_selections() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    s.take_commits();
    assert!(s.select_task(E.0) && s.select_link(DepId(1)));
    let same = |s: &Session| {
        let task = s.selection().map(|v| v.task);
        (s.history_len(), s.revision(), s.selected_link(), task)
    };
    let before = same(&s);
    let json = r#"{"key":"l9","pred":"t3","succ":"t5","kind":"SS","lag":60}"#;
    let dep = s.add_link_by_key(json).unwrap();
    let link = s.schedule().dependencies()[dep.0 as usize];
    assert_eq!(
        (link.pred, link.succ, link.kind, link.lag),
        (C, E, LinkType::SS, 60)
    );
    assert_eq!((s.host_keys().link("l9"), same(&s)), (Some(dep), before));
    s.set_link_by_key(r#"{"key":"l9","kind":"FF"}"#).unwrap();
    s.set_link_by_key(r#"{"pred":"t3","succ":"t5","lag":0}"#)
        .unwrap();
    let link = s.schedule().dependencies()[dep.0 as usize];
    assert_eq!((link.kind, link.lag, same(&s)), (LinkType::FF, 0, before));
    s.remove_link_by_key(r#"{"pred":"t3","succ":"t5"}"#)
        .unwrap();
    assert_eq!(s.host_keys().link("l9"), None);
    assert_eq!(
        (s.schedule().is_dependency_live(dep), same(&s)),
        (false, before)
    );
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 0);
    s.remove_link_by_key(r#"{"pred":"t3","succ":"t4"}"#)
        .unwrap();
    assert_eq!(s.selected_link(), None, "the selected link is gone");
}

#[test]
fn refusals_name_the_member_and_change_nothing() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    let dep = DepId(1);
    s.edit(Edit::RemoveLink { dep }).unwrap();
    s.assign_key(KeyTarget::Link(dep), "l1").unwrap();
    let held = |s: &Session| (s.history_len(), s.schedule().dependencies().to_vec());
    let before = held(&s);
    #[rustfmt::skip]
    let cases = [
        ["add", r#"{"pred":"zz","succ":"t3"}"#, "unknownKey", "pred"],
        ["add", r#"{"pred":"t3","succ":"zz"}"#, "unknownKey", "succ"],
        ["add", r#"{"succ":"t3"}"#, "noKey", "pred"],
        ["add", r#"{"key":"l0","pred":"t3","succ":"t5"}"#, "duplicateKey", "key"],
        ["add", r#"{"pred":"t2","succ":"t3"}"#, "alreadyLinked", ""],
        ["add", r#"{"key":42,"pred":"t3","succ":"t5"}"#, "badValue", "key"],
        ["add", r#"{"key":"","pred":"t3","succ":"t5"}"#, "badValue", "key"],
        ["set", r#"{"key":42,"lag":1}"#, "badValue", "key"],
        ["set", r#"{"key":"zz","lag":1}"#, "unknownKey", "key"],
        ["set", r#"{"key":"l1","lag":1}"#, "unknownLink", ""],
        ["remove", r#"{"key":""}"#, "badValue", "key"],
        ["set", r#"{"pred":"t3","succ":"t5","lag":1}"#, "unknownLink", ""],
        ["remove", r#"{"pred":"t5","succ":"t3"}"#, "unknownLink", ""],
        ["remove", "[1]", "badJson", ""],
    ];
    for [call, json, kind, member] in cases {
        let result = match call {
            "add" => s.add_link_by_key(json).map(|_| ()),
            "set" => s.set_link_by_key(json),
            _ => s.remove_link_by_key(json),
        };
        let want = (kind, (!member.is_empty()).then(|| member.to_owned()));
        assert_eq!(result.map_err(|e| (e.kind, e.member)), Err(want), "{json}");
        assert_eq!(held(&s), before, "{json}");
    }
}

#[test]
fn undo_and_redo_after_a_host_link_add_never_stick() {
    let mut s = keyed();
    person_links_x(&mut s);
    assert_eq!(s.undo(), Ok(true));
    let dep = s
        .add_link_by_key(r#"{"key":"l9","pred":"t5","succ":"x"}"#)
        .unwrap();
    assert!(s.schedule().is_dependency_live(dep));
    drain(&mut s);
    assert!(s.schedule().is_dependency_live(dep));
}

#[test]
fn a_host_add_forgets_the_undo_of_a_persons_remove_of_that_pair() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    let dep = DepId(0);
    s.edit(Edit::RemoveLink { dep }).unwrap();
    s.add_link_by_key(r#"{"key":"n","pred":"t2","succ":"t3"}"#)
        .unwrap();
    assert_eq!(s.undo(), Ok(true));
    let live = s.schedule().live_dependencies();
    let pair = live.filter(|(_, l)| (l.pred, l.succ) == (B, C));
    assert_eq!((pair.count(), name(&s, A)), (1, "A"));
}

#[test]
fn a_host_add_forgets_every_step_on_the_persons_removed_link() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    let dep = DepId(0);
    s.set_link_text(dep, "SS").unwrap();
    s.edit(Edit::RemoveLink { dep }).unwrap();
    s.add_link_by_key(r#"{"key":"n","pred":"t2","succ":"t3"}"#)
        .unwrap();
    assert_eq!(s.history_len(), (1, 0));
    drain(&mut s);
}

#[test]
fn undo_and_redo_after_a_host_link_set_never_stick() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    s.set_link_text(DepId(0), "SS").unwrap();
    s.set_link_by_key(r#"{"key":"l0","kind":"FF"}"#).unwrap();
    assert_eq!(s.schedule().dependencies()[0].kind, LinkType::FF);
    drain(&mut s);
}

#[test]
fn undo_and_redo_after_a_host_link_remove_never_stick() {
    let mut s = keyed();
    let dep = person_links_x(&mut s);
    s.remove_link_by_key(r#"{"pred":"t5","succ":"x"}"#).unwrap();
    assert!(!s.schedule().is_dependency_live(dep));
    drain(&mut s);
}
