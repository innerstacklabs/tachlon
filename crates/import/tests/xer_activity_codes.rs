//! gs#714: the P6 activity codes of each XER activity reach the imported task.

use import::{ActivityCode, Imported, ProjectSelection, parse_xer};
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

/// Three activities (A1, A2, A3 = tasks 101, 102, 103). `actv` is the `TASKACTV` rows as
/// `task_id\tactv_code_type_id\tactv_code_id`; the file has two code types, Discipline (7001) and
/// Phase (7002), and four codes.
fn file(actv: &[&str], definitions: bool) -> Vec<u8> {
    let mut text = String::from("ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD\n");
    table(
        &mut text,
        "PROJECT",
        "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id",
        &["1\tHAND\tY\t2024-01-01 08:00\t1"],
    );
    table(
        &mut text,
        "PROJWBS",
        "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name",
        &["10\t1\t\t1\tY\tRoot"],
    );
    let tasks: Vec<String> = ["A1", "A2", "A3"]
        .iter()
        .enumerate()
        .map(|(index, code)| {
            format!(
                "{}\t1\t10\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t24\t24\t\t\t",
                101 + index
            )
        })
        .collect();
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    table(&mut text, "TASK", TASK_FIELDS, &tasks);
    if definitions {
        definition_tables(&mut text);
    }
    table(
        &mut text,
        "TASKACTV",
        "task_id\tactv_code_type_id\tactv_code_id",
        actv,
    );
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&format!("1\tCA_Project\t{WEEK}")],
    );
    text.push_str("%E\n");
    text.into_bytes()
}

fn definition_tables(text: &mut String) {
    table(
        text,
        "ACTVTYPE",
        "actv_code_type_id\tactv_short_len\tseq_num\tactv_code_type\tactv_code_type_scope",
        &[
            "7001\t8\t1\tDiscipline\tAS_Global",
            "7002\t8\t2\tPhase\tAS_Global",
        ],
    );
    table(
        text,
        "ACTVCODE",
        "actv_code_id\tparent_actv_code_id\tactv_code_type_id\tactv_code_name\tshort_name\tseq_num",
        &[
            "7101\t\t7001\tElectrical\tELEC\t1",
            "7102\t\t7001\tCivil works\t\t2",
            "7201\t\t7002\tPhase one\tP1\t1",
        ],
    );
}

fn import(actv: &[&str]) -> Imported {
    parse_xer(&file(actv, true), &ProjectSelection::Unspecified)
        .expect("hand-built XER should import")
}

fn code(code_type: &str, value: &str) -> ActivityCode {
    ActivityCode {
        code_type: code_type.to_owned(),
        value: value.to_owned(),
    }
}

fn codes<'a>(imported: &'a Imported, task_code: &str) -> &'a [ActivityCode] {
    &imported
        .tasks
        .iter()
        .find(|task| task.code.as_deref() == Some(task_code))
        .expect("activity should be imported")
        .activity_codes
}

#[test]
fn each_activity_gets_its_codes_of_two_types_and_an_unassigned_one_gets_none() {
    let imported = import(&["101\t7001\t7101", "101\t7002\t7201", "102\t7001\t7102"]);
    assert_eq!(
        codes(&imported, "A1"),
        [code("Discipline", "ELEC"), code("Phase", "P1")]
    );
    // No short name: the code's full name is its value.
    assert_eq!(codes(&imported, "A2"), [code("Discipline", "Civil works")]);
    assert_eq!(codes(&imported, "A3"), []);
    let summary = imported.tasks.iter().find(|task| task.code.is_none());
    assert!(summary.is_some_and(|task| task.activity_codes.is_empty()));
    assert!(
        !imported
            .unsupported
            .iter()
            .any(|line| line.contains("activity code")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn an_assignment_naming_a_code_the_file_lacks_is_dropped_and_noted() {
    let imported = import(&["101\t7001\t7101", "102\t7001\t7999", "103\t7009\t7101"]);
    assert_eq!(codes(&imported, "A1"), [code("Discipline", "ELEC")]);
    assert_eq!(codes(&imported, "A2"), []);
    assert_eq!(codes(&imported, "A3"), []);
    assert!(
        imported.unsupported.iter().any(|line| line
            .starts_with("Activity codes whose code or code type is not in the file were not read; no dates are affected")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn assignments_without_any_definition_tables_are_dropped_and_noted() {
    let imported = parse_xer(
        &file(&["101\t7001\t7101", "102\t7001\t7102"], false),
        &ProjectSelection::Unspecified,
    )
    .expect("hand-built XER should import");
    assert!(
        imported
            .tasks
            .iter()
            .all(|task| task.activity_codes.is_empty())
    );
    assert!(
        imported.unsupported.iter().any(|line| line
            .starts_with("Activity codes whose code or code type is not in the file were not read; no dates are affected")),
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn a_file_with_no_code_rows_gives_every_task_no_codes() {
    let imported = import(&[]);
    assert!(
        imported
            .tasks
            .iter()
            .all(|task| task.activity_codes.is_empty())
    );
}

#[test]
fn synth_file_gives_every_activity_one_discipline_code() {
    let spec = Spec::small(7);
    let imported = parse_xer(&write(&spec), &ProjectSelection::Unspecified)
        .expect("synthetic XER should import");
    let mut seen = std::collections::BTreeSet::new();
    let mut activities = 0;
    for task in &imported.tasks {
        if task.code.is_none() {
            assert!(task.activity_codes.is_empty(), "a summary has no codes");
            continue;
        }
        activities += 1;
        let [only] = task.activity_codes.as_slice() else {
            panic!("expected one code, got {:?}", task.activity_codes);
        };
        assert_eq!(only.code_type, "Discipline");
        seen.insert(only.value.clone());
    }
    assert_eq!(activities, spec.counts().activities);
    let expected: std::collections::BTreeSet<String> = (0..6).map(|n| format!("D{n}")).collect();
    assert_eq!(seen, expected);
}
