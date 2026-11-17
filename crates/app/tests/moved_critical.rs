//! The critical-path clause of the move note against two full solves (gs#703): over a random
//! chain of duration edits, link edits, undos and redos, each note's joined and left counts equal
//! the criticality difference of `compute()` before and after, task by task.

#![expect(clippy::unwrap_used, reason = "a test's unwrap is its assertion")]

use crate::common;

use app::{Loaded, Session, edit::Edit, load_bytes};
use common::clock;
use engine::{DAY, DepId, LinkType, Rng, Schedule, Task, TaskId};
use import::ProjectSelection;
use timeline::Timeline;

const LEDGER_XER: &[u8] =
    include_bytes!("../../../packages/gantt/tests/fixtures/ledger-timeline.xer");

fn ledger() -> Session {
    let Loaded {
        schedule,
        solution,
        order,
        outline,
        origin,
        meta,
        ..
    } = load_bytes(LEDGER_XER, &ProjectSelection::Unspecified, &clock).expect("the file loads");
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 600.0, 1.0),
        meta,
        clock,
    )
    .with_outline(outline)
    .expect("a valid outline")
    .with_origin(origin)
}

fn critical(s: &Session) -> Vec<bool> {
    let fresh = s.schedule().clone().compute().expect("acyclic");
    fresh
        .dates
        .iter()
        .map(engine::TaskDates::is_critical)
        .collect()
}

/// (joined, left) between two criticality vectors.
fn difference(before: &[bool], after: &[bool]) -> (u32, u32) {
    let count = |from: bool| {
        before
            .iter()
            .zip(after)
            .filter(|&(&b, &a)| b == from && a != from)
            .count() as u32
    };
    (count(false), count(true))
}

#[test]
fn random_edit_and_undo_chains_count_the_criticality_difference_of_two_computes() {
    let (mut notes, mut joined, mut left) = (0u32, 0u32, 0u32);
    for seed in 0..40u64 {
        let mut s = ledger();
        let mut rng = Rng::new(seed ^ 0x0c41_fca1);
        let n = s.schedule().tasks().len() as i64;
        for step in 0..25 {
            let before = critical(&s);
            let seq = s.move_report().map_or(0, |r| r.seq);
            match rng.range(0, 6) {
                0 | 1 => {
                    let task = TaskId(rng.range(0, n - 1) as u32);
                    let minutes = rng.range(0, 40) * 480;
                    let _ = s.edit(Edit::SetDuration { task, minutes });
                }
                2 | 3 => {
                    let pred = TaskId(rng.range(0, n - 1) as u32);
                    let succ = TaskId(rng.range(0, n - 1) as u32);
                    let lag = rng.range(0, 3) * 480;
                    let _ = s.edit(Edit::AddLink {
                        pred,
                        succ,
                        kind: LinkType::FS,
                        lag,
                    });
                }
                4 => {
                    let task = TaskId(rng.range(0, n - 1) as u32);
                    let snet = Some(rng.range(0, 60) * DAY);
                    let _ = s.edit(Edit::Move { task, snet });
                }
                5 => {
                    let _ = s.undo();
                }
                _ => {
                    let _ = s.redo();
                }
            }
            let Some(r) = s.move_report().filter(|r| r.seq != seq) else {
                continue;
            };
            let expected = difference(&before, &critical(&s));
            assert_eq!(
                (r.joined, r.left),
                expected,
                "seed {seed} step {step}: {}",
                r.text
            );
            let clause = format!("critical {} joined, {} left", expected.0, expected.1);
            assert_eq!(
                r.text.contains("critical"),
                expected != (0, 0),
                "{}",
                r.text
            );
            assert!(
                expected == (0, 0) || r.text.ends_with(&clause),
                "{}",
                r.text
            );
            notes += 1;
            joined += expected.0;
            left += expected.1;
        }
    }
    // The chains really exercise what they claim to.
    assert!(
        notes > 200 && joined > 20 && left > 20,
        "{notes} {joined} {left}"
    );
}

