use super::*;

/// A refusal's kind and member.
fn named(error: app::host::UpdateError) -> (&'static str, Option<String>) {
    (error.kind, error.member)
}

fn keyed_links() -> Session {
    let mut s = keyed();
    s.assign_key(KeyTarget::Link(DepId(0)), "l0").unwrap();
    s
}

#[test]
fn a_host_link_add_set_and_remove_leave_the_undo_list_the_outbox_and_the_selections() {
    let mut s = keyed_links();
    rename(&mut s, A, "Mine");
    s.take_commits();
    assert!(s.select_task(E.0) && s.select_link(DepId(1)));
    let same = |s: &Session| {
        let task = s.selection().map(|selected| selected.task);
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
    assert_eq!(
        (link.kind, link.lag),
        (LinkType::FF, 0),
        "by key and by its ends"
    );
    assert_eq!(same(&s), before);
    s.remove_link_by_key(r#"{"pred":"t3","succ":"t5"}"#)
        .unwrap();
    assert_eq!(s.host_keys().link("l9"), None, "the key is free");
    assert_eq!(
        (s.schedule().is_dependency_live(dep), same(&s)),
        (false, before)
    );
    let out: Value = serde_json::from_str(&s.take_commits()).unwrap();
    assert_eq!(out["commits"].as_array().unwrap().len(), 0, "no commit");
    s.remove_link_by_key(r#"{"key":"l0"}"#).unwrap();
    assert_eq!(s.selected_link(), before.2, "another link stays selected");
    s.remove_link_by_key(r#"{"pred":"t3","succ":"t4"}"#)
        .unwrap();
    assert_eq!(s.selected_link(), None, "the selected link is gone");
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(name(&s, A), "A");
}

#[test]
fn a_bad_record_an_unknown_key_or_a_refused_link_is_named_and_changes_nothing() {
    let mut s = keyed_links();
    rename(&mut s, A, "a1");
    let held = |s: &Session| {
        let live = s.schedule().live_dependencies().count();
        (s.history_len(), s.revision(), live)
    };
    let before = held(&s);
    let cases = [
        (
            "add",
            r#"{"pred":"nobody","succ":"t3"}"#,
            "unknownKey",
            Some("pred"),
        ),
        (
            "add",
            r#"{"pred":"t3","succ":"nobody"}"#,
            "unknownKey",
            Some("succ"),
        ),
        ("add", r#"{"succ":"t3"}"#, "noKey", Some("pred")),
        (
            "add",
            r#"{"pred":"t3","succ":"t5","type":"SS"}"#,
            "notUpdatable",
            Some("type"),
        ),
        (
            "add",
            r#"{"pred":"t3","succ":"t5","kind":"XX"}"#,
            "badValue",
            Some("kind"),
        ),
        (
            "add",
            r#"{"key":"l0","pred":"t3","succ":"t5"}"#,
            "duplicateKey",
            Some("key"),
        ),
        ("add", r#"{"pred":"t2","succ":"t3"}"#, "alreadyLinked", None),
        ("add", r#"{"pred":"t3","succ":"t2"}"#, "linkCycle", None),
        ("add", r#"{"pred":"t0","succ":"t3"}"#, "linkToSummary", None),
        (
            "set",
            r#"{"key":"nolink","lag":1}"#,
            "unknownKey",
            Some("key"),
        ),
        (
            "set",
            r#"{"pred":"t3","succ":"t5","lag":1}"#,
            "unknownLink",
            None,
        ),
        (
            "remove",
            r#"{"pred":"t5","succ":"t3"}"#,
            "unknownLink",
            None,
        ),
        ("remove", "[1]", "badJson", None),
    ];
    for (call, json, kind, member) in cases {
        let error = match call {
            "add" => s.add_link_by_key(json).map(|_| ()),
            "set" => s.set_link_by_key(json),
            _ => s.remove_link_by_key(json),
        };
        let want = (kind, member.map(str::to_owned));
        assert_eq!(error.map_err(named), Err(want), "{call} {json}");
        assert_eq!(held(&s), before, "{call} {json} changes nothing");
    }
}

#[test]
fn an_add_whose_key_a_person_removed_brings_that_link_back_and_forgets_their_remove_step() {
    let mut s = keyed_links();
    rename(&mut s, A, "a1");
    s.edit(Edit::RemoveLink { dep: DepId(0) }).unwrap();
    let dep = s.add_link_by_key(r#"{"key":"l0","pred":"t2","succ":"t3","lag":30}"#);
    assert_eq!(dep, Ok(DepId(0)));
    assert_eq!(s.schedule().dependencies()[0].lag, 30);
    assert_eq!(
        s.history_len(),
        (1, 0),
        "their remove is forgotten, the rename kept"
    );
    assert_eq!(s.undo(), Ok(true));
    assert_eq!(
        (name(&s, A), s.schedule().is_dependency_live(DepId(0))),
        ("A", true)
    );
}

#[test]
fn a_host_link_added_to_or_removed_from_a_row_a_person_inserted_forgets_the_steps_that_name_the_pair()
 {
    let mut s = keyed_links();
    rename(&mut s, E, "e1");
    assert!(s.select_task(C.0));
    let x = s.insert_below_selection().unwrap();
    s.assign_key(KeyTarget::Task(x), "x").unwrap();
    let (kind, lag) = (LinkType::FS, 0);
    s.edit(Edit::AddLink {
        pred: E,
        succ: x,
        kind,
        lag,
    })
    .unwrap();
    s.remove_link_by_key(r#"{"pred":"t5","succ":"x"}"#).unwrap();
    assert_eq!(
        s.history_len(),
        (2, 0),
        "their add of the dropped link is forgotten"
    );
    s.edit(Edit::AddLink {
        pred: E,
        succ: x,
        kind,
        lag,
    })
    .unwrap();
    assert_eq!(s.undo(), Ok(true));
    s.add_link_by_key(r#"{"key":"l9","pred":"t5","succ":"x"}"#)
        .unwrap();
    assert_eq!(
        s.history_len(),
        (2, 0),
        "their undone add of the pair is forgotten"
    );
    assert_eq!(s.undo(), Ok(true));
    assert!(s.schedule().is_deleted(x), "one Undo takes back the insert");
}
