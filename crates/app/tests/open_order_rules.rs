//! gs#865: the rules that break ties in the opened order. The start sort uses the solved start;
//! activities that start together list by activity ID with digit runs as numbers, and sibling WBS
//! nodes keep the file's `seq_num` order, not name order.

use crate::common;

use app::load_bytes;
use common::clock;
use import::ProjectSelection;
use std::fmt::Write as _;

const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    let _ = write!(text, "%T\t{name}\n%F\t{fields}\n");
    for row in rows {
        let _ = writeln!(text, "%R\t{row}");
    }
}

/// The outline of a file with a root, `WP-B` (`seq_num` 1) and `WP-A` (`seq_num` 2), and the
/// given `(task_code, wbs_id)` activities, all with no links and one file start, so every activity
/// in a node starts together. Each row is the activity code, or the WBS node's name.
fn rows(activities: &[(&str, u32)]) -> Vec<String> {
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
        .enumerate()
        .map(|(index, (code, wbs))| {
            format!(
                "{}\t1\t{wbs}\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t24\t24\t2024-01-02 08:00",
                index + 1
            )
        })
        .collect();
    let tasks: Vec<&str> = tasks.iter().map(String::as_str).collect();
    table(
        &mut text,
        "TASK",
        "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tearly_start_date",
        &tasks,
    );
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&format!("1\tCA_Project\t{WEEK}")],
    );
    text.push_str("%E\n");
    let opened =
        load_bytes(text.as_bytes(), &ProjectSelection::Unspecified, &clock).expect("loads");
    opened
        .outline
        .preorder()
        .iter()
        .map(|task| {
            let meta = &opened.meta[task.0 as usize];
            meta.code.clone().unwrap_or_else(|| meta.name.clone())
        })
        .collect()
}

#[test]
fn activities_that_start_together_list_by_id_with_digit_runs_as_numbers() {
    let listed = rows(&[("A1-10", 20), ("A1-2", 20), ("A1-1001", 20), ("A1-201", 20)]);
    assert_eq!(listed[2..], ["A1-2", "A1-10", "A1-201", "A1-1001"]);
}

#[test]
fn sibling_wbs_nodes_keep_the_files_seq_num_order_not_name_order() {
    let listed = rows(&[("X-1", 30), ("Y-1", 20)]);
    // WP-B has seq_num 1 and WP-A has 2, so WP-B lists first although "WP-A" sorts first by name.
    assert_eq!(listed[1..], ["WP-B", "Y-1", "WP-A", "X-1"]);
}
