//! gs#703: the XER writer's task values. A written file reads back with every duration to the
//! minute, the start-no-earlier-than constraints, the project start, the percent complete and the
//! data date; what the file cannot carry is named in `left_out`.

use engine::{Calendar, Limit, Pin, Schedule, Task, TaskId};
use import::{
    Imported, ImportedTask, ProjectSelection, XerLayout, XerOptions, export_xer, parse_xer,
};
use xer::ConstraintType;

const EXPORTED_AT: i64 = 19_787 * 1_440;
/// 2024-01-01, a Monday.
const ORIGIN_DAY: i64 = 19_723;

/// One leaf task: its code, duration in minutes, constraint and percent complete.
struct Leaf {
    code: &'static str,
    duration: i64,
    snet: Option<i64>,
    progress: Option<u8>,
}

fn leaf(code: &'static str, duration: i64, snet: Option<i64>) -> Leaf {
    Leaf {
        code,
        duration,
        snet,
        progress: None,
    }
}

fn calendar() -> Calendar {
    let day = vec![(480, 960)];
    let week = [
        day.clone(),
        day.clone(),
        day.clone(),
        day.clone(),
        day,
        Vec::new(),
        Vec::new(),
    ];
    Calendar::with_exceptions(week, &[]).expect("calendar should build")
}

fn meta(leaf: &Leaf) -> ImportedTask {
    ImportedTask {
        uid: 0,
        code: Some(leaf.code.to_owned()),
        name: leaf.code.to_owned(),
        outline_level: 1,
        parent: None,
        file_start: None,
        file_finish: None,
        in_progress: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
        baseline_start: None,
        baseline_finish: None,
        progress: leaf.progress,
        physical: None,
        activity_codes: Vec::new(),
        wbs_code: None,
        floor: None,
        deadline: None,
        resource_names: Vec::new(),
    }
}

