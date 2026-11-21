# Set a baseline, read what slipped
<!-- journey: J3 -->

{{#include ../../packages/gantt/guide/baseline.md:goal}}

## By hand

{{#include ../../packages/gantt/guide/baseline.md:set}}
On this page, press **Baseline** in the toolbar: the button's tooltip confirms it. The button stays pressed, and a **Clear baseline** button appears beside it. A P6 XER file opens with no baseline: press **Baseline**, or compare with an earlier file.

{{#include ../../packages/gantt/guide/baseline.md:compare}}
On this page, open the **Open…** menu and choose **Compare with a file...**, then choose the earlier file. Four lists under the toolbar name the tasks and the links that are only in the earlier file and those only in this one (new work), for example `A1020 Pour slab` and `A1010 → A1020 (FS)` (a link is the same in both files when its two IDs and its type are), the first 200 of each and then `and N more`; close them with the ×.

   **Example: an activity that was renumbered.** Last month's file has `A1020 Pour slab`, and this
   month's file has the same task as `A1025 Pour slab`. The IDs differ, so the two tasks do not
   match. The older file's list shows `A1020 Pour slab` under **Only in the older file**, and this
   file's list shows `A1025 Pour slab` under **Only in this file**. The chip counts one removed
   and one added. `A1025 Pour slab` has no baseline entry, so its ΔStart, ΔFinish and ΔFloat cells
   are empty. To match them, give the task its old ID, `A1020`, in this file and compare again, or
   read the two lists as one task.

{{#include ../../packages/gantt/guide/baseline.md:add}}
On this page, while any exist, an **Add new** button shows in the Baseline group, and it goes away when nothing is added.

{{#include ../../packages/gantt/guide/baseline.md:read}}

{{#include ../../packages/gantt/guide/baseline.md:slipped}}
On this page, the four are buttons in the Filters panel, and the others are in [Find your way around a big schedule](j2-navigate.md). With a Project file's own baseline, the **Float lost** button is disabled and says so.

{{#include ../../packages/gantt/guide/baseline.md:moved}}
The page on changing dates is [Change dates and see what moved](j7-change.md).

## How to read what slipped

{{#include ../../packages/gantt/guide/baseline.md:reading}}
On this page, the buttons are **Slipped**, **Baseline** and **Add new**.

## Where it is checked

`packages/gantt/tests/baseline.spec.ts` asserts the Baseline button, the status line, the undo, the Slipped
button, the Add new button and the tooltip when the button is pressed again.
`packages/gantt/tests/baseline-columns.spec.ts` checks the variance columns: the `+5d` after an edit, no
columns without a baseline, the critical color, the hold during a drag, the summary rows and
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
