//! gantt#2106: a value outside the engine's range does not stop a P6 file from opening: it is left
//! out or set to the limit, with one counted note for its kind. The bounds themselves are tested
//! beside `range.rs`; here each reader is shown to call it.

use engine::{MAX_DATE, MAX_DURATION, Pin, Task};
use import::{Imported, ImportedTask, ProjectSelection, parse_xer};

/// The last minute `MAX_DATE` allows from the origin Monday 2024-01-01, and a year far past it.
const AT_BOUND: &str = "3026-01-28 00:00";
const YEAR_9999: &str = "9999-12-31 00:00";
const DATE: &str = "Tasks with a date more than 1,000 years from the schedule start: the date was not read, so their dates can differ from the file's: ";
const LAG: &str = "Links with a lag over 100 years either way: the lag was not read, so dates can differ from the file's: ";
const DURATION: &str = "Tasks with a duration over 100 years: the duration was set to 100 years of 366 days, so their dates can differ from the file's: ";
const REMAINING: &str = "Tasks with a remaining duration over 100 years: the remaining duration was not read, so their dates can differ from the file's: ";
const DATA_DATE: &str = "The data date is more than 1,000 years from the schedule start and was not read; progress is measured without it, so dates can differ from the file's: ";

/// A project starting Monday 2024-01-01 08:00 with its data date, the given TASK rows and the
/// given relationships.
fn import(data_date: &str, tasks: &[String], links: &[String]) -> Imported {
    let rows = |rows: &[String]| {
        rows.iter()
            .map(|r| ["%R\t", r, "\n"].concat())
            .collect::<String>()
    };
    let text = format!(
        "ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD\n\
         %T\tPROJECT\n%F\tproj_id\tproj_short_name\texport_flag\tplan_start_date\tlast_recalc_date\n\
         %R\t1\tHAND\tY\t2024-01-01 08:00\t{data_date}\n\
         %T\tPROJWBS\n%F\twbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name\n\
         %R\t10\t1\t\t1\tY\tRoot\n\
         %T\tTASK\n%F\ttask_id\tproj_id\twbs_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tact_start_date\tact_end_date\n{}\
         %T\tTASKPRED\n%F\ttask_pred_id\ttask_id\tpred_task_id\tpred_type\tlag_hr_cnt\n{}%E\n",
        rows(tasks),
        rows(links),
    );
    parse_xer(text.as_bytes(), &ProjectSelection::Unspecified).expect("imports")
}

/// One task row: its id, status, target and remaining hours, and the cells from `cstr_type` on.
fn row(id: usize, status: &str, hours: &str, remain: &str, cells: &str) -> String {
    format!("{id}\t1\t10\tA{id}\tA{id}\tTT_Task\t{status}\t{hours}\t{remain}\t{cells}")
}

/// What follows `note` in the first line that starts with it: its count and first ids.
fn counted<'a>(i: &'a Imported, note: &str) -> Option<&'a str> {
    i.unsupported
        .iter()
        .find_map(|line| line.strip_prefix(note))
}

/// A task by its code, in the engine and as imported: a task's place depends on its dates.
fn task<'a>(i: &'a Imported, code: &str) -> (&'a Task, &'a ImportedTask) {
    let at = i.tasks.iter().position(|t| t.code.as_deref() == Some(code));
    (
        &i.schedule.tasks()[at.expect("task")],
        &i.tasks[at.expect("task")],
    )
}

/// Where the engine reads a kind of date.
type Read = fn(&Imported, &str) -> Option<i64>;

fn pin(imported: &Imported, code: &str) -> Option<i64> {
    task(imported, code).0.limit.pin.map(|p| match p {
        Pin::Start(at) | Pin::Finish(at) => at,
    })
}

#[test]
fn a_date_outside_the_range_is_left_out_with_one_counted_note_per_kind() {
    let kinds: [(&str, Read); 8] = [
        ("CS_MSOA", |i, c| task(i, c).0.snet),
        ("CS_MSOB", |i, c| task(i, c).0.limit.snlt),
        ("CS_MEOA", |i, c| task(i, c).0.limit.fnet),
        ("CS_MEOB", |i, c| task(i, c).0.limit.fnlt),
        ("CS_MSO", pin),
        ("CS_MEO", pin),
        ("START", |i, c| task(i, c).1.actual_start),
        ("FINISH", |i, c| task(i, c).1.actual_finish),
    ];
    for (kind, read) in kinds {
        let (status, cells) = match kind {
            "START" => ("TK_Active", "\t\t{d}\t".to_owned()),
            "FINISH" => ("TK_Complete", "\t\t\t{d}".to_owned()),
            constraint => ("TK_NotStart", format!("{constraint}\t{{d}}\t\t")),
        };
        let row = |id, date: &str| row(id, status, "1", "1", &cells.replace("{d}", date));
        let imported = import("", &[row(1, YEAR_9999), row(2, AT_BOUND), row(3, "")], &[]);
        let at = |code| read(&imported, code);
        assert_eq!((at("A1"), at("A2")), (at("A3"), Some(MAX_DATE)), "{kind}");
        assert_eq!(counted(&imported, DATE), Some("1 (first: A1)"), "{kind}");
    }
}

#[test]
fn a_duration_remaining_lag_or_data_date_outside_the_range_is_counted() {
    // 878,400 hours is `MAX_DURATION`.
    let started = "\t\t2024-01-01 08:00\t";
    let tasks = [
        row(1, "TK_Active", "878401", "878401", started),
        row(2, "TK_NotStart", "1", "1", "\t\t\t"),
    ];
    let imported = import(YEAR_9999, &tasks, &["1\t2\t1\tPR_FS\t878401".to_owned()]);
    assert_eq!(task(&imported, "A1").0.duration, MAX_DURATION);
    assert_eq!(task(&imported, "A1").1.remaining, None);
    assert_eq!(imported.schedule.dependencies()[0].lag, 0);
    assert_eq!(imported.data_date, None);
    for (note, count) in [
        (DURATION, "1 (first: A1)"),
        (REMAINING, "1 (first: A1)"),
        (LAG, "1 (first: A2)"),
        (DATA_DATE, "1 (first: 1)"),
    ] {
        assert_eq!(counted(&imported, note), Some(count), "{note}");
    }
    let kept = import(AT_BOUND, &tasks, &[]);
    assert_eq!(kept.data_date, Some(MAX_DATE));
}

#[test]
fn a_task_with_two_dates_outside_the_range_is_counted_once() {
    let both = format!("\t\t{YEAR_9999}\t{YEAR_9999}");
    let tasks = [
        row(1, "TK_Complete", "1", "1", &both),
        row(2, "TK_NotStart", "1", "1", "\t\t\t"),
    ];
    let imported = import("", &tasks, &[]);
    assert_eq!(counted(&imported, DATE), Some("1 (first: A1)"));
}

#[test]
fn two_tasks_with_one_code_are_counted_twice() {
    let started = format!("\t\t{YEAR_9999}\t");
    let tasks = [
        row(1, "TK_Active", "1", "1", &started),
        row(2, "TK_Active", "1", "1", &started).replace("A2", "A1"),
    ];
    let imported = import("", &tasks, &[]);
    assert_eq!(counted(&imported, DATE), Some("2 (first: A1, A1)"));
}
