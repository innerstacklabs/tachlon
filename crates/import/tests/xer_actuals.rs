//! gs#703: an XER activity's actual start, actual finish and remaining duration reach the
//! imported task, and the engine schedules them as decision 0008 says.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use engine::{Instant, Solution};
use import::{Imported, ProjectSelection, parse_xer};
use std::fmt::Write as _;

const DAY: Instant = 1_440;
const HOUR: Instant = 60;

/// Monday to Friday, 08:00 to 16:00: eight working hours a day.
fn week() -> String {
    let days: String = (2..=6)
        .map(|day| format!("(0||{day}()((0||0(s|08:00|f|16:00)())))"))
        .collect::<Vec<_>>()
        .concat();
    format!("(0||CalendarData()((0||DaysOfWeek()({days}))(0||Exceptions()())))")
}

fn table(text: &mut String, name: &str, fields: &str, rows: &[&str]) {
    write!(text, "%T\t{name}\n%F\t{fields}\n").expect("a String takes writes");
    for row in rows {
        writeln!(text, "%R\t{row}").expect("a String takes writes");
    }
}

/// One project starting Monday 2024-01-01 08:00 (the schedule origin is that midnight) with its
/// data date at Thursday 2024-01-04 08:00, and the given TASK rows (the columns below).
fn file(tasks: &[&str]) -> Vec<u8> {
    let mut text = String::from("ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD\n");
    table(
        &mut text,
        "PROJECT",
        "proj_id\tproj_short_name\texport_flag\tplan_start_date\tlast_recalc_date\tclndr_id",
        &["1\tHAND\tY\t2024-01-01 08:00\t2024-01-04 08:00\t1"],
    );
    table(
        &mut text,
        "PROJWBS",
        "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name",
        &["10\t1\t\t1\tY\tRoot"],
    );
    table(
        &mut text,
        "TASK",
        "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tact_start_date\tact_end_date",
        tasks,
    );
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tclndr_type\tclndr_data",
        &[&format!("1\tCA_Project\t{}", week())],
    );
    text.push_str("%E\n");
    text.into_bytes()
}

/// The schedule as the app builds it: the data date and each activity's actuals handed to the
/// engine before the first compute.
fn solve(imported: &mut Imported) -> Solution {
    imported.schedule.set_data_date(imported.data_date);
    for (index, task) in imported.tasks.iter().enumerate() {
        let id = engine::TaskId(index as u32);
        imported
            .schedule
            .set_actual_start(id, task.actual_start)
            .expect("live task");
        imported
            .schedule
            .set_actual_finish(id, task.actual_finish)
            .expect("live task");
        imported
            .schedule
            .set_remaining(id, task.remaining)
            .expect("live task");
    }
    imported.schedule.compute().expect("compute")
}

/// The imported task with this activity code.
fn index_of(imported: &Imported, code: &str) -> usize {
    imported
        .tasks
        .iter()
        .position(|task| task.code.as_deref() == Some(code))
        .expect("the activity")
}

#[test]
fn an_active_activity_keeps_its_actual_start_and_remaining() {
    // A1: three days of work (24 h), started Tuesday 08:00, one day (8 h) remaining.
    let bytes =
        file(&["101\t1\t10\t1\tA1\tName A1\tTT_Task\tTK_Active\t24\t8\t2024-01-02 08:00\t"]);
    let mut imported = parse_xer(&bytes, &ProjectSelection::Unspecified).expect("import");
    let a1 = index_of(&imported, "A1");
    let task = &imported.tasks[a1];
    assert_eq!(task.actual_start, Some(DAY + 8 * HOUR));
    assert_eq!(task.actual_finish, None);
    assert_eq!(task.remaining, Some(8 * HOUR));
    assert_eq!(imported.data_date, Some(3 * DAY + 8 * HOUR));

    // The start stays at the actual start (Tuesday 08:00). The one remaining day cannot start
    // before the data date, Thursday 08:00, so it ends Thursday 16:00.
    let solution = solve(&mut imported);
    assert_eq!(solution.dates[a1].es, DAY + 8 * HOUR);
    assert_eq!(solution.dates[a1].ef, 3 * DAY + 16 * HOUR);
}

#[test]
fn a_complete_activity_is_fixed_at_its_actuals() {
    // A2: planned 24 h, finished Monday 08:00 to Tuesday 16:00, before the data date.
    let bytes = file(&[
        "102\t1\t10\t1\tA2\tName A2\tTT_Task\tTK_Complete\t24\t0\t2024-01-01 08:00\t2024-01-02 16:00",
    ]);
    let mut imported = parse_xer(&bytes, &ProjectSelection::Unspecified).expect("import");
    let a2 = index_of(&imported, "A2");
    let task = &imported.tasks[a2];
    assert_eq!(task.actual_start, Some(8 * HOUR));
    assert_eq!(task.actual_finish, Some(DAY + 16 * HOUR));

    // The data date (Thursday) does not move it.
    let solution = solve(&mut imported);
    assert_eq!(solution.dates[a2].es, 8 * HOUR);
    assert_eq!(solution.dates[a2].ef, DAY + 16 * HOUR);
}

#[test]
fn an_actual_after_the_data_date_is_counted_once_for_its_activity() {
    // The data date is Thursday 2024-01-04 08:00: A3 started Friday, A1 started Tuesday.
    let bytes = file(&[
        "101\t1\t10\t1\tA1\tName A1\tTT_Task\tTK_Active\t24\t8\t2024-01-02 08:00\t",
        "103\t1\t10\t1\tA3\tName A3\tTT_Task\tTK_Active\t24\t8\t2024-01-05 08:00\t",
    ]);
    let imported = parse_xer(&bytes, &ProjectSelection::Unspecified).expect("import");
    let notes: Vec<&String> = imported
        .unsupported
        .iter()
        .filter(|line| line.contains("after the data date"))
        .collect();
    assert_eq!(notes.len(), 1, "{:?}", imported.unsupported);
    assert!(notes[0].ends_with(": 1 (first: A3)"), "{}", notes[0]);
}
