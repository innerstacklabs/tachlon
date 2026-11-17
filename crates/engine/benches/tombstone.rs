//! Dead slots cost nothing measurable (gs#301, decision 0001 amendment).
//!
//! Every group runs `generate(42, 50_000)` plus 0, 25% or 100% extra deleted
//! tasks (`dead/<extra>`), appended without links so the live work is the same
//! in every row. Compare the three numbers of a group.
//!
//! * `tombstone_compute`: forward + backward pass, order cached.
//! * `tombstone_graph_rebuild`: drop the cached order and rebuild it (the one
//!   cost that grows with the slot count).
//! * `tombstone_cone`: one drag step of a critical task with `recompute_cone`.
//! * `tombstone_cone_finish`: the project-finish-moving drag, which reruns the
//!   backward pass.

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
use engine::{DAY, Schedule, Task, generate};

const LIVE: usize = 50_000;
const SEED: u64 = 42;
/// Dead slots appended after the live tasks.
const DEAD: [usize; 3] = [0, LIVE / 4, LIVE];

/// The generated schedule with `extra` deleted tasks appended.
fn with_dead(extra: usize) -> Schedule {
    let mut s = generate(SEED, LIVE);
    for _ in 0..extra {
        let id = s
            .add_task(Task {
                duration: 480,
                calendar: s.tasks()[0].calendar,
                snet: None,
                limit: engine::Limit::default(),
                deleted: false,
                actual_start: None,
                actual_finish: None,
                remaining: None,
            })
            .expect("a task");
        s.delete_task(id).expect("deletes");
    }
    s
}

fn compute(c: &mut Criterion) {
    let mut g = c.benchmark_group("tombstone_compute");
    g.sample_size(20);
    for extra in DEAD {
        let mut s = with_dead(extra);
        s.compute().expect("computes");
        g.bench_with_input(BenchmarkId::new("dead", extra), &extra, |b, _| {
            b.iter(|| s.compute().expect("computes"));
        });
    }
    g.finish();
}

fn graph_rebuild(c: &mut Criterion) {
    let mut g = c.benchmark_group("tombstone_graph_rebuild");
    g.sample_size(20);
    for extra in DEAD {
        let mut s = with_dead(extra);
        g.bench_with_input(BenchmarkId::new("dead", extra), &extra, |b, _| {
            b.iter(|| {
                s.clear_cached_order();
                s.topological_order().expect("orders")
            });
        });
    }
    g.finish();
}

/// The first task in the order with zero float, so a drag reaches a cone.
fn critical(s: &mut Schedule) -> engine::TaskId {
    let sol = s.compute().expect("computes");
    let t = (0..LIVE)
        .filter(|&i| sol.dates[i].is_critical())
        .min_by_key(|&i| sol.dates[i].es)
        .expect("a critical task");
    engine::TaskId(t as u32)
}

fn cone(c: &mut Criterion) {
    for (name, shift) in [
        ("tombstone_cone", DAY / 2),
        ("tombstone_cone_finish", 20 * DAY),
    ] {
        let mut g = c.benchmark_group(name);
        g.sample_size(20);
        for extra in DEAD {
            let mut s = with_dead(extra);
            let task = critical(&mut s);
            let mut sol = s.compute().expect("computes");
            let (original, later) = (s.tasks()[task.0 as usize].snet, sol.get(task).es + shift);
            let mut away = false;
            g.bench_with_input(BenchmarkId::new("dead", extra), &extra, |b, _| {
                b.iter(|| {
                    s.set_snet(task, if away { original } else { Some(later) })
                        .expect("task exists");
                    away = !away;
                    s.recompute_cone(&mut sol, task).expect("computes");
                });
            });
        }
        g.finish();
    }
}

criterion_group!(tombstone, compute, graph_rebuild, cone);
criterion_main!(tombstone);
