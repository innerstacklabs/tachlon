//! `Session::range_text` (gs#703, R5): the range, or the selected row, as a header line and one
//! tab-separated line per visible row, in the columns the table shows from the start.

use crate::common;

use app::meta::TaskMeta;
use app::outline::{Outline, demo_layout};
use app::{PageSize, Session, edit::Edit};
use common::baseline::{set, slipping_edit};
use common::clock;
use common::collapse::{N, session, standard_calendar};
use engine::{Schedule, Task, TaskId};
use timeline::Timeline;

/// The demo session with an activity ID and a WBS code on every task (`A<i>`, `W.<i>`).
fn coded_session() -> Session {
    let mut layout = demo_layout(N);
    for (i, meta) in layout.meta.iter_mut().enumerate() {
        meta.code = Some(format!("A{i}"));
        meta.wbs_code = Some(format!("W.{i}"));
    }
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("session takes the outline")
}

fn lines(text: &str) -> Vec<Vec<&str>> {
    text.lines()
        .map(|line| line.split('\t').collect())
        .collect()
}

/// The three rows from the first visible leaf: its task and the two rows after it.
fn first_three(session: &mut Session) -> Vec<u32> {
    let order = session.timeline().order();
    let first = order[order.len() / 2];
    session.select(first);
    assert!(session.extend_selection(1).is_some());
    assert!(session.extend_selection(1).is_some());
    let tasks = session.range_tasks();
    assert_eq!(tasks.len(), 3);
    tasks
}

#[test]
fn a_three_row_range_copies_as_a_header_and_three_tab_separated_lines() {
    let mut session = session(N);
    let tasks = first_three(&mut session);

    let text = session.range_text().expect("a range copies");
    let rows = lines(&text);

    assert_eq!(rows[0], ["Task", "Dur", "Start", "Finish", "Float"]);
    assert_eq!(rows.len(), 4, "a header and one line per row");
    for (row, &task) in rows[1..].iter().zip(&tasks) {
        assert_eq!(row.len(), 5, "no ID or WBS column in the demo");
        assert_eq!(row[0], session.task_label(task));
        assert_eq!(row[1], session.task_duration(task));
        let table: serde_json::Value =
            serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("rows are JSON");
        let at = session.timeline().order().iter().position(|&t| t == task);
        let table_row = &table[at.expect("task is on a row")];
        assert_eq!(row[2], table_row[1].as_str().expect("start text"));
        assert_eq!(row[3], table_row[2].as_str().expect("finish text"));
    }
}

#[test]
fn id_and_wbs_columns_come_first_when_the_file_has_them() {
    let mut session = coded_session();
    let tasks = first_three(&mut session);

    let text = session.range_text().expect("a range copies");
    let rows = lines(&text);

    assert_eq!(
        rows[0],
        ["ID", "WBS", "Task", "Dur", "Start", "Finish", "Float"]
    );
    assert_eq!(rows[1][0], session.task_code(tasks[0]));
    assert_eq!(
        rows[1][1],
        session.meta()[tasks[0] as usize].wbs_code_text()
    );
    assert_eq!(rows[1][2], session.task_label(tasks[0]));
}

#[test]
fn with_no_range_the_selected_row_copies_and_with_nothing_selected_nothing_does() {
    let mut session = session(N);
    assert_eq!(session.range_text(), None, "nothing selected");

    let task = session.timeline().order()[3];
    session.select(task);
    let text = session.range_text().expect("the selected row copies");
    let rows = lines(&text);

    assert_eq!(rows.len(), 2, "a header and the one row");
    assert_eq!(rows[1][0], session.task_label(task));
}

#[test]
fn a_tab_or_newline_in_a_name_cannot_break_the_line() {
    let mut layout = demo_layout(N);
    let leaves = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("outline");
    let target = layout
        .order
        .iter()
        .map(|&t| t as usize)
        .find(|&t| !leaves.is_summary(engine::TaskId(t as u32)))
        .expect("a leaf");
    layout.meta[target].name = "Pour\tslab\nlevel 3".to_string();
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule).expect("outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    let mut session = Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        layout.meta,
        clock,
    )
    .with_outline(outline)
    .expect("session takes the outline");
    session.select(target as u32);

    let text = session.range_text().expect("the selected row copies");
    let rows = lines(&text);

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[1].len(), 5);
    assert_eq!(rows[1][0], "Pour slab level 3");
}

#[test]
fn the_copy_adds_the_variance_columns_while_a_baseline_is_set() {
    let mut session = session(N);
    set(&mut session);
    let edit = slipping_edit(&mut session, &[]);
    session.edit(edit).expect("the slipping edit applies");
    let order = session.timeline().order().to_vec();
    let leaf = *order
        .iter()
        .find(|&&t| session.summary_span(t).is_none() && session.task_float(t) != "0")
        .expect("a leaf");
    let (summary, _) = order
        .iter()
        .map(|&t| (t, session.summary_span(t)))
        .find(|(_, span)| span.is_some())
        .expect("a summary");
    session.select(summary);
    let under = session
        .outline()
        .expect("outline")
        .subtree(engine::TaskId(summary))
        .len();
    assert_eq!(
        session.range_text().expect("copies").lines().count(),
        1 + under
    );

    let table: serde_json::Value =
        serde_json::from_str(&session.table_rows_json(0, u32::MAX)).expect("rows are JSON");
    let cells_of = |task: u32| {
        let at = order.iter().position(|&t| t == task).expect("on a row");
        (6..9)
            .map(|k| table[at][k].as_str().unwrap_or("").to_string())
            .collect::<Vec<_>>()
    };

    session.select(summary);
    let text = session.range_text().expect("copies");
    let rows = lines(&text);
    assert_eq!(
        rows[0],
        [
            "Task",
            "Dur",
            "Start",
            "Finish",
            "Float",
            "Start variance",
            "Finish variance",
            "Float variance"
        ]
    );
    assert_eq!(rows[1][5..], ["", "", ""], "a summary has no variance");

    session.select(leaf);
    let text = session.range_text().expect("the leaf copies");
    let rows = lines(&text);
    assert_eq!(rows[1].len(), 8);
    assert_eq!(rows[1][5..], cells_of(leaf)[..]);
    assert!(
        rows[1][5..]
            .iter()
            .any(|cell| !cell.is_empty() && *cell != "0"),
        "the leaf has moved: {:?}",
        rows[1]
    );
}

