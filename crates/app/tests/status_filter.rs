//! The Status row filter (gs#703): the table keeps the leaves that are In progress, Not started or
//! Complete, by the card's rule, and the summaries above them as context.

use crate::common;

use app::{
    RowFilter, Session, edit::Edit, meta::TaskMeta, outline::Outline, status_filter::StatusKind,
};
use common::{clock, collapse::standard_calendar};
use engine::{Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;

/// Task 0 is a summary over three leaves: task 1 has an actual start only, task 2 an actual start
/// and an actual finish, task 3 neither.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let task = |actual_start, actual_finish| Task {
        duration: 8 * HOUR,
        calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start,
        actual_finish,
        remaining: None,
    };
    for t in [
        task(None, None),
        task(Some(8 * HOUR), None),
        task(Some(8 * HOUR), Some(16 * HOUR)),
        task(None, None),
    ] {
        schedule
            .add_task(t)
            .expect("the calendar is in the schedule");
    }
    let mut meta = vec![TaskMeta::default(); 4];
    for leaf in &mut meta[1..] {
        leaf.parent = Some(TaskId(0));
    }
    let wbs: Vec<TaskId> = (0..4).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a one-summary outline");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new((0..4).collect(), 1_000.0, 400.0, 1.0),
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
fn each_status_keeps_its_leaves_and_their_summaries() {
    let mut s = session();
    for (kind, leaf) in [
        (StatusKind::InProgress, 1),
        (StatusKind::Complete, 2),
        (StatusKind::NotStarted, 3),
    ] {
        assert!(s.set_status_filter(Some(kind)));
        assert_eq!(s.status_filter(), Some(kind));
        assert_eq!(s.row_filter(), RowFilter::Status(kind));
        assert_eq!(
            listed(&s),
            vec![0, leaf],
            "{kind:?} keeps its leaf and the summary"
        );
    }
    assert!(s.set_status_filter(None));
    assert_eq!(s.status_filter(), None);
    assert_eq!(listed(&s), vec![0, 1, 2, 3], "clearing keeps all");
}

#[test]
fn it_is_one_filter_at_a_time() {
    let mut s = session();
    assert!(s.set_critical_only(true));
    assert!(s.set_status_filter(Some(StatusKind::InProgress)));
    assert!(!s.critical_only());
    // Clearing the status leaves a different filter on.
    assert!(s.set_longest_path(true));
    assert!(s.set_status_filter(None));
    assert!(s.longest_path());
}

#[test]
fn the_filter_follows_an_edit() {
    let mut s = session();
    assert!(s.set_status_filter(Some(StatusKind::NotStarted)));
    assert_eq!(listed(&s), vec![0, 3]);
    s.edit(Edit::SetProgress {
        task: TaskId(3),
        actual_start: Some(8 * HOUR),
        actual_finish: None,
        remaining: Some(8 * HOUR),
    })
    .expect("applies");
    assert_eq!(
        listed(&s),
        Vec::<u32>::new(),
        "no leaf is left, so no summary"
    );
}
