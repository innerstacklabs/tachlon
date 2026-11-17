//! The Start constraint row filter (gs#703 K5): the table keeps the tasks whose start-no-earlier-than
//! is strictly above their import floor, and the summaries above them as context.

use crate::common;

use app::{RowFilter, Session, edit::Edit, meta::TaskMeta, outline::Outline};
use common::{clock, collapse::standard_calendar};
use engine::{Schedule, Task, TaskId};
use timeline::Timeline;

const DAY: i64 = 24 * 60;

/// Task 0 is a summary over leaves 1 and 2, task 3 a summary over leaf 4. Every task has the floor
/// 0. Leaf 1 starts no earlier than a day above it, leaf 2 exactly at it, leaf 4 has none.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for snet in [None, Some(DAY), Some(0), None, None] {
        let task = Task {
            duration: 480,
            calendar,
            snet,
            limit: engine::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        schedule
            .add_task(task)
            .expect("the calendar is in the schedule");
    }
    let mut meta = vec![TaskMeta::default(); 5];
    for (leaf, parent) in [(1, 0), (2, 0), (4, 3)] {
        meta[leaf].parent = Some(TaskId(parent));
    }
    for entry in &mut meta {
        entry.floor = Some(0);
    }
    let wbs: Vec<TaskId> = (0..5).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a two-summary outline");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new((0..5).collect(), 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

fn listed(s: &Session) -> Vec<u32> {
    s.timeline().order().to_vec()
}

#[test]
fn the_filter_keeps_only_tasks_with_a_constraint_above_their_floor() {
    let mut s = session();
    assert!(s.set_start_constraint_filter(true));
    assert!(s.start_constraint_filter());
    assert_eq!(s.row_filter(), RowFilter::StartConstraint);
    // Leaf 1 stays with its summary; leaf 2 is at its floor and leaf 4 has none, so the second
    // summary goes with them.
    assert_eq!(listed(&s), vec![0, 1]);

    assert!(s.set_start_constraint_filter(false));
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert_eq!(listed(&s), vec![0, 1, 2, 3, 4]);
}

#[test]
fn a_summary_shows_when_a_task_under_it_does() {
    let mut s = session();
    assert!(s.set_start_constraint_filter(true));
    assert!(!listed(&s).contains(&3));
    // Leaf 4 is dragged past its floor: it joins, and its summary shows for it.
    s.edit(Edit::Move {
        task: TaskId(4),
        snet: Some(2 * DAY),
    })
    .expect("applies");
    assert_eq!(listed(&s), vec![0, 1, 3, 4]);
}
