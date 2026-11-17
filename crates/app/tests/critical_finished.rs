//! A finished activity is not critical (gs#825): P6 and MS Project leave finished work off the
//! critical path, and the remaining work decides it. A finished task keeps float 0 (decision
//! 0008) but is not drawn, marked or listed as critical, and "Critical only" leaves it out.

use crate::common;

use app::{Session, edit::Edit, load_bytes, meta::TaskMeta, outline::Outline};
use common::{clock, collapse::standard_calendar};
use engine::{Dependency, LinkType, Schedule, Task, TaskId};
use import::ProjectSelection;
use timeline::Timeline;

/// The screencast's P6 file: actual dates and a data date.
const FIT_OUT: &[u8] = include_bytes!("../../../demos/screencast/fixtures/fit-out.xer");

const HOUR: i64 = 60;

fn session_of(
    mut schedule: Schedule,
    order: Vec<u32>,
    meta: Vec<TaskMeta>,
    outline: Outline,
) -> Session {
    let solution = schedule.compute().expect("computes");
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1_000.0, 400.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("the outline fits")
}

fn critical(session: &Session, task: u32) -> bool {
    session.solution().dates[task as usize].is_critical()
}

fn listed(session: &Session, task: u32) -> bool {
    session.timeline().order().contains(&task)
}

/// Three tasks in a finish-to-start chain, all on the critical path.
fn chain() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(standard_calendar());
    for _ in 0..3 {
        schedule
            .add_task(Task {
                duration: 8 * HOUR,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("the calendar belongs to the schedule");
    }
    for (pred, succ) in [(0, 1), (1, 2)] {
        schedule
            .add_link(Dependency {
                pred: TaskId(pred),
                succ: TaskId(succ),
                kind: LinkType::FS,
                lag: 0,
            })
            .expect("links");
    }
    let meta = vec![TaskMeta::default(); 3];
    let wbs = [TaskId(0), TaskId(1), TaskId(2)];
    let outline = Outline::new(&meta, &wbs, &schedule).expect("a flat outline");
    session_of(schedule, vec![0, 1, 2], meta, outline)
}

#[test]
fn a_finished_task_on_a_zero_float_chain_is_not_critical_and_the_rest_of_the_chain_is() {
    let mut s = chain();
    assert!((0..3).all(|t| critical(&s, t)));

    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(0),
        actual_finish: Some(8 * HOUR),
        remaining: None,
    })
    .expect("applies");

    let first = s.solution().dates[0];
    assert_eq!(
        first.float, 0,
        "decision 0008: a finished task's float is 0"
    );
    assert!(!critical(&s, 0), "the finished task is not critical");
    assert!(
        critical(&s, 1) && critical(&s, 2),
        "unfinished work stays critical"
    );

    assert!(s.set_critical_only(true));
    assert!(!listed(&s, 0), "Critical only leaves the finished task out");
    assert!(listed(&s, 1) && listed(&s, 2));
}

#[test]
fn a_started_task_with_remaining_work_stays_critical() {
    let mut s = chain();
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(0),
        actual_finish: None,
        remaining: Some(4 * HOUR),
    })
    .expect("applies");
    assert!(critical(&s, 0));
}

#[test]
fn undoing_the_finish_makes_the_task_critical_again() {
    let mut s = chain();
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(0),
        actual_finish: Some(8 * HOUR),
        remaining: None,
    })
    .expect("applies");
    assert!(!critical(&s, 0));
    assert!(s.undo().expect("undoes"));
    assert!(critical(&s, 0));
}

#[test]
fn no_finished_activity_in_the_p6_sample_is_critical_and_unfinished_ones_still_are() {
    let loaded = load_bytes(FIT_OUT, &ProjectSelection::Unspecified, &|| 0.0).expect("opens");
    let finished: Vec<u32> = loaded
        .schedule
        .tasks()
        .iter()
        .enumerate()
        .filter(|(_, task)| task.actual_finish.is_some())
        .map(|(i, _)| i as u32)
        .collect();
    let mut s = session_of(loaded.schedule, loaded.order, loaded.meta, loaded.outline);
    assert_ne!(finished, [] as [u32; 0]);
    assert!(
        finished
            .iter()
            .all(|&t| s.solution().dates[t as usize].float == 0),
        "the sample's finished activities sit at zero float"
    );

    let flagged = finished.iter().filter(|&&t| critical(&s, t)).count();
    assert_eq!(flagged, 0, "finished activities flagged critical");
    let unfinished_critical = (0..s.solution().dates.len() as u32)
        .filter(|t| !finished.contains(t) && critical(&s, *t))
        .count();
    assert!(
        unfinished_critical > 0,
        "the remaining work is still critical"
    );

    assert!(s.set_critical_only(true));
    assert!(finished.iter().all(|&t| !listed(&s, t)));
}

#[test]
fn a_baseline_set_on_the_p6_sample_reads_no_joined_and_no_left() {
    let loaded = load_bytes(FIT_OUT, &ProjectSelection::Unspecified, &|| 0.0).expect("opens");
    let mut s = session_of(loaded.schedule, loaded.order, loaded.meta, loaded.outline);
    s.edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    let diff = s.baseline_diff().expect("a baseline is in force");
    assert_eq!((diff.joined, diff.left), (0, 0), "{diff:?}");
}

#[test]
fn a_task_finished_after_the_baseline_is_not_counted_as_leaving_the_path() {
    let mut s = chain();
    s.edit(Edit::SetBaseline { baseline: None })
        .expect("a baseline sets");
    s.edit(Edit::SetProgress {
        task: TaskId(0),
        actual_start: Some(0),
        actual_finish: Some(8 * HOUR),
        remaining: None,
    })
    .expect("applies");
    let diff = s.baseline_diff().expect("a baseline is in force");
    assert_eq!((diff.joined, diff.left), (0, 0), "{diff:?}");
}
