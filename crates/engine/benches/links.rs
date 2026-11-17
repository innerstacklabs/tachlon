//! What a link edit costs (gs#343, decision 0001 amendment). `generate(42, n)`
//! at 10k, 50k and 100k tasks.
//!
//! * `link_graph_rebuild`: drop the cached order and rebuild it.
//! * `link_add_compute`: `add_link` of a link that closes no loop, then a full
//!   `compute` (the cost of a person adding a link), then `remove_dependency`
//!   to undo it.
//! * `link_reach_full_cone`: `link_cycle(last, head)` on `stress(42, n)`,
//!   whose head task reaches most of the schedule, with `last` the task a
//!   breadth-first search from the head visits last: the search walks the
//!   whole cone before it finds the loop. The cone size is printed.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::float_cmp,
    reason = "test code: Dave 2026-11-06, looser rules for tests"
)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use engine::{Dependency, LinkType, Schedule, TaskId, generate, stress};

const SIZES: [usize; 3] = [10_000, 50_000, 100_000];
const SEED: u64 = 42;

fn fresh(n: usize) -> Schedule {
    let mut s = generate(SEED, n);
    s.compute().expect("computes");
    s
}

fn graph_rebuild(c: &mut Criterion) {
    let mut g = c.benchmark_group("link_graph_rebuild");
    g.sample_size(20);
    for n in SIZES {
        let mut s = fresh(n);
        g.bench_with_input(BenchmarkId::new("tasks", n), &n, |b, _| {
            b.iter(|| {
                s.clear_cached_order();
                s.topological_order().expect("orders")
            });
        });
    }
    g.finish();
}

/// The first and last task of the topological order: the longest search, and
/// a link between them (in that direction) is always legal.
fn ends(s: &mut Schedule) -> (TaskId, TaskId) {
    let order = s.topological_order().expect("orders");
    (order[0], *order.last().expect("tasks"))
}

fn add_compute(c: &mut Criterion) {
    let mut g = c.benchmark_group("link_add_compute");
    g.sample_size(20);
    for n in SIZES {
        let mut s = fresh(n);
        let (first, last) = ends(&mut s);
        let dep = Dependency {
            pred: first,
            succ: last,
            kind: LinkType::FS,
            lag: 0,
        };
        g.bench_with_input(BenchmarkId::new("tasks", n), &n, |b, _| {
            b.iter(|| {
                let id = s.add_link(dep).expect("adds");
                let sol = s.compute().expect("computes");
                s.remove_dependency(id).expect("removes");
                sol
            });
        });
    }
    g.finish();
}

/// The tasks downstream of `root` over the live links, in the order a
/// breadth-first search from it visits them (`root` first).
fn cone(s: &Schedule, root: TaskId) -> Vec<usize> {
    let mut succs = vec![Vec::new(); s.tasks().len()];
    for (_, d) in s.live_dependencies() {
        succs[d.pred.0 as usize].push(d.succ.0 as usize);
    }
    let mut seen = vec![false; succs.len()];
    let mut queue = vec![root.0 as usize];
    seen[root.0 as usize] = true;
    let mut head = 0;
    while head < queue.len() {
        let t = queue[head];
        head += 1;
        for &n in &succs[t] {
            if !seen[n] {
                seen[n] = true;
                queue.push(n);
            }
        }
    }
    queue
}

#[expect(
    clippy::print_stderr,
    reason = "the bench reports how many tasks the head reaches, as context for its timings"
)]
fn reach_full_cone(c: &mut Criterion) {
    let mut g = c.benchmark_group("link_reach_full_cone");
    g.sample_size(20);
    for n in SIZES {
        let (mut s, head) = stress(SEED, n);
        s.compute().expect("computes");
        let reached = cone(&s, head);
        // The task the search reaches last: a link from it back to the head
        // closes the loop only after the search has walked the whole cone.
        let last = TaskId(*reached.last().expect("a cone") as u32);
        assert!(s.link_cycle(last, head).expect("checks").is_some());
        eprintln!(
            "link_reach_full_cone/{n}: the head reaches {} of {} tasks",
            reached.len(),
            s.tasks().len()
        );
        g.bench_with_input(BenchmarkId::new("tasks", n), &n, |b, _| {
            b.iter(|| s.link_cycle(last, head).expect("checks"));
        });
    }
    g.finish();
}

criterion_group!(benches, graph_rebuild, add_compute, reach_full_cone);
criterion_main!(benches);
