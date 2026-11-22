//! gs#703: the XER writer's `PROJWBS` rows. Each summary is written with a short name, a
//! sequence number among its siblings, its parent and the project-node flag.

use engine::{Calendar, Schedule, Task};
use import::{Imported, ImportedTask, XerLayout, XerOptions, export_xer};

const EXPORTED_AT: i64 = 19_787 * 1_440;

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

fn meta(name: &str, code: Option<&str>, wbs_code: Option<&str>, level: u8) -> ImportedTask {
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
        wbs_code: wbs_code.map(str::to_owned),
        floor: None,
        deadline: None,
        resource_names: Vec::new(),
    }
}

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
fn each_wbs_row_carries_its_short_name_sequence_parent_and_root_flag() {
    let imported = build(vec![
        meta("Site", None, Some("1"), 1),
        meta("Civil", None, Some("1.2"), 2),
        meta("Dig", Some("A1"), None, 3),
        meta("Elsewhere", None, Some("9.9"), 2),
        meta("Pour", Some("B1"), None, 3),
    ]);
    let options = XerOptions {
        exported_at: EXPORTED_AT,
        name: "Exported".to_owned(),
        layout: XerLayout::FromXer,
    };

    let exported = export_xer(&imported, &options).expect("export");
    let parsed = xer::parse(&exported.bytes).expect("the file should parse");
    let data = xer::read(&parsed).expect("the file should read");

    assert_eq!(data.wbs_rows.len(), 3);
    let row = |name: &str| {
        data.wbs_rows
            .iter()
            .find(|row| row.wbs_name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("a PROJWBS row named {name}"))
    };
    let (root, civil, elsewhere) = (row("Site"), row("Civil"), row("Elsewhere"));

    assert_eq!(root.parent_wbs_id, None);
    assert_eq!(root.proj_node_flag, Some(true));
    assert_eq!(root.wbs_short_name.as_deref(), Some("1"));

    // The parent's code `1` is a prefix of `1.2`, so only the part after it is the short name.
    assert_eq!(civil.wbs_short_name.as_deref(), Some("2"));
    assert_eq!(civil.seq_num, Some(1));
    assert_eq!(civil.parent_wbs_id, Some(root.wbs_id));
    assert_eq!(civil.proj_node_flag, Some(false));

    // `9.9` is not under `1`, so the whole code is the short name.
    assert_eq!(elsewhere.wbs_short_name.as_deref(), Some("9.9"));
    assert_eq!(elsewhere.seq_num, Some(2));
    assert_eq!(elsewhere.parent_wbs_id, Some(root.wbs_id));
    assert_eq!(elsewhere.proj_node_flag, Some(false));
}
