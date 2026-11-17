# Task identity and display rows

Status: accepted (karpathy, 2026-10-23)

## Context

`TaskId` is currently the index into `Schedule::tasks`. The same value indexes
`Solution::dates`, topology arrays, scene links, and table data. The code does
not delete tasks or rebuild row order today. (`crates/engine/src/model.rs`, `TaskId` and `Schedule`; `crates/engine/src/cpm.rs`, `Solution`;
`docs/ARCHITECTURE.md`, “Task-indexed and row-indexed structures”)

gs#48 will add one `Edit` type and inverse edits. gs#47 will add collapsed rows.
gs#50 will insert and delete tasks. These need stable identity and independent display order.

## Decision 1: task identity

Use stable task IDs with tombstones and periodic compaction.

| Option | Description | Decision |
|---|---|---|
| A | A deleted task keeps its `TaskId`; `Task.deleted` marks it. IDs change only during compaction. | Adopt |
| B | Every delete compacts tasks and returns a `Remap` for all task-ID holders. | Reject |

Under option A, the inverse of delete clears `Task.deleted`. The task returns
with the same ID, so gs#48 undo entries keep their original `TaskId`s.

Under option B, undo inserts at an old index and shifts every later ID. Every
delete and undo must rewrite the undo stack and all other task-ID holders:

- `Schedule::tasks` positions and `Dependency::pred` and `Dependency::succ`.
- `Graph::order`, `Graph::pred_start`, `Graph::succ_start`, and topology and
  cycle scratch that contains task IDs.
- `Solution::dates` and the full-solve and cone scratch arrays and entries.
- `Timeline::order`, `BarDrag::task`, and timeline interaction state.
- `LinkIndex` links and its `row_of` scratch.
- `Session::schedule`, `Session::solution`, `Session::applied`,
  `DragInfo::task`, and `Press::task`.
- `App::head`, `App::links`, status values, and table JSON task IDs.
- Browser table state that retains task IDs.

(`docs/ARCHITECTURE.md`, “Task-indexed and row-indexed structures”; `crates/engine/src/model.rs`, `Schedule`, `Dependency`, and `Graph`; `crates/engine/src/cpm.rs`, `Solution` and `Schedule::recompute_cone`; `crates/timeline/src/lib.rs`, `Timeline` and `BarDrag`; `crates/timeline/src/scene.rs`, `LinkIndex::new`; `crates/app/src/lib.rs`, `Session`, `DragInfo`, and `Press`)

Put `deleted: bool` in `Task`. The reason is layout, not speed. The measured
`size_of::<Task>()` is 32 bytes both with and without this field because the
field fits in existing padding, so the field costs no memory. The benchmarks
do not separate the two layouts. Each patched variant had
only 2 runs, and four field-in-`Task` ratios (0.94, 0.92, 0.88, 0.92) fall
outside the stated plus or minus 5% noise band, in the faster direction. The
separate array's +10% on `drag_step_cone/100000` is likewise inside what two
runs can show. Treat the bench numbers as inconclusive, not as a measured cost
of either layout. The Measurements section records the full results.

`topo::build` should omit tombstoned tasks and dependencies that touch them
when it builds `Graph::order` and the CSR arrays. The passes then need no
deletion check. The measured branch covers only the checks inside the passes.
The cost of filtering in `topo::build` was not measured.
(`crates/engine/src/topo.rs`, `build` and `csr`; `crates/engine/src/cpm.rs`, `Schedule::compute`,
`Schedule::recompute_cone`, and `Pass::backward_all`)

Option A also requires these changes:

- `Solution::dates` keeps one slot per task ID. A deleted task's slot is
  stale, and readers skip it.
- `Schedule::compute` takes project finish as the maximum early finish of
  live tasks only. It currently takes the maximum over every `ef`.
- `Schedule::recompute_cone` currently finds the edited task in
  `Graph::order` with `position(...)`. It must return an error for a deleted
  task instead of expecting every task to be present.
- Deleting a task also tombstones dependencies that touch it. This keeps
  `DepId` stable.
- The generator and oracle tests add `deleted: false` to task literals.

(`crates/engine/src/cpm.rs`, `Schedule::compute` and `Schedule::recompute_cone`; `crates/engine/src/generate.rs`, `assemble`)

Compaction runs only at a quiescent point, never during a drag. It runs when
the undo history is empty or is being cleared, such as on save, and when
tombstones exceed 25% of tasks. The threshold is tunable and is not measured.
Compaction returns a `Remap` from old ID to new ID. `Session` coordinates
applying it to every retained holder in the architecture inventory, then
rebuilds `Graph`. (`crates/app/src/lib.rs`, `Session`;
`crates/engine/src/topo.rs`, `build`)

