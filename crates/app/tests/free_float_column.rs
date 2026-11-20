//! The free float column's values (gantt#151): slot 9 of `table_rows_json` is the free float as
//! text, `null` where the table shows an empty cell. The expected values are worked by hand on the
//! engine's day network, never read back from `free_floats`, except where a test says it compares
//! the two on a larger schedule.

use crate::common;

use app::{
    Session,
    edit::{Edit, format_float_days},
    meta::TaskMeta,
    outline::{Outline, demo_layout},
};
use common::{clock, setup_with};
use engine::{Calendar, Dependency, Limit, LinkType, Schedule, Task, TaskId};
use import::ActivityCode;
use serde_json::Value;
use timeline::Timeline;

/// A Monday-to-Friday 08:00-17:00 day: 540 working minutes.
const DAY: i64 = 540;

fn week() -> Calendar {
    let day = vec![(480, 1020)];
    Calendar::new(
        [
            day.clone(),
            day.clone(),
            day.clone(),
            day.clone(),
            day,
            vec![],
            vec![],
        ],
        &[],
    )
    .expect("a valid week")
}

/// Tasks 0 to 4: `first`, `mid`, `last` in a chain of days, `long` (three days) also before
/// `last`, and `five` days, which sets the project finish on Friday. Total float, by hand:
/// 2d, 2d, 1d, 1d, 0d. Free float: 0d, 1d, 1d, 0d, 0d.
fn days() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(week());
    for length in [1, 1, 1, 3, 5] {
        schedule
            .add_task(Task {
                duration: length * DAY,
                calendar,
                snet: None,
                limit: Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("the calendar belongs to the schedule");
    }
    for (pred, succ) in [(0, 1), (1, 2), (3, 2)] {
        schedule
            .add_link(Dependency {
                pred: TaskId(pred),
                succ: TaskId(succ),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("links");
    }
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new(vec![0, 1, 2, 3, 4], 1_000.0, 400.0, 1.0);
    Session::new(
        schedule,
        solution,
        timeline,
        vec![TaskMeta::default(); 5],
        clock,
    )
}

fn rows(session: &Session) -> Vec<Vec<Value>> {
    let rows: Value = serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("JSON");
    rows.as_array()
        .expect("an array")
        .iter()
        .map(|row| row.as_array().expect("a row").clone())
        .collect()
}

fn slot(rows: &[Vec<Value>], at: usize) -> Vec<Value> {
    rows.iter().map(|row| row[at].clone()).collect()
}

#[test]
fn slot_nine_is_the_free_float_and_slot_three_the_total_float_on_one_network() {
    let rows = rows(&days());
    let text = |days: &[&str]| days.iter().map(|d| Value::from(*d)).collect::<Vec<_>>();
    assert_eq!(
        slot(&rows, 3),
        [2 * DAY, 2 * DAY, DAY, DAY, 0].map(Value::from)
    );
    assert_eq!(slot(&rows, 9), text(&["0d", "1d", "1d", "0d", "0d"]));
}

#[test]
fn a_finished_task_has_no_free_float_cell_and_the_rest_keep_theirs() {
    let mut s = days();
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(480),
        actual_finish: Some(1020),
        remaining: Some(0),
    })
    .expect("progress sets");
    let rows = rows(&s);
    assert_eq!(rows[0][9], Value::Null, "first is finished");
    assert!(
        rows[1..].iter().all(|row| row[9].is_string()),
        "the others show a value: {:?}",
        slot(&rows, 9)
    );
}

#[test]
fn a_summary_and_a_header_have_an_empty_free_float_cell() {
    let mut layout = demo_layout(60);
    for (i, meta) in layout.meta.iter_mut().enumerate() {
        meta.activity_codes = vec![ActivityCode {
            code_type: "Phase".to_owned(),
            value: ["Civil", "MEP"][i % 2].to_owned(),
        }];
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("an outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let mut s = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("a matching outline");
    let ungrouped = rows(&s);
    let summaries = ungrouped
        .iter()
        .filter(|row| row[5] != 0)
        .collect::<Vec<_>>();
    assert!(!summaries.is_empty(), "the outline has summaries");
    assert!(summaries.iter().all(|row| row[9].is_null()));
    assert!(
        ungrouped
            .iter()
            .filter(|row| row[5] == 0)
            .any(|row| row[9].is_string()),
        "a leaf shows a value"
    );
    assert!(s.set_group_by(Some("Phase")));
    let grouped = rows(&s);
    let headers = grouped
        .iter()
        .filter(|row| row[5] == 3 || row[5] == 4)
        .collect::<Vec<_>>();
    assert!(!headers.is_empty(), "grouping adds header rows");
    assert!(grouped.iter().all(|row| row.len() == 10), "ten slots a row");
    assert!(headers.iter().all(|row| row[9].is_null()));
}

/// The column reads the engine per row. After an edit, an undo and a drag frame, each leaf's
/// value is what the whole-schedule list gives for the solution the session holds.
#[test]
fn every_row_equals_the_whole_list_after_an_edit_a_drag_frame_and_an_undo() {
    let (mut s, task, _, (x, y)) = setup_with(true);
    let agrees = |s: &Session, when: &str| {
        let whole = s
            .schedule()
            .free_floats(s.solution())
            .expect("the whole list computes");
        let listed = rows(s);
        let mut shown = 0;
        for row in listed.iter().take(60) {
            let t = row[0].as_u64().expect("a task") as usize;
            if s.solution().dates[t].finished {
                assert_eq!(row[9], Value::Null, "{when}: task {t} is finished");
                continue;
            }
            let day = app::edit::day_minutes(s.schedule().calendar_of(TaskId(t as u32)));
            let want = format_float_days(whole[t], day);
            assert_eq!(row[9], Value::from(want), "{when}: task {t}");
            shown += 1;
        }
        assert!(shown > 20, "{when}: rows compared");
    };
    agrees(&s, "open");
    s.edit(Edit::SetDuration {
        task: TaskId(task),
        minutes: 3 * DAY,
    })
    .expect("a duration sets");
    agrees(&s, "after a duration edit");
    assert!(s.undo().expect("undoes"));
    agrees(&s, "after the undo");
    s.pointer_down(x, y, false);
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    assert!(s.drag_info().is_some(), "a drag is under way");
    agrees(&s, "on a drag frame");
    s.pointer_up();
    agrees(&s, "after the drag");
}

/// A structural edit drops the engine's graph and the recompute after it builds one again, so the
/// column still has a value for every unfinished row after a link is added, taken back and put back.
#[test]
fn a_link_edit_an_undo_and_a_redo_leave_every_row_with_its_value() {
    let mut s = days();
    let whole = |s: &Session| -> Vec<Value> {
        let all = s.schedule().free_floats(s.solution()).expect("computes");
        (0..5)
            .map(|t| {
                let day = app::edit::day_minutes(s.schedule().calendar_of(TaskId(t)));
                Value::from(format_float_days(all[t as usize], day))
            })
            .collect()
    };
    s.edit(Edit::AddLink {
        pred: TaskId(0),
        succ: TaskId(4),
        kind: LinkType::FS,
        lag: 0,
    })
    .expect("links");
    assert_eq!(slot(&rows(&s), 9), whole(&s), "after the link");
    assert_ne!(
        slot(&rows(&s), 9),
        ["0d", "1d", "1d", "0d", "0d"].map(Value::from),
        "the link changed a value"
    );
    assert!(s.undo().expect("undoes"));
    assert_eq!(slot(&rows(&s), 9), whole(&s), "after the undo");
    assert!(s.redo().expect("redoes"));
    assert_eq!(slot(&rows(&s), 9), whole(&s), "after the redo");
}
