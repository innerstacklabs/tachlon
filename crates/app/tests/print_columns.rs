//! `Session::print_pages_on` with a list of columns (gs#703, J5): the printed list has the columns the screen shows, in the
//! screen's order, and a hidden Float does not collapse the name column.

use crate::common;

use app::{PageSize, Session, meta::TaskMeta};
use common::{clock, collapse::standard_calendar};
use engine::{DAY, Schedule, Task};
use timeline::Timeline;

/// Three tasks with long ID and WBS codes; the first has started when `started`.
fn session(started: bool) -> Session {
    spread(started, 0, false)
}

/// Three tasks, the first has started when `started`, each starting `gap` days after the one before, with the codes of a real
/// file when `short` and long ID and WBS codes otherwise.
fn spread(started: bool, gap: i64, short: bool) -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for i in 0..3 {
        let actual_start = (started && i == 0).then_some(0);
        let task = Task {
            duration: DAY,
            calendar,
            snet: (gap > 0).then_some(i * gap * DAY),
            limit: engine::Limit::default(),
            deleted: false,
            actual_start,
            actual_finish: None,
            remaining: None,
        };
        schedule.add_task(task).expect("calendar is the schedule's");
    }
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let meta = (0..3)
        .map(|i| TaskMeta {
            name: format!("Task {i}"),
            code: Some(if short {
                format!("A10{i}0")
            } else {
                format!("ACT-000{i}-LONG-CODE")
            }),
            wbs_code: Some(if short {
                format!("1.2.{i}")
            } else {
                format!("HAND.WORK.PACKAGE.LONG.{i}")
            }),
            ..TaskMeta::default()
        })
        .collect();
    Session::new(schedule, solution, timeline, meta, clock)
}

fn names(columns: &[&str]) -> Vec<String> {
    columns.iter().map(ToString::to_string).collect()
}

/// The `(row, clip width, text)` of each list cell on page 1 of the print of `columns`; the row is the cell's `y`.
fn cells(session: &Session, columns: Option<&[String]>) -> Vec<(String, f64, String)> {
    let svg = &session.print_pages_on("Plan", "", PageSize::Letter, columns, None, None)[0];
    let attr = |cell: &str, key: &str| {
        let rest = cell.split_once(key).expect("the cell has the attribute").1;
        rest[..rest.find('"').expect("the attribute ends")].to_string()
    };
    let cells = svg.split("<clipPath id=\"pc").skip(1);
    cells
        .map(|cell| {
            let text = cell.split_once("<text ").expect("the cell has text").1;
            let text = text.split_once('>').expect("the text opens").1;
            let width = attr(cell, "width=\"").parse().expect("a width");
            (
                attr(cell, "y=\""),
                width,
                text.split("</text>").next().unwrap_or("").to_string(),
            )
        })
        .collect()
}

/// The header cells: the cells on the first cell's row.
fn head(session: &Session, columns: Option<&[String]>) -> Vec<String> {
    let all = cells(session, columns);
    let first = all[0].0.clone();
    all.into_iter()
        .take_while(|c| c.0 == first)
        .map(|c| c.2)
        .collect()
}

#[test]
fn a_hidden_float_leaves_the_name_column_its_room() {
    let session = session(false);
    let columns = names(&["ID", "WBS", "Task", "Dur", "Start", "Finish"]);
    assert_eq!(head(&session, Some(&columns)), columns);
    // The long ID and WBS shrink so the name keeps 180 px (less its 4 px gutter), as they do with Float on the page.
    let task = cells(&session, Some(&columns))
        .into_iter()
        .find(|c| c.2 == "Task");
    assert!(
        task.as_ref().is_some_and(|c| c.1 >= 176.0),
        "the name column is cut short: {task:?}"
    );
}

#[test]
fn a_moved_start_prints_where_the_screen_puts_it() {
    let session = session(false);
    let standard = cells(&session, None);
    let columns = names(&["Task", "Dur", "Finish", "Start", "Float"]);
    assert_eq!(head(&session, Some(&columns)), columns);
    // Row one starts at cell 7 of the standard page (ID, WBS, Task, Dur, Start, Finish, Float) and at cell 5 of this one.
    let moved = cells(&session, Some(&columns));
    assert_eq!(moved[5 + 2].2, standard[7 + 5].2, "Finish is third");
    assert_eq!(moved[5 + 3].2, standard[7 + 4].2, "Start is fourth");
}

#[test]
fn a_free_float_column_prints_where_the_columns_name_it_and_only_then() {
    // Three tasks of one length that all start together: each has no room before the project finish.
    let session = spread(false, 0, true);
    let columns = names(&["Task", "Free float", "Float"]);
    assert_eq!(head(&session, Some(&columns)), columns);
    let printed = cells(&session, Some(&columns));
    let free: Vec<_> = printed
        .iter()
        .skip(4)
        .step_by(3)
        .map(|c| c.2.as_str())
        .collect();
    let float: Vec<_> = printed
        .iter()
        .skip(5)
        .step_by(3)
        .map(|c| c.2.as_str())
        .collect();
    assert_eq!((free, float), (vec!["0d"; 3], vec!["0d"; 3]), "{printed:?}");
    // A print that does not name it has the columns it always had.
    assert!(!head(&session, None).contains(&"Free float".to_string()));
}

