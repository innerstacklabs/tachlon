//! The hover card lists the names of a task's resources (gs#703): filled at open, moved by no edit,
//! undo or redo, and empty for a task with none.

use crate::common;

use app::{Session, load_bytes};
use common::clock;
use engine::TaskId;
use import::ProjectSelection;
use timeline::Timeline;

/// Two tasks: the first has two resources, the second none.
const PROJECT: &str = "<Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-07T08:00:00</StartDate><Tasks>\
<Task><UID>1</UID><Name>T1</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>2</UID><Name>T2</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
</Tasks><Resources><Resource><UID>1</UID><Name>Crew A</Name></Resource>\
<Resource><UID>2</UID><Name>Crew \"B\"</Name></Resource></Resources>\
<Assignments><Assignment><TaskUID>1</TaskUID><ResourceUID>1</ResourceUID></Assignment>\
<Assignment><TaskUID>1</TaskUID><ResourceUID>2</ResourceUID></Assignment></Assignments></Project>";

/// A session over the file as `App::create` builds it, resources included.
fn open() -> Session {
    let loaded = load_bytes(PROJECT.as_bytes(), &ProjectSelection::Unspecified, &clock)
        .expect("the file loads");
    let timeline = Timeline::new(loaded.order, 1200.0, 600.0, 1.0);
    Session::new(
        loaded.schedule,
        loaded.solution,
        timeline,
        loaded.meta,
        clock,
    )
    .with_origin(loaded.origin)
    .with_assignments(loaded.resources)
}

fn names(s: &Session, task: u32) -> Vec<String> {
    s.task_details(task).expect("the task exists").resources
}

fn none_listed() -> Vec<String> {
    Vec::new()
}

#[test]
fn the_card_lists_a_tasks_resources_through_an_edit_undo_and_redo() {
    let mut session = open();
    let held = session.resources(TaskId(0));
    assert!(held.len() == 2 && held.iter().all(|a| a.units == 100));
    let kept = vec!["Crew A".to_string(), "Crew \"B\"".to_string()];
    assert_eq!(names(&session, 0), kept);
    assert_eq!(
        names(&session, 1),
        none_listed(),
        "a task with none lists none"
    );
    let json: serde_json::Value =
        serde_json::from_str(&session.task_details_json(0).expect("the task exists")).unwrap();
    assert_eq!(json["resources"], serde_json::json!(kept));
    let none: serde_json::Value =
        serde_json::from_str(&session.task_details_json(1).expect("the task exists")).unwrap();
    assert_eq!(none["resources"], serde_json::json!([]));

    session
        .set_start_text(TaskId(0), "2026-01-19")
        .expect("the start sets");
    assert_eq!(names(&session, 0), kept);
    assert!(session.undo().expect("undoes"));
    assert_eq!(names(&session, 0), kept);
    assert!(session.redo().expect("redoes"));
    assert_eq!(names(&session, 0), kept);
    assert_eq!(names(&session, 1), none_listed());
}

#[test]
fn the_list_column_joins_a_tasks_names_and_shows_only_for_a_file_that_names_one() {
    let session = open();
    assert!(session.has_resources());
    assert_eq!(session.task_resources_text(0), "Crew A, Crew \"B\"");
    assert_eq!(session.task_resources_text(1), "");
    let unnamed = open().with_resources(vec![vec![String::new()]]);
    assert!(!unnamed.has_resources(), "an empty name is no name");
    let skipped = open().with_resources(vec![vec![String::new(), "Ana".to_string()]]);
    assert_eq!(skipped.task_resources_text(0), "Ana");
}