#[test]
fn the_copy_adds_the_progress_columns_when_the_file_has_progress() {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for (start, finish, remaining) in [
        (Some(2010), None, Some(600)),
        (Some(480), Some(3900), None),
        (None, None, None),
    ] {
        schedule
            .add_task(Task {
                duration: 2400,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: start,
                actual_finish: finish,
                remaining,
            })
            .expect("task");
    }
    let solution = schedule.compute().expect("computes");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let mut session = Session::new(
        schedule,
        solution,
        timeline,
        vec![TaskMeta::default(); 3],
        clock,
    );
    session.select(0);
    assert!(session.extend_selection(1).is_some());
    assert!(session.extend_selection(1).is_some());

    let text = session.range_text().expect("a range copies");
    let rows = lines(&text);

    assert_eq!(
        rows[0][5..],
        ["Actual start", "Actual finish", "Remaining", "% complete"]
    );
    for task in 0..3u32 {
        let row = &rows[1 + task as usize];
        assert_eq!(row.len(), 9, "no variance columns without a baseline");
        assert_eq!(
            row[5..],
            [
                session.task_actual_start(task),
                session.task_actual_finish(task),
                session.task_remaining(task),
                session.task_percent(task)
            ]
        );
    }
    assert_eq!(rows[1][8], "75%");
    assert_eq!(rows[3][5..], ["", "", "", ""], "work not started is empty");

    set(&mut session);
    let text = session.range_text().expect("copies");
    assert_eq!(
        lines(&text)[0][5..8],
        ["Start variance", "Finish variance", "Float variance"],
        "variance comes before progress, as the table orders them"
    );
}

#[test]
fn a_selected_summary_copies_its_whole_subtree_collapsed_children_included() {
    let mut session = session(N);
    let summary = common::collapse::summaries(&session)[0];
    let subtree: Vec<u32> = session
        .outline()
        .expect("outline")
        .subtree(summary)
        .iter()
        .map(|task| task.0)
        .collect();
    assert!(subtree.len() > 1, "the summary has work under it");

    assert!(session.toggle(summary), "collapse the summary");
    session.select(summary.0);
    assert!(session.range_tasks().is_empty(), "no range");

    let text = session.range_text().expect("a summary copies");
    let rows = lines(&text);
    assert_eq!(rows.len(), 1 + subtree.len(), "a header and the subtree");
    let labels: Vec<&str> = rows[1..].iter().map(|row| row[0]).collect();
    let expected: Vec<String> = subtree
        .iter()
        .map(|&task| session.task_label(task).to_string())
        .collect();
    assert_eq!(labels, expected, "outline order, parent first");
}

#[test]
fn list_text_has_a_header_and_every_visible_row() {
    let mut session = session(N);
    let all_text = session.list_text();
    let all = lines(&all_text);
    assert_eq!(all[0], ["Task", "Dur", "Start", "Finish", "Float"]);
    let table = |s: &Session| -> usize {
        let json: serde_json::Value =
            serde_json::from_str(&s.table_rows_json(0, u32::MAX)).expect("rows are JSON");
        json.as_array().expect("an array").len()
    };
    assert_eq!(all.len(), 1 + table(&session), "a header and every row");

    assert!(session.set_critical_only(true));
    let filtered_text = session.list_text();
    let filtered = lines(&filtered_text);
    assert_eq!(filtered.len(), 1 + table(&session));
    assert!(filtered.len() < all.len(), "the filter hides rows");
}

#[test]
fn a_finished_task_has_an_empty_float_cell_in_the_copy_and_the_print_as_the_grid_shows() {
    let (mut s, _) = common::demo_session(10);
    let order = s.timeline().order().to_vec();
    s.edit(Edit::SetProgress {
        task: TaskId(order[0]),
        actual_start: Some(480),
        actual_finish: Some(960),
        remaining: Some(0),
    })
    .expect("progress sets");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&s.table_rows_json(0, u32::MAX)).expect("rows are JSON");
    let grid: Vec<String> = rows
        .iter()
        .map(|row| match row[3] {
            serde_json::Value::Null => String::new(),
            _ => s.task_float(row[0].as_u64().expect("a task") as u32),
        })
        .collect();
    assert_eq!(grid[0], "", "the finished task's grid cell is empty");
    assert!(grid[1..].iter().any(|cell| !cell.is_empty()));

    let text = s.list_text();
    let copied: Vec<&str> = lines(&text)[1..].iter().map(|row| row[4]).collect();
    assert_eq!(copied, grid);

    let columns = ["Task".to_string(), "Float".to_string()];
    let svg = &s.print_pages_on("Plan", "", PageSize::Letter, Some(&columns), None, None)[0];
    let cells = svg.split("<clipPath id=\"pc").skip(1).map(|cell| {
        let text = cell.split_once("<text ").expect("the cell has text").1;
        let text = text.split_once('>').expect("the text opens").1;
        text.split("</text>").next().unwrap_or("")
    });
    let printed: Vec<&str> = cells.skip(2).skip(1).step_by(2).collect();
    assert_eq!(printed, grid);
}
