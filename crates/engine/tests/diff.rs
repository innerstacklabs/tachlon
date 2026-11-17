//! `engine::diff`: what differs between two solutions (gs#703).
//!
//! One hand-solved case, one case for tasks added and removed, and a random
//! edit chain whose every step is checked against two full `compute()` calls.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

mod common;

use common::{cont_cal, dep, fuzz, seeds, task};
use engine::LinkType::FS;
use engine::{Dependency, Rng, Schedule, Solution, TaskDates, TaskId, diff};

const H: i64 = 60;

/// Hand-solved on a 24/7 calendar, in hours: A(3) -> B(4), A -> C(2),
/// B -> D(5), C -> D, D -> E(1).
///
/// Before the edit, C lasts 2 hours:
///   A 0-3, B 3-7, C 3-5 (late 5-7, float 2), D 7-12, E 12-13. Critical A, B, D, E.
/// The edit makes C last 6 hours. A and C now take 3 + 6 = 9 hours before D,
/// B's 3 + 4 = 7 hours leave it 2 hours of float:
///   A 0-3, B 3-7 (late 5-9, float 2), C 3-9 (float 0), D 9-14, E 14-15.
///   Critical A, C, D, E. Project finish 13 -> 15.
#[test]
#[expect(
    clippy::many_single_char_names,
    reason = "the tasks are named by the letters the doc comment and the comments use"
)]
fn the_diff_equals_a_hand_solved_five_task_case() {
    let mut s = Schedule::new();
    let c = s.add_calendar(cont_cal());
    let a = task(&mut s, c, 3 * H, None);
    let b = task(&mut s, c, 4 * H, None);
    let cc = task(&mut s, c, 2 * H, None);
    let d = task(&mut s, c, 5 * H, None);
    let e = task(&mut s, c, H, None);
    for (p, q) in [(a, b), (a, cc), (b, d), (cc, d), (d, e)] {
        dep(&mut s, p, q, FS, 0);
    }
    let before = s.compute().unwrap();
    assert_eq!(before.project_finish, 13 * H);
    s.set_duration(cc, 6 * H).unwrap();
    let after = s.compute().unwrap();
    assert_eq!(after.project_finish, 15 * H);

    let got = diff(&before, &after);

    // (start change, finish change, float change) per task, by hand.
    let want = [
        (a, 0, 0, 0),
        (b, 0, 0, 2 * H),
        (cc, 0, 4 * H, -2 * H),
        (d, 2 * H, 2 * H, 0),
        (e, 2 * H, 2 * H, 0),
    ];
    assert_eq!(got.changes.len(), want.len());
    for (change, &(t, start, finish, float)) in got.changes.iter().zip(&want) {
        assert_eq!(change.task, t);
        assert_eq!(change.start, start, "start change of {t}");
        assert_eq!(change.finish, finish, "finish change of {t}");
        assert_eq!(change.float, float, "float change of {t}");
    }
    assert_eq!(got.joined_critical, vec![cc]);
    assert_eq!(got.left_critical, vec![b]);
    assert_eq!(got.project_finish, 2 * H);
    assert_eq!((got.added, got.removed), (0, 0));
}

/// A solution with `n` tasks, task `i` starting at `i` and lasting one minute.
fn solution_of(n: usize) -> Solution {
    let dates = (0..n as i64)
        .map(|i| TaskDates {
            es: i,
            ef: i + 1,
            ls: i,
            lf: i + 1,
            float: 0,
            finished: false,
        })
        .collect();
    Solution {
        dates,
        project_finish: n as i64,
    }
}

/// Tasks are matched by id: ids only in the new solution are added, ids only
/// in the old one are removed, and the added or removed tasks appear nowhere
/// else in the diff.
#[test]
fn tasks_only_in_one_solution_are_counted_not_compared() {
    let (short, long) = (solution_of(3), solution_of(5));

    let grew = diff(&short, &long);
    assert_eq!((grew.added, grew.removed), (2, 0));
    assert_eq!(grew.changes.len(), 3);
    assert!(grew.changes.iter().all(|c| (c.start, c.finish) == (0, 0)));
    assert_eq!(grew.project_finish, 2);
    assert!(grew.joined_critical.is_empty() && grew.left_critical.is_empty());

    let shrank = diff(&long, &short);
    assert_eq!((shrank.added, shrank.removed), (0, 2));
    assert_eq!(shrank.changes.len(), 3);
    assert_eq!(shrank.project_finish, -2);

    let same = diff(&long, &long);
    assert_eq!((same.added, same.removed), (0, 0));
    assert_eq!(same.project_finish, 0);
}

/// Two empty solutions differ in nothing.
#[test]
fn two_empty_solutions_have_an_empty_diff() {
    let d = diff(&solution_of(0), &solution_of(0));
    assert!(d.changes.is_empty() && d.joined_critical.is_empty() && d.left_critical.is_empty());
    assert_eq!((d.project_finish, d.added, d.removed), (0, 0, 0));
}

