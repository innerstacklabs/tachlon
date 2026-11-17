//! Cycle detection, input validation, and the topological-order cache.

mod common;

use common::{cont_cal, dep, task};
use engine::LinkType::{FS, SS};
use engine::{Dependency, Schedule, ScheduleError, Task, TaskId};

fn assert_is_cycle(s: &Schedule, cycle: &[TaskId]) {
    assert_ne!(cycle, []);
    for i in 0..cycle.len() {
        let (p, q) = (cycle[i], cycle[(i + 1) % cycle.len()]);
        assert!(
            s.dependencies().iter().any(|d| d.pred == p && d.succ == q),
            "{p} -> {q} is not a dependency"
        );
    }
}

#[test]
fn three_cycle_is_named() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let t: Vec<TaskId> = (0..5).map(|_| task(&mut s, c, 60, None)).collect();
    dep(&mut s, t[0], t[1], FS, 0); // a tail into the cycle
    dep(&mut s, t[1], t[2], FS, 0);
    dep(&mut s, t[2], t[3], SS, 0);
    dep(&mut s, t[3], t[1], FS, 0);
    dep(&mut s, t[3], t[4], FS, 0); // a task after it
    match s.compute() {
        Err(ScheduleError::Cycle(cycle)) => {
            let mut sorted = cycle.clone();
            sorted.sort();
            assert_eq!(
                sorted,
                vec![t[1], t[2], t[3]],
                "exactly the cycle, not the tail"
            );
            assert_is_cycle(&s, &cycle);
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn two_cycle_among_acyclic_tasks() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let t: Vec<TaskId> = (0..6).map(|_| task(&mut s, c, 60, None)).collect();
    dep(&mut s, t[0], t[1], FS, 0);
    dep(&mut s, t[1], t[2], FS, 0);
    dep(&mut s, t[4], t[5], FS, 0);
    dep(&mut s, t[5], t[4], FS, 0);
    match s.topological_order() {
        Err(ScheduleError::Cycle(cycle)) => {
            assert_eq!(cycle.len(), 2);
            assert_is_cycle(&s, &cycle);
        }
        other => panic!("expected a cycle, got {other:?}"),
    }
}

#[test]
fn validation() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let a = task(&mut s, c, 60, None);
    let bad_cal = Task {
        duration: 1,
        calendar: engine::CalendarId(9),
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    assert!(matches!(
        s.add_task(bad_cal),
        Err(ScheduleError::UnknownCalendar(_))
    ));
    let neg = Task {
        duration: -1,
        calendar: c,
        snet: None,
        limit: engine::Limit::default(),
        deleted: false,
        actual_start: None,
        actual_finish: None,
        remaining: None,
    };
    assert!(matches!(
        s.add_task(neg),
        Err(ScheduleError::NegativeDuration(_))
    ));
    let self_dep = Dependency {
        pred: a,
        succ: a,
        kind: FS,
        lag: 0,
    };
    assert_eq!(
        s.add_dependency(self_dep),
        Err(ScheduleError::SelfDependency(a))
    );
    let unknown = Dependency {
        pred: a,
        succ: TaskId(7),
        kind: FS,
        lag: 0,
    };
    assert_eq!(
        s.add_dependency(unknown),
        Err(ScheduleError::UnknownTask(TaskId(7)))
    );
}

#[test]
fn order_cache_survives_constraint_changes_only() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let a = task(&mut s, c, 60, None);
    let b = task(&mut s, c, 60, None);
    dep(&mut s, a, b, FS, 0);
    let order = s.topological_order().unwrap();
    assert_eq!(order, vec![a, b]);
    assert!(s.has_cached_order());
    s.set_snet(a, Some(600)).unwrap();
    assert!(s.has_cached_order(), "a constraint change keeps the order");
    assert_eq!(s.compute().unwrap().get(b).es, 660);
    dep(&mut s, b, a, FS, 0);
    assert!(!s.has_cached_order(), "a new dependency drops it");
    assert!(matches!(s.compute(), Err(ScheduleError::Cycle(_))));
}
