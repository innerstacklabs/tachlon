# Update progress against a data date
<!-- journey: J8 -->

{{#include ../../packages/gantt/guide/progress.md:goal}}

<figure class="shot">
<img src="media/j8-progress.webp" alt="A P6 office fit-out file five weeks in: the task list shows the % complete, Actual start and Actual finish cells filled in, and the chart has a dashed Data date line with finished tasks to its left and the critical path in red to its right">
<figcaption>A schedule with progress: the actuals sit in the task list and the data date is a line on the chart.</figcaption>
</figure>

## By hand

{{#include ../../packages/gantt/guide/progress.md:show}}

{{#include ../../packages/gantt/guide/progress.md:type}}

{{#include ../../packages/gantt/guide/progress.md:summary}}

{{#include ../../packages/gantt/guide/progress.md:move}}

On this page, the toolbar shows the data date as `Data date YYYY-MM-DD HH:MM`. With no data date, an empty box labeled **Data date** sits there
instead. To move it, click the date (or focus it and press `Enter` or `Space`) and type a day
written `YYYY-MM-DD`, then press `Enter`. `ArrowUp` and `ArrowDown` step the box by a day and
`Shift` with either steps a week; the date applies only when you press `Enter`, and `Escape`
puts the shown date back. Text that is not a date is refused in the alert line ("Write the data
date as YYYY-MM-DD."), and the box keeps focus and your text. An empty box plus `Enter` clears
the data date. `Enter` on the date as it stands adds no undo step.

{{#include ../../packages/gantt/guide/progress.md:moves}}

The note at the top right of the chart is described in [Change dates and see what moved](j7-change.md).

{{#include ../../packages/gantt/guide/progress.md:goto}}

{{#include ../../packages/gantt/guide/progress.md:status}}

On this page, the **Status** choice in the Filters panel does this; the **Status** entry at the top clears it. The other filters are in
[Find your way around a big schedule](j2-navigate.md).

## Where it is checked

`packages/gantt/tests/progress-toggle.spec.ts` asserts the Progress button: the four cells appear empty on a
file with no actuals and go again, the chart works without the toolbar, and a summary reads the
duration-weighted % of its leaves.
`packages/gantt/tests/progress-edit.spec.ts` types into the cells with the mouse and keyboard: the actual
start and its undo, the unchanged commit, Remaining and Actual finish with one undo each, the
refusal messages, `Tab` and `Shift+Tab`, and `100` in % complete.
`packages/gantt/tests/percent-cell-scrolled.spec.ts` checks that a press on a cell with the table scrolled
right goes to that cell and never back to Name.
`packages/gantt/tests/data-date.spec.ts` asserts the data date box: a typed day moves the line and the
unstarted bars, a refused entry, the arrow keys, an empty box and the undo and redo of each.
`packages/gantt/tests/goto-data-date.spec.ts` asserts `t`, and `packages/gantt/tests/filter-status.spec.ts` the Status
choice.
`crates/app/tests/data_date_edit.rs` checks the data date move against hand-solved dates, and
`crates/app/tests/progress_edit.rs` and `progress_rollup.rs` the progress entries and the summary
figure.
