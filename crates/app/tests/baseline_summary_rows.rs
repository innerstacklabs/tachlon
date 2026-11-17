//! A summary row's variance cells while a baseline is set (gs#703): its rolled-up span read
//! against the rolled-up span of the baseline's own dates, in working days; Float stays blank.

use crate::common;

use app::Session;
use app::edit::Edit;
use app::meta::TaskMeta;
use app::outline::Outline;
use common::baseline::set;
use engine::{Calendar, Dependency, LinkType, Schedule, Task, TaskId};
use serde_json::Value;
use timeline::Timeline;

const DAYS: i64 = 8 * 60;

/// "Plan" (task 0, a summary) over "Draw" (1, 4 days) then "Check" (2, 3 days), and "Lead" (3, 2
/// days, a top-level task) before Draw. Plan starts after Lead, at 2 days, and ends at 9.
fn plan() -> Session {
    let mut schedule = Schedule::new();
    let work = vec![(9 * 60, 17 * 60)];
    let week = std::array::from_fn(|day| if day < 5 { work.clone() } else { Vec::new() });
    let calendar = schedule.add_calendar(Calendar::new(week, &[]).expect("a calendar"));
    for days in [0, 4, 3, 2] {
        schedule
            .add_task(Task {
                duration: days * DAYS,
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
    for (pred, succ) in [(3, 1), (1, 2)] {
        schedule
            .add_dependency(Dependency {
                pred: TaskId(pred),
                succ: TaskId(succ),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("a link");
    }
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
        row("Lead", None, 1),
    ];
    let wbs: Vec<TaskId> = (0..4).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a valid outline");
    let timeline = Timeline::new(vec![0, 1, 2, 3], 1_000.0, 400.0, 1.0);
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
    [6, 7, 8].map(|i| row[i].as_str().map(str::to_string))
}

fn text(start: Option<&str>, finish: Option<&str>) -> [Option<String>; 3] {
    [start, finish, None].map(|cell| cell.map(str::to_string))
}

fn set_days(session: &mut Session, task: u32, days: i64) {
    session
        .edit(Edit::SetDuration {
            task: TaskId(task),
            minutes: days * DAYS,
        })
        .expect("a duration applies");
}

#[test]
fn a_summary_row_carries_the_variance_of_its_rolled_up_span() {
    let mut session = plan();
    set(&mut session);
    assert_eq!(variance(&session, 0), text(Some("0"), Some("0")), "at rest");

    set_days(&mut session, 2, 8); // Check, the last child: +5 days

    assert_eq!(variance(&session, 0), text(Some("0"), Some("+5d")), "Plan");
    assert_eq!(variance(&session, 2)[1].as_deref(), Some("+5d"), "Check");

    assert_eq!(session.undo(), Ok(true));
    assert_eq!(variance(&session, 0), text(Some("0"), Some("0")), "undone");
    assert_eq!(session.redo(), Ok(true));
    assert_eq!(
        variance(&session, 0),
        text(Some("0"), Some("+5d")),
        "redone"
    );
}

#[test]
fn a_task_added_since_the_baseline_is_left_out_of_the_baseline_span() {
    let mut session = plan();
    set(&mut session);
    assert!(session.select_task(2), "Check selects");
    let added = session.insert_below_selection().expect("a row inserts");
    set_days(&mut session, added.0, 20);

    // The new task starts at 0 and ends at 20 days; the baseline's Plan was 2 to 9 days.
    assert_eq!(variance(&session, 0), text(Some("-2d"), Some("+11d")));
    assert_eq!(variance(&session, added.0), [None, None, None], "no entry");
}

#[test]
fn the_baseline_spans_follow_a_structure_edit() {
    let mut session = plan();
    set(&mut session);
    assert_eq!(
        variance(&session, 0),
        text(Some("0"), Some("0")),
        "read once"
    );

    // Lead moves under Plan: Plan's baseline span now takes in Lead's 0 to 2 days, as its own does.
    assert!(session.select_task(3), "Lead selects");
    session.indent_selected().expect("Lead indents under Plan");
    assert_eq!(
        variance(&session, 0),
        text(Some("0"), Some("0")),
        "indented"
    );

    assert_eq!(session.undo(), Ok(true));
    assert_eq!(variance(&session, 0), text(Some("0"), Some("0")), "undone");
}
