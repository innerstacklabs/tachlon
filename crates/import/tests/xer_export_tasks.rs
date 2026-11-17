//! gs#703: the XER writer's WBS and task identity. A written file reads back with the same task
//! names, codes, outline levels and WBS codes, and a file with two projects comes back as two.

use engine::{Calendar, Schedule, Task};
use import::{
    Imported, ImportedTask, ProjectSelection, XerLayout, XerOptions, export_xer, parse_xer,
};
use xer::synth::{Spec, write};

const EXPORTED_AT: i64 = 19_787 * 1_440;

fn options(layout: XerLayout) -> XerOptions {
    XerOptions {
        exported_at: EXPORTED_AT,
        name: "Exported".to_owned(),
        layout,
    }
}

/// What a task shows after a round trip: name, code, outline level and WBS code.
type Shown = (String, Option<String>, u8, Option<String>);

fn shown(imported: &Imported) -> Vec<Shown> {
    imported
        .tasks
        .iter()
        .map(|task| {
            (
                task.name.clone(),
                task.code.clone(),
                task.outline_level,
                task.wbs_code.clone(),
            )
        })
        .collect()
}

fn read_back(imported: &Imported, layout: XerLayout) -> Imported {
    let exported = export_xer(imported, &options(layout)).expect("export should work");
    parse_xer(&exported.bytes, &ProjectSelection::All).expect("the written file should import")
}

#[test]
fn activities_and_summaries_read_back_with_names_codes_levels_and_wbs_codes() {
    let bytes = write(&Spec::small(7));
    let imported = parse_xer(&bytes, &ProjectSelection::Unspecified).expect("synth should import");
    assert!(imported.tasks.iter().any(|task| task.code.is_some()));
    assert!(imported.tasks.iter().any(|task| task.code.is_none()));

    let again = read_back(&imported, XerLayout::FromXer);

    assert_eq!(by_code(&again), by_code(&imported));
}

/// The rows sorted by WBS code and activity code. The written file carries each started task's
/// actual start and no early dates, so the reader lists a started activity first in its node
/// (gs#819): compare the rows by code, not by position.
fn by_code(imported: &Imported) -> Vec<Shown> {
    let mut rows = shown(imported);
    rows.sort_by(|a, b| (&a.3, &a.1, &a.0).cmp(&(&b.3, &b.1, &b.0)));
    rows
}

#[test]
fn two_projects_come_back_as_two_projects() {
    let bytes = include_bytes!("../../../packages/gantt/tests/fixtures/two-projects.xer");
    let imported = parse_xer(bytes, &ProjectSelection::All).expect("fixture should import");
    let roots = |imported: &Imported| -> Vec<String> {
        imported
            .tasks
            .iter()
            .filter(|task| task.outline_level == 1)
            .map(|task| task.name.clone())
            .collect()
    };
    assert_eq!(roots(&imported).len(), 2);

    let exported = export_xer(&imported, &options(XerLayout::FromXer)).expect("export");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");
    assert_eq!(data.project_rows.len(), 2);

    let again = parse_xer(&exported.bytes, &ProjectSelection::All).expect("import");
    assert_eq!(roots(&again), roots(&imported));
    assert_eq!(by_code(&again), by_code(&imported));
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
fn a_level_1_leaf_joins_the_project_before_it() {
    let imported = build(vec![
        meta("First", None, 1),
        meta("Dig", Some("A1"), 2),
        meta("Second", None, 1),
        meta("Pour", Some("B1"), 2),
        meta("Cure", Some("B2"), 1),
    ]);

    let exported = export_xer(&imported, &options(XerLayout::FromXer)).expect("export");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");
    let root_of = |proj_id| {
        data.wbs_rows
            .iter()
            .find(|row| row.proj_id == proj_id && row.proj_node_flag == Some(true))
            .map(|row| row.wbs_id)
    };
    let cure = data
        .task_rows
        .iter()
        .find(|row| row.task_code.as_deref() == Some("B2"))
        .expect("the leaf should be written");
    assert_eq!(cure.proj_id, data.project_rows[1].proj_id);
    assert_eq!(Some(cure.wbs_id), root_of(cure.proj_id));

    let again = read_back(&imported, XerLayout::FromXer);

    let names: Vec<(&str, u8)> = again
        .tasks
        .iter()
        .map(|task| (task.name.as_str(), task.outline_level))
        .collect();
    assert_eq!(
        names,
        [
            ("First", 1),
            ("Dig", 2),
            ("Second", 1),
            ("Pour", 2),
            ("Cure", 2)
        ]
    );
}

#[test]
fn a_leaf_without_an_activity_id_is_filled_with_a_unique_one() {
    let bytes = write(&Spec::small(7));
    let mut imported =
        parse_xer(&bytes, &ProjectSelection::Unspecified).expect("synth should import");
    for task in &mut imported.tasks {
        task.code = None;
    }
    // One leaf keeps its code, so a filled one must step past it.
    let leaves: Vec<usize> = (0..imported.tasks.len())
        .filter(|&index| {
            imported
                .tasks
                .get(index + 1)
                .is_none_or(|next| next.outline_level <= imported.tasks[index].outline_level)
        })
        .collect();
    imported.tasks[leaves[1]].code = Some("A1000".to_owned());

    let again = read_back(&imported, XerLayout::FromXer);

    let codes: Vec<&str> = again
        .tasks
        .iter()
        .filter_map(|task| task.code.as_deref())
        .collect();
    assert_eq!(codes.len(), leaves.len());
    let mut unique = codes.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), codes.len());
    // The first leaf has no code before it, so it steps past the taken A1000; the third follows
    // the second (A1000) and steps past the largest.
    assert!(codes.contains(&"A1010"));
    assert!(codes.contains(&"A1011"));
    // Nothing is stored back.
    assert_eq!(
        imported.tasks.iter().filter(|t| t.code.is_some()).count(),
        1
    );
}
