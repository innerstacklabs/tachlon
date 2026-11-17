//! The equivalence helper for duration edits (gs#58): after one duration
//! change, `recompute_duration_cone_moved` must equal a full compute, and its
//! `moved` list must be the exact early-date diff in topological order.

use engine::{DurationCone, Moved, Schedule, Solution, TaskId};

/// The tasks whose early dates differ between two solutions, each with its
/// early dates in `old` (what `Moved` must carry), in the order of
/// `order` (the schedule's topological order).
pub fn early_diff(order: &[TaskId], old: &Solution, new: &Solution) -> Vec<Moved> {
    let early = |s: &Solution, t: TaskId| (s.get(t).es, s.get(t).ef);
    order
        .iter()
        .copied()
        .filter(|&t| early(old, t) != early(new, t))
        .map(|t| Moved {
            task: t,
            was_es: old.get(t).es,
            was_ef: old.get(t).ef,
        })
        .collect()
}

/// A stale entry for the list a call is handed: the call must clear it.
pub fn moved_at(task: u32) -> Moved {
    Moved {
        task: TaskId(task),
        was_es: 0,
        was_ef: 0,
    }
}

/// Set `task`'s duration and bring `sol` up to date by the duration cone.
/// `sol` must be the full compute of the schedule before the call. When a full
/// compute of the edited schedule succeeds, `sol` must equal it and `moved`
/// must be the early-date diff; when it fails, the cone must fail too, leave
/// `sol` alone and report nothing (the old duration is restored). Returns
/// whether the edit was applied.
pub fn assert_duration_cone(
    s: &mut Schedule,
    sol: &mut Solution,
    task: TaskId,
    duration: i64,
    label: &str,
) -> bool {
    assert_duration_cone_outcome(s, sol, task, duration, label).is_some()
}

/// [`assert_duration_cone`], returning how the applied edit was solved, or
/// `None` when the edit was refused.
pub fn assert_duration_cone_outcome(
    s: &mut Schedule,
    sol: &mut Solution,
    task: TaskId,
    duration: i64,
    label: &str,
) -> Option<DurationCone> {
    let old = s.tasks()[task.0 as usize].duration;
    s.set_duration(task, duration).expect("task exists");
    let want = s.clone().compute();
    let before = sol.clone();
    let mut moved = vec![moved_at(u32::MAX)];
    let got = s.recompute_duration_cone_moved(sol, task, &mut moved);
    let ctx = format!("{label}: {task} duration {old} -> {duration}");
    match (want, got) {
        (Ok(want), Ok(how)) => {
            if let Some(i) = (0..want.dates.len()).find(|&i| sol.dates[i] != want.dates[i]) {
                panic!(
                    "{ctx}: task {i}: cone {:?}, full {:?}",
                    sol.dates[i], want.dates[i]
                );
            }
            assert_eq!(sol, &want, "{ctx}");
            let order = s.topological_order().expect("computed");
            assert_eq!(moved, early_diff(&order, &before, &want), "{ctx}: moved");
            Some(how)
        }
        (Err(_), Err(_)) => {
            assert_eq!(sol, &before, "{ctx}: a refused edit changed the solution");
            assert!(moved.is_empty(), "{ctx}: refused edit reported moves");
            s.set_duration(task, old).expect("task exists");
            None
        }
        (want, got) => panic!("{ctx}: full {want:?}, cone {got:?}"),
    }
}
