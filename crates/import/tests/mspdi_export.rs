//! The MS Project XML writer: read a file, write it, read the result, and the two reads agree on
//! everything the reader maps (`docs/IMPORT.md`). `Schedule` has no `PartialEq`, so the
//! comparison is a signature built here (gs#703, R7). Whether MS Project opens the file is not
//! tested: that is checked by a person who has a copy.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Limit, Pin, TaskId};
use import::{Imported, ImportedTask, export_mspdi, parse_mspdi};

const LINKS: &str = include_str!("fixtures/links.xml");
const SUMMARY: &str = include_str!("fixtures/summary.xml");
const CONSTRAINTS: &str = include_str!("fixtures/constraints.xml");
const BASELINE: &str = include_str!("fixtures/baseline.xml");
const HOLIDAYS: &str = include_str!("fixtures/holidays.xml");
const PERCENT: &str = include_str!("fixtures/percent.xml");
const OVERLAPPING: &str = include_str!("fixtures/overlapping-periods.xml");

/// Everything the MS Project reader maps, as plain comparable values: the calendars' content (not
/// their ids), each live task's fields with its calendar's content, each link with its lag
/// calendar, the dates and the codes.
#[derive(Debug, PartialEq)]
struct Signature {
    origin_day: i64,
    project_start: Option<i64>,
    data_date: Option<i64>,
    calendars: Vec<String>,
    tasks: Vec<String>,
    links: Vec<String>,
}

fn calendar_text(calendar: &engine::Calendar) -> String {
    format!("{:?} {:?}", calendar.week(), calendar.exceptions())
}

fn signature(imported: &Imported) -> Signature {
    let schedule = &imported.schedule;
    let calendars: Vec<String> = schedule.calendars().iter().map(calendar_text).collect();
    let tasks = imported
        .tasks
        .iter()
        .zip(schedule.tasks())
        .map(|(meta, task)| {
            let meta = ImportedTask {
                resource_names: Vec::new(),
                ..meta.clone()
            };
            format!(
                "{meta:?} duration {} snet {:?} limit {:?} calendar {} deleted {}",
                task.duration,
                task.snet,
                task.limit,
                calendars
                    .get(task.calendar.0 as usize)
                    .map_or("none", String::as_str),
                task.deleted
            )
        })
        .collect();
    let links = schedule
        .dependencies()
        .iter()
        .enumerate()
        .map(|(index, dep)| {
            let id = engine::DepId(u32::try_from(index).expect("few links"));
            format!(
                "{} {:?} {} lag {} calendar {:?}",
                dep.pred,
                dep.kind,
                dep.succ,
                dep.lag,
                schedule.dependency_lag_calendar(id)
            )
        })
        .collect();
    Signature {
        origin_day: imported.origin_day,
        project_start: imported.project_start,
        data_date: imported.data_date,
        calendars,
        tasks,
        links,
    }
}

fn written(imported: &Imported) -> String {
    let exported = export_mspdi(imported).expect("the project should be writable");
    String::from_utf8(exported.bytes).expect("the file is UTF-8")
}

/// Read `xml`, write it, read the result; the two signatures must be equal.
fn round_trip(name: &str, xml: &str) -> (Imported, String) {
    let first = parse_mspdi(xml).unwrap_or_else(|error| panic!("{name} should import: {error}"));
    let text = written(&first);
    let second = parse_mspdi(&text)
        .unwrap_or_else(|error| panic!("the file written from {name} should import: {error}"));
    assert_eq!(
        signature(&first),
        signature(&second),
        "{name}: the second read differs from the first"
    );
    (first, text)
}

#[test]
fn links_summary_and_constraints_read_back_to_the_same_signature() {
    for (name, xml) in [
        ("links.xml", LINKS),
        ("summary.xml", SUMMARY),
        ("constraints.xml", CONSTRAINTS),
    ] {
        round_trip(name, xml);
    }
}

#[test]
fn the_other_fixtures_read_back_to_the_same_signature() {
    for (name, xml) in [
        ("baseline.xml", BASELINE),
        ("holidays.xml", HOLIDAYS),
        ("percent.xml", PERCENT),
        ("overlapping-periods.xml", OVERLAPPING),
    ] {
        round_trip(name, xml);
    }
}

