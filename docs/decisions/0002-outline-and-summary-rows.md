# Outline and summary rows

Status: accepted on its seats' approval

The rulings cited below are karpathy's rulings on gs#47, 2026-10-23 01:40.

## Context

Decision 0001 gives each task a stable `TaskId` and a separate row index, and
leaves the hierarchy open: "gs#47 must define the task hierarchy. This record
only consumes visible task IDs in display order."
(`docs/decisions/0001-task-identity-and-rows.md`, Decision 2)

Until now `TaskMeta::parent` was documented as "always an earlier task". That
rule ties the hierarchy to task numbering, so it fails as soon as a summary
task is added after its children, or a task is inserted (gs#50). This record
replaces it with a forest rule.

## Decision 1: the hierarchy is a forest over `TaskId`

- `TaskMeta::parent` (`crates/app/src/meta.rs`) is the only stored link. A task
  with no parent is a root. A parent can have any id.
- A task with at least one child is a **summary**. The rest are **leaves**.
  Nothing else marks a task as a summary.
- `Outline` (`crates/app/src/outline.rs`) is the validated forest. It stores
  each task's parent, its children, a preorder, and each task's position in
  that preorder, so the subtree of a task is a slice of the preorder.
- The forest is app data. The `engine` crate never sees it, which keeps the
  dependency direction in `docs/ARCHITECTURE.md`.

## Decision 2: `wbs` order

`wbs` is the order of tasks in the outline, as a list of every `TaskId` once.
The children of one parent, and the roots, keep the order they have in `wbs`.
The preorder follows from it: a parent, then its children in `wbs` order, each
with its own subtree.

Task ids carry no order. The demo layout (Decision 5) gives leaves ids by
early start, so id order and `wbs` order differ on purpose.

`Outline::new(meta, wbs, schedule)` refuses:

| Error | Cause |
|---|---|
| `WrongLength` | `meta`, `wbs` and the schedule differ in length, `wbs` repeats or omits an id, or a parent id is out of range |
| `Cycle` | A task is its own ancestor, a self-parent included. The check is that a walk from the roots reaches every task |
| `SummaryHasDependency(DepId)` | A dependency has a summary as its predecessor or successor (Decision 4) |

The checks run in that order.

## Decision 3: collapse semantics

These rules fix what the collapse feature must do. This record adds no
collapse flags and no row index; they come with the collapse PR.

- Every summary is expanded on load.
- Collapsing a summary hides its descendants, at every depth.
- The hidden tasks leave the visible task set. `Session` hands the new visible
  order to `Timeline` through the setter from Decision 2 of record 0001, then
  rebuilds `LinkIndex`.
- `c` (jump to the next critical task) skips hidden tasks.
- A link with a hidden end is dropped from the scene. No link is redrawn to the
  collapsed summary.

Collapse changes display only. It never changes dates, constraints or
dependencies.

## Decision 4: no dependencies on summaries

A *live* dependency (not removed, neither end deleted) may not have a summary at
either end (ruling 5). `Outline::new` refuses a schedule that has one, with
`SummaryHasDependency` and the first offending `DepId`; a removed or dead link
may touch a summary, so undo can rebuild the outline around one.

The engine schedules leaves. A summary's dates are meant to be derived from its
descendants, so a link to a summary would make a derived date drive a scheduled
one.

## Decision 5: the demo layout

`demo(n)` keeps `n` as the total task count; `web/verify.mjs` asserts
`s0.n === n`. The layout is `outline::demo_layout`.

1. `names::generate(SEED, n)` gives the outline by position, unchanged. A
   position with children is a summary; the rest are leaves, `L` of them.
2. `engine::generate(SEED, L)` gives the leaf schedule unchanged, with ids
   `0..L`. `n - L` zero-duration milestones on calendar 0, with no links, are
   appended as the summary tasks, ids `L..n`. They do not change any leaf's
   dates. One `compute` solves the result.
3. The leaves are ranked by `order_by_start` over the first `L` tasks. The
   k-th leaf position takes the k-th ranked leaf. The k-th summary position
   takes the k-th appended task. `parent` is rewritten through that map, and
   `wbs` is the list of tasks in position order.

At 50,000 tasks this is 43,364 leaves and 6,636 summaries, 1,210 of them at
level 1. Summary bars are not drawn as rollups yet, so a summary is a
zero-width milestone. The level-1 summary width, measured as the span of its
leaves, has a median of 0.10% of the project span
(`crates/app/tests/outline.rs`, `level_one_summaries_are_narrow`), and the test
asserts it stays under 1%.

The stress fixture (`?stress`, `?measure=1`) stays as it is (ruling 11): it
keeps its flat `names::generate` metadata and builds no `Outline`.

## Not in this record

Collapse flags, the row index, summary rollups and editing the hierarchy (gs#50)
are later work. Their names and shapes are not fixed here.
