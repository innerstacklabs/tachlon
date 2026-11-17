//! gs#819: under one WBS node the activities list by start date, then by activity ID with its
//! digit runs compared as numbers; sibling WBS nodes keep the file's `seq_num` order, and an MS
//! Project file keeps its tasks in file order.

use import::{Imported, ProjectSelection, parse_mspdi, parse_xer};
use std::fmt::Write as _;

const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    write!(text, "%T\t{name}\n%F\t{fields}\n").expect("a String takes writes");
    for row in rows {
        writeln!(text, "%R\t{row}").expect("a String takes writes");
    }
}

/// A one-project file with a root, two work packages (`seq_num` 1 is `WP-B`, 2 is `WP-A`, so the
/// file order is not the name order) and the given `(task_id, task_code, wbs_id, early_start, act_start)`.
fn import(activities: &[(u32, &str, u32, &str, &str)]) -> Imported {
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
        "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_short_name\twbs_name",
        &[
            "10\t1\t\t1\tY\tROOT\tRoot",
            "20\t1\t10\t1\tN\tWP-B\tWP-B",
            "30\t1\t10\t2\tN\tWP-A\tWP-A",
        ],
    );
    let tasks: Vec<String> = activities
        .iter()
        .map(|(id, code, wbs, start, actual)| {
            format!("{id}\t1\t{wbs}\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t24\t24\t{start}\t{actual}")
        })
        .collect();
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    table(
        &mut text,
        "TASK",
        "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tearly_start_date\tact_start_date",
        &tasks,
    );
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&format!("1\tCA_Project\t{WEEK}")],
    );
    text.push_str("%E\n");
    parse_xer(text.as_bytes(), &ProjectSelection::Unspecified).expect("hand-built XER imports")
}

fn names(imported: &Imported) -> Vec<&str> {
    imported
        .tasks
        .iter()
        .map(|task| task.code.as_deref().unwrap_or(task.name.as_str()))
        .collect()
}

#[test]
fn activities_list_by_start_then_by_id_with_digit_runs_as_numbers() {
    let imported = import(&[
        (1, "A1-1001", 20, "2024-12-21 08:00", ""),
        (2, "A1-10", 20, "2024-03-04 08:00", ""),
        (3, "A1-2", 20, "2024-03-04 08:00", ""),
        (4, "A1-201", 20, "2024-07-25 08:00", ""),
        (5, "A1-1", 20, "2024-01-01 08:00", ""),
        (6, "B-9", 30, "2024-05-01 08:00", ""),
        (7, "B-1", 30, "2024-06-01 08:00", ""),
    ]);
    // WP-B stays before WP-A (file `seq_num` order); inside each, start then numeric ID.
    assert_eq!(
        names(&imported),
        [
            "HAND", "WP-B", "A1-1", "A1-2", "A1-10", "A1-201", "A1-1001", "WP-A", "B-9", "B-1"
        ]
    );
}

#[test]
fn an_activity_without_a_start_date_lists_last_by_id() {
    let imported = import(&[
        (1, "A-10", 20, "", ""),
        (2, "A-2", 20, "", ""),
        (3, "A-30", 20, "2024-09-02 08:00", ""),
    ]);
    assert_eq!(names(&imported)[2..], ["A-30", "A-2", "A-10"]);
}

#[test]
fn an_ms_project_file_keeps_its_tasks_in_file_order() {
    let task = |uid: u32, id: u32, name: &str, start: &str| {
        format!(
            "<Task><UID>{uid}</UID><ID>{id}</ID><Name>{name}</Name><OutlineLevel>1</OutlineLevel>\
             <Start>{start}</Start><Duration>PT8H0M0S</Duration></Task>"
        )
    };
    let xml = format!(
        "<Project xmlns=\"http://schemas.microsoft.com/project\"><StartDate>2024-01-01T08:00:00</StartDate>\
         <Tasks>{}{}{}</Tasks></Project>",
        task(1, 1, "T-10", "2024-12-02T08:00:00"),
        task(2, 2, "T-2", "2024-01-02T08:00:00"),
        task(3, 3, "T-1", "2024-06-03T08:00:00"),
    );
    let imported = parse_mspdi(&xml).expect("MSPDI imports");
    assert_eq!(names(&imported), ["T-10", "T-2", "T-1"]);
}

#[test]
fn a_started_activity_lists_by_its_actual_start_not_its_restart() {
    // A progressed export: the started activity's early start is its remaining work near the data
    // date, while the actual start is where P6 shows it.
    let imported = import(&[
        (1, "A-1", 20, "2024-06-03 08:00", ""),
        (2, "A-2", 20, "2024-09-02 08:00", "2024-01-08 08:00"),
        (3, "A-3", 20, "2024-04-01 08:00", ""),
    ]);
    assert_eq!(names(&imported)[2..], ["A-2", "A-3", "A-1"]);
}
