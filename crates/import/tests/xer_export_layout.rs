//! gs#703: the XER writer's layout. `XerLayout::One` lays every task out as one project named from
//! the options, even when the tasks hold several level-1 summaries, and a deleted task is left out
//! of the file.

use engine::{Calendar, Schedule, Task, TaskId};
use import::{Imported, ImportedTask, XerLayout, XerOptions, export_xer};

const EXPORTED_AT: i64 = 19_787 * 1_440;

fn options(name: &str) -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: name.to_owned(),
        layout: XerLayout::One,
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

fn meta(name: &str, code: Option<&str>, level: u8) -> ImportedTask {
    ImportedTask {
        uid: 0,
        code: code.map(str::to_owned),
        name: name.to_owned(),
        outline_level: level,
        parent: None,
        file_start: None,
        file_finish: None,
        in_progress: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
        baseline_start: None,
        baseline_finish: None,
        progress: None,
        physical: None,
        activity_codes: Vec::new(),
        wbs_code: None,
        floor: None,
        deadline: None,
        resource_names: Vec::new(),
    }
}

/// A schedule of tasks with the given metadata, all on one calendar.
fn build(tasks: Vec<ImportedTask>) -> Imported {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(calendar());
    for task in &tasks {
        schedule
            .add_task(Task {
                duration: if task.code.is_some() { 480 } else { 0 },
                calendar,
                snet: None,
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
        tasks,
        unsupported: Vec::new(),
        data_date: None,
        origin_day: 19_723,
        project_start: None,
    }
}

#[test]
fn one_layout_writes_one_project_for_two_level_1_summaries() {
    let imported = build(vec![
        meta("First", None, 1),
        meta("Dig", Some("A1"), 2),
        meta("Second", None, 1),
        meta("Pour", Some("B1"), 2),
    ]);

    let exported = export_xer(&imported, &options("Whole")).expect("export should work");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(data.project_rows.len(), 1);
    let project = &data.project_rows[0];
    assert_eq!(project.proj_short_name.as_deref(), Some("Whole"));
    let roots = data
        .wbs_rows
        .iter()
        .filter(|row| row.proj_node_flag == Some(true))
        .count();
    assert_eq!(roots, 1);
    assert_eq!(data.task_rows.len(), 2);
    assert!(
        data.task_rows
            .iter()
            .all(|row| row.proj_id == project.proj_id)
    );
}

#[test]
fn a_deleted_task_is_left_out_of_the_file() {
    let mut imported = build(vec![
        meta("Dig", Some("A1"), 1),
        meta("Pour", Some("A2"), 1),
        meta("Cure", Some("A3"), 1),
    ]);
    imported
        .schedule
        .delete_task(TaskId(1))
        .expect("task should delete");

    let exported = export_xer(&imported, &options("Whole")).expect("export should work");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    let names: Vec<&str> = data
        .task_rows
        .iter()
        .filter_map(|row| row.task_name.as_deref())
        .collect();
    assert_eq!(names, ["Dig", "Cure"]);
    assert!(!names.contains(&"Pour"));
}
