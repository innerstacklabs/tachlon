//! gs#590: the calendar warnings of an XER file list only the calendars the opened projects use.

use import::{Imported, ProjectSelection, parse_xer};
use std::fmt::Write as _;

const GOOD_WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|17:00)())))(0||3()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";
/// Monday has two overlapping periods, so the XER reader drops the second and warns.
const OVERLAP_WEEK: &str = "(0||CalendarData()((0||DaysOfWeek()((0||2()((0||0(s|08:00|f|12:00)())(0||1(s|11:00|f|13:00)())))(0||3()((0||0(s|08:00|f|17:00)())))))(0||Exceptions()())))";
const OVERLAP_NOTE: &str =
    "a working period that overlaps another was not read; working hours can differ from the file's";

fn table(text: &mut String, name: &str, fields: &str, rows: &[String]) {
    write!(text, "%T\t{name}\n%F\t{fields}\n").expect("a String takes writes");
    for row in rows {
        writeln!(text, "%R\t{row}").expect("a String takes writes");
    }
}

/// Project `n` has calendar `calendar` and one activity `n`0 using `task_calendar`.
fn project_rows(n: u32, calendar: u32, task_calendar: u32) -> [String; 3] {
    [
        format!("{n}\tP{n}\tY\t2024-01-01 08:00\t{calendar}"),
        format!("{n}0\t{n}\t\t1\tY\tRoot"),
        format!(
            "{n}0\t{n}\t{n}0\t{task_calendar}\tA{n}\tActivity {n}\tTT_Task\tTK_NotStart\t24\t24\t\t\t"
        ),
    ]
}

/// Each calendar is (id, base, blob); each project is (project calendar, activity calendar).
fn file(calendars: &[(u32, Option<u32>, &str)], projects: &[(u32, u32)]) -> Vec<u8> {
    let mut text = String::from("ERMHDR\t19.12\t2024-03-01\tProject\tadmin\tadmin\tdb\tPM\tUSD\n");
    let mut project = Vec::new();
    let mut wbs = Vec::new();
    let mut task = Vec::new();
    for (index, (calendar, task_calendar)) in projects.iter().enumerate() {
        let n = u32::try_from(index).expect("few projects") + 1;
        let [p, w, t] = project_rows(n, *calendar, *task_calendar);
        project.push(p);
        wbs.push(w);
        task.push(t);
    }
    table(
        &mut text,
        "PROJECT",
        "proj_id\tproj_short_name\texport_flag\tplan_start_date\tclndr_id",
        &project,
    );
    table(
        &mut text,
        "PROJWBS",
        "wbs_id\tproj_id\tparent_wbs_id\tseq_num\tproj_node_flag\twbs_name",
        &wbs,
    );
    table(
        &mut text,
        "TASK",
        "task_id\tproj_id\twbs_id\tclndr_id\ttask_code\ttask_name\ttask_type\tstatus_code\ttarget_drtn_hr_cnt\tremain_drtn_hr_cnt\tcstr_type\tcstr_date\tcstr_type2",
        &task,
    );
    let rows: Vec<String> = calendars
        .iter()
        .map(|(id, base, blob)| {
            let base = base.map_or_default(|base| base.to_string());
            format!("{id}\t{base}\tCA_Project\t{blob}")
        })
        .collect();
    table(
        &mut text,
        "CALENDAR",
        "clndr_id\tbase_clndr_id\tclndr_type\tclndr_data",
        &rows,
    );
    text.push_str("%E\n");
    text.into_bytes()
}

fn open(bytes: &[u8], selection: &ProjectSelection) -> Imported {
    parse_xer(bytes, selection).expect("XER imports")
}

fn overlap_notes(imported: &Imported) -> Vec<&str> {
    imported
        .unsupported
        .iter()
        .map(String::as_str)
        .filter(|line| line.contains(OVERLAP_NOTE))
        .collect()
}

fn finishes(imported: &mut Imported) -> Vec<i64> {
    let solution = imported.schedule.compute().expect("schedule computes");
    (0..imported.tasks.len())
        .map(|index| {
            let id = engine::TaskId(u32::try_from(index).expect("few tasks"));
            solution.get(id).ef
        })
        .collect()
}

#[test]
fn an_unused_child_calendar_adds_no_note_and_a_used_one_does() {
    let bytes = file(
        &[
            (1, None, GOOD_WEEK),
            (2, Some(1), OVERLAP_WEEK),
            (3, Some(1), OVERLAP_WEEK),
        ],
        &[(1, 2)],
    );
    let imported = open(&bytes, &ProjectSelection::Unspecified);
    assert_eq!(
        overlap_notes(&imported),
        [
            "Calendar 2: a working period that overlaps another was not read; working hours can differ from the file's: 1"
        ],
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn an_unused_calendar_changes_no_imported_date() {
    let with_unused = file(
        &[
            (1, None, GOOD_WEEK),
            (2, Some(1), OVERLAP_WEEK),
            (3, Some(1), OVERLAP_WEEK),
        ],
        &[(1, 2)],
    );
    let without = file(
        &[(1, None, GOOD_WEEK), (2, Some(1), OVERLAP_WEEK)],
        &[(1, 2)],
    );
    let mut left = open(&with_unused, &ProjectSelection::Unspecified);
    let mut right = open(&without, &ProjectSelection::Unspecified);
    assert_eq!(finishes(&mut left), finishes(&mut right));
}

#[test]
fn a_file_whose_calendars_are_all_used_keeps_every_note_it_had() {
    let bytes = file(&[(1, None, OVERLAP_WEEK)], &[(1, 1)]);
    let imported = open(&bytes, &ProjectSelection::Unspecified);
    assert_eq!(
        overlap_notes(&imported),
        [
            "Calendar 1: a working period that overlaps another was not read; working hours can differ from the file's: 1"
        ]
    );
}

#[test]
fn a_problem_in_the_base_of_a_used_calendar_is_still_noted() {
    let bytes = file(
        &[
            (1, None, OVERLAP_WEEK),
            (2, Some(1), "(0||CalendarData()())"),
        ],
        &[(2, 2)],
    );
    let imported = open(&bytes, &ProjectSelection::Unspecified);
    assert_eq!(
        overlap_notes(&imported),
        [
            "Calendar 1: a working period that overlaps another was not read; working hours can differ from the file's: 1"
        ],
        "{:?}",
        imported.unsupported
    );
}

#[test]
fn a_calendar_only_an_unopened_project_uses_adds_no_note() {
    let bytes = file(
        &[(1, None, GOOD_WEEK), (2, None, OVERLAP_WEEK)],
        &[(1, 1), (2, 2)],
    );
    let one = open(&bytes, &ProjectSelection::Only(vec![1]));
    assert_eq!(overlap_notes(&one), Vec::<&str>::new());
    let both = open(&bytes, &ProjectSelection::All);
    assert_eq!(
        overlap_notes(&both),
        [
            "Calendar 2: a working period that overlaps another was not read; working hours can differ from the file's: 1"
        ]
    );
}
