//! Clear filters (gs#703): one call turns the row filter and the Lookahead window off, so the table
//! lists every row again; it waits for a bar press.

use crate::common;

use app::{RowFilter, Session, meta::TaskMeta, outline::Outline, status_filter::StatusKind};
use common::{clock, collapse::standard_calendar};
use engine::{Schedule, Task, TaskId};
use timeline::Timeline;

const HOUR: i64 = 60;

/// Task 0 is a summary over three leaves; task 1 has an actual start only, so Status can narrow it.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    let task = |actual_start| Task {
        duration: 8 * HOUR,
        calendar,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start,
        actual_finish: None,
        remaining: None,
    };
    for t in [task(None), task(Some(8 * HOUR)), task(None), task(None)] {
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

fn rows(s: &Session) -> usize {
    s.timeline().order().len()
}

/// A canvas point on a leaf's bar, found by hit test so the press below lands on it.
fn leaf_bar_point(s: &Session) -> (f64, f64) {
    (0..400)
        .step_by(4)
        .flat_map(|y| {
            (0..1000)
                .step_by(4)
                .map(move |x| (f64::from(x), f64::from(y)))
        })
        .find(|&(x, y)| {
            s.timeline()
                .hit_test(&s.solution().dates, x, y)
                .is_some_and(|task| task != 0)
        })
        .expect("a leaf bar is on the canvas")
}

#[test]
fn it_turns_the_filter_and_lookahead_off_and_lists_every_row() {
    let mut s = session();
    assert!(s.set_status_filter(Some(StatusKind::InProgress)));
    assert!(s.set_lookahead(0, 520));
    assert_eq!(rows(&s), 2, "the filter hides two leaves");
    assert!(s.clear_filters());
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert_eq!(s.row_filter().name(), "off", "Status.filter reads off");
    assert!(s.lookahead().is_none());
    assert_eq!(rows(&s), 4);
}

#[test]
fn it_clears_a_lookahead_alone() {
    let mut s = session();
    assert!(s.set_lookahead(10_000_000, 1));
    assert_eq!(rows(&s), 0);
    assert!(s.clear_filters());
    assert_eq!(rows(&s), 4);
}

#[test]
fn it_waits_for_a_bar_press() {
    let mut s = session();
    assert!(s.set_status_filter(Some(StatusKind::InProgress)));
    assert!(s.set_lookahead(0, 520));
    let before = rows(&s);
    let (x, y) = leaf_bar_point(&s);
    s.pointer_down(x, y, false);
    assert!(
        !s.set_lookahead(0, 2),
        "the press is active, so a filter change waits"
    );
    assert!(!s.clear_filters());
    assert_eq!(s.row_filter(), RowFilter::Status(StatusKind::InProgress));
    assert!(s.lookahead().is_some());
    assert_eq!(rows(&s), before);
}