#[test]
fn progress_off_prints_no_progress_columns_on_a_file_with_actuals() {
    let session = session(true);
    // With no columns named, every column the file has is a cell.
    assert_eq!(head(&session, None).len(), 11);
    let off = names(&["Task", "Dur", "Start", "Finish", "Float"]);
    assert_eq!(head(&session, Some(&off)), off);
    let on = head(
        &session,
        Some(&names(&["Task", "Dur", "Actual start", "% complete"])),
    );
    assert_eq!((on.len(), on[3].as_str()), (4, "% complete"), "{on:?}");
}

#[test]
fn every_column_the_screen_shows_prints_whole_with_progress_on() {
    let session = session(true);
    let columns = names(&[
        "Task",
        "% complete",
        "Actual start",
        "Actual finish",
        "Remaining",
        "Dur",
        "Start",
        "Finish",
        "Float",
    ]);
    let all = cells(&session, Some(&columns));
    let cut: Vec<_> = all
        .iter()
        .filter(|c| c.1 <= 0.0 || c.2.ends_with('…'))
        .collect();
    assert!(
        cut.is_empty(),
        "cells clipped to nothing or cut short: {cut:?}"
    );
    assert_eq!(head(&session, Some(&columns)), columns);
}

/// ID, WBS, the progress and the variance columns, with the screen's others between them, in the screen's order.
const ALL: &str = "ID,WBS,Task,% complete,Actual start,Actual finish,Remaining,Dur,Start,Finish,Float,Start variance,Finish variance,Float variance";

#[test]
fn a_narrow_timeline_names_its_months() {
    // Three tasks 100 days apart, January to July: a month is 22 px or so of the panel.
    let session = spread(true, 100, true);
    let columns = names(&ALL.split(',').collect::<Vec<_>>()[2..11]);
    let svg = &session.print_pages_on("Plan", "", PageSize::Letter, Some(&columns), None, None)[0];
    // The panel's text in the page's ink is the months' names; the axis's own labels are in the theme's colours.
    let panel = svg
        .split("<svg x=")
        .nth(1)
        .and_then(|s| s.split("</svg>").next());
    let months: String = panel
        .expect("the panel")
        .split("fill=\"#1f2328\">")
        .skip(1)
        .map(|t| &t[..1])
        .collect();
    assert_eq!(months, "JFMAMJJ");
}

#[test]
fn every_column_prints_whole_with_id_wbs_variance_and_progress_on() {
    let mut session = spread(true, 0, true);
    common::baseline::set(&mut session);
    let columns = names(&ALL.split(',').collect::<Vec<_>>());
    let plan = session.print_plan_on(PageSize::Letter, Some(&columns), None);
    assert!(plan.list_font >= 9.0, "list text {} px < 9", plan.list_font);
    let all = cells(&session, Some(&columns));
    let cut: Vec<_> = all
        .iter()
        .filter(|c| c.1 <= 0.0 || c.2.ends_with('…') || c.2.contains(':'))
        .collect();
    // A date is the day alone: no cell holds a time of day.
    assert!(
        cut.is_empty(),
        "cells cut short, or dated with a time: {cut:?}"
    );
    assert!(all.iter().any(|c| c.2.len() == 10 && c.2.starts_with("20")));
    assert_eq!(
        head(&session, Some(&columns)),
        names(&PRINTED.split(',').collect::<Vec<_>>())
    );
}

/// What [`ALL`] prints as heads: the variance columns are "Start var.", "Finish var." and "Float var.".
const PRINTED: &str = "ID,WBS,Task,% complete,Actual start,Actual finish,Remaining,Dur,Start,Finish,Float,Start var.,Finish var.,Float var.";

#[test]
fn the_variance_heads_print_short() {
    let mut session = spread(true, 0, true);
    common::baseline::set(&mut session);
    let columns = names(&ALL.split(',').collect::<Vec<_>>());
    let printed = head(&session, Some(&columns));
    assert_eq!(printed[11..], ["Start var.", "Finish var.", "Float var."]);
}

#[test]
fn a_long_wbs_code_keeps_its_last_part() {
    // Long ID and WBS codes with every column on: the list is full and WBS shrinks below its code.
    let session = session(true);
    let columns = names(&ALL.split(',').take(11).collect::<Vec<_>>());
    let all = cells(&session, Some(&columns));
    for (row, cell) in all.iter().skip(1).step_by(11).skip(1).enumerate() {
        let tail = format!(".LONG.{row}");
        assert!(
            cell.2.starts_with('…') && cell.2.ends_with(&tail),
            "WBS is not cut from its start: {cell:?}"
        );
    }
}
