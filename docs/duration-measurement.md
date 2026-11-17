# Duration edit: how often the cone gives up, and what that costs

gs#519, part of gs#58. `Schedule::recompute_duration_cone_moved` solves a
duration edit with a cone of two passes or gives up and runs a full `compute`
(`DurationCone::Fallback`). This page says how often it gives up, what each
outcome costs and where the cone would stop paying. No constant changed: the
limits stay at `max(1, min(2048, live / 10))` admitted tasks and 16,384
dependency inspections.

## What is measured

All on `stress(20261020, 50_000)`, native, release profile.

| Bench | What it runs |
|---|---|
| `duration_dist` (`crates/engine/benches/duration_dist.rs`, not Criterion) | 200 tasks drawn with `Rng::new(20261020)`, each grown by 60 working minutes from the unedited solution. Per `DurationCone` class: count (the frequency), p50 and p95 of the cone call, tasks examined (`test_hooks::ADMITTED`) and tasks changed (the `Moved` list). The control is a full `compute` of the same edit. Every cone result is checked against it. |
| `duration_finish` | The first critical task in start order, plus 20 working days. The finish moves. |
| `duration_large` | The stress head, plus 20 working days: a large cone that stays under the limit. |
| `duration_backward` | Task 5, plus 60 minutes: no successor moves, a predecessor's late dates do. |
| `duration_full`, `duration_cone` | Existing: a pinned task, plus 60 minutes, at 10,000, 50,000 and 100,000 tasks. |

Each new group (`crates/engine/benches/cpm/duration_groups.rs`) times a `cone`
and a `full` run of the same edit. Its setup asserts the `DurationCone` class,
the admitted count and the solution against a full `compute`, in both
directions, before it times anything. Run `cargo bench -p engine --bench
duration_dist` and `cargo bench -p engine --bench cpm -- duration_`.

## Results

Mac mini (Apple M4, 10 cores, macOS 26.7, rustc 1.99.1), 2026-11-07, fresh
clone at the bench head 7a1bf5f6. Mini load (`fleetwatch`) was 3.72 to 3.54
around the three `duration_dist` runs, 3.33 before and 2.74 after the
Criterion run: not an idle machine. The run's table is in the description of gantt#1564.
A hub run of the same code (load 3.8 before, 2.1 after) is a smoke check, not a
figure.

**Frequency** (`duration_dist`, +60 minutes, three runs, same counts each time):

| Class | Edits | Cone call p50 / p95 | Examined p50 / p95 | Changed p50 / p95 |
|---|---|---|---|---|
| `Cone` | 198 of 200 (99.0%) | 7 µs / 125-127 µs | 12 / 361 | 2 / 64 |
| `Fallback(BackwardLimit)` | 1 (0.5%) | 11.5-12.1 ms | 2048 | 693 |
| `Fallback(ForwardLimit)` | 1 (0.5%) | 11.3-11.9 ms | 2048 | 1007 |

The control, a full `compute`, took p50 10.9-11.2 ms and p95 11.4-11.6 ms. No
`FinishMove`, `EdgeLimit` or `EsAfterLs` appeared. The fallback rows are single
samples.

**Cost** (Criterion, median of 20 samples, `cone` against `full`):

| Group | Class | cone | full |
|---|---|---|---|
| `duration_finish` | `Fallback(ForwardLimit)`, finish moves (not `FinishMove`: the cone reaches 2,048 admitted tasks before the finish check) | 11.76 ms | 11.45 ms |
| `duration_large` | `Cone`, 1,000 to 2,048 admitted | 223 µs | 11.46 ms |
| `duration_backward` | `Cone`, only the task's own early dates move | 1.21 µs | 11.58 ms |
| `duration_cone` / `duration_full`, 10,000 tasks | `Cone` | 70.1 µs | 2.18 ms |
| `duration_cone` / `duration_full`, 50,000 tasks | `Cone` | 69.8 µs | 11.2 ms |
| `duration_cone` / `duration_full`, 100,000 tasks | `Cone` | 12.3 µs | 25.7 ms |

## Crossover between cone and full compute

- **A cone that solves the edit does not lose.** The largest cone the groups
  reach (1,000 to 2,048 admitted tasks) costs 2% of a full `compute`.
- **A fallback costs the attempt on top of the full compute.** `duration_finish`
  measures it: 0.32 ms (2.8%) more than the full compute alone, in the
  `duration_dist` fallbacks 0.5 to 0.8 ms.
- **The cone path loses only if most edits fall back.** With the p50 cone call
  when it solves the edit, the p50 call when it falls back, and the control,
  the cone path is faster on average while fewer than 95.9%, 96.8% and 91.3%
  of edits fall back (the three runs). Measured: 1.0%.
- **Per examined task**, the cone costs 0.11 to 0.35 µs (`duration_large`;
  `duration_dist` p95). Dividing the full compute's 10.9 ms by that gives about
  30,000 to 100,000 examined tasks before a cone would match it, 15 to 50 times
  the 2,048 limit. This is a division of two medians, not a measured crossover.

## What this does not show

- One fixture, one seed, one machine. The three groups run at 50,000 tasks
  only. The MacBook run of gs#519's ruling Q1 is karpathy's call.
- `duration_dist` draws +60 minutes. A longer edit changes the mix: the first
  critical task, plus 20 days, falls back every time (`duration_finish`).