Compaction waits for empty undo history. It also invalidates any `TaskId` held
by JavaScript, including table IDs, so the app must replace that state. Its
cost includes a graph rebuild.
The measured `compute_cold` minus `compute` cost was about 0.2 ms at 10k
tasks, 1.5 ms at 50k, and 2.5 ms to 5.7 ms at 100k across two noisy runs.

Option B avoids dead slots and reader-side skipping. It loses because delete
and undo renumber identities and rewrite every holder on every operation.

### Amendment (gs#301, ruling devteam-1-95, 2026-10-25)

Decision 1 stands. gs#50's plan (PR 1, gs#301) settles what it left open, and
three lines above are replaced by it:

- *Dependency tombstones are derived, not stored.* "Deleting a task also
  tombstones dependencies that touch it" is replaced by: a dependency is dead
  if and only if one of its ends is deleted. There is no per-dependency flag,
  so restoring a task restores its links and a delete cannot desync them.
  `DepId` stays stable because the dead links keep their slot in
  `Schedule::dependencies`. gs#49's remove-link brings its own flag.
- *The filter lives in `topo::build`, not in the passes.* `topo::build` leaves
  a deleted task out of `Graph::order` and a dead dependency out of the CSR
  arrays (which still hold the original `DepId`s). The passes and `compute`
  need no deletion check. A dead task's `Solution` slot is all zero, so the
  project finish needs no live-only maximum either (a dead `ef` is 0, never
  above a live one). The cost was measured (`crates/engine/benches/tombstone.rs`):
  dead slots add a graph-rebuild cost per slot and nothing measurable to
  `compute` or the cone.
- *Readers that scan every slot or every dependency check liveness:*
  `longest_path`, the no-graph fallback of `driving_predecessors` and
  `driven_successors`, and the cone root (a deleted root is
  `ScheduleError::UnknownTask`, as is any mutator given a deleted task).
- *Compaction is out of gs#50.* The `Remap` and its save-time policy are
  decided with the J9 document-format record. Until then a delete costs one
  `Task`, one `TaskDates` and one `TaskMeta` slot.

### Amendment (gs#343, gs#49 PR 1, karpathy rulings 2026-10-25)

Decision 1 and the gs#301 amendment stand. gs#49 lets a person add, remove and
restore a link, and settles what the gs#301 note left to it ("gs#49's
remove-link brings its own flag"):

- *A removed link is a flag beside the slot.* `Schedule` holds a private
  `removed: Vec<bool>` the length of `deps`, pushed by both adds.
  `Dependency` stays the file's plain value. A link is live if and only if it
  is not removed and neither end is deleted (`Schedule::is_dependency_live`,
  `live_dependencies`). The two causes stay apart: restoring a task never
  restores a link a person removed, and removing a link whose end is deleted
  keeps it removed after `restore_task`.
- *`DepId` is stable and a re-add takes a new slot.* A removed link keeps its
  slot and id, so `restore_dependency` brings back the same link and
  anything holding the id (undo, a selection) stays valid. Adding the same
  pair again is a new slot with a new id. Restoring the old one while the new
  one is live is refused (`DuplicateDependency`), and so is a restore that
  would close a loop, so a restore is checked like an add. A removed link
  with a deleted end cannot be restored (`UnknownTask`, nothing changes)
  until its task is restored, and `restore_task` then checks each link it
  brings back like `restore_dependency` (`DuplicateDependency` or `Cycle`,
  and the task stays deleted), so no link a person restores comes back
  unchecked. Undo and redo use `restore_task_replay`, which skips those two
  checks: history puts back exactly what it recorded, repeated links of an
  import included.
- *`topo::build` filters by index.* It takes the `removed` slice and leaves a
  removed link out of the CSR arrays, as it does a dead one, so the passes and
  the cone need no change. The no-graph fallback of `driving_predecessors` and
  `driven_successors` checks the same liveness by index (`live_at`).
- *Two doors.* `add_link` is for people: ends live, not a task to itself, no
  live link from the same predecessor to the same successor (any type; the
  other direction is a different link), and no loop; `link_cycle` is the plain
  search on the cached graph and returns the loop's path
  (`Cycle`, each task a predecessor of the next). On `Err` nothing changes and
  the cached graph stays. `add_dependency` stays unchecked for importers, so a
  file with a cycle or a repeated pair loads as before and `compute` reports it.
- *Compaction is still out.* Removed slots, like deleted tasks, stay until the
  J9 document-format decision settles a `Remap` and its save-time policy; a
  removed link costs one `Dependency` and one `bool`.
