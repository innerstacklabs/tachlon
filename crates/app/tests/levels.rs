use crate::common;

use app::{
    Session,
    meta::TaskMeta,
    outline::{Outline, demo_layout},
};
use common::clock;
use engine::{Calendar, Schedule, Task, TaskId};
use timeline::{HIDDEN_ROW, Timeline};

const N: usize = 2_000;

fn standard_calendar() -> Calendar {
    let day = vec![(480, 720), (780, 1020)];
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
    .expect("valid standard calendar")
}

fn schedule_with_tasks(task_count: usize) -> Schedule {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in 0..task_count {
        schedule
            .add_task(Task {
                duration: 0,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("calendar belongs to the schedule");
    }
    schedule
}

fn task_meta(parent: Option<TaskId>, level: u8) -> TaskMeta {
    TaskMeta {
        name: "task".to_string(),
        parent,
        outline_level: level,
        code: None,
        ..TaskMeta::default()
    }
}

fn hand_session() -> Session {
    // A(A1(A1a, A1b), A2), B(B1(B1a)), C
    let mut schedule = schedule_with_tasks(9);
    let solution = schedule.compute().expect("zero-duration schedule computes");
    let meta = vec![
        task_meta(None, 1),
        task_meta(Some(TaskId(0)), 2),
        task_meta(Some(TaskId(1)), 3),
        task_meta(Some(TaskId(1)), 3),
        task_meta(Some(TaskId(0)), 2),
        task_meta(None, 1),
        task_meta(Some(TaskId(5)), 2),
        task_meta(Some(TaskId(6)), 3),
        task_meta(None, 1),
    ];
    let wbs: Vec<_> = (0..9).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("valid three-level forest");
    let timeline = Timeline::new((0..9).collect(), 1_000.0, 400.0, 1.0);
    Session::new(schedule, solution, timeline, Vec::new(), clock)
        .with_outline(outline)
        .expect("outline matches the schedule")
}

fn demo_session(n: usize) -> Session {
    let layout = demo_layout(n);
    let outline = Outline::new(&layout.meta, &layout.wbs, &layout.schedule)
        .expect("demo layout has a valid outline");
    let timeline = Timeline::new(layout.order, 1_000.0, 400.0, 1.0);
    Session::new(
        layout.schedule,
        layout.solution,
        timeline,
        Vec::new(),
        clock,
    )
    .with_outline(outline)
    .expect("outline matches the demo schedule")
}

fn kept_by_filter(session: &Session, task: TaskId) -> bool {
    let outline = session.outline().expect("test session has an outline");
    let children = outline.children(task);
    children.is_empty() && session.solution().dates[task.0 as usize].is_critical()
        || children
            .iter()
            .copied()
            .any(|child| kept_by_filter(session, child))
}

#[test]
fn collapse_levels_install_the_exact_rows_and_flags() {
    let mut session = hand_session();

    assert!(session.collapse_to_level(1));
    assert_eq!(session.timeline().order(), [0, 5, 8]);
    assert!(
        [0, 1, 5, 6]
            .into_iter()
            .all(|task| session.is_collapsed(TaskId(task)))
    );

    assert!(session.collapse_to_level(2));
    assert_eq!(session.timeline().order(), [0, 1, 4, 5, 6, 8]);
    assert!(!session.is_collapsed(TaskId(0)));
    assert!(session.is_collapsed(TaskId(1)));
    assert!(!session.is_collapsed(TaskId(5)));
    assert!(session.is_collapsed(TaskId(6)));

    assert!(session.collapse_to_level(3));
    assert_eq!(session.timeline().order(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(
        [0, 1, 5, 6]
            .into_iter()
            .all(|task| !session.is_collapsed(TaskId(task)))
    );

    assert!(session.collapse_to_level(1));
    assert!(session.expand_all());
    assert_eq!(session.timeline().order(), [0, 1, 2, 3, 4, 5, 6, 7, 8]);
    assert!(
        [0, 1, 5, 6]
            .into_iter()
            .all(|task| !session.is_collapsed(TaskId(task)))
    );
}

#[test]
fn invalid_or_busy_level_commands_change_nothing() {
    let mut session = hand_session();
    let before = (session.timeline().order().to_vec(), session.stamp());
    assert!(!session.collapse_to_level(0));
    assert_eq!(
        (session.timeline().order().to_vec(), session.stamp()),
        before
    );

    let mut schedule = schedule_with_tasks(3);
    let solution = schedule.compute().expect("flat schedule computes");
    let timeline = Timeline::new(vec![0, 1, 2], 1_000.0, 400.0, 1.0);
    let mut flat = Session::new(schedule, solution, timeline, Vec::new(), clock);
    assert!(!flat.collapse_to_level(1));
    assert!(!flat.expand_all());

    let mut busy = demo_session(N);
    let (row, task) = {
        let outline = busy.outline().expect("demo has an outline");
        busy.timeline()
            .order()
            .iter()
            .copied()
            .enumerate()
            .find(|&(_, task)| {
                !outline.is_summary(TaskId(task))
                    && busy.solution().dates[task as usize].ef
                        - busy.solution().dates[task as usize].es
                        >= 800
            })
            .expect("demo has a wide leaf bar")
    };
    let start = busy.solution().dates[task as usize].es;
    common::zoom_to(&mut busy, 100.0, start as f64 - 50_000.0);
    busy.scroll_to_row(row);
    let point = (
        busy.timeline().viewport.x_of(start as f64) + 2.0,
        busy.timeline().viewport.row_top(row) + busy.timeline().viewport.row_height / 2.0,
    );
    assert_eq!(
        busy.timeline()
            .hit_test(&busy.solution().dates, point.0, point.1),
        Some(task)
    );
    busy.pointer_down(point.0, point.1, false);
    let before = (busy.timeline().order().to_vec(), busy.stamp());
    assert!(!busy.collapse_to_level(1));
    assert!(!busy.expand_all());
    assert_eq!((busy.timeline().order().to_vec(), busy.stamp()), before);
    busy.pointer_up();
}

#[test]
fn level_commands_bump_once_and_never_touch_history() {
    let mut session = hand_session();
    assert!(session.toggle(TaskId(0)) && session.toggle(TaskId(5)));
    let rows = session.timeline().order().to_vec();
    let history = session.history_len();
    let stamp = session.stamp();

    assert!(session.collapse_to_level(1));
    assert_eq!(session.timeline().order(), rows);
    assert_eq!(session.stamp(), stamp.wrapping_add(1));
    assert_eq!(session.history_len(), history);
    assert!(!session.can_undo());

    let stamp = session.stamp();
    assert!(session.collapse_to_level(1));
    assert_eq!(session.stamp(), stamp);
    assert_eq!(session.history_len(), history);

    assert!(session.expand_all());
    assert_eq!(session.stamp(), stamp.wrapping_add(1));
    assert_eq!(session.history_len(), history);
    assert!(!session.can_undo());
}

#[test]
fn level_commands_compose_with_the_critical_filter() {
    let mut session = demo_session(N);
    assert!(session.set_critical_only(true));
    let filter_only = session.timeline().order().to_vec();
    let depth_one_filtered: Vec<_> = {
        let outline = session.outline().expect("demo has an outline");
        filter_only
            .iter()
            .copied()
            .filter(|&task| outline.depth(TaskId(task)) == 1)
            .collect()
    };
    assert!(
        filter_only
            .iter()
            .copied()
            .all(|task| kept_by_filter(&session, TaskId(task)))
    );

    assert!(session.collapse_to_level(1));
    assert!(session.critical_only());
    assert_eq!(session.timeline().order(), depth_one_filtered);

    assert!(session.expand_all());
    assert!(session.critical_only());
    assert_eq!(session.timeline().order(), filter_only);
}

#[test]
fn demo_level_one_and_expand_all_have_the_expected_counts() {
    let mut session = demo_session(N);
    let depth_one_count = session
        .outline()
        .expect("demo has an outline")
        .preorder()
        .iter()
        .filter(|&&task| session.outline().expect("demo has an outline").depth(task) == 1)
        .count();

    assert!(session.collapse_to_level(1));
    assert_eq!(session.timeline().order().len(), depth_one_count);
    assert!(session.timeline().order().iter().all(|&task| {
        session
            .outline()
            .expect("demo has an outline")
            .depth(TaskId(task))
            == 1
    }));

    assert!(session.expand_all());
    assert_eq!(session.timeline().order().len(), N);
}

#[test]
fn level_and_selected_commands_keep_selection_visible() {
    let mut session = hand_session();
    session.select(2);
    assert!(session.collapse_to_level(1));
    let selection = session.selection().expect("ancestor is selected");
    assert_eq!(selection.task, 0);
    assert_ne!(selection.row, HIDDEN_ROW);
    assert!(session.timeline().row_of_task(selection.task).is_some());

    assert!(session.expand_all());
    session.select(8);
    assert!(session.collapse_to_level(1));
    assert_eq!(
        session.selection().expect("root leaf stays selected").task,
        8
    );

    assert!(session.expand_all());
    session.select(1);
    assert!(session.collapse_selected());
    assert!(session.is_collapsed(TaskId(1)));
    assert!(!session.collapse_selected());
    assert!(session.expand_selected());
    assert!(!session.is_collapsed(TaskId(1)));
    assert!(!session.expand_selected());

    session.select(2);
    assert!(session.collapse_selected());
    assert_eq!(session.selection().expect("parent is selected").task, 1);

    session.select(8);
    assert!(!session.collapse_selected());
    assert!(!session.expand_selected());
}

#[test]
fn expand_selected_branch_opens_every_summary_below() {
    let mut session = hand_session();
    assert!(session.collapse_to_level(1));
    session.select(0);
    assert!(session.expand_selected_branch());
    assert_eq!(session.timeline().order(), [0, 1, 2, 3, 4, 5, 8]);
    assert!(!session.expand_selected_branch());

    // An open summary with a closed one below it: the closed one opens, a sibling branch stays closed.
    assert!(session.collapse_to_level(2));
    session.select(0);
    assert!(session.expand_selected_branch());
    assert_eq!(session.timeline().order(), [0, 1, 2, 3, 4, 5, 6, 8]);
    assert!(session.is_collapsed(TaskId(6)));

    // A leaf and no selection do nothing.
    session.select(2);
    assert!(!session.expand_selected_branch());
    session.clear_selection();
    assert!(!session.expand_selected_branch());
}