fn set_data_date(s: &mut Session, days: i64) {
    s.edit(Edit::SetDataDate {
        date: Some(days * DAY),
    })
    .expect("applies");
}

/// A data-date change recomputes the whole schedule but takes no before-state, so its note says
/// nothing of the critical path (and never counts every critical task as joined). Undo and redo
/// of it are the same.
#[test]
fn a_data_date_change_makes_no_critical_clause_on_a_fresh_session() {
    let mut s = ledger();
    set_data_date(&mut s, 20);
    let r = s.move_report().expect("the data date moved tasks");
    assert_eq!((r.joined, r.left), (0, 0), "{}", r.text);
    assert!(!r.text.contains("critical"), "{}", r.text);

    assert!(s.undo().unwrap());
    let r = s.move_report().expect("undo moved them back");
    assert_eq!((r.joined, r.left), (0, 0), "{}", r.text);
    assert!(s.redo().unwrap());
    let r = s.move_report().expect("redo moved them again");
    assert!(!r.text.contains("critical"), "{}", r.text);
}

/// After an earlier edit the buffer holds that edit's before-state: the data-date note must not
/// compare against it.
#[test]
fn a_data_date_change_after_an_edit_does_not_reuse_its_before_state() {
    let mut s = ledger();
    s.edit(Edit::SetDuration {
        task: TaskId(16),
        minutes: 4800,
    })
    .expect("applies");
    set_data_date(&mut s, 20);
    let r = s.move_report().expect("the data date moved tasks");
    assert!(!r.text.contains("critical"), "{}", r.text);
    assert!(s.undo().unwrap());
    assert!(
        !s.move_report()
            .expect("undo reports")
            .text
            .contains("critical")
    );
}

/// A refused edit leaves no before-state behind for the next change.
#[test]
fn a_refused_edit_leaves_no_before_state_for_a_data_date_change() {
    let mut s = ledger();
    let _ = s.edit(Edit::AddLink {
        pred: TaskId(9),
        succ: TaskId(9),
        kind: LinkType::FS,
        lag: 0,
    });
    set_data_date(&mut s, 20);
    let r = s.move_report().expect("the data date moved tasks");
    assert!(!r.text.contains("critical"), "{}", r.text);
}

/// A drag release names no critical change, but the undo of the drag is a discrete edit and
/// counts like one.
#[test]
fn undo_of_a_drag_counts_the_criticality_difference_of_two_computes() {
    let (mut s, _, _, (x, y)) = common::setup_with(true);
    s.pointer_down(x, y, false);
    s.pointer_move(x + 200.0, y);
    s.apply_drag();
    s.pointer_up();
    let r = s.move_report().expect("the drag moved others");
    assert_eq!(
        (r.joined, r.left),
        (0, 0),
        "a drag release does not say it yet"
    );
    let before = critical(&s);
    assert!(s.undo().unwrap());
    let expected = difference(&before, &critical(&s));
    let r = s.move_report().expect("undo moved the others back");
    assert_eq!((r.joined, r.left), expected, "{}", r.text);
}

fn assert_equals_compute(s: &Session) {
    assert_eq!(*s.solution(), s.schedule().clone().compute().unwrap());
}

fn set_duration(s: &mut Session, task: TaskId, minutes: i64) {
    s.edit(Edit::SetDuration { task, minutes })
        .expect("applies");
}

fn early(s: &Session) -> Vec<(i64, i64)> {
    s.solution().dates.iter().map(|d| (d.es, d.ef)).collect()
}

