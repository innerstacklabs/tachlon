//! A host sends new durations and start dates for many tasks by key: one recompute, the selection and the
//! person's undo list kept, every refusal named and nothing written.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common::{outline_session, setup};
use crate::proposal::ghosts;
use crate::what_moves;
use app::{
    Session,
    document::{Extras, write},
    edit::Edit,
    host::KeyTarget,
};
use engine::{LinkType, TaskId};

/// A session over the demo outline with its first 30 leaf tasks keyed `k0` to `k29`.
fn keyed() -> (Session, Vec<TaskId>) {
    let (mut session, _) = outline_session(80);
    let outline = session.outline().unwrap();
    let all = outline.preorder().iter().copied();
    let summary = outline
        .preorder()
        .iter()
        .copied()
        .find(|&t| outline.is_summary(t));
    let leaves: Vec<TaskId> = all.filter(|&t| !outline.is_summary(t)).take(30).collect();
    for (n, &task) in leaves.iter().enumerate() {
        session
            .assign_key(KeyTarget::Task(task), &format!("k{n}"))
            .unwrap();
    }
    session
        .assign_key(KeyTarget::Task(summary.unwrap()), "sum")
        .unwrap();
    session
        .edit(Edit::AddLink {
            pred: leaves[20],
            succ: leaves[21],
            kind: LinkType::FS,
            lag: 0,
        })
        .unwrap();
    session
        .edit(Edit::Rename {
            task: leaves[25],
            name: "Mine".into(),
        })
        .unwrap();
    (session, leaves)
}

fn saved(session: &Session) -> String {
    write(
        session,
        &Extras {
            name: "u",
            notes: &[],
            source: None,
        },
    )
}

/// Records for `k0` to `k{count - 1}`: `120 + n` minutes and a start.
fn records(count: usize) -> String {
    let each = (0..count).map(|n| {
        let day = 1 + n % 9;
        format!(
            r#"{{"key":"k{n}","duration":{},"start":"2026-03-0{day}"}}"#,
            120 + n
        )
    });
    format!("[{}]", each.collect::<Vec<_>>().join(","))
}

#[test]
fn twenty_tasks_take_new_values_in_one_call_and_the_person_keeps_their_place() {
    let (mut session, leaves) = keyed();
    session.select(leaves[3].0);
    session.scroll_by(60.0);
    let (depth, revision, stamp) = (session.history_len(), session.revision(), session.stamp());
    let view = session.timeline().viewport.clone();
    let done = session.update_tasks(&records(20)).unwrap();
    assert_eq!(done.tasks, 20);
    let origin = session.origin();
    for (n, &task) in leaves[..20].iter().enumerate() {
        let got = &session.schedule().tasks()[task.0 as usize];
        assert_eq!(got.duration, 120 + n as i64);
        assert_eq!(
            got.snet,
            origin.parse_day(&format!("2026-03-0{}", 1 + n % 9))
        );
    }
    let fresh = session.schedule().clone().compute().unwrap();
    for &task in &leaves {
        assert_eq!(
            fresh.dates[task.0 as usize],
            session.solution().dates[task.0 as usize]
        );
    }
    assert_eq!(
        (session.history_len(), session.revision()),
        (depth, revision)
    );
    assert!(session.stamp() > stamp);
    assert_eq!(session.selection().map(|s| s.task), Some(leaves[3].0));
    assert_eq!(session.timeline().viewport, view);
}

#[test]
fn an_undo_after_an_update_undoes_the_earlier_rename_and_leaves_the_values() {
    let (mut session, leaves) = keyed();
    session.update_tasks(&records(20)).unwrap();
    assert!(session.undo().unwrap());
    assert_ne!(session.meta()[leaves[25].0 as usize].name, "Mine");
    assert_eq!(
        session.schedule().tasks()[leaves[4].0 as usize].duration,
        124
    );
}

