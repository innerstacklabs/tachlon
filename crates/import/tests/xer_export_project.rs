//! The XER writer's frame: the header and the one `PROJECT` row come from the options, and the
//! file is a pure function of its inputs (gs#703).

use engine::{Calendar, Schedule, Task};
use import::{Imported, XerLayout, XerOptions, export_xer};

/// 2024-03-05 14:30 as minutes since 1970-01-01.
const EXPORTED_AT: i64 = 19_787 * 1_440 + 14 * 60 + 30;

fn week() -> [Vec<(u16, u16)>; 7] {
    let day = vec![(480, 960)];
    [
        day.clone(),
        day.clone(),
        day.clone(),
        day.clone(),
        day,
        Vec::new(),
        Vec::new(),
    ]
}

/// Two calendars, and one task on the second.
fn imported() -> Imported {
    let mut schedule = Schedule::new();
    schedule.add_calendar(Calendar::with_exceptions(week(), &[]).expect("calendar should build"));
    let second = schedule
        .add_calendar(Calendar::with_exceptions(week(), &[]).expect("calendar should build"));
    schedule
        .add_task(Task {
            duration: 480,
            calendar: second,
            snet: None,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        })
        .expect("task should add");
    Imported {
        schedule,
        tasks: Vec::new(),
        unsupported: Vec::new(),
        data_date: None,
        origin_day: 19_723,
        project_start: None,
    }
}

fn options(name: &str) -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: name.to_owned(),
        layout: XerLayout::One,
    }
}

#[test]
fn a_written_file_has_one_project_row_named_from_the_options() {
    let exported = export_xer(&imported(), &options("Tower block")).expect("export should work");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(data.project_rows.len(), 1);
    let project = &data.project_rows[0];
    assert_eq!(project.proj_short_name.as_deref(), Some("Tower block"));
    // The first task's calendar, so the reader adds none the tasks do not use.
    assert_eq!(project.clndr_id, Some(2));
}

#[test]
fn the_header_carries_the_export_time_from_the_options() {
    let exported = export_xer(&imported(), &options("P")).expect("export should work");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(data.header.version.as_deref(), Some("19.12"));
    assert_eq!(data.header.export_date, Some(EXPORTED_AT));

    let later = XerOptions {
        exported_at: EXPORTED_AT + 1,
        ..options("P")
    };
    let again = export_xer(&imported(), &later).expect("export should work");
    assert_ne!(again.bytes, exported.bytes);
    let same = export_xer(&imported(), &options("P")).expect("export should work");
    assert_eq!(same.bytes, exported.bytes);
}
