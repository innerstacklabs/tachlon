//! gs#703: the XER writer's round trip. Writing, reading and writing again gives the same bytes,
//! and a file read, written and read again shows the same tasks, links and lag calendar.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

mod common;

use common::{WEEK, handbuilt, regular_task};
use engine::{Dependency, LagCalendar, LinkType, Task, TaskId};
use import::{Imported, ProjectSelection, XerLayout, XerOptions, export_xer, parse_xer};
use xer::synth::{Spec, write};

const EXPORTED_AT: i64 = 19_787 * 1_440;

fn options() -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: "Exported".to_owned(),
        layout: XerLayout::FromXer,
    }
}

fn fixtures() -> Vec<(&'static str, Vec<u8>)> {
    vec![
        (
            "one-project",
            include_bytes!("../../../packages/gantt/tests/fixtures/one-project.xer").to_vec(),
        ),
        (
            "two-projects",
            include_bytes!("../../../packages/gantt/tests/fixtures/two-projects.xer").to_vec(),
        ),
        (
            "ledger-timeline",
            include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer").to_vec(),
        ),
        ("synth", write(&Spec::small(7))),
    ]
}

fn import(bytes: &[u8]) -> Imported {
    parse_xer(bytes, &ProjectSelection::All).expect("the file should import")
}

fn export(imported: &Imported) -> Vec<u8> {
    export_xer(imported, &options())
        .expect("export should work")
        .bytes
}

/// What a person sees of a schedule, apart from the dates a started task's actuals move: each
/// activity's name, code, level and duration, each link between activities, and the lag calendar.
type Signature = (
    Vec<(String, String, u8, i64)>,
    Vec<(String, String, LinkType, i64)>,
    LagCalendar,
);

fn signature(imported: &Imported) -> Signature {
    let code = |id: TaskId| imported.tasks[id.0 as usize].code.clone();
    let mut tasks: Vec<_> = imported
        .tasks
        .iter()
        .zip(imported.schedule.tasks())
        .filter(|(task, schedule)| task.code.is_some() && !schedule.deleted)
        .map(|(task, schedule)| {
            (
                task.code.clone().unwrap_or_default(),
                task.name.clone(),
                task.outline_level,
                schedule.duration,
            )
        })
        .collect();
    tasks.sort();
    let mut links: Vec<_> = imported
        .schedule
        .live_dependencies()
        .filter_map(|(_, link)| Some((code(link.pred)?, code(link.succ)?, link.kind, link.lag)))
        .collect();
    links.sort_by_key(|(pred, succ, kind, lag)| (pred.clone(), succ.clone(), *lag, *kind as u8));
    (tasks, links, imported.schedule.lag_calendar())
}

/// The first write drops a started task's actuals (`left_out` says so) and the reader lists a task
/// by its start, so the first read-back can order tasks differently from the source. From then
/// on nothing is lost: the second write and the third are the same bytes.
#[test]
fn write_read_write_is_byte_identical() {
    for (name, bytes) in fixtures() {
        let imported = import(&bytes);
        let linked = imported.schedule.live_dependencies().count() > 0;
        let first = export(&imported);
        assert!(
            !linked || String::from_utf8_lossy(&first).contains("%T\tTASKPRED"),
            "{name}: the links should be written"
        );

        let second = export(&import(&first));
        let third = export(&import(&second));

        assert_eq!(second, third, "{name}");
    }
}

#[test]
fn parse_export_parse_has_the_same_signature() {
    for (name, bytes) in fixtures() {
        let imported = import(&bytes);
        let before = signature(&imported);
        assert!(!before.0.is_empty(), "{name}: it should have activities");

        let again = import(&export(&imported));

        assert_eq!(signature(&again), before, "{name}");
    }
}

fn two_activities() -> Imported {
    let tasks = [regular_task(1, "A", "8"), regular_task(2, "B", "8")];
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    parse_xer(
        &handbuilt("2024-03-04 08:00", &tasks, &["2\t1\tPR_FS\t0"], WEEK),
        &ProjectSelection::Unspecified,
    )
    .expect("hand-built XER should import")
}

fn position(imported: &Imported, code: Option<&str>) -> TaskId {
    let index = imported
        .tasks
        .iter()
        .position(|task| task.code.as_deref() == code)
        .expect("the task should be imported");
    TaskId(index as u32)
}

fn left_out(imported: &Imported) -> Vec<String> {
    export_xer(imported, &options())
        .expect("export should work")
        .left_out
}

fn link(pred: TaskId, succ: TaskId) -> Dependency {
    Dependency {
        pred,
        succ,
        kind: LinkType::FS,
        lag: 0,
    }
}

#[test]
fn a_link_with_a_summary_end_is_listed_as_left_out() {
    let mut imported = two_activities();
    let root = position(&imported, None);
    let a = position(&imported, Some("A"));
    imported
        .schedule
        .add_dependency(link(root, a))
        .expect("link");

    let lines = left_out(&imported);

    assert_eq!(
        lines,
        vec![format!(
            "link from task {} to task {}: an end is a summary, the link is not written",
            root.0, a.0
        )]
    );
}

#[test]
fn a_link_with_a_lag_calendar_of_its_own_is_listed_as_left_out() {
    let mut imported = two_activities();
    let (id, _) = imported
        .schedule
        .live_dependencies()
        .next()
        .expect("the link should be imported");
    imported
        .schedule
        .set_dependency_lag_calendar(id, Some(LagCalendar::TwentyFourHour))
        .expect("the lag calendar should set");

    let lines = left_out(&imported);

    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].ends_with("its own lag calendar is not written, the schedule's is used"));
}

#[test]
fn a_fixed_lag_calendar_that_is_not_the_projects_is_listed_as_left_out() {
    let mut imported = two_activities();
    let other = imported
        .schedule
        .add_calendar(imported.schedule.calendar_of(TaskId(0)).clone());
    imported
        .schedule
        .set_lag_calendar(LagCalendar::Fixed(other))
        .expect("the lag calendar should set");

    let lines = left_out(&imported);

    assert_eq!(
        lines,
        vec![
            "project 1: the lag calendar is a calendar other than the project's own, written as the project's"
        ]
    );
}

#[test]
fn a_link_to_a_task_with_no_metadata_is_not_reported_as_a_summary() {
    let mut imported = two_activities();
    let a = position(&imported, Some("A"));
    let task = Task {
        duration: 480,
        calendar: imported.schedule.tasks()[a.0 as usize].calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    let extra = imported.schedule.add_task(task).expect("task should add");
    imported
        .schedule
        .add_dependency(link(a, extra))
        .expect("link");

    let lines = left_out(&imported);

    assert_eq!(
        lines,
        vec![format!(
            "link from task {} to task {}: an end has no row in the file, the link is not written",
            a.0, extra.0
        )]
    );
}

#[test]
fn a_baseline_is_listed_as_not_written() {
    let mut imported = two_activities();
    imported.tasks[1].baseline_start = Some(0);

    assert_eq!(
        left_out(&imported),
        vec!["the baseline on 1 task was not written"]
    );
}

#[test]
fn a_deadline_is_listed_as_not_written() {
    let mut imported = two_activities();
    imported.tasks[1].deadline = Some(480);
    imported.tasks[2].deadline = Some(960);

    assert_eq!(
        left_out(&imported),
        vec!["the deadline on 2 tasks was not written"]
    );
}

#[test]
fn a_schedule_with_no_baseline_or_deadline_lists_neither() {
    assert_eq!(left_out(&two_activities()), Vec::<String>::new());
}
