//! The chain of SNET edits the cone tests run, and the progress sweep that builds on it, shared by
//! `cone.rs`: after each edit the cone result must equal a full compute.

use super::{check_invariants, equiv, fuzz, oracle};
use engine::{DAY, Rng, Schedule, Solution, TaskId, stress};

/// What a chain of edits did, to show the test reaches the cases it names.
#[derive(Default, Debug)]
pub struct Seen {
    pub edits: usize,
    /// The edit changed no early date of the edited task.
    pub unchanged: usize,
    /// The edit moved the project finish (the backward pass runs in full).
    pub finish_moved: usize,
    /// The edit left the project finish alone but moved a task's float.
    pub float_only: usize,
    /// The edit moved at least ten tasks.
    pub wide: usize,
    /// The edit made the schedule infeasible; both paths said so.
    pub refused: usize,
}

impl Seen {
    pub fn add(&mut self, o: &Seen) {
        self.edits += o.edits;
        self.unchanged += o.unchanged;
        self.finish_moved += o.finish_moved;
        self.float_only += o.float_only;
        self.wide += o.wide;
        self.refused += o.refused;
    }
}

/// The first task whose dates differ, or the project finish.
pub fn first_difference(a: &Solution, b: &Solution) -> Option<String> {
    if a.dates.len() != b.dates.len() {
        return Some(format!("{} dates against {}", a.dates.len(), b.dates.len()));
    }
    if let Some(i) = (0..a.dates.len()).find(|&i| a.dates[i] != b.dates[i]) {
        return Some(format!(
            "task {i}: cone {:?}, full {:?}",
            a.dates[i], b.dates[i]
        ));
    }
    (a.project_finish != b.project_finish).then(|| {
        format!(
            "project finish: cone {}, full {}",
            a.project_finish, b.project_finish
        )
    })
}

/// Edit one random task's SNET `edits` times. After each edit the cone
/// result must equal a full compute of the edited schedule; a refused edit
/// (both say infeasible) is rolled back and must leave the solution alone.
pub fn run_chain(s: &mut Schedule, rng: &mut Rng, edits: usize, label: &str) -> Seen {
    let n = s.tasks().len() as i64;
    let mut sol = s.compute().expect("the base schedule computes");
    let mut seen = Seen::default();
    let mut moved_list = Vec::new();
    for step in 0..edits {
        let task = TaskId(rng.range(0, n - 1) as u32);
        let before = s.tasks()[task.0 as usize].snet;
        let horizon = sol.project_finish + 2 * 7 * DAY;
        let new = match rng.range(0, 5) {
            0 => None,
            // The scale of a drag: from the current start, a few days either way.
            1 | 2 => {
                let es = sol.get(task).es;
                Some((es + rng.range(-5 * DAY, 5 * DAY)).max(0))
            }
            3 => Some(rng.range(0, horizon)),
            // Past the whole project: the finish moves.
            4 => Some(sol.project_finish + rng.range(0, 7 * DAY)),
            _ => before,
        };
        s.set_snet(task, new).expect("task exists");
        let ctx = format!("{label}, step {step}: {task} snet {before:?} -> {new:?}");
        let old = sol.clone();
        let want = s.compute();
        moved_list.push(equiv::moved_at(u32::MAX));
        let got = s.recompute_cone_moved(&mut sol, task, &mut moved_list);
        seen.edits += 1;
        match (want, got) {
            (Ok(want), Ok(())) => {
                if let Some(d) = first_difference(&sol, &want) {
                    panic!("{ctx}: {d}");
                }
                // The exact diff, in topological order: each task whose early
                // dates changed, with the early dates it had before the edit.
                let order = s.topological_order().expect("computed");
                let expected_moved = equiv::early_diff(&order, &old, &sol);
                assert_eq!(moved_list, expected_moved, "{ctx}: wrong moved list");
                let moved_count = moved_list.len();
                let float_moved =
                    (0..sol.dates.len()).any(|i| old.dates[i].float != sol.dates[i].float);
                seen.unchanged += (moved_count == 0) as usize;
                seen.wide += (moved_count >= 10) as usize;
                if sol.project_finish != old.project_finish {
                    seen.finish_moved += 1;
                } else if float_moved {
                    seen.float_only += 1;
                }
            }
            (Err(want), Err(got)) => {
                assert_eq!(got, want, "{ctx}: refused differently");
                assert_eq!(sol, old, "{ctx}: a refused edit changed the solution");
                assert!(moved_list.is_empty(), "{ctx}: refused edit reported moves");
                s.set_snet(task, before).expect("task exists");
                seen.refused += 1;
            }
            (want, got) => panic!("{ctx}: full {want:?}, cone {got:?}"),
        }
    }
    // The invariants assume `ef == fin(es)` for the planned duration, which a pinned bar breaks, and
    // a late limit no date meets gives a negative float.
    if !oracle::has_progress(s) && !oracle::has_limits(s) {
        check_invariants(s, &sol);
    }
    seen
}

/// The stress schedules with work under way. An actual start (or finish) is set and a data date is
/// moved between chains of SNET drags; those two edits change late dates the cones do not
/// re-derive for the edited task (`Schedule::set_actual_start`), so each is followed by a full
/// `compute`, and the chain then continues from that solution by the cone alone.
#[expect(
    clippy::print_stderr,
    reason = "the sweep reports its seed count; cargo test shows it with --nocapture"
)]
pub fn stress_progress_sweep(seed_list: Vec<u64>) -> Seen {
    eprintln!(
        "sweep-run: stress with progress ({} seeds)",
        seed_list.len()
    );
    let mut seen = Seen::default();
    for seed in seed_list {
        let (mut s, _) = stress(seed, 2_000);
        let mut rng = Rng::new(seed ^ 0x0ac7);
        let span = s.compute().expect("the schedule computes").project_finish;
        fuzz::add_progress(&mut s, seed, 0.05, span / 2);
        for round in 0..3 {
            let n = s.tasks().len() as i64;
            let task = TaskId(rng.range(0, n - 1) as u32);
            let at = rng.range(0, span / 2);
            // An actual start set on a task, or the data date moved; a refused edit is taken back.
            let (old_start, old_dd) = (s.tasks()[task.0 as usize].actual_start, s.data_date());
            if round == 1 {
                s.set_data_date(Some(rng.range(0, span / 2)))
                    .expect("in range");
            } else {
                s.set_actual_start(task, Some(at))
                    .expect("the edit is accepted");
            }
            if s.compute().is_err() {
                // Retained logic can make an edit infeasible.
                s.set_data_date(old_dd).expect("in range");
                s.set_actual_start(task, old_start)
                    .expect("the edit is accepted");
            }
            seen.add(&run_chain(
                &mut s,
                &mut rng,
                12,
                &format!("stress seed {seed}, progress round {round}"),
            ));
        }
    }
    seen
}
