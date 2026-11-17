//! The duration groups of gs#519 on `stress(20261020, 50_000)`: the cone call
//! of three edits and, beside it, a full `compute` of the same edit.
//!
//! * `duration_finish`: the first critical task, plus 20 working days. The
//!   finish moves, but the forward pass reaches the admitted limit first, so
//!   the call ends in `Fallback(ForwardLimit)`, not `FinishMove`.
//! * `duration_large`: the stress head, plus 20 working days; `Cone`, about
//!   1,800 tasks admitted.
//! * `duration_backward`: a task whose growth moves no successor and moves a
//!   predecessor's late dates; `Cone`.
//!
//! The setup asserts each group's class and admitted count, both directions.

use std::ops::RangeInclusive;

use criterion::Criterion;
use engine::test_hooks::ADMITTED;
use engine::{DurationCone, FallbackReason, Minutes, Moved, Schedule, Solution, TaskId, stress};

use crate::duration::{DELTA, SEED, moved_between};

const N: usize = 50_000;
/// A working day of the stress fixture's calendars, in minutes.
const WORKDAY: Minutes = 480;
/// The `duration_backward` target; the setup asserts what it moves.
const BACKWARD_ONLY: u32 = 5;

/// A task's duration grown by `delta` and put back, on a copy of the fixture.
struct Edit {
    s: Schedule,
    sol: Solution,
    task: TaskId,
    original: Minutes,
    delta: Minutes,
    moved: Vec<Moved>,
}

impl Edit {
    fn new(s: &Schedule, sol: &Solution, case: &Case) -> Edit {
        let original = s.tasks()[case.task.0 as usize].duration;
        let (s, sol) = (s.clone(), sol.clone());
        let (task, delta, moved) = (case.task, case.delta, Vec::new());
        Edit {
            s,
            sol,
            task,
            original,
            delta,
            moved,
        }
    }

    fn set(&mut self, longer: bool) {
        let d = self.original + if longer { self.delta } else { 0 };
        self.s.set_duration(self.task, d).expect("task exists");
    }

    /// The edit through the cone, as `Edit::SetDuration` runs it.
    fn cone(&mut self, longer: bool) -> DurationCone {
        self.set(longer);
        let task = self.task;
        let call = self
            .s
            .recompute_duration_cone_moved(&mut self.sol, task, &mut self.moved);
        call.expect("computes")
    }

    /// The control: the same edit by a full `compute`, and the report of what moved.
    fn full(&mut self, longer: bool) {
        self.set(longer);
        let new = self.s.compute().expect("computes");
        moved_between(&self.sol, &new, &mut self.moved);
        self.sol = new;
    }
}

struct Case {
    name: &'static str,
    task: TaskId,
    delta: Minutes,
    class: DurationCone,
    /// Tasks the cone admitted, in both directions.
    admitted: RangeInclusive<usize>,
    /// Only the task's own early dates move, and a predecessor's late dates.
    backward_only: bool,
}

fn run(c: &mut Criterion, case: &Case, s: &Schedule, base: &Solution) {
    let (name, mut e, mut full) = (
        case.name,
        Edit::new(s, base, case),
        Edit::new(s, base, case),
    );
    for longer in [true, false] {
        let before = e.sol.clone();
        assert_eq!(e.cone(longer), case.class, "{name}: longer={longer}");
        let admitted = ADMITTED.get();
        assert!(
            case.admitted.contains(&admitted),
            "{name}: admitted {admitted}"
        );
        assert_eq!(e.sol, e.s.clone().compute().expect("computes"));
        let finish_moved = e.sol.project_finish != before.project_finish;
        assert_eq!(
            finish_moved,
            case.class != DurationCone::Cone,
            "{name}: finish"
        );
        let late = |s: &Solution, i: usize| (s.dates[i].ls, s.dates[i].lf);
        let late_moved = (0..N)
            .filter(|&i| late(&before, i) != late(&e.sol, i))
            .count();
        let ok = e.moved.len() == 1 && late_moved >= 2;
        assert!(
            ok || !case.backward_only,
            "{name}: {} early, {late_moved} late",
            e.moved.len()
        );
    }
    let mut g = c.benchmark_group(name);
    g.sample_size(20);
    let mut longer = true;
    g.bench_function("cone", |b| {
        b.iter(|| {
            e.cone(longer);
            longer = !longer;
        });
    });
    longer = true;
    g.bench_function("full", |b| {
        b.iter(|| {
            full.full(longer);
            longer = !longer;
        });
    });
    g.finish();
}

pub fn groups(c: &mut Criterion) {
    let (mut s, head) = stress(SEED, N);
    let base = s.compute().expect("stress fixture computes");
    let first_critical = (0..N as u32)
        .filter(|&t| base.dates[t as usize].is_critical())
        .min_by_key(|&t| (base.dates[t as usize].es, t))
        .expect("a critical task");
    let limit = N.div_ceil(10).clamp(1, 2_048);
    let case = |name, task, delta, class, admitted, backward_only| Case {
        name,
        task,
        delta,
        class,
        admitted,
        backward_only,
    };
    let cone = DurationCone::Cone;
    let fallback = DurationCone::Fallback(FallbackReason::ForwardLimit);
    let cases = [
        case(
            "duration_finish",
            TaskId(first_critical),
            20 * WORKDAY,
            fallback,
            limit..=limit,
            false,
        ),
        case(
            "duration_large",
            head,
            20 * WORKDAY,
            cone,
            1_000..=limit,
            false,
        ),
        case(
            "duration_backward",
            TaskId(BACKWARD_ONLY),
            DELTA,
            cone,
            2..=100,
            true,
        ),
    ];
    for case in &cases {
        run(c, case, &s, &base);
    }
}
