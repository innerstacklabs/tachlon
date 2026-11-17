//! The Joined critical and Left critical row filters (gs#703): with a baseline set, the table keeps
//! the tasks that are critical now and were not then, or the reverse, and the summaries above
//! them. Their sets are the ones `Status.baseline` counts as `joined` and `left`.

use crate::common;

use app::baseline::{Baseline, BaselineTask};
use app::edit::Edit;
use app::{RowFilter, Session, meta::TaskMeta, outline::Outline};
use common::{baseline, clock};
use engine::{Calendar, Schedule, Task, TaskId};
use std::sync::Arc;
use timeline::Timeline;

const DAY: i64 = 480;

fn calendar() -> Calendar {
    let day = vec![(480_u16, 960)];
    let week = std::array::from_fn(|d| if d < 5 { day.clone() } else { vec![] });
    Calendar::new(week, &[]).expect("a valid calendar")
}

/// Task 0 is a summary over the leaves 1 to 3, which have no links: 40, 30 and 20 days. Task 1
/// sets the project finish, so it alone is critical; tasks 2 and 3 have 10 and 20 days of float.
fn session() -> Session {
    let mut schedule = Schedule::new();
    let cal = schedule.add_calendar(calendar());
    for days in [1, 40, 30, 20] {
        let task = Task {
            duration: days * DAY,
            calendar: cal,
            snet: None,
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

fn set_days(s: &mut Session, task: u32, days: i64) {
    let edit = Edit::SetDuration {
        task: TaskId(task),
        minutes: days * DAY,
    };
    s.edit(edit).expect("applies");
}

#[test]
fn joined_keeps_the_tasks_that_became_critical_since_the_baseline() {
    let mut s = session();
    baseline::set(&mut s);
    // Task 2 grows to the project's length: it joins the path, task 1 stays on it.
    set_days(&mut s, 2, 40);
    let chip = s.baseline_diff().expect("a baseline is set");
    assert_eq!((chip.joined, chip.left), (1, 0), "the chip's counts");

    assert!(s.set_joined_critical_only(true));
    assert!(s.joined_critical_only());
    assert_eq!(s.row_filter(), RowFilter::JoinedCritical);
    // Task 2 and its summary; the count of leaf rows is the chip's count.
    assert_eq!(listed(&s), vec![0, 2]);

    assert!(s.set_joined_critical_only(false));
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert_eq!(listed(&s), vec![0, 1, 2, 3]);
}

#[test]
fn left_keeps_the_tasks_that_were_critical_and_are_not_now() {
    let mut s = session();
    baseline::set(&mut s);
    // Task 1 shrinks below task 2: task 2 is the project's finish now.
    set_days(&mut s, 1, 25);
    let chip = s.baseline_diff().expect("a baseline is set");
    assert_eq!((chip.joined, chip.left), (1, 1), "the chip's counts");

    assert!(s.set_left_critical_only(true));
    assert_eq!(s.row_filter(), RowFilter::LeftCritical);
    assert_eq!(listed(&s), vec![0, 1]);
    assert!(s.set_joined_critical_only(true));
    assert!(!s.left_critical_only(), "one filter at a time");
    assert_eq!(listed(&s), vec![0, 2]);
}

#[test]
fn the_filters_follow_an_edit_and_its_undo() {
    let mut s = session();
    baseline::set(&mut s);
    assert!(s.set_joined_critical_only(true));
    assert_eq!(listed(&s), Vec::<u32>::new(), "nothing has joined yet");
    set_days(&mut s, 2, 40);
    assert_eq!(listed(&s), vec![0, 2]);
    s.undo().expect("undoes");
    assert_eq!(listed(&s), Vec::<u32>::new());
}

#[test]
fn they_need_a_baseline_and_go_off_when_it_is_cleared() {
    let mut s = session();
    assert!(!s.set_joined_critical_only(true));
    assert!(!s.set_left_critical_only(true));
    baseline::set(&mut s);
    assert!(s.set_left_critical_only(true));
    s.edit(Edit::ClearBaseline).expect("clears");
    assert_eq!(s.row_filter(), RowFilter::Off);
    assert_eq!(listed(&s), vec![0, 1, 2, 3]);
}

#[test]
fn a_task_the_baseline_has_no_float_for_is_in_neither_filter() {
    let mut s = session();
    // A baseline as a file saves it: dates, no float.
    let tasks = s
        .solution()
        .dates
        .iter()
        .map(|d| {
            Some(BaselineTask {
                es: d.es,
                ef: d.ef,
                float: None,
            })
        })
        .collect();
    let from_file = Baseline::from_parts(tasks, s.solution().project_finish);
    s.edit(Edit::SetBaseline {
        baseline: Some(Arc::new(from_file)),
    })
    .expect("a baseline sets");
    set_days(&mut s, 2, 40);
    assert!(s.set_joined_critical_only(true));
    assert_eq!(listed(&s), Vec::<u32>::new());
    assert!(s.set_left_critical_only(true));
    assert_eq!(listed(&s), Vec::<u32>::new());
}