/// Two chains on a Monday-to-Friday calendar of eight-hour days: A (10 days) then B (10 days),
/// and C (15 days) alone. A and B are critical; C has 5 days of float.
fn two_chains() -> Session {
    let mut schedule = Schedule::new();
    let calendar = schedule.add_calendar(common::collapse::standard_calendar());
    for days in [10, 10, 15] {
        schedule
            .add_task(Task {
                duration: days * 480,
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
    }
    schedule
        .add_link(engine::Dependency {
            pred: TaskId(0),
            succ: TaskId(1),
            kind: LinkType::FS,
            lag: 0,
        })
        .expect("a link");
    let solution = schedule.compute().expect("acyclic");
    let order = timeline::order_by_start(&solution.dates);
    let meta = (0..3)
        .map(|i| app::meta::TaskMeta {
            name: format!("T{i}"),
            ..Default::default()
        })
        .collect();
    Session::new(
        schedule,
        solution,
        Timeline::new(order, 1200.0, 600.0, 1.0),
        meta,
        clock,
    )
}

#[test]
fn the_note_names_tasks_that_joined_and_left_the_critical_path() {
    let mut s = two_chains();
    let crit = |s: &Session| -> Vec<bool> {
        s.solution()
            .dates
            .iter()
            .map(engine::TaskDates::is_critical)
            .collect()
    };
    assert_eq!(crit(&s), [true, true, false]);
    let c = early(&s)[2];
    // A drops to 2 days: the chain is 12 days, shorter than C, which turns critical with the
    // same early dates. Only B moves besides A.
    set_duration(&mut s, TaskId(0), 2 * 480);
    assert_eq!(early(&s)[2], c, "C's early dates did not move");
    assert_eq!(crit(&s), [false, false, true]);
    let r = s.move_report().expect("B moved");
    assert_eq!((r.others, r.joined, r.left), (1, 1, 2), "{}", r.text);
    assert_eq!(
        r.text,
        "1 task moved · finish -7 days · critical 1 joined, 2 left"
    );
    assert_equals_compute(&s);
}

#[test]
fn a_link_edit_names_the_critical_path_change() {
    let mut s = two_chains();
    // C then A: all three are on one chain of 35 days, so C joins and nothing leaves.
    s.edit(Edit::AddLink {
        pred: TaskId(2),
        succ: TaskId(0),
        kind: LinkType::FS,
        lag: 0,
    })
    .expect("applies");
    let r = s.move_report().expect("A and B moved");
    assert_eq!((r.others, r.joined, r.left), (2, 1, 0), "{}", r.text);
    assert!(
        r.text.ends_with(" · critical 1 joined, 0 left"),
        "{}",
        r.text
    );
    assert_equals_compute(&s);

    // Removing it puts C back off the path.
    let dep = DepId(u32::try_from(s.schedule().dependencies().len() - 1).unwrap());
    s.edit(Edit::RemoveLink { dep }).expect("applies");
    let r = s.move_report().expect("A and B moved back");
    assert_eq!((r.joined, r.left), (0, 1), "{}", r.text);
    assert_equals_compute(&s);
}

#[test]
fn undo_swaps_joined_and_left() {
    let mut s = two_chains();
    set_duration(&mut s, TaskId(0), 2 * 480);
    let r = s.move_report().expect("B moved");
    assert_eq!((r.joined, r.left), (1, 2));

    assert!(s.undo().unwrap());
    let r = s.move_report().expect("undo moved B back");
    assert_eq!((r.others, r.joined, r.left), (1, 2, 1), "{}", r.text);
    assert_eq!(
        r.text,
        "1 task moved · finish +7 days · critical 2 joined, 1 left"
    );

    assert!(s.redo().unwrap());
    let r = s.move_report().expect("redo moved B again");
    assert_eq!((r.joined, r.left), (1, 2), "{}", r.text);
    assert_equals_compute(&s);
}

#[test]
fn a_criticality_change_alone_makes_no_note_when_only_the_edited_task_moves() {
    let mut s = two_chains();
    // B is last in its chain: shortening it moves nothing else, yet the chain becomes shorter
    // than C and the critical path changes.
    set_duration(&mut s, TaskId(1), 480);
    assert!(s.solution().dates[2].is_critical(), "C turned critical");
    assert!(s.move_report().is_none());
}
