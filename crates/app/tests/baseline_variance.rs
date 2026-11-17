//! The baseline's variance cells in the task table's rows (gs#703): the change in working days of a
//! task's own calendar since the baseline, and when a row has none.

use crate::common;

use app::Session;
use app::baseline::{Baseline, BaselineTask};
use app::edit::Edit;
use app::meta::TaskMeta;
use app::outline::Outline;
use common::baseline::set;
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use serde_json::Value;
use std::sync::Arc;
use timeline::Timeline;

const DAYS: i64 = 8 * 60;

/// "Plan" (task 0, a summary) over "Draw" (1, 4 days) then "Check" (2, 3 days) on a Monday-to-Friday
/// calendar of 8-hour days, "Side" (3, 2 days, linked to nothing) and "Short" (4, 2 days of a
/// 4-hour-day calendar, linked to nothing). Draw starts at 0, Check at 4 days, the project ends at
/// 7 days, so Side has 5 days of float.
fn plan() -> Session {
    let mut schedule = Schedule::new();
    let week = |hours: u16| {
        let work = vec![(9 * 60, 9 * 60 + hours * 60)];
        std::array::from_fn(|day| if day < 5 { work.clone() } else { Vec::new() })
    };
    let full = schedule.add_calendar(Calendar::new(week(8), &[]).expect("a calendar"));
    let half = schedule.add_calendar(Calendar::new(week(4), &[]).expect("a calendar"));
    for (days, calendar) in [(0, full), (4, full), (3, full), (2, full), (2, half)] {
        let minutes_a_day = if calendar == half { 4 * 60 } else { DAYS };
        schedule
            .add_task(Task {
                duration: days * minutes_a_day,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
    }
    schedule
        .add_dependency(Dependency {
            pred: TaskId(1),
            succ: TaskId(2),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a link");
    let solution = schedule.compute().expect("acyclic");
    let row = |name: &str, parent: Option<u32>, level: u8| TaskMeta {
        name: name.to_string(),
        parent: parent.map(TaskId),
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    };
    let meta = vec![
        row("Plan", None, 1),
        row("Draw", Some(0), 2),
        row("Check", Some(0), 2),
        row("Side", Some(0), 2),
        row("Short", Some(0), 2),
    ];
    let wbs: Vec<TaskId> = (0..5).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new(vec![0, 1, 2, 3, 4], 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, meta, common::clock)
        .with_outline(outline)
        .expect("the outline fits")
}

/// The three variance cells (start, finish, float) of `task`'s table row, as text or `None`.
fn variance(session: &Session, task: u32) -> [Option<String>; 3] {
    let rows: Value = serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("JSON");
    let row = rows
        .as_array()
        .expect("rows")
        .iter()
        .find(|row| row[0] == task)
        .expect("the task has a row")
        .as_array()
        .expect("a row");
    assert_eq!(row.len(), 9, "six cells, then three of variance");
    [6, 7, 8].map(|i| row[i].as_str().map(str::to_string))
}

fn cells(texts: [&str; 3]) -> [Option<String>; 3] {
    texts.map(|text| Some(text.to_string()))
}

fn set_days(session: &mut Session, task: u32, days: i64, minutes_a_day: i64) {
    session
        .edit(Edit::SetDuration {
            task: TaskId(task),
            minutes: days * minutes_a_day,
        })
        .expect("a duration applies");
}

#[test]
fn table_rows_carry_variance_in_working_days() {
    let mut session = plan();
    set(&mut session);
    for task in [1, 2, 3] {
        assert_eq!(variance(&session, task), cells(["0", "0", "0"]), "{task}");
    }

    set_days(&mut session, 1, 9, DAYS); // Draw, +5 days: Check follows it, the project ends 5 later

    assert_eq!(variance(&session, 1), cells(["0", "+5d", "0"]), "Draw");
    assert_eq!(variance(&session, 2), cells(["+5d", "+5d", "0"]), "Check");
    assert_eq!(
        variance(&session, 3),
        cells(["0", "0", "+5d"]),
        "Side has more float"
    );
}

#[test]
fn a_task_with_no_entry_carries_null_and_a_summary_has_no_float() {
    let mut session = plan();
    set(&mut session);
    set_days(&mut session, 1, 9, DAYS);
    let added = session.insert_below_selection().expect("a row inserts");

    // Plan's span runs from Draw's start to Check's finish: its start is the same, its finish +5d.
    assert_eq!(
        variance(&session, 0),
        [Some("0".to_string()), Some("+5d".to_string()), None],
        "the summary"
    );
    assert_eq!(variance(&session, added.0), [None, None, None], "no entry");
    assert_eq!(
        variance(&session, 2),
        cells(["+5d", "+5d", "0"]),
        "a leaf still reads"
    );
}

#[test]
fn without_a_baseline_every_variance_cell_is_null() {
    let mut session = plan();
    set_days(&mut session, 1, 9, DAYS);
    for task in 0..5 {
        assert_eq!(variance(&session, task), [None, None, None], "{task}");
    }

    set(&mut session);
    assert_eq!(variance(&session, 2), cells(["0", "0", "0"]));
    session.edit(Edit::ClearBaseline).expect("clears");
    assert_eq!(variance(&session, 2), [None, None, None], "cleared");
}

#[test]
fn variance_follows_undo_and_the_tasks_own_calendar() {
    let mut session = plan();
    set(&mut session);

    // Short runs on 4-hour days: two more of them is 2d there, where 8-hour days would read 1d.
    set_days(&mut session, 4, 4, 4 * 60);
    assert_eq!(variance(&session, 4), cells(["0", "+2d", "-2d"]), "Short");

    assert_eq!(session.undo(), Ok(true));
    assert_eq!(variance(&session, 4), cells(["0", "0", "0"]), "undone");
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(variance(&session, 4), cells(["0", "+2d", "-2d"]), "redone");

    set_days(&mut session, 4, 1, 4 * 60); // a day shorter than the baseline's two
    assert_eq!(variance(&session, 4), cells(["0", "-1d", "+1d"]), "earlier");
}

#[test]
fn variance_cells_are_held_while_a_bar_is_dragged() {
    let (mut session, task, _, point) = common::setup_with(true);
    set(&mut session);
    let cells = |session: &Session| -> Vec<Option<String>> {
        let rows: Value =
            serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("JSON");
        rows.as_array()
            .expect("rows")
            .iter()
            .find(|row| row[0] == task)
            .map(|row| {
                [6, 7, 8]
                    .map(|i| row[i].as_str().map(str::to_string))
                    .to_vec()
            })
            .expect("the task has a row")
    };
    assert!(cells(&session).iter().all(Option::is_some), "text at rest");

    session.pointer_down(point.0, point.1, false);
    session.pointer_move(point.0 + common::TRAVEL_PX, point.1);
    session.apply_drag();
    assert!(session.drag_started(), "the drag is on");
    assert_eq!(
        cells(&session),
        vec![None, None, None],
        "held during the drag"
    );

    session.pointer_up();
    assert!(
        cells(&session).iter().all(Option::is_some),
        "back at the release"
    );
}

#[test]
fn a_baseline_with_no_float_leaves_the_float_variance_null() {
    let mut session = plan();
    let with_float = set(&mut session);
    // The same snapshot as a file would give it: dates, no float.
    let tasks = (0..5)
        .map(|i| {
            with_float
                .task(TaskId(i))
                .map(|t| BaselineTask { float: None, ..*t })
        })
        .collect();
    let without = Baseline::from_parts(tasks, with_float.project_finish());
    session
        .edit(Edit::SetBaseline {
            baseline: Some(Arc::new(without)),
        })
        .expect("a baseline sets");
    set_days(&mut session, 1, 9, DAYS);

    assert_eq!(
        variance(&session, 2),
        [Some("+5d".to_string()), Some("+5d".to_string()), None],
        "dates still read, float does not"
    );
}

#[test]
fn a_baseline_date_before_the_origin_leaves_that_cell_null() {
    let mut session = plan();
    let now = set(&mut session);
    // A file's baseline can state a date before the schedule's origin: Draw starts and finishes
    // before it, Check is as it was.
    let tasks = (0..5)
        .map(|i| {
            now.task(TaskId(i)).map(|t| match i {
                1 => BaselineTask {
                    es: -3_840,
                    ef: -3_000,
                    float: None,
                },
                _ => BaselineTask { float: None, ..*t },
            })
        })
        .collect();
    let before = Baseline::from_parts(tasks, now.project_finish());
    session
        .edit(Edit::SetBaseline {
            baseline: Some(Arc::new(before)),
        })
        .expect("a baseline sets");

    assert_eq!(
        variance(&session, 1),
        [None, None, None],
        "Draw has no date to read against"
    );
    assert_eq!(
        variance(&session, 2),
        [Some("0".to_string()), Some("0".to_string()), None],
        "Check reads"
    );
}