- *Costs* (measured with `crates/engine/benches/links.rs`, native, this
  machine; numbers in the PR for gs#343): a full `compute` after a link edit,
  and a cycle check that walks the whole cone of a stress fixture's head.

## Decision 2: display order

Use a separate row index and rebuild it wholesale when collapse state or the
visible task set changes.

| Option | Description | Decision |
|---|---|---|
| A | Keep `rows: Vec<TaskId>` from row to task and an inverse `row_of` from task to row. Rebuild both wholesale. | Adopt now |
| B | Splice rows and patch the inverse map incrementally. | Defer until measurement requires it |
| C | Make rows equal task indexes. | Reject |

Option C cannot hide tasks from the middle for collapse or insert a row
between existing rows without renumbering task IDs.

`Timeline::order` is already the row-to-task index. `order_by_start` builds it
once, and no current method rewrites it. Its sort key is `(early start, task id)`.
Deleted tasks must be filtered out before sorting.
(`crates/timeline/src/lib.rs`, `Timeline::order` and `order_by_start`)

The row index lives in the `timeline` crate and is owned by `Timeline`.
`Timeline` is the only component that maps rows to pixels. `Session` in the
`app` crate is the single writer because it owns the
schedule, collapse state, and edit history. When the visible set changes,
`Session` hands `Timeline` a new order through one setter and then rebuilds
`LinkIndex`. The inverse `row_of` supports `LinkIndex::new` and “scroll to task.”
(`crates/timeline/src/lib.rs`, `Timeline`; `crates/timeline/src/scene.rs`, `LinkIndex::new`;
`crates/app/src/lib.rs`, `Session`)

The engine owns task identity and dates. It never sees display rows. This
preserves the dependency direction: `engine` depends on neither `timeline`
nor `app`. (`docs/ARCHITECTURE.md`, “Crate dependency direction”)

gs#47 must define the task hierarchy. This record only consumes visible task
IDs in display order. It does not define why a task is hidden.

The measured wholesale costs are one-off costs per collapse or insert, not
per frame. A collapse is judged against one 16.7 ms frame. An insert is a
discrete edit, not a per-frame path, so its budget is 100 ms:

- A collapse that re-sorts and rebuilds `LinkIndex` costs about 12.5 ms in
  total (6.1 ms plus 6.4 ms), which leaves about 4 ms of the frame.
- An insert also needs a graph rebuild of 2.5 ms to 5.7 ms on top of the row
  index. The new solution then comes from the full-compute fallback (about
  33 ms at 100k), because `Solution::dates` is the wrong length after an
  insert. The total is about 48 ms to 51 ms at 100k (6.1 + 6.4 + 2.5 to 5.7
  + 33), dominated by the full compute. That is inside the 100 ms budget.

So a collapse at 100k fits in one frame only barely, and an insert at 100k
fits its own budget but not a frame. Use option B if a measured collapse
stalls a frame. An insert does not meet that trigger, because it is not a
per-frame path. Making inserts cheaper (grow `Solution::dates`, and
`recompute_cone` from the new task instead of the full compute) is gs#50's
job.

## Consequences for gs#47/#48/#50

- **gs#47:** Define a hierarchy model outside this record. Collapse filters
  visible task IDs, calls the `Timeline` order setter, then calls
  `LinkIndex::new`.
- **gs#48:** `Edit` variants carry `TaskId`. The inverse of delete clears the
  tombstone. The undo history must be empty before compaction.
- **gs#50:** Insert appends a task to `Schedule::tasks` to obtain a new ID,
  then places it in the row index. Delete tombstones the task and its
  dependencies.

The hierarchy representation, setter API, edit variants, save-time policy, and
deleted-task error variant remain unresolved for karpathy.

## Measurements

The machine was the hub with 16 cores and load average 1.5 to 2.5. The command
was `cargo bench -p engine --bench cpm -- --warm-up-time 1 --measurement-time 4`.
Results are Criterion medians. Base has 7 runs; each patched variant has 2.

A first attempt at load 6 to 11 produced nonsense ratios of 1.5x to 4x and
was discarded.

Quiet-box noise is about plus or minus 5%. An unmodified control build
differed from itself by up to 3.4%. A single patched result that moves more
than that, such as the field variant's -12% on `stress_drag_cone/100000` or the
separate array's +10% on `drag_step_cone/100000`, is within what 2 runs and
code-layout luck can produce, so neither is read as a real speed-up or cost.

| bench | base median | separate-array check | ratio | field-in-Task check | ratio |
|---|---|---|---|---|---|
| compute/10000 | 2.63 ms | 2.66 ms | 1.01 | 2.64 ms | 1.00 |
| compute/50000 | 14.29 ms | 14.41 ms | 1.01 | 14.56 ms | 1.02 |
| compute/100000 | 32.91 ms | 32.82 ms | 1.00 | 32.68 ms | 0.99 |
| stress_drag_cone/10000 | 56.8 us | 58.7 us | 1.03 | 53.6 us | 0.94 |
| stress_drag_cone/50000 | 118.0 us | 120.5 us | 1.02 | 108.8 us | 0.92 |
| stress_drag_cone/100000 | 175.1 us | 184.6 us | 1.05 | 153.9 us | 0.88 |
| drag_step_cone/10000 | 137.0 us | 138.8 us | 1.01 | 135.9 us | 0.99 |
| drag_step_cone/50000 | 261.9 us | 273.1 us | 1.04 | 239.9 us | 0.92 |
| drag_step_cone/100000 | 713.4 us | 787.4 us | 1.10 | 705.2 us | 0.99 |
| drag_finish_cone/10000 | 1.66 ms | 1.68 ms | 1.01 | 1.67 ms | 1.00 |
| drag_finish_cone/50000 | 11.09 ms | 11.55 ms | 1.04 | 11.21 ms | 1.01 |
| drag_finish_cone/100000 | 33.13 ms | 34.47 ms | 1.04 | 33.25 ms | 1.00 |

The field-in-`Task` ratios run from 0.88 to 1.02. Four of them (0.94, 0.92,
0.88, 0.92 on `stress_drag_cone/10000`, `/50000`, `/100000` and
`drag_step_cone/50000`) fall outside the plus or minus 5% band, all on the fast
side, so the claim "inside the noise band on every benchmark" does not hold.
The separate-array ratios run from 1.00 to 1.10; 1.10 is
`drag_step_cone/100000`. With 2 runs per patched variant and a control that
drifted 3.4% from itself, these data do not tell the two layouts apart, and
neither ratio set is evidence of a real cost or a real speed-up. The choice of
`Task.deleted` rests on the layout fact above. Tests were not run on the
patched tree. No deleted task existed, so these results measure branch cost,
not skipped work.

| task count | graph rebuild run | graph rebuild run |
|---|---|---|
| 10000 | 0.26 ms | 0.21 ms |
| 50000 | 1.47 ms | 1.52 ms |
| 100000 | 5.7 ms | 2.5 ms |

Graph rebuild is `compute_cold` minus `compute`. The 100k result is noisy.

| n | `order_by_start` full re-sort | filter collapsed 1/4 | `LinkIndex::new` |
|---|---|---|---|
| 10000 | 0.38 ms | 0.006 ms | 0.47 ms |
| 50000 | 2.7 ms | 0.08 ms | 2.9 ms |
| 100000 | 6.1 ms | 0.16 ms | 6.4 ms |

Row-index numbers are the best of 30 timings of one call, on a generated
schedule, under the optimised test profile. The table shows one of three test
runs; the spread across the three was under 3%. A 60 Hz frame is 16.7 ms. A
collapse that re-sorts and rebuilds `LinkIndex` at 100k tasks costs about
12.5 ms in total. These costs occur once per collapse, insert, or compaction,
not per frame.

## Appendix: the measured patch

This is the condensed field-in-`Task` variant. Omitted `generate.rs` hunks add
`deleted: false` to two `Task` literals. The full patch and logs were in
uncommitted scratch space.

```diff
diff --git a/crates/engine/src/cpm.rs b/crates/engine/src/cpm.rs
@@
 impl Pass<'_> {
+    #[inline(always)]
+    fn is_dead(&self, i: usize) -> bool {
+        self.tasks[i].deleted
+    }
@@
         for &pi in self.g.order.iter().rev() {
             let p = pi as usize;
+            if self.is_dead(p) {
+                continue;
+            }
@@
         for &si in &g.order {
             let s = si as usize;
+            if pass.is_dead(s) {
+                continue;
+            }
@@
             if si != task.0 && !moved[s] {
                 continue;
             }
+            if pass.is_dead(s) {
+                continue;
+            }
diff --git a/crates/engine/src/model.rs b/crates/engine/src/model.rs
@@
 pub struct Task {
+    pub deleted: bool,
```

## Amended by decision 0016

### Amendment (gantt#2110, decision 0016, karpathy 2026-11-16)

Decision 1 and both amendments above stand. [Decision 0016](0016-host-identity.md) settles what they left open about a host that keeps its own ids:

- *A `TaskId` is identity inside one session only.* It never moves between a load and the next load, open or remount. It is not stable across a save: the document writer renumbers tasks to live display order. A `TaskId` or a `DepId` handed to a host is named session-only in the docs, and a host does not store one.
- *A host's identity for a task, a link or a calendar is its key,* opaque text kept in one module (`crates/app/src/host_keys.rs`) with the one map from task key to `TaskId`. Two records of one kind with the same key are refused at the door. Every host-facing call and event takes and returns the key.
- *A tombstone keeps its key.* A deleted task's key stays taken, so undo brings back the same keyed task.
- *Compaction is still out.* When its `Remap` arrives, it is applied to the key map in that module, as to every other holder in Decision 1's list.