#[test]
fn each_refusal_is_named_and_leaves_the_document_byte_equal() {
    let (mut session, leaves) = keyed();
    session.select(leaves[29].0);
    session.delete_selected().unwrap();
    let cases = [
        (r#"{"key":"nope"}"#, "unknownKey", None),
        (r#"{"key":"k29"}"#, "unknownKey", None),
        (r#"{"key":"k1"}"#, "duplicateKey", None),
        (r#"{"key":"sum","duration":1}"#, "notUpdatable", Some("key")),
        (r#"{"duration":1}"#, "noKey", None),
        (
            r#"{"key":"k2","parent":"k3"}"#,
            "notUpdatable",
            Some("parent"),
        ),
        (
            r#"{"key":"k2","mso":"2026-03-09"}"#,
            "notUpdatable",
            Some("mso"),
        ),
        (
            r#"{"key":"k2","start":"2026-02-30"}"#,
            "badValue",
            Some("start"),
        ),
        (
            r#"{"key":"k2","duration":-1}"#,
            "badValue",
            Some("duration"),
        ),
        (
            r#"{"key":"k2","duration":"3"}"#,
            "badValue",
            Some("duration"),
        ),
        (r#"{"key":"k2","name":"  "}"#, "emptyName", Some("name")),
        (
            r#"{"key":"k2","name":"a / b"}"#,
            "invalidName",
            Some("name"),
        ),
        (r#"{"key":"k2","name":7}"#, "badValue", Some("name")),
        (
            r#"{"key":"k2","progress":101}"#,
            "badPercent",
            Some("progress"),
        ),
        (
            r#"{"key":"k2","progress":-1}"#,
            "badPercent",
            Some("progress"),
        ),
        (
            r#"{"key":"k2","progress":"5"}"#,
            "badValue",
            Some("progress"),
        ),
    ];
    for (bad, kind, member) in cases {
        let before = saved(&session);
        let json = format!(r#"[{{"key":"k1","duration":9}},{bad}]"#);
        let error = session.update_tasks(&json).unwrap_err();
        assert_eq!(
            (error.kind, error.record, error.member.as_deref()),
            (kind, Some(1), member)
        );
        assert_eq!(saved(&session), before, "{bad}");
    }
}

#[test]
fn a_setter_refusal_at_record_15_puts_back_records_1_to_14() {
    let (mut session, _) = keyed();
    let before = saved(&session);
    let fourteen = records(14);
    let json = format!(
        r#"{},{{"key":"k14","duration":200000000}}]"#,
        &fourteen[..fourteen.len() - 1]
    );
    let error = session.update_tasks(&json).unwrap_err();
    assert_eq!(error.kind, "schedule");
    assert_eq!(saved(&session), before);
}

#[test]
fn a_start_is_held_as_snet_unless_the_record_has_one_and_null_clears_it() {
    let (mut session, leaves) = keyed();
    let at = leaves[0].0 as usize;
    let day = |text: &str| session.origin().parse_day(text).unwrap();
    let (monday, tuesday) = (day("2026-03-09"), day("2026-03-10"));
    session
        .update_tasks(r#"[{"key":"k0","start":"2026-03-09"}]"#)
        .unwrap();
    assert_eq!(session.schedule().tasks()[at].snet, Some(monday));
    let both = r#"[{"key":"k0","snet":"2026-03-10T09:30","start":"2026-03-09"}]"#;
    session.update_tasks(both).unwrap();
    assert_eq!(session.schedule().tasks()[at].snet, Some(tuesday + 570));
    session
        .update_tasks(r#"[{"key":"k0","snet":null}]"#)
        .unwrap();
    assert_eq!(session.schedule().tasks()[at].snet, None);
}

#[test]
fn an_update_during_a_bar_press_is_refused_as_busy() {
    let (mut session, _, _, (x, y)) = setup();
    session.pointer_down(x, y, false);
    let json = r#"[{"key":"k0","duration":1}]"#;
    assert_eq!(session.update_tasks(json).unwrap_err().kind, "busy");
}

#[test]
fn a_name_is_trimmed_as_a_typed_rename_is_and_the_undo_list_is_left() {
    let (mut session, leaves) = keyed();
    let depth = session.history_len();
    session
        .update_tasks(r#"[{"key":"k4","name":"  Pour slab "}]"#)
        .unwrap();
    assert_eq!(session.meta()[leaves[4].0 as usize].name, "Pour slab");
    assert_eq!(session.history_len(), depth);
}

#[test]
fn a_progress_sets_what_a_typed_percent_sets_against_the_records_own_duration() {
    let (mut session, leaves) = keyed();
    let (mut typed, _) = keyed();
    let task = leaves[4];
    typed
        .update_tasks(r#"[{"key":"k4","duration":480}]"#)
        .unwrap();
    typed.set_percent_text(task, "33").unwrap();
    let depth = session.history_len();
    session
        .update_tasks(r#"[{"key":"k4","duration":480,"progress":33}]"#)
        .unwrap();
    let progress = |s: &Session| {
        let t = &s.schedule().tasks()[task.0 as usize];
        (t.actual_start, t.actual_finish, t.remaining)
    };
    assert_eq!(progress(&session), progress(&typed));
    assert_eq!(progress(&session).2, Some(300));
    assert_eq!(session.task_percent(task.0), "33%");
    assert_eq!(session.history_len(), depth);
}

#[test]
fn a_progress_that_sets_an_actual_after_the_data_date_is_refused_and_changes_nothing() {
    let (session, leaves) = keyed();
    let early = session.solution().dates[leaves[4].0 as usize].es - 1;
    let mut session = session.with_data_date(Some(early));
    let before = saved(&session);
    let json = r#"[{"key":"k1","name":"Renamed"},{"key":"k4","progress":50}]"#;
    let error = session.update_tasks(json).unwrap_err();
    assert_eq!(
        (error.kind, error.record, error.member.as_deref()),
        ("actualAfterDataDate", Some(1), Some("progress"))
    );
    assert_eq!(saved(&session), before);
}

fn actuals(session: &Session, key: usize) -> (Option<i64>, Option<i64>) {
    let task = session.task_for_key(&format!("k{key}")).unwrap();
    let t = &session.schedule().tasks()[task.0 as usize];
    (t.actual_start, t.actual_finish)
}

/// `first` as its own call, then `percent` typed: the dialog plans each field against the state the last left.
fn then_typed(first: &str, key: usize, percent: &str) -> Session {
    let (mut session, leaves) = keyed();
    session.update_tasks(first).unwrap();
    session.set_percent_text(leaves[key], percent).unwrap();
    session
}

#[test]
fn a_shortened_task_marked_done_finishes_at_its_new_early_finish() {
    let (mut session, _) = keyed();
    session
        .update_tasks(r#"[{"key":"k4","duration":2400}]"#)
        .unwrap();
    let long = actuals(&session, 4);
    let both = r#"[{"key":"k4","duration":480,"progress":100}]"#;
    let updated = session.update_tasks(both).unwrap();
    let typed = then_typed(r#"[{"key":"k4","duration":480}]"#, 4, "100");
    assert_eq!(updated.solves, 2);
    assert_eq!(actuals(&session, 4), actuals(&typed, 4));
    assert_ne!(actuals(&session, 4).1, long.1);
}

#[test]
fn a_start_and_a_progress_in_one_record_start_the_task_at_that_start() {
    let (mut session, _) = keyed();
    let both = r#"[{"key":"k4","start":"2027-01-04","progress":33}]"#;
    let before = actuals(&session, 4).0;
    session.update_tasks(both).unwrap();
    let typed = then_typed(r#"[{"key":"k4","start":"2027-01-04"}]"#, 4, "33");
    assert_eq!(actuals(&session, 4), actuals(&typed, 4));
    assert_ne!(actuals(&session, 4).0, before);
}

#[test]
fn a_predecessors_duration_moves_a_successors_progress_in_the_same_call() {
    let (mut session, leaves) = keyed();
    let typed = then_typed(r#"[{"key":"k20","duration":4800}]"#, 21, "100");
    let before = session.solution().dates[leaves[21].0 as usize].es;
    session
        .update_tasks(r#"[{"key":"k20","duration":4800},{"key":"k21","progress":100}]"#)
        .unwrap();
    assert_eq!(actuals(&session, 21), actuals(&typed, 21));
    assert_ne!(actuals(&session, 21).0, Some(before));
}

#[test]
fn a_call_that_mixes_no_progress_with_timing_solves_once() {
    let (mut session, _) = keyed();
    let stamp = session.stamp();
    let call = r#"[{"key":"k4","name":"Pour"},{"key":"k5","progress":50}]"#;
    let updated = session.update_tasks(call).unwrap();
    assert_eq!((updated.solves, session.stamp() - stamp), (1, 1));
}

#[test]
fn a_held_proposal_follows_an_update_of_the_tasks_it_rests_on() {
    let keyed_a = || {
        let mut session = what_moves::session();
        session.assign_key(KeyTarget::Task(TaskId(0)), "a").unwrap();
        session
    };
    let longer = r#"[{"key":"a","duration":960}]"#;
    let mut held = keyed_a();
    held.propose_duration(1, 3);
    let before = ghosts(&mut held);
    held.update_tasks(longer).unwrap();
    held.refresh_proposal();
    let mut again = keyed_a();
    again.update_tasks(longer).unwrap();
    again.propose_duration(1, 3);
    let after = ghosts(&mut held);
    assert_ne!(after, before, "the ghosts moved with A's new length");
    assert_eq!(after, ghosts(&mut again), "as if proposed after the update");
}
