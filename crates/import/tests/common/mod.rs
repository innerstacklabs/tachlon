//! A hand-built XER file for the export tests. `table` and `handbuilt` are copies of the ones in
//! `tests/xer.rs`, which stays as it is: a test crate cannot share a helper with another one
//! except through this directory, and moving the originals would change that file.
#![allow(
    dead_code,
    reason = "each test crate that includes this module uses only some of its items"
)]

pub const WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||1()())(0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))(0||4()((0||0(s|08:00|f|17:00)())))(0||5()((0||0(s|08:00|f|17:00)())))(0||6()((0||0(s|08:00|f|17:00)())))(0||7()())))(0||Exceptions()())))";
const PROJECT_FIELDS: &str = "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id";
const WBS_FIELDS: &str = "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name";
const TASK_FIELDS: &str = "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2";
const PRED_FIELDS: &str = "task_id\tpred_task_id\tpred_type\tlag_hr_cnt";

pub fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    text.push_str("%T\t");
    text.push_str(name);
    text.push_str("\n%F\t");
    text.push_str(fields);
    text.push('\n');
    for row in rows {
        text.push_str("%R\t");
        text.push_str(row);
        text.push('\n');
    }
}

pub fn handbuilt(start: &str, tasks: &[&str], links: &[&str], calendar: &str) -> Vec<u8> {
    let mut text = String::from(
        "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tProject Management\tUSD\n",
    );
    let project = format!("1\tHAND\tY\t{start}\t1");
    table(&mut text, "PROJECT", PROJECT_FIELDS, &[&project]);
    table(&mut text, "PROJWBS", WBS_FIELDS, &["10\t1\t\t1\tY\tRoot"]);
    table(&mut text, "TASK", TASK_FIELDS, tasks);
    table(&mut text, "TASKPRED", PRED_FIELDS, links);
    table(
        &mut text,
        "SCHEDOPTIONS",
        "proj_id\tsched_calendar_on_relationship_lag",
        &["1\trcal_Successor"],
    );
    let calendar = format!("1\tCA_Project\t{calendar}");
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&calendar],
    );
    text.push_str("%E\n");
    text.into_bytes()
}

pub fn regular_task(id: u32, code: &str, hours: &str) -> String {
    format!("{id}\t1\t10\t1\t{code}\tName {code}\tTT_Task\tTK_NotStart\t{hours}\t{hours}\t\t\t")
}
