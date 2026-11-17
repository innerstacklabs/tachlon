//! The Float lost row filter (gs#703): the table keeps the tasks whose total float is at least
//! five working days of their own calendar below the baseline's, and the summaries above them.

use crate::common;

use app::edit::Edit;
use app::{
    RowFilter, Session,
    baseline::{Baseline, BaselineTask},
    meta::TaskMeta,
    outline::Outline,
};
use common::{baseline, clock};
use engine::{Dependency, LinkType, Schedule, Task, TaskId};
use timeline::Timeline;

const DAY: i64 = 480;

/// Task 0 is a summary over the leaves 1 to 8. Task 1 runs 60 days and ends the project. Chains
/// R1 -> A (2, 3), R2 -> B (4, 5) and S -> E (6, 7) are ten days each, so every one has float.
/// Task 8 is a lone leaf.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(
        engine::Calendar::new(
            std::array::from_fn(|d| if d < 5 { vec![(480, 960)] } else { vec![] }),
            &[],
        )
        .expect("a valid calendar"),
    );
    for days in [1, 60, 10, 10, 10, 10, 10, 10, 10] {
        schedule
            .add_task(Task {
                duration: days * DAY,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("the calendar is in the schedule");
    }
    for (pred, succ) in [(2, 3), (4, 5), (6, 7)] {
        schedule
            .add_link(Dependency {
                pred: TaskId(pred),
                succ: TaskId(succ),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("links");
    }
    let mut meta = vec![TaskMeta::default(); 9];
    for leaf in &mut meta[1..] {
        leaf.parent = Some(TaskId(0));
    }
    let wbs: Vec<TaskId> = (0..9).map(TaskId).collect();
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a one-summary outline");
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new((0..9).collect(), 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

fn lengthen(s: &mut Session, task: u32, days: i64) {
    let minutes = s.schedule().tasks()[task as usize].duration + days * DAY;
    s.edit(Edit::SetDuration {
        task: TaskId(task),
        minutes,
    })
    .expect("applies");
}

fn listed(s: &Session) -> Vec<u32> {
    s.timeline().order().to_vec()
}

#[test]
fn float_lost_keeps_the_tasks_that_lost_five_working_days_and_their_summaries() {
    let mut s = session();
    baseline::set(&mut s);
    lengthen(&mut s, 2, 5); // R1 and A lose exactly five days
    lengthen(&mut s, 4, 4); // R2 and B lose four
    lengthen(&mut s, 6, -3); // S and E gain three
    let place = s.place_below(None);
    s.edit(Edit::Insert {
        place,
        name: "Added since".into(),
    })
    .expect("inserts");
    let added = s.schedule().tasks().len() as u32 - 1;

    let float = |s: &Session, t: u32| s.solution().dates[t as usize].float;
    let then = |s: &Session, t: u32| {
        s.baseline()
            .unwrap()
            .task(TaskId(t))
            .unwrap()
            .float
            .unwrap()
    };
    assert_eq!([2, 3].map(|t| then(&s, t) - float(&s, t)), [5 * DAY; 2]);
    assert_eq!([4, 5].map(|t| then(&s, t) - float(&s, t)), [4 * DAY; 2]);
    assert_eq!([6, 7].map(|t| float(&s, t) - then(&s, t)), [3 * DAY; 2]);
    assert!(s.baseline().unwrap().task(TaskId(added)).is_none());

    assert!(s.set_float_lost(true));
    assert!(s.float_lost());
    assert_eq!(s.row_filter(), RowFilter::FloatLost);
    // The summary (0) stays for the two tasks under it; the inserted task is in neither.
    assert_eq!(listed(&s), vec![0, 2, 3]);
    assert!(!listed(&s).contains(&added));

    assert!(s.set_float_lost(false));
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert!(listed(&s).len() > 3);
}

#[test]
fn it_is_one_filter_at_a_time_and_ends_with_the_baseline() {
    let mut s = session();
    assert!(!s.set_float_lost(true), "no baseline, no filter");
    baseline::set(&mut s);
    assert!(s.set_critical_only(true));
    assert!(s.set_float_lost(true));
    assert!(!s.critical_only() && s.float_lost());
    assert!(s.set_longest_path(false), "another off leaves it on");
    assert!(s.float_lost());
    assert!(s.set_longest_path(true));
    assert!(!s.float_lost());

    assert!(s.set_float_lost(true));
    s.edit(Edit::ClearBaseline).expect("clears");
    assert_eq!(s.row_filter(), RowFilter::Off);
}

#[test]
fn a_baseline_from_a_file_has_no_float_to_compare() {
    let s = session();
    let tasks = (0..9)
        .map(|t| {
            let dates = &s.solution().dates[t];
            Some(BaselineTask {
                es: dates.es,
                ef: dates.ef,
                float: None,
            })
        })
        .collect();
    let finish = s.solution().project_finish;
    let mut s = s.with_baseline(Baseline::from_parts(tasks, finish));
    assert!(!s.baseline_has_float());
    assert!(!s.set_float_lost(true));
    assert_eq!(s.row_filter(), RowFilter::Off);
}

#[test]
fn a_finished_task_that_lost_five_days_is_not_kept() {
    let mut s = session();
    baseline::set(&mut s);
    lengthen(&mut s, 2, 5); // R1 and A lose five days
    assert!(s.set_float_lost(true));
    assert_eq!(listed(&s), vec![0, 2, 3]);

    // A is worked and finished in its first ten days. Its float is 0, which is below the
    // baseline's by far more than five days, so only the finished test can keep it out.
    s.edit(Edit::SetProgress {
        task: TaskId(3),
        actual_start: Some(0),
        actual_finish: Some(10 * DAY),
        remaining: None,
    })
    .expect("applies");
    let then = s
        .baseline()
        .unwrap()
        .task(TaskId(3))
        .unwrap()
        .float
        .unwrap();
    let dates = &s.solution().dates[3];
    assert!(dates.finished && then - dates.float >= 5 * DAY);
    assert!(!listed(&s).contains(&3));
}
