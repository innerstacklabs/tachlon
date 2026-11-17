# Set a baseline, read what slipped
<!-- journey: J3 -->

**Goal:** keep the plan as agreed, change the schedule for months, and read at any time what has
slipped against that plan. The baseline is a saved copy of every task's dates, and everything on
this page is the schedule set against it.

## By hand

1. **Set the baseline.** Press **Baseline** in the toolbar. It keeps the dates as they are now
   for every task. The button's tooltip (and, on the developer page, the status line, the `s` key) confirms it, for example `Baseline set at data
   date 14 Oct: 59 rows`, or `Baseline set: 59 rows` when the schedule has no data date. The
   button stays pressed, and a **Clear baseline** button appears beside it. Press **Baseline**
   again to keep the dates as they are then in its place: the text names the data date it was
   set at, so after a data date move you can see that the second press took. Ctrl+Z takes either
   press back, and Ctrl+Z again clears the baseline. A Project file that carries its own Baseline
   0 opens with the baseline already set. A P6 XER file opens with no baseline: press **Baseline**, or compare with an earlier file.
   **Compare with last month's file instead.** Open the **Open…** menu and choose
   **Compare with a file...**, then choose the earlier XER or MS Project XML file of the same project, and it
   becomes the baseline: its tasks are matched to this schedule's by activity ID for a P6 file and by task UID for an MS Project file, and the chip names what
   slipped since, for example `vs baseline: 3 slipped · finish +2 days`. A file that is not a
   schedule, or has no task with an ID this schedule has, is refused with a message and nothing
   changes. Four lists under the toolbar name the tasks and the links that are only in the earlier file and those only in this one (new work), for example
   `A1020 Pour slab` and `A1010 → A1020 (FS)` (a link is the same in both files when its two IDs and its type are), the first 200 of each and then `and N more`; close them with the ×. Ctrl+Z puts the
   comparison that was there before back.
   **Example: an activity that was renumbered.** Last month's file has `A1020 Pour slab`, and this
   month's file has the same task as `A1025 Pour slab`. The IDs differ, so the two tasks do not
   match. The older file's list shows `A1020 Pour slab` under **Only in the older file**, and this
   file's list shows `A1025 Pour slab` under **Only in this file**. The chip counts one removed
   and one added. `A1025 Pour slab` has no baseline entry, so its ΔStart, ΔFinish and ΔFloat cells
   are empty. To match them, give the task its old ID, `A1020`, in this file and compare again, or
   read the two lists as one task.
2. **Add new.** A task you insert after the baseline has no baseline dates. While any exist, an
   **Add new** button shows in the Baseline group. Press it and each of those tasks takes its
   current dates as baseline; every other entry stays as it was, so what already slipped still
   reads as slipped. The button goes away when nothing is added, and Ctrl+Z takes it back.
3. **Read the variance columns.** With a baseline set, the task list gets three columns after
   the others, **ΔStart**, **ΔFinish** and **ΔFloat**, in working days of the task's own
   calendar: `+5d` is five working days later (or, in ΔFloat, five more of float), `-2d` is two
   earlier, and a middot is no change. A later finish and less float read in the critical
   colour, and no change reads in the muted colour, so the eye goes to the rows that moved. A
   summary row reads the variance of its rolled-up span, and its ΔFloat stays blank. While you
   drag a bar the three columns are held empty, and they come back at the release. The
   **Variance** button hides them and shows them again; they stay hidden after an edit, and
   clearing the baseline takes the button away. The hover card of a task with a baseline entry
   also gives its baseline dates and variance.
4. **Read the comparison bars.** Each task bar has a thin gray bar under it at the baseline's
   dates. Edit a duration or drag a bar and the task bar moves while the gray bar stays, so the
   gap between them is the slip.
5. **Read the summary.** With a baseline set, a chip at the bottom right of
   the chart say how the schedule differs, for example `vs baseline: 3 slipped`, then `finish
   +2 days` when the project finish moved later and a critical clause when tasks joined or left
   the critical path, and `added` and `removed` counts when tasks came or went. A task counts as
   slipped only if it finishes later than its baseline and is live in both. The line follows each
   edit, undo and redo.
6. **Keep only what slipped.** Four filters in the Filters panel appear once a baseline is set,
   and they work like the others in [Find your way around a big schedule](j2-navigate.md), one
   at a time: **Slipped** (`Shift+S`) keeps the tasks that finish later than the baseline,
   **Joined critical** the tasks that are critical now and were not, **Left critical** the
   reverse, and **Float lost** the tasks with five or more working days less float than the
   baseline. A Project file's own baseline has no float, so Float lost is disabled and says so; a
   baseline taken with **Compare with a file...** carries float, so Float lost works with it.
   Clearing the baseline hides the buttons.
7. **Show moved.** This is the other filter. **Show moved** (`Shift+M`) keeps what the last edit
   or drag moved, not what has slipped since the baseline; see [Change dates and see what
   moved](j7-change.md). Use it right after a change to see its effect, and Slipped to see the
   total over the whole job.

## How to read what slipped

Start with the chip: how many tasks slipped and how far the finish moved. Press **Slipped**
to list them, and read ΔFinish down the list: the largest numbers are the tasks that slipped
most. Where a task slipped and ΔFloat is negative, it ate float it had; where Joined critical
lists it, the slip changed the critical path. Look at the gray bars to see the gap on the
chart, and click a bar to see the driving path from the new dates. When you accept the new
dates as the plan, **Baseline** keeps them again; **Add new** keeps the slips and only baselines
what you added.

## Where it is checked

`packages/gantt/tests/baseline.spec.ts` asserts the Baseline button, the status line, the undo, the Slipped
button, the Add new button and the tooltip when the button is pressed again.
`packages/gantt/tests/baseline-columns.spec.ts` checks the variance columns: the `+5d` after an edit, no
columns without a baseline, the critical colour, the hold during a drag, the summary rows and
the Variance button.
`packages/gantt/tests/baseline-bars.spec.ts` reads the pixels of the gray bars and the gap an edit opens.
`packages/gantt/tests/baseline-summary.spec.ts` asserts the chip and the Slipped row count, and
`packages/gantt/tests/card-baseline.spec.ts` the baseline lines on the hover card.
`crates/app/tests/compare_file.rs` asserts the example: the lists, the removed and added counts and
the missing baseline entry of a task with no partner.
`crates/app/tests/filter_float_lost.rs` asserts the Float lost filter.
`crates/app/tests/baseline_add.rs`, `baseline_variance.rs` and `baseline_bars.rs` check Add new,
the variance cells and the bars in a frame, `baseline_open.rs` the Project file's own baseline,
and `crates/app/tests/filter_moved.rs` the Show moved filter.
