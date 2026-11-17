//! gs#703: the WBS code of each XER task is the `wbs_short_name` of its WBS path, joined with dots.

use import::{Imported, ProjectSelection, parse_xer};
use std::fmt::Write as _;
use xer::synth::{Spec, write};

const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";
const TASK_FIELDS: &str = "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2";

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    write!(text, "%T\t{name}\n%F\t{fields}\n").expect("a String takes writes");
    for row in rows {
        writeln!(text, "%R\t{row}").expect("a String takes writes");
    }
}

/// A one-project file. `wbs` rows are `wbs_id, parent_wbs_id, seq_num, proj_node_flag,
/// wbs_short_name, wbs_name`; `activities` are `(task_id, task_code, wbs_id)`.
fn import(wbs: &[&str], activities: &[(u32, &str, u32)]) -> Imported {
    let mut text = String::from("ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD\n");
    table(
        &mut text,
        "PROJECT",
        "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id",
        &["1\tHAND\tY\t2024-01-01 08:00\t1"],
    );
    let wbs: Vec<String> = wbs
        .iter()
        .map(|row| row.replacen('\t', "\t1\t", 1))
        .collect();
    let wbs: Vec<&str> = wbs.iter().map(String::as_str).collect();
    table(
        &mut text,
        "PROJWBS",
        "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_short_name\twbs_name",
        &wbs,
    );
    let tasks: Vec<String> = activities
        .iter()
        .map(|(id, code, wbs_id)| {
            format!("{id}\t1\t{wbs_id}\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t24\t24\t\t\t")
        })
        .collect();
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    table(&mut text, "TASK", TASK_FIELDS, &tasks);
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&format!("1\tCA_Project\t{WEEK}")],
    );
    text.push_str("%E\n");
    parse_xer(text.as_bytes(), &ProjectSelection::Unspecified)
        .expect("hand-built XER should import")
}

/// The WBS code of the activity `task_code`.
fn activity<'a>(imported: &'a Imported, task_code: &str) -> Option<&'a str> {
    imported
        .tasks
        .iter()
        .find(|task| task.code.as_deref() == Some(task_code))
        .expect("activity should be imported")
        .wbs_code
        .as_deref()
}

/// The WBS code of the summary named `name`.
fn summary<'a>(imported: &'a Imported, name: &str) -> Option<&'a str> {
    imported
        .tasks
        .iter()
        .find(|task| task.code.is_none() && task.name == name)
        .expect("summary should be imported")
        .wbs_code
        .as_deref()
}

// Wire rows: the project id column is spliced in after `wbs_id` by `import`.
const ROOT: &str = "10\t\t1\tY\tHAND\tRoot";

#[test]
fn a_task_gets_the_short_names_from_the_root_to_its_own_node_joined_with_dots() {
    let imported = import(
        &[
            ROOT,
            "20\t10\t1\tN\t1\tBuilding",
            "30\t20\t1\tN\t2\tFoundation",
            "50\t10\t2\tN\tB\tSite",
        ],
        &[
            (101, "A1", 30),
            (102, "A2", 10),
            (103, "A3", 50),
            (104, "A4", 20),
        ],
    );
    assert_eq!(activity(&imported, "A1"), Some("HAND.1.2"));
    assert_eq!(activity(&imported, "A2"), Some("HAND"), "on the root");
    assert_eq!(activity(&imported, "A3"), Some("HAND.B"));
    assert_eq!(activity(&imported, "A4"), Some("HAND.1"));
    assert_eq!(summary(&imported, "Building"), Some("HAND.1"));
    assert_eq!(summary(&imported, "Foundation"), Some("HAND.1.2"));
    assert_eq!(summary(&imported, "Site"), Some("HAND.B"));
    assert_eq!(summary(&imported, "HAND"), Some("HAND"), "the project root");
}

#[test]
fn a_node_with_no_short_name_adds_no_segment() {
    let imported = import(
        &[ROOT, "20\t10\t1\tN\t\tUnnamed", "30\t20\t1\tN\t7\tSeven"],
        &[(101, "A1", 30), (102, "A2", 20)],
    );
    assert_eq!(activity(&imported, "A1"), Some("HAND.7"));
    assert_eq!(activity(&imported, "A2"), Some("HAND"));
}

#[test]
fn a_project_with_no_real_root_has_no_root_segment() {
    let imported = import(&["20\t\t1\tN\t4\tLone"], &[(101, "A1", 20)]);
    assert_eq!(activity(&imported, "A1"), Some("4"));
    assert_eq!(
        summary(&imported, "HAND"),
        None,
        "the synthetic root has no code"
    );
}

#[test]
fn an_activity_on_an_unknown_wbs_node_takes_the_root_code_and_no_short_names_gives_none() {
    let imported = import(&[ROOT], &[(101, "A1", 999)]);
    assert_eq!(activity(&imported, "A1"), Some("HAND"));
    let imported = import(
        &["10\t\t1\tY\t\tRoot", "20\t10\t1\tN\t\tKid"],
        &[(101, "A1", 20)],
    );
    assert_eq!(activity(&imported, "A1"), None);
    assert!(imported.tasks.iter().all(|task| task.wbs_code.is_none()));
}

#[test]
fn a_parent_cycle_ends_and_still_gives_each_node_a_code() {
    let imported = import(
        &[
            ROOT,
            "30\t40\t2\tN\tC1\tCycle one",
            "40\t30\t3\tN\tC2\tCycle two",
        ],
        &[(101, "A1", 30)],
    );
    assert!(activity(&imported, "A1").is_some_and(|code| code.starts_with("HAND")));
}

#[test]
fn synth_file_gives_every_task_a_code_under_its_root() {
    let spec = Spec::small(7);
    let imported = parse_xer(&write(&spec), &ProjectSelection::Unspecified)
        .expect("synthetic XER should import");
    let root = imported.tasks[0].wbs_code.as_deref().expect("root code");
    assert!(root.starts_with("WBS-"), "{root}");
    for task in &imported.tasks {
        let code = task.wbs_code.as_deref().expect("every task has a code");
        assert!(code.starts_with(root), "{code} under {root}");
    }
    assert!(
        imported.tasks.iter().any(|task| task
            .wbs_code
            .as_deref()
            .is_some_and(|code| code.contains('.'))),
        "some task is below a child node"
    );
}
