//! gs#703: the XER writer's links and lag calendar. A written file reads back with the same
//! links (type and lag), the same lag calendar, and no link to a deleted task.

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
use engine::{CalendarId, LagCalendar, LinkType, TaskId};
use import::{Imported, ProjectSelection, XerLayout, XerOptions, export_xer, parse_xer};

const EXPORTED_AT: i64 = 19_787 * 1_440;

fn options() -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: "Exported".to_owned(),
        layout: XerLayout::FromXer,
    }
}

/// Five activities, one link of each type and a fifth with a negative lag. Lags are in hours.
fn five() -> Imported {
    let tasks: Vec<String> = ["A", "B", "C", "D", "E"]
        .iter()
        .zip(1..)
        .map(|(code, id)| regular_task(id, code, "8"))
        .collect();
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    let links = [
        "2\t1\tPR_FS\t16",
        "3\t1\tPR_SS\t1.5",
        "4\t2\tPR_FF\t0",
        "5\t3\tPR_SF\t-8",
    ];
    parse_xer(
        &handbuilt("2024-03-04 08:00", &tasks, &links, WEEK),
        &ProjectSelection::Unspecified,
    )
    .expect("hand-built XER should import")
}

fn read_back(imported: &Imported) -> Imported {
    let exported = export_xer(imported, &options()).expect("export should work");
    parse_xer(&exported.bytes, &ProjectSelection::All).expect("the written file should import")
}

/// The live links as (predecessor code, successor code, type, lag in minutes), sorted.
fn links(imported: &Imported) -> Vec<(String, String, LinkType, i64)> {
    let code = |id: TaskId| {
        imported.tasks[id.0 as usize]
            .code
            .clone()
            .unwrap_or_default()
    };
    let mut found: Vec<_> = imported
        .schedule
        .live_dependencies()
        .map(|(_, link)| (code(link.pred), code(link.succ), link.kind, link.lag))
        .collect();
    found.sort_by_key(|(pred, succ, _, _)| (pred.clone(), succ.clone()));
    found
}

#[test]
fn every_link_type_with_lag_reads_back() {
    let imported = five();
    let expected = vec![
        ("A".to_owned(), "B".to_owned(), LinkType::FS, 960),
        ("A".to_owned(), "C".to_owned(), LinkType::SS, 90),
        ("B".to_owned(), "D".to_owned(), LinkType::FF, 0),
        ("C".to_owned(), "E".to_owned(), LinkType::SF, -480),
    ];
    assert_eq!(links(&imported), expected);

    let again = read_back(&imported);

    assert_eq!(links(&again), expected);
}

#[test]
fn the_lag_calendar_option_reads_back() {
    for choice in [
        LagCalendar::Successor,
        LagCalendar::Predecessor,
        LagCalendar::TwentyFourHour,
        LagCalendar::Fixed(CalendarId(0)),
    ] {
        let mut imported = five();
        imported
            .schedule
            .set_lag_calendar(choice)
            .expect("the lag calendar should set");

        let again = read_back(&imported);

        assert_eq!(again.schedule.lag_calendar(), choice, "{choice:?}");
    }
}

#[test]
fn a_link_with_a_deleted_end_is_not_written() {
    let mut imported = five();
    // C is the end of two links.
    let c = imported
        .tasks
        .iter()
        .position(|task| task.code.as_deref() == Some("C"))
        .expect("C should be imported");
    imported
        .schedule
        .delete_task(TaskId(c as u32))
        .expect("C should delete");

    let exported = export_xer(&imported, &options()).expect("export should work");

    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let rows = parsed.table("TASKPRED").map(xer::Table::len);
    assert_eq!(rows, Some(2), "only A-B and B-D remain");
    let again = parse_xer(&exported.bytes, &ProjectSelection::All).expect("import");
    let codes: Vec<_> = links(&again)
        .into_iter()
        .map(|(pred, succ, _, _)| (pred, succ))
        .collect();
    assert_eq!(
        codes,
        vec![
            ("A".to_owned(), "B".to_owned()),
            ("B".to_owned(), "D".to_owned())
        ]
    );
}
