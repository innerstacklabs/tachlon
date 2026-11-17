//! The edits a typed list of predecessors (`3, 5SS+1d`) plans: the links to make, change and
//! remove, and a sentence naming the item of a mistake.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::links::build_with;
use app::{
    Session,
    edit::{Edit, EditError},
    history::Source,
};
use engine::{DepId, LinkType, Minutes, TaskId};

const DAY: Minutes = 24 * 60;
const TARGET: TaskId = TaskId(4);
const CODES: [&str; 6] = ["A5", "A5SS", "3", "5", "9", "L"];

/// Six tasks with Activity IDs (`CODES`); the target (4) precedes `loop` (5) and follows nothing.
fn schedule(with_ids: bool) -> Session {
    let names = ["A5", "A5SS", "three", "five", "target", "loop"];
    build_with(&names.map(|name| (name, None, 60)), &[(4, 5)], |meta| {
        for (meta, code) in meta.iter_mut().zip(CODES).filter(|_| with_ids) {
            meta.code = Some(code.to_string());
        }
    })
}

fn plan(session: &Session, text: &str) -> Result<Vec<Edit>, EditError> {
    session.plan_predecessors_text(TARGET, text)
}

fn apply(session: &mut Session, text: &str) {
    let edits = plan(session, text).unwrap();
    session.edit_group(edits, Source::Typed).unwrap();
}

fn add(pred: u32, kind: LinkType, lag: Minutes) -> Edit {
    let (pred, succ) = (TaskId(pred), TARGET);
    Edit::AddLink {
        pred,
        succ,
        kind,
        lag,
    }
}

#[test]
fn a_list_plans_one_link_per_item_and_one_undo_takes_them_all_back() {
    let mut session = schedule(true);
    let expected = vec![add(2, LinkType::FS, 0), add(3, LinkType::SS, DAY)];
    assert_eq!(plan(&session, " 5ss+1d ,3,").unwrap().len(), 2);
    assert_eq!(plan(&session, "3, 5SS+1d").unwrap(), expected);
    apply(&mut session, "3, 5SS+1d");
    assert_eq!(plan(&session, "3, 5SS+1d").unwrap(), vec![]);
    assert!(session.undo().unwrap());
    assert_eq!(session.schedule().live_dependencies().count(), 1);
}

#[test]
fn a_changed_or_dropped_item_plans_that_link_alone_and_empty_text_removes_all() {
    let mut session = schedule(true);
    apply(&mut session, "3, 5SS+1d");
    let (dep, kind, lag) = (DepId(1), LinkType::FF, 2 * DAY);
    let edits = plan(&session, "3FF+2d, 5SS+1d").unwrap();
    assert_eq!(edits, vec![Edit::SetLink { dep, kind, lag }]);
    let remove = |dep| Edit::RemoveLink { dep: DepId(dep) };
    assert_eq!(plan(&session, "5SS+1d").unwrap(), vec![remove(1)]);
    assert_eq!(plan(&session, "").unwrap(), vec![remove(1), remove(2)]);
}

#[test]
fn each_refusal_names_its_item() {
    let session = schedule(true);
    let faults = [
        ("nope", "\"nope\" is not the ID of a task."),
        (
            "3, 9",
            "\"9\" is this task, and a task cannot follow itself.",
        ),
        ("3, 5, 3", "\"3\" is in the list more than once."),
        ("3XX", "\"3XX\": start with FS, SS, FF or SF, such as SS+2d"),
    ];
    for (text, sentence) in faults {
        let error = plan(&session, text).unwrap_err();
        assert_eq!(error.to_string(), sentence, "{text}");
        assert_eq!(error.kind(), "predecessor");
    }
}

#[test]
fn an_id_that_is_a_task_wins_over_a_shorter_id_with_a_type() {
    let session = schedule(true);
    let plan = |text| plan(&session, text).unwrap();
    assert_eq!(plan("A5SS"), vec![add(1, LinkType::FS, 0)]);
    assert_eq!(plan("A5SS+1d"), vec![add(0, LinkType::SS, DAY)]);
}

#[test]
fn a_schedule_with_no_activity_ids_refuses_a_list_and_plans_nothing_for_empty_text() {
    let session = schedule(false);
    let error = plan(&session, "1").unwrap_err();
    assert_eq!(
        error.to_string(),
        "This schedule has no Activity IDs to type."
    );
    assert_eq!(plan(&session, "").unwrap(), vec![]);
}

/// The link changes of each commit the host has not read, as `op slot field`.
fn link_changes(session: &mut Session) -> Vec<Vec<String>> {
    let out: serde_json::Value = serde_json::from_str(&session.take_commits()).unwrap();
    let change = |c: &serde_json::Value| {
        let field = c["field"].as_str().unwrap_or("");
        format!("{} {} {field}", c["op"].as_str().unwrap(), c["link"])
    };
    let all = |c: &serde_json::Value| {
        c["changes"]
            .as_array()
            .unwrap()
            .iter()
            .map(change)
            .collect()
    };
    out["commits"].as_array().unwrap().iter().map(all).collect()
}

#[test]
fn a_typed_list_makes_changes_and_removes_links_in_one_undo_step() {
    let mut session = schedule(true);
    session.set_predecessors_text(TARGET, "3, 5").unwrap();
    link_changes(&mut session);
    session.set_predecessors_text(TARGET, "3, 5").unwrap();
    assert_eq!(link_changes(&mut session), Vec::<Vec<String>>::new());
    session.set_predecessors_text(TARGET, "3SS+1d, A5").unwrap();
    let want = [["delete 2 ", "update 1 type", "update 1 lag", "create 3 "]];
    assert_eq!(link_changes(&mut session), want);
    assert_eq!(session.predecessors_text(TARGET).unwrap(), "3 SS+1d, A5");
    assert!(session.undo().unwrap());
    assert_eq!(session.predecessors_text(TARGET).unwrap(), "3, 5");
    session.set_predecessors_text(TARGET, "A5 SS").unwrap();
    let text = session.predecessors_text(TARGET).unwrap();
    assert_eq!(text, "A5 SS", "A5SS is a task of its own");
    assert_eq!(plan(&session, &text).unwrap(), vec![]);
}