/// One random edit: a duration, a start-no-earlier-than constraint, a new task
/// or a new link, a deletion or a restore. An edit the schedule refuses (an
/// unknown task, a repeated link) changes nothing, and the diff of that step
/// must then be empty. Returns the kind of edit tried, for the coverage count.
fn random_edit(s: &mut Schedule, rng: &mut Rng) -> usize {
    let n = s.tasks().len() as i64;
    let any = |rng: &mut Rng| TaskId(rng.range(0, n - 1) as u32);
    let kind = rng.range(0, 5) as usize;
    match kind {
        0 | 1 => {
            let t = any(rng);
            let _ = s.set_duration(t, rng.range(0, 600));
        }
        2 => {
            let t = any(rng);
            let snet = rng.chance(0.7).then(|| rng.range(0, 14 * 1440));
            let _ = s.set_snet(t, snet);
        }
        3 => {
            let calendar = engine::CalendarId(rng.range(0, s.calendars().len() as i64 - 1) as u32);
            let _ = s.add_task(engine::Task {
                duration: rng.range(0, 300),
                calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            });
            // A link from a lower id to the new, highest id cannot close a loop.
            let new = TaskId(n as u32);
            let _ = s.add_dependency(Dependency {
                pred: any(rng),
                succ: new,
                kind: engine::LinkType::FS,
                lag: rng.range(-120, 120),
            });
        }
        4 => {
            let _ = s.delete_task(any(rng));
        }
        _ => {
            let _ = s.restore_task(any(rng));
        }
    }
    kind
}

/// What `diff` must report for two full solutions, written as a plain
/// subtraction, task by task.
fn assert_diff_is_subtraction(old: &Solution, new: &Solution, label: &str) {
    let got = diff(old, new);
    let both = old.dates.len().min(new.dates.len());

    assert_eq!(got.changes.len(), both, "{label}: one change per shared id");
    let (mut joined, mut left) = (Vec::new(), Vec::new());
    for i in 0..both {
        let (o, n) = (&old.dates[i], &new.dates[i]);
        let c = &got.changes[i];
        assert_eq!(c.task, TaskId(i as u32), "{label}: change order");
        assert_eq!(c.start, n.es - o.es, "{label}: start of task {i}");
        assert_eq!(c.finish, n.ef - o.ef, "{label}: finish of task {i}");
        assert_eq!(c.float, n.float - o.float, "{label}: float of task {i}");
        if n.float == 0 && o.float != 0 {
            joined.push(TaskId(i as u32));
        }
        if o.float == 0 && n.float != 0 {
            left.push(TaskId(i as u32));
        }
    }
    assert_eq!(
        got.joined_critical, joined,
        "{label}: joined the critical path"
    );
    assert_eq!(got.left_critical, left, "{label}: left the critical path");
    assert_eq!(
        got.project_finish,
        new.project_finish - old.project_finish,
        "{label}: project finish"
    );
    assert_eq!(
        got.added,
        new.dates.len().saturating_sub(old.dates.len()),
        "{label}: added"
    );
    assert_eq!(
        got.removed,
        old.dates.len().saturating_sub(new.dates.len()),
        "{label}: removed"
    );
}

/// Build a schedule, apply random edits, `compute()` before and after each,
/// and check the diff of the two solutions against a per-task subtraction. The
/// diff of the first solution against the last must equal the same
/// subtraction, and the starts of a task add up along the chain.
#[test]
fn random_edit_chains_diff_equal_two_full_computes() {
    let (mut edits, mut moved, mut joined, mut left, mut added) = ([0usize; 6], 0, 0, 0, 0);
    for seed in seeds(150) {
        let mut s = fuzz::schedule(seed);
        let mut rng = Rng::new(seed ^ 0x0d1f_f0ed);
        let first = s.compute().unwrap();
        let mut old = first.clone();
        let mut start_sum: Vec<i64> = vec![0; first.dates.len()];
        for step in 0..10 {
            edits[random_edit(&mut s, &mut rng)] += 1;
            let new = s.compute().unwrap();
            let label = format!("seed {seed} step {step}");
            assert_diff_is_subtraction(&old, &new, &label);

            let d = diff(&old, &new);
            moved += d
                .changes
                .iter()
                .filter(|c| c.start != 0 || c.finish != 0)
                .count();
            joined += d.joined_critical.len();
            left += d.left_critical.len();
            added += d.added;
            for c in &d.changes {
                start_sum[c.task.0 as usize] += c.start;
            }
            start_sum.resize(new.dates.len(), 0);
            old = new;
        }
        assert_diff_is_subtraction(&first, &old, &format!("seed {seed} first to last"));
        for (i, sum) in start_sum.iter().enumerate().take(first.dates.len()) {
            assert_eq!(
                *sum,
                old.dates[i].es - first.dates[i].es,
                "seed {seed} task {i}"
            );
        }
    }
    // The chains really exercise what they claim to.
    assert!(edits.iter().all(|&k| k > 50), "edit kinds {edits:?}");
    assert!(moved > 100 && joined > 20 && left > 20 && added > 20);
}
