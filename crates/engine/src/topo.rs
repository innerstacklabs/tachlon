//! Topological order by Kahn's algorithm; a cycle is reported by naming one.

use crate::model::{Dependency, Graph, ScheduleError, Task, TaskId};

/// CSR of the live dependencies by `key`. The items are indices into `deps`
/// (`DepId`s), so a dead or removed dependency leaves a gap in the ids, not a
/// renumbering.
fn csr(
    n: usize,
    deps: &[Dependency],
    live: impl Fn(usize, &Dependency) -> bool,
    key: impl Fn(&Dependency) -> u32,
) -> (Vec<u32>, Vec<u32>) {
    let mut start = vec![0u32; n + 1];
    let mut count = 0;
    for (_, d) in deps.iter().enumerate().filter(|(i, d)| live(*i, d)) {
        start[key(d) as usize + 1] += 1;
        count += 1;
    }
    for i in 0..n {
        start[i + 1] += start[i];
    }
    let mut fill = start.clone();
    let mut items = vec![0u32; count];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "i is a dependency slot below the dependency count, which DepId holds in u32 \
                  (unchecked at Schedule::add_dependency)"
    )]
    for (i, d) in deps.iter().enumerate().filter(|(i, d)| live(*i, d)) {
        let k = key(d) as usize;
        items[fill[k] as usize] = i as u32;
        fill[k] += 1;
    }
    (start, items)
}

/// The graph of the live tasks and the dependencies between them. A deleted
/// task is not in the order and a removed dependency or one with a deleted end
/// is in no adjacency list; ids are the slots' own (decision 0001, amended).
pub(crate) fn build(
    tasks: &[Task],
    deps: &[Dependency],
    removed: &[bool],
) -> Result<Graph, ScheduleError> {
    let n = tasks.len();
    let live = |i: usize, d: &Dependency| {
        !removed[i] && !tasks[d.pred.0 as usize].deleted && !tasks[d.succ.0 as usize].deleted
    };
    let (pred_start, pred_deps) = csr(n, deps, live, |d| d.succ.0);
    let (succ_start, succ_deps) = csr(n, deps, live, |d| d.pred.0);
    let mut indeg: Vec<u32> = (0..n).map(|i| pred_start[i + 1] - pred_start[i]).collect();
    #[expect(
        clippy::cast_possible_truncation,
        reason = "n is the task count, which TaskId holds in u32 (unchecked at Schedule::add_task)"
    )]
    let mut order: Vec<u32> = (0..n as u32)
        .filter(|&i| indeg[i as usize] == 0 && !tasks[i as usize].deleted)
        .collect();
    let live_count = tasks.iter().filter(|t| !t.deleted).count();
    let mut head = 0;
    while head < order.len() {
        let t = order[head] as usize;
        head += 1;
        for &d in &succ_deps[succ_start[t] as usize..succ_start[t + 1] as usize] {
            let s = deps[d as usize].succ.0;
            indeg[s as usize] -= 1;
            if indeg[s as usize] == 0 {
                order.push(s);
            }
        }
    }
    if order.len() < live_count {
        return Err(ScheduleError::Cycle(find_cycle(
            deps,
            &pred_start,
            &pred_deps,
            &indeg,
        )));
    }
    let mut rank = vec![u32::MAX; n];
    #[expect(
        clippy::cast_possible_truncation,
        reason = "at indexes the order, which lists each task once: below the task count, a u32 \
                  (unchecked at Schedule::add_task)"
    )]
    for (at, &t) in order.iter().enumerate() {
        rank[t as usize] = at as u32;
    }
    Ok(Graph {
        order,
        rank,
        pred_start,
        pred_deps,
        succ_start,
        succ_deps,
    })
}

/// Every task Kahn could not order still has a predecessor that Kahn could not
/// order either, so walking predecessors from one of them must repeat a task.
fn find_cycle(
    deps: &[Dependency],
    pred_start: &[u32],
    pred_deps: &[u32],
    indeg: &[u32],
) -> Vec<TaskId> {
    let stuck = |i: usize| indeg[i] > 0;
    let mut seen_at = vec![usize::MAX; indeg.len()];
    let mut path: Vec<u32> = Vec::new();
    let mut cur = (0..indeg.len())
        .find(|&i| stuck(i))
        .expect("a stuck task exists");
    while seen_at[cur] == usize::MAX {
        seen_at[cur] = path.len();
        #[expect(
            clippy::cast_possible_truncation,
            reason = "cur is a task slot below the task count, which TaskId holds in u32 \
                      (unchecked at Schedule::add_task)"
        )]
        path.push(cur as u32);
        let preds = &pred_deps[pred_start[cur] as usize..pred_start[cur + 1] as usize];
        cur = preds
            .iter()
            .map(|&d| deps[d as usize].pred.0 as usize)
            .find(|&p| stuck(p))
            .expect("a stuck task has a stuck predecessor");
    }
    // path[seen_at[cur]..] walks successor -> predecessor; reverse it so each
    // task is a predecessor of the next.
    let mut cycle: Vec<TaskId> = path[seen_at[cur]..].iter().map(|&i| TaskId(i)).collect();
    cycle.reverse();
    cycle
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Minutes;
    use crate::model::{CalendarId, LinkType};

    fn task(deleted: bool) -> Task {
        Task {
            duration: 60 as Minutes,
            calendar: CalendarId(0),
            snet: None,
            limit: crate::model::Limit::default(),
            deleted,
            actual_start: None,
            actual_finish: None,
            remaining: None,
        }
    }

    fn link(p: u32, s: u32) -> Dependency {
        Dependency {
            pred: TaskId(p),
            succ: TaskId(s),
            kind: LinkType::FS,
            lag: 0,
        }
    }

    #[test]
    fn build_skips_dead_tasks_and_keeps_dep_ids() {
        // 0 -> 1 -> 2 -> 3, with 1 deleted: links 0 and 1 are dead, link 2 lives.
        let tasks = [task(false), task(true), task(false), task(false)];
        let deps = [link(0, 1), link(1, 2), link(2, 3)];
        let g = build(&tasks, &deps, &[false; 3]).unwrap();
        assert_eq!(g.order, vec![0, 2, 3]);
        assert_eq!(g.pred_deps, vec![2]);
        assert_eq!(g.succ_deps, vec![2]);
        assert_eq!(g.pred_start, vec![0, 0, 0, 0, 1]);
    }

    #[test]
    fn a_cycle_through_a_dead_task_is_no_cycle() {
        let deps = [link(0, 1), link(1, 2), link(2, 0)];
        let live = [task(false), task(false), task(false)];
        assert!(matches!(
            build(&live, &deps, &[false; 3]),
            Err(ScheduleError::Cycle(_))
        ));
        let dead = [task(false), task(true), task(false)];
        assert_eq!(build(&dead, &deps, &[false; 3]).unwrap().order, vec![2, 0]);
    }
}