#[test]
fn a_status_date_and_a_lag_in_elapsed_time_survive() {
    let xml = LINKS.replace(
        "<StartDate>2026-01-05T08:00:00</StartDate>",
        "<StartDate>2026-01-05T08:00:00</StartDate><StatusDate>2026-01-08T16:30:00</StatusDate>",
    );
    let xml = xml.replace(
        "<LinkLag>4800</LinkLag><LagFormat>7</LagFormat>",
        "<LinkLag>4800</LinkLag><LagFormat>8</LagFormat>",
    );
    let (first, _) = round_trip("links.xml with a status date and elapsed lags", &xml);
    assert!(first.data_date.is_some());
    assert!(
        first
            .schedule
            .dependencies()
            .iter()
            .enumerate()
            .any(|(index, _)| first
                .schedule
                .dependency_lag_calendar(engine::DepId(index as u32))
                .is_some()),
        "the fixture should hold an elapsed lag"
    );
}

#[test]
fn an_elapsed_duration_and_a_project_start_off_monday_survive() {
    let xml = LINKS
        .replace("2026-01-05T08:00:00", "2026-01-07T09:30:00")
        .replace(
            "<Duration>PT24H0M0S</Duration><DurationFormat>7</DurationFormat>",
            "<Duration>PT24H0M0S</Duration><DurationFormat>8</DurationFormat>",
        );
    let (first, _) = round_trip("links.xml starting on a Wednesday, one elapsed task", &xml);
    assert_eq!(first.project_start, Some(2 * 1440 + 9 * 60 + 30));
    assert!(first.schedule.calendars().len() > 1, "the elapsed calendar");
}

