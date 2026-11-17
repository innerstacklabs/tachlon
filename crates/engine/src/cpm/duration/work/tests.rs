//! The work queue alone: order, dedupe and the push count.

use super::super::seam::ENQUEUED;
use super::*;
use crate::calendar::Calendar;
use crate::model::{Dependency, LinkType, Schedule, Task, TaskId};

/// Tasks `0..n` linked `n-1 -> n-2 -> ... -> 0`, so a task's rank is `n - 1 -
/// id`: the ids run against the order. Returns the schedule with its graph built.
fn reversed(n: u32) -> Schedule {
    let mut s = Schedule::new();
    let day = vec![(0, 1440)];
    let cal = s.add_calendar(Calendar::new(std::array::from_fn(|_| day.clone()), &[]).unwrap());
    for _ in 0..n {
        let task = Task {
            duration: 10,
            calendar: cal,
            snet: None,
            limit: crate::model::Limit::default(),
            deleted: false,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        };
        s.add_task(task).unwrap();
    }
    for i in 1..n {
        let dep = Dependency {
            pred: TaskId(i),
            succ: TaskId(i - 1),
            kind: LinkType::FS,
            lag: 0,
        };
        s.add_dependency(dep).unwrap();
    }
    s.graph().unwrap();
    s
}

fn drain(q: &mut WorkQueue<'_>) -> Vec<u32> {
    std::iter::from_fn(|| q.pop()).collect()
}

#[test]
fn forward_pops_ascending_rank_and_backward_descending() {
    let s = reversed(6);
    let g = s.graph.as_ref().unwrap();
    assert_eq!(g.order, [5, 4, 3, 2, 1, 0], "the ids run against the order");
    for (direction, want) in [
        (Direction::Forward, [5, 3, 2, 0]),
        (Direction::Backward, [0, 2, 3, 5]),
    ] {
        let mut q = WorkQueue::new(g, direction);
        for t in [0, 3, 5, 2] {
            q.push(t);
        }
        assert_eq!(drain(&mut q), want, "{direction:?}");
    }
}

#[test]
fn a_task_reached_from_several_sides_is_queued_once() {
    let s = reversed(4);
    let g = s.graph.as_ref().unwrap();
    ENQUEUED.set(0);
    let mut q = WorkQueue::new(g, Direction::Forward);
    assert!(q.push(2));
    assert!(!q.push(2), "already queued");
    assert!(q.push(1));
    assert!(!q.push(2));
    assert_eq!(ENQUEUED.get(), 2, "only the pushes that queued count");
    assert_eq!(drain(&mut q), [2, 1]);
    assert_eq!(q.pop(), None);
}

#[test]
fn work_found_while_draining_comes_out_in_order() {
    // Pop 3, discover 1 and 2 (higher ranks, in any order), 2 twice.
    let s = reversed(5);
    let g = s.graph.as_ref().unwrap();
    let mut q = WorkQueue::new(g, Direction::Forward);
    q.push(3);
    let mut got = vec![q.pop().unwrap()];
    for t in [1, 2, 1, 2] {
        q.push(t);
    }
    got.extend(drain(&mut q));
    assert_eq!(got, [3, 2, 1]);
}
