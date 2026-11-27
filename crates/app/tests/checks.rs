//! The DCMA link counts, against one hand-counted six-task fixture.
//!
//! ```text
//! 0 summary { 1, 2 }   2 and 3 follow in a chain; 5 is finished and linkless
//! 1 -> 2  FS  lag   0
//! 2 -> 3  FS  lag +480
//! 3 -> 4  SS  lag -240
//! 1 -> 4  FF  lag   0
//! ```
//!
//! Unfinished leaves are 1, 2, 3 and 4. Task 1 has no predecessor and task 4 no successor. The
//! summary (0) has no links and the finished task (5) has none, and neither counts.
//!
//! The missing resources count is read from a small project file (`opened`), the way `App::create`
//! opens it, because only a file names a resource.

use crate::common;

use app::{
    Session, checks::Checks, checks::schedule::ScheduleChecks, load_bytes, meta::TaskMeta,
    outline::Outline,
};
use common::clock;
use engine::{Calendar, Dependency, Limit, LinkType, Minutes, Pin, Schedule, Task, TaskId};
use import::ProjectSelection;
use timeline::Timeline;

fn link(pred: u32, succ: u32, kind: LinkType, lag: Minutes) -> Dependency {
    Dependency {
        pred: TaskId(pred),
        succ: TaskId(succ),
        kind,
        lag,
    }
}

fn meta(parent: Option<TaskId>, level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    }
}

fn fixture() -> Session {
    let day = vec![(480, 720), (780, 1020)];
    let calendar = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    for index in 0..6 {
        schedule
            .add_task(Task {
                duration: 480,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: (index == 5).then_some(0),
                actual_finish: (index == 5).then_some(480),
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    for dep in [
        link(1, 2, LinkType::FS, 0),
        link(2, 3, LinkType::FS, 480),
        link(3, 4, LinkType::SS, -240),
        link(1, 4, LinkType::FF, 0),
    ] {
        schedule.add_dependency(dep).expect("tasks exist");
    }
    let solution = schedule.compute().expect("the fixture has no loop");
    let metas = vec![
        meta(None, 1),
        meta(Some(TaskId(0)), 2),
        meta(Some(TaskId(0)), 2),
        meta(None, 1),
        meta(None, 1),
        meta(None, 1),
    ];
    let wbs: Vec<_> = (0..6).map(TaskId).collect();
    let outline = Outline::new(&metas, &wbs, &schedule).expect("valid outline");
    let timeline = Timeline::new((0..6).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .expect("outline matches the schedule")
}

#[test]
fn missing_logic_counts_unfinished_leaves_without_a_predecessor_or_successor() {
    let checks = Checks::of(&fixture());

    assert_eq!(checks.missing_logic, 2);
}

#[test]
fn open_starts_and_open_ends_count_the_unfinished_leaves_with_one_side_loose() {
    let checks = Checks::of(&fixture());

    assert_eq!((checks.open_starts, checks.open_ends), (1, 1));
}

#[test]
fn leads_are_the_links_with_negative_lag() {
    let checks = Checks::of(&fixture());

    assert_eq!(checks.leads, 1);
}

#[test]
fn lags_are_the_links_with_positive_lag() {
    let checks = Checks::of(&fixture());

    assert_eq!(checks.lags, 1);
}

#[test]
fn relationship_types_report_the_share_of_finish_to_start_links() {
    let checks = Checks::of(&fixture());

    assert_eq!((checks.links, checks.finish_to_start), (4, 2));
    assert_eq!(checks.finish_to_start_share(), Some(0.5));
}

/// A four-day predecessor from Monday 08:00, so it finishes on day 4 (Thursday), and a one-day
/// finish-to-start successor pinned to start at 08:00 on day `pin_day` (1 is Monday).
fn pinned(pin_day: i64) -> Session {
    let day = vec![(480, 720), (780, 1020)];
    let calendar = Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("valid standard calendar");
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar);
    for (duration, pin) in [
        (4 * 480, None),
        (480, Some(Pin::Start((pin_day - 1) * 1440 + 480))),
    ] {
        schedule
            .add_task(Task {
                duration,
                calendar,
                snet: None,
                limit: Limit {
                    pin,
                    ..Limit::default()
                },
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule
        .add_dependency(link(0, 1, LinkType::FS, 0))
        .expect("tasks exist");
    let solution = schedule.compute().expect("the fixture has no loop");
    let timeline = Timeline::new(vec![0, 1], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
}

/// The pin on day 3 sits before the day-4 finish of its predecessor, so the link is broken; a pin
/// on day 5 sits after it, so the same link is not.
#[test]
fn pin_broken_links_counts_the_links_into_a_task_pinned_before_its_predecessor_finishes() {
    assert_eq!(Checks::of(&pinned(3)).pin_broken_links, 1);
    assert_eq!(Checks::of(&pinned(5)).pin_broken_links, 0);
}

/// A project file as `App::create` opens it: tasks 0 to 4 are the summary Hull over Keel (1 day),
/// Deck (1 day) and Mast (1 day), and the milestone Done (no duration). `assignments` is the
/// `Resources` and `Assignments` part of the file, or nothing.
fn opened(assignments: &str) -> Session {
    let file = format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\">\
<StartDate>2026-01-07T08:00:00</StartDate><Tasks>\
<Task><UID>1</UID><Name>Hull</Name><OutlineLevel>1</OutlineLevel><Summary>1</Summary></Task>\
<Task><UID>2</UID><Name>Keel</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>3</UID><Name>Deck</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>4</UID><Name>Mast</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>\
<Task><UID>5</UID><Name>Done</Name><OutlineLevel>1</OutlineLevel><Summary>0</Summary><Duration>PT0S</Duration></Task>\
</Tasks>{assignments}</Project>"
    );
    let l = load_bytes(file.as_bytes(), &ProjectSelection::Unspecified, &clock).expect("loads");
    let timeline = Timeline::new(l.order, 1200.0, 600.0, 1.0);
    Session::new(l.schedule, l.solution, timeline, l.meta, clock)
        .with_origin(l.origin)
        .with_outline(l.outline)
        .expect("the outline fits")
        .with_assignments(l.resources)
}

/// Two assignments (Keel, Deck) and one task with none (Mast): the count is 1. The summary Hull
/// and the milestone Done have no resource either and are not counted.
#[test]
fn missing_resources_counts_the_unfinished_leaves_with_a_duration_and_no_resource() {
    let session = opened(
        "<Resources><Resource><UID>1</UID><Name>Crew A</Name></Resource></Resources>\
<Assignments><Assignment><TaskUID>2</TaskUID><ResourceUID>1</ResourceUID></Assignment>\
<Assignment><TaskUID>3</TaskUID><ResourceUID>1</ResourceUID></Assignment></Assignments>",
    );

    assert_eq!(ScheduleChecks::of(&session).missing_resources, Some(1));
}

/// A file that names no resource has not been checked: `None`, not a 0 that reads as a clean pass.
#[test]
fn missing_resources_is_none_when_the_file_names_no_resource() {
    let session = opened("");

    assert_eq!(ScheduleChecks::of(&session).missing_resources, None);
}