#[test]
fn the_written_bytes_are_well_formed_and_the_same_every_time() {
    for xml in [LINKS, SUMMARY, CONSTRAINTS, HOLIDAYS] {
        let first = parse_mspdi(xml).expect("fixture imports");
        let once = written(&first);
        assert_eq!(once, written(&first), "writing twice gives the same bytes");
        let second = parse_mspdi(&once).expect("the written file is well formed XML");
        assert_eq!(
            once,
            written(&second),
            "a second generation writes the same file"
        );
        assert!(once.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    }
}

#[test]
fn names_with_markup_and_spaces_survive() {
    let xml = LINKS.replace(
        "<Name>A</Name>",
        "<Name>  Dig &amp; pour &lt;slab&gt; \"east\"\tend  </Name>",
    );
    let (first, text) = round_trip("links.xml with a markup name", &xml);
    assert_eq!(
        first.tasks[0].name, "  Dig & pour <slab> \"east\"\tend  ",
        "the fixture's name decodes"
    );
    assert!(text.contains("&amp;") && text.contains("&lt;slab&gt;"));
}

#[test]
fn left_out_names_what_the_file_does_not_carry() {
    let (_, text) = round_trip("constraints.xml", CONSTRAINTS);
    let first = parse_mspdi(CONSTRAINTS).expect("fixture imports");
    let left_out = export_mspdi(&first).expect("writable").left_out;
    let any = |needle: &str| left_out.iter().any(|line| line.contains(needle));
    assert!(
        any("finish no later than constraint (type 7)"),
        "{left_out:?}"
    );
    // Task 2's must-start-on and task 1's start-no-earlier-than are written, so not listed.
    assert!(
        text.contains("<ConstraintType>2</ConstraintType>"),
        "{text}"
    );
    assert!(!any("task 1: its") && !any("task 2: its"), "{left_out:?}");
    assert!(!any("deadline"), "{left_out:?}");
    assert!(any("resource"), "{left_out:?}");
    assert!(
        text.contains("<Deadline>2026-01-12T16:00:00</Deadline>"),
        "{text}"
    );
    assert!(!text.contains("<Assignment"), "no resource is written");
    // Calendars and the constraints are written, so not listed.
    assert!(!any("start-no-earlier-than"), "{left_out:?}");
}

/// A deadline reads back as the instant the reader first gave, before the origin included; a task
/// with none, and one whose deadline is not a date, write no element.
#[test]
fn a_deadline_before_the_origin_reads_back_and_a_task_without_one_writes_none() {
    let early = CONSTRAINTS.replace(
        "2026-01-12T16:00:00</Deadline>",
        "2025-12-29T16:00:00</Deadline>",
    );
    let (first, text) = round_trip("constraints.xml with an early deadline", &early);
    let second = parse_mspdi(&text).expect("the written file imports");
    let deadlines = |imported: &Imported| -> Vec<Option<i64>> {
        imported.tasks.iter().map(|task| task.deadline).collect()
    };
    assert!(
        deadlines(&first)[1].is_some_and(|at| at < 0),
        "{:?}",
        deadlines(&first)
    );
    assert_eq!(deadlines(&first), deadlines(&second));
    assert_eq!(text.matches("<Deadline>").count(), 1, "{text}");
    let unreadable = CONSTRAINTS.replace("2026-01-12T16:00:00</Deadline>", "soon</Deadline>");
    let (_, text) = round_trip("constraints.xml with an unreadable deadline", &unreadable);
    assert!(!text.contains("<Deadline>"), "{text}");
}

#[test]
fn a_project_with_nothing_to_leave_out_lists_nothing() {
    let first = parse_mspdi(LINKS).expect("fixture imports");
    let exported = export_mspdi(&first).expect("writable");
    assert_eq!(exported.left_out, Vec::<String>::new());
}

/// The reader keeps no `Summary` flag (a summary is a zero-duration task), so the signature cannot
/// see it: the file's own elements are read here.
#[test]
fn summaries_milestones_and_constraints_are_written_as_their_elements() {
    let (_, summary) = round_trip("summary.xml", SUMMARY);
    assert_eq!(
        summary.matches("<Summary>1</Summary>").count(),
        2,
        "{summary}"
    );
    assert_eq!(summary.matches("<Summary>0</Summary>").count(), 3);
    assert!(summary.contains("<OutlineLevel>2</OutlineLevel>"));

    let (_, links) = round_trip("links.xml", LINKS);
    assert_eq!(links.matches("<Summary>1</Summary>").count(), 0);
    assert_eq!(links.matches("<PredecessorLink>").count(), 6);
    assert!(
        links.contains("<LinkLag>4800</LinkLag>"),
        "a lag of 480 minutes, in tenths"
    );

    let (_, constraints) = round_trip("constraints.xml", CONSTRAINTS);
    assert_eq!(
        constraints
            .matches("<ConstraintType>4</ConstraintType>")
            .count(),
        1
    );
    assert!(constraints.contains("<ConstraintDate>2026-01-14T08:00:00</ConstraintDate>"));
    assert!(constraints.contains("<StartDate>2026-01-05T08:00:00</StartDate>"));

    let milestone = LINKS.replace(
        "<Duration>PT8H0M0S</Duration><DurationFormat>7</DurationFormat>",
        "<Duration>PT0S</Duration><Milestone>1</Milestone>",
    );
    let (_, text) = round_trip("links.xml with milestones", &milestone);
    assert!(text.contains("<Milestone>1</Milestone>"), "{text}");
    assert!(text.contains("<Duration>PT0H0M0S</Duration>"));
}

/// A parent with no duration of its own but its own `Start` and `Finish` cannot be a summary: the
/// reader drops a summary's dates. It is written as a task, and says so.
#[test]
fn a_zero_duration_parent_with_its_own_dates_keeps_them() {
    let xml = SUMMARY.replacen(
        "<Summary>1</Summary><Duration>PT0S</Duration>",
        "<Summary>0</Summary><Start>2026-01-05T08:00:00</Start>\
         <Finish>2026-01-06T16:00:00</Finish><Duration>PT0S</Duration>",
        1,
    );
    assert_ne!(xml, SUMMARY, "the fixture's first summary changed");
    let (first, text) = round_trip("summary.xml with a dated parent", &xml);
    assert!(first.tasks[0].file_start.is_some() && first.tasks[0].file_finish.is_some());
    assert_eq!(text.matches("<Summary>1</Summary>").count(), 1, "{text}");
    let left_out = export_mspdi(&first).expect("writable").left_out;
    assert!(
        left_out
            .iter()
            .any(|line| line.contains("task 1") && line.contains("not a summary")),
        "{left_out:?}"
    );
}

/// The reader raises every task to the project start, so a constraint earlier than it cannot
/// come back. The file does not carry it, and `left_out` says so.
#[test]
fn a_constraint_earlier_than_the_project_start_is_listed() {
    let mut first = parse_mspdi(LINKS).expect("fixture imports");
    let start = first.project_start.expect("the fixture states a start");
    first
        .schedule
        .set_snet(engine::TaskId(0), Some(0))
        .expect("task 0 exists");
    let exported = export_mspdi(&first).expect("writable");
    assert!(
        exported
            .left_out
            .iter()
            .any(|line| line.contains("task 1") && line.contains("earlier than the project start")),
        "{:?}",
        exported.left_out
    );
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    assert!(
        !text.contains("<ConstraintType>"),
        "no constraint is written: {text}"
    );
    let second = parse_mspdi(&text).expect("the file reads back");
    assert_eq!(
        second.schedule.tasks()[0].snet,
        Some(start),
        "floored at the start"
    );
}

#[test]
fn a_status_date_is_written_as_the_status_date() {
    let xml = LINKS.replace(
        "<StartDate>2026-01-05T08:00:00</StartDate>",
        "<StartDate>2026-01-05T08:00:00</StartDate><StatusDate>2026-01-08T16:30:00</StatusDate>",
    );
    let (_, text) = round_trip("links.xml with a status date", &xml);
    assert!(
        text.contains("<StatusDate>2026-01-08T16:30:00</StatusDate>"),
        "{text}"
    );
}

#[test]
fn a_started_and_a_finished_task_read_back_with_their_actuals() {
    let xml = LINKS
        .replace(
            "<UID>1</UID><Name>A</Name>",
            "<UID>1</UID><Name>A</Name><ActualStart>2026-01-05T08:00:00</ActualStart>\
<RemainingDuration>PT8H0M0S</RemainingDuration>",
        )
        .replace(
            "<UID>2</UID><Name>B</Name>",
            "<UID>2</UID><Name>B</Name><ActualStart>2026-01-07T08:00:00</ActualStart>\
<ActualFinish>2026-01-09T16:00:00</ActualFinish>",
        );
    let (first, text) = round_trip("links.xml with actuals", &xml);
    assert!(first.tasks[0].in_progress && first.tasks[0].remaining == Some(480));
    let second = parse_mspdi(&text).expect("the file written imports");
    let (started, finished) = (&second.tasks[0], &second.tasks[1]);
    assert!(started.in_progress && started.actual_start.is_some());
    assert_eq!(started.actual_start, first.tasks[0].actual_start);
    assert_eq!(started.remaining, Some(480));
    assert!(!finished.in_progress && finished.actual_start.is_some());
    assert_eq!(finished.actual_finish, first.tasks[1].actual_finish);
    assert!(finished.actual_finish.is_some(), "{text}");
    let left_out = export_mspdi(&first).expect("writable").left_out;
    assert_eq!(left_out, Vec::<String>::new());
}

#[test]
fn an_actual_finish_with_no_actual_start_is_listed() {
    let mut first = parse_mspdi(LINKS).expect("imports");
    first.tasks[0].actual_finish = Some(480);
    let exported = export_mspdi(&first).expect("writable");
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    assert!(!text.contains("<ActualFinish>"), "{text}");
    assert!(
        exported
            .left_out
            .iter()
            .any(|line| line.contains("task 1") && line.contains("actual finish")),
        "{:?}",
        exported.left_out
    );
}

#[test]
fn a_file_with_no_start_date_is_written_from_the_schedule_origin_and_says_so() {
    let xml = LINKS.replace("<StartDate>2026-01-05T08:00:00</StartDate>", "");
    let first = parse_mspdi(&xml).expect("imports");
    assert_eq!(first.project_start, None);
    let exported = export_mspdi(&first).expect("writable");
    assert!(
        exported
            .left_out
            .iter()
            .any(|line| line.contains("project start")),
        "{:?}",
        exported.left_out
    );
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    let second = parse_mspdi(&text).expect("the file reads back");
    assert_eq!(second.origin_day, first.origin_day, "the origin is kept");
}

/// Six tasks of `links.xml`, each with one limit.
fn with_limits() -> Imported {
    let mut imported = parse_mspdi(LINKS).expect("fixture imports");
    let at = |day: i64| day * 1440 + 480;
    let limits = [
        Limit {
            alap: true,
            ..Limit::default()
        },
        Limit {
            pin: Some(Pin::Start(at(2))),
            ..Limit::default()
        },
        Limit {
            pin: Some(Pin::Finish(at(3) + 480)),
            ..Limit::default()
        },
        Limit {
            snlt: Some(at(4)),
            ..Limit::default()
        },
        Limit {
            fnet: Some(at(5) + 480),
            ..Limit::default()
        },
        Limit {
            fnlt: Some(at(6) + 480),
            ..Limit::default()
        },
    ];
    for (index, limit) in (0..).zip(limits) {
        imported
            .schedule
            .set_limit(TaskId(index), limit)
            .expect("the task exists");
    }
    imported
}

/// Each limit is written as its `ConstraintType` and `ConstraintDate` and reads back as the same
/// limit, with nothing left out. As late as possible has no date.
#[test]
fn every_limit_is_written_with_its_type_and_reads_back() {
    let first = with_limits();
    let exported = export_mspdi(&first).expect("writable");
    assert_eq!(exported.left_out, Vec::<String>::new());
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    for (code, date) in [
        ("1", None),
        ("2", Some("2026-01-07T08:00:00")),
        ("3", Some("2026-01-08T16:00:00")),
        ("5", Some("2026-01-09T08:00:00")),
        ("6", Some("2026-01-10T16:00:00")),
        ("7", Some("2026-01-11T16:00:00")),
    ] {
        let kind = format!("<ConstraintType>{code}</ConstraintType>");
        assert_eq!(text.matches(&kind).count(), 1, "{kind}: {text}");
        if let Some(date) = date {
            assert!(text.contains(&format!("<ConstraintDate>{date}</ConstraintDate>")));
        }
    }
    assert_eq!(text.matches("<ConstraintDate>").count(), 5, "{text}");
    let second = parse_mspdi(&text).expect("the written file reads back");
    assert_eq!(signature(&first), signature(&second));
}

/// A soft Start On or Finish On is a pair the file has no type for: the no-earlier-than half is
/// written (4 or 6) and the other half is listed. A pin outranks the other limits.
#[test]
fn a_pair_of_limits_writes_the_no_earlier_than_half_and_lists_the_other() {
    let mut first = parse_mspdi(LINKS).expect("fixture imports");
    let start = first.project_start.expect("the fixture states a start");
    let (late, later) = (start + 2 * 1440, start + 3 * 1440);
    let tasks = [
        (
            late,
            Limit {
                snlt: Some(later),
                ..Limit::default()
            },
        ),
        (
            start,
            Limit {
                fnet: Some(late),
                fnlt: Some(later),
                ..Limit::default()
            },
        ),
        (
            start,
            Limit {
                pin: Some(Pin::Start(late)),
                fnlt: Some(later),
                ..Limit::default()
            },
        ),
    ];
    for (index, (snet, limit)) in (0..).zip(tasks) {
        let id = TaskId(index);
        first
            .schedule
            .set_snet(id, Some(snet))
            .expect("the task exists");
        first
            .schedule
            .set_limit(id, limit)
            .expect("the task exists");
    }
    let exported = export_mspdi(&first).expect("writable");
    let listed = |uid: u32, half: &str| {
        let line = format!("task {uid}: its {half} constraint is not written");
        exported.left_out.iter().any(|out| out.contains(&line))
    };
    assert!(listed(1, "start no later than"), "{:?}", exported.left_out);
    assert!(listed(2, "finish no later than"), "{:?}", exported.left_out);
    assert!(listed(3, "finish no later than"), "{:?}", exported.left_out);
    let text = String::from_utf8(exported.bytes).expect("UTF-8");
    for code in ["4", "6", "2"] {
        let kind = format!("<ConstraintType>{code}</ConstraintType>");
        assert_eq!(text.matches(&kind).count(), 1, "{kind}: {text}");
    }
    let second = parse_mspdi(&text).expect("the written file reads back");
    let tasks = second.schedule.tasks();
    assert_eq!((tasks[0].snet, tasks[0].limit.snlt), (Some(late), None));
    assert_eq!(
        (tasks[1].limit.fnet, tasks[1].limit.fnlt),
        (Some(late), None)
    );
    assert_eq!(tasks[2].limit.pin, Some(Pin::Start(late)));
}