fn build(leaves: &[Leaf], project_start: Option<i64>, data_date: Option<i64>) -> Imported {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar());
    for leaf in leaves {
        schedule
            .add_task(Task {
                duration: leaf.duration,
                calendar,
                snet: leaf.snet,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("task should add");
    }
    Imported {
        schedule,
        tasks: leaves.iter().map(meta).collect(),
        unsupported: Vec::new(),
        data_date,
        origin_day: ORIGIN_DAY,
        project_start,
    }
}

fn export(imported: &Imported) -> import::Exported {
    let options = XerOptions {
        exported_at: EXPORTED_AT,
        name: "Exported".to_owned(),
        layout: XerLayout::One,
    };
    export_xer(imported, &options).expect("export should work")
}

fn read_back(imported: &Imported) -> Imported {
    parse_xer(&export(imported).bytes, &ProjectSelection::All)
        .expect("the written file should import")
}

/// The index of the task with `code`.
fn position(imported: &Imported, code: &str) -> usize {
    imported
        .tasks
        .iter()
        .position(|task| task.code.as_deref() == Some(code))
        .expect("the task should be there")
}

#[test]
fn durations_read_back_to_the_minute() {
    // 1 and 7 minutes are not whole hundredths of an hour; 2 400 is five working days.
    let imported = build(
        &[
            leaf("A1", 1, None),
            leaf("A2", 7, None),
            leaf("A3", 480, None),
            leaf("A4", 2_400, None),
            leaf("A5", 0, None),
        ],
        None,
        None,
    );

    let again = read_back(&imported);

    for (code, minutes) in [("A1", 1), ("A2", 7), ("A3", 480), ("A4", 2_400), ("A5", 0)] {
        let task = &again.schedule.tasks()[position(&again, code)];
        assert_eq!(task.duration, minutes, "{code}");
    }
}

#[test]
fn a_start_no_earlier_than_reads_back_as_the_same_constraint() {
    let imported = build(
        &[leaf("A1", 480, None), leaf("A2", 480, Some(2_880 + 480))],
        Some(0),
        None,
    );

    let again = read_back(&imported);

    let snet = |code: &str| again.schedule.tasks()[position(&again, code)].snet;
    assert_eq!(snet("A2"), Some(2_880 + 480));
    assert_eq!(snet("A1"), Some(0));
}

#[test]
fn an_activity_at_the_project_start_gets_no_constraint_row() {
    let imported = build(
        &[leaf("A1", 480, Some(0)), leaf("A2", 480, Some(1_440))],
        Some(0),
        None,
    );
    let bytes = export(&imported).bytes;
    let parsed = xer::parse(&bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    let row = |code: &str| {
        data.task_rows
            .iter()
            .find(|row| row.task_code.as_deref() == Some(code))
            .expect("the task should be written")
    };
    assert!(row("A1").cstr_type.is_none());
    assert!(row("A1").cstr_date.is_none());
    assert!(row("A2").cstr_type.is_some());
    assert_eq!(row("A2").cstr_date, Some(ORIGIN_DAY * 1_440 + 1_440));
}

#[test]
fn a_finish_pin_and_a_second_limit_write_both_constraints_and_read_back() {
    let (pin, fnet) = (4_320 + 1_020, 2_880 + 1_020);
    let limit = Limit {
        pin: Some(Pin::Finish(pin)),
        fnet: Some(fnet),
        ..Limit::default()
    };
    let mut imported = build(&[leaf("A1", 480, None)], Some(0), None);
    imported
        .schedule
        .set_limit(TaskId(0), limit)
        .expect("the limit should set");
    let exported = export(&imported);
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    let row = &data.task_rows[0];
    assert_eq!(row.cstr_type, Some(ConstraintType::MandatoryFinish));
    assert_eq!(row.cstr_date, Some(ORIGIN_DAY * 1_440 + pin));
    assert_eq!(row.cstr_type2, Some(ConstraintType::FinishOnOrAfter));
    assert_eq!(row.cstr_date2, Some(ORIGIN_DAY * 1_440 + fnet));
    assert_eq!(exported.left_out, Vec::<String>::new());
    let again = read_back(&imported);
    assert_eq!(again.schedule.tasks()[position(&again, "A1")].limit, limit);
}

#[test]
fn a_given_project_start_is_written_as_plan_start_date() {
    // Tuesday 09:00, later than the origin Monday and than the constraint on A1.
    let start = 1_440 + 540;
    let imported = build(&[leaf("A1", 480, Some(60))], Some(start), None);
    let bytes = export(&imported).bytes;
    let parsed = xer::parse(&bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(
        data.project_rows[0].plan_start_date,
        Some(ORIGIN_DAY * 1_440 + start)
    );
    let again = read_back(&imported);
    assert_eq!(again.origin_day, ORIGIN_DAY);
    assert_eq!(
        again.schedule.tasks()[position(&again, "A1")].snet,
        Some(start)
    );
}

#[test]
fn a_missing_project_start_is_the_earliest_constraint() {
    let imported = build(
        &[
            leaf("A1", 480, Some(4_320 + 540)),
            leaf("A2", 480, Some(2_880 + 540)),
            leaf("A3", 480, None),
        ],
        None,
        None,
    );
    let bytes = export(&imported).bytes;
    let parsed = xer::parse(&bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(
        data.project_rows[0].plan_start_date,
        Some(ORIGIN_DAY * 1_440 + 2_880 + 540)
    );
    let none = build(&[leaf("A1", 480, None)], None, None);
    let bytes = export(&none).bytes;
    let parsed = xer::parse(&bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");
    assert_eq!(
        data.project_rows[0].plan_start_date,
        Some(ORIGIN_DAY * 1_440)
    );
}

#[test]
fn percent_complete_and_the_data_date_read_back() {
    let mut leaves = [leaf("A1", 480, None), leaf("A2", 480, None)];
    leaves[0].progress = Some(40);
    let imported = build(&leaves, Some(0), Some(1_440 + 480));

    let again = read_back(&imported);

    assert_eq!(again.tasks[position(&again, "A1")].progress, Some(40));
    assert_eq!(again.tasks[position(&again, "A2")].progress, None);
    assert_eq!(again.data_date, Some(1_440 + 480));
}

#[test]
fn a_started_task_reads_back_started_with_its_actual_start_and_remaining() {
    let mut imported = build(
        &[leaf("A1", 480, None), leaf("A2", 480, None)],
        Some(0),
        None,
    );
    imported.tasks[1].in_progress = true;
    imported.tasks[1].actual_start = Some(0);
    imported.tasks[1].remaining = Some(120);

    assert_eq!(export(&imported).left_out, Vec::<String>::new());
    let again = read_back(&imported);

    let (started, waiting) = (
        &again.tasks[position(&again, "A2")],
        &again.tasks[position(&again, "A1")],
    );
    assert!(started.in_progress);
    assert_eq!(started.actual_start, Some(0));
    assert_eq!(started.remaining, Some(120));
    assert!(!waiting.in_progress && waiting.actual_start.is_none());
}
