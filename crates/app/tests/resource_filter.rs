//! The Resource row filter (gs#703): the table keeps the leaves that name one resource, and the
//! summaries above them as context.

use crate::common;

use app::{RowFilter, Session, edit::Edit, load_bytes};
use common::clock;
use engine::TaskId;
use import::ProjectSelection;
use timeline::Timeline;

/// Ids 0 to 4: summary Hull over Keel (Crew A) and Deck (A and Crew B), summary Fit-out over Mast (B).
const PROJECT: &str = "<Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-07T08:00:00</StartDate><Tasks>\
<Task><UID>1</UID><Name>Hull</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary></Task>\
<Task><UID>2</UID><Name>Keel</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>3</UID><Name>Deck</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>4</UID><Name>Fit-out</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary></Task>\
<Task><UID>5</UID><Name>Mast</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
</Tasks><Resources><Resource><UID>1</UID><Name>Crew A</Name></Resource><Resource><UID>2</UID><Name>Crew B</Name></Resource></Resources>\
<Assignments><Assignment><TaskUID>2</TaskUID><ResourceUID>1</ResourceUID></Assignment>\
<Assignment><TaskUID>3</TaskUID><ResourceUID>1</ResourceUID></Assignment><Assignment><TaskUID>3</TaskUID><ResourceUID>2</ResourceUID></Assignment>\
<Assignment><TaskUID>5</TaskUID><ResourceUID>2</ResourceUID></Assignment></Assignments></Project>";

/// A session over the file as `App::create` builds it, outline and resources included.
fn open() -> Session {
    let l = load_bytes(PROJECT.as_bytes(), &ProjectSelection::Unspecified, &clock).expect("loads");
    let timeline = Timeline::new(l.order, 1200.0, 600.0, 1.0);
    Session::new(l.schedule, l.solution, timeline, l.meta, clock)
        .with_origin(l.origin)
        .with_outline(l.outline)
        .expect("the outline fits")
        .with_assignments(l.resources)
}

/// The listed task ids, in id order.
fn listed(s: &Session) -> Vec<u32> {
    let mut ids = s.timeline().order().to_vec();
    ids.sort_unstable();
    ids
}

#[test]
fn a_resource_keeps_its_tasks_and_the_summary_above_them() {
    let mut s = open();
    assert_eq!(s.resource_names(), vec!["Crew A", "Crew B"]);
    assert!(s.set_resource_filter(Some("Crew A")));
    assert_eq!(
        (s.resource_filter(), s.row_filter().name()),
        (Some("Crew A"), "resource")
    );
    assert_eq!(
        listed(&s),
        vec![0, 1, 2],
        "Fit-out has nothing left under it"
    );
    assert!(s.set_resource_filter(Some("Crew B")));
    assert_eq!(listed(&s), vec![0, 2, 3, 4]);
    for name in ["Crew C", "", "crew b"] {
        assert!(!s.set_resource_filter(Some(name)), "{name:?} is refused");
        assert_eq!(listed(&s), vec![0, 2, 3, 4]);
    }
    assert!(s.set_resource_filter(None));
    assert_eq!(listed(&s), vec![0, 1, 2, 3, 4]);
}

#[test]
fn it_replaces_another_filter_and_off_leaves_a_different_one_on() {
    let mut s = open();
    assert!(s.set_critical_only(true) && s.set_resource_filter(Some("Crew B")));
    assert!(s.set_critical_only(true));
    assert_eq!(s.resource_filter(), None);
    assert!(s.set_resource_filter(None));
    assert_eq!(s.row_filter(), RowFilter::Critical);
}

#[test]
fn the_rows_follow_an_edit_and_a_deleted_task_gives_up_its_names() {
    let mut s = open();
    assert!(s.set_resource_filter(Some("Crew A")));
    s.edit(Edit::Delete { task: TaskId(1) }).expect("deletes");
    assert_eq!(listed(&s), vec![0, 2], "Keel left, Deck stays under Hull");
    s.edit(Edit::Delete { task: TaskId(4) }).expect("deletes");
    s.edit(Edit::Delete { task: TaskId(2) }).expect("deletes");
    assert_eq!(s.resource_names(), Vec::<String>::new());
    assert!(
        !s.set_resource_filter(Some("Crew B")),
        "no live task names it"
    );
}
