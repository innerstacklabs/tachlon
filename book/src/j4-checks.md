# Check a schedule
<!-- journey: J4 -->

{{#include ../../packages/gantt/guide/checks.md:goal}}

<figure class="shot">
<img src="media/j4-checks.webp" alt="The office fit-out sample with the Checks window open over the chart: eleven lines, each with a check's name, its count and one sentence on what it counts, from Missing logic 1 to Execution index 1.00">
<figcaption>The Checks window lists the counts and does not say pass or fail.</figcaption>
</figure>

## By hand

1. **Open the panel.** Click **Checks** in the toolbar, the button with three short lines, beside
   **Open…** and the export button. A window titled **Checks** opens over the chart with fourteen
   lines, each with the check's name, its number and one sentence on what it counts. A line with a problem to show adds a second sentence, in the text color, on why that kind of problem matters; a line that counts `0` adds none. Try it on the
   [office fit-out sample](/?sample=office-fitout).
2. **Read the lines.** On the fit-out sample, the lines from Missing logic to Missing resources read Missing logic `1`, Open starts `0`, Open ends `1`, Leads `0`, Lags `7`, Finish to
   start `63 of 69 links`, High float `0`, High duration `0`, Hard constraints `0`, Negative float `0`,
   Invalid dates `0` and Missing resources `0`.
3. **Close it.** **Close** or `Escape` shuts the window, and the chart gets its keys back.
4. **It counts again each time it opens.** The window shows the schedule as it was when you opened
   it. On the fit-out, click the bar of **Detailed design and specification**, press `]` to select
   one of its links, press `Delete`, and open **Checks**: Missing logic reads `2`, Open starts `1`, Open ends `1` and Finish to
   start reads `62 of 68 links`. Close it and press `Ctrl+Z`, and the panel reads `1`, `0`, `1` and `63 of
   69 links` again.
{{#include ../../packages/gantt/guide/checks.md:missing}}

{{#include ../../packages/gantt/guide/checks.md:show}}
On this page, each of those lines has a **Show** button while its count is above 0: it closes the window and keeps only the tasks that line counts, the same as the Check choice in **Filters** (the row count reads `Check = Open ends`). Hover bars for the card that shows each task's predecessor and successor counts (see [Find your way around a big schedule](j2-navigate.md)), then add the link as in [Build a schedule from the keyboard](j6-build.md).

{{#include ../../packages/gantt/guide/checks.md:leads}}
{{#include ../../packages/gantt/guide/checks.md:lead-fix}}
See step 5 of [Build a schedule from the keyboard](j6-build.md).

{{#include ../../packages/gantt/guide/checks.md:share}}
On this page, the share reads, for example, `63 of 69 links`; a file with no links reads `no links`.

{{#include ../../packages/gantt/guide/checks.md:counts}}

{{#include ../../packages/gantt/guide/checks.md:invalid}}
To move the data date, see [Update progress against a data date](j8-progress.md). On the fit-out, typing `2027-01-04` in the data date box makes this `13`.

{{#include ../../packages/gantt/guide/checks.md:no-date}}
On this page, with no data date the line reads `needs a data date`. The [50,000-task demo](/?sample=demo-50k) has none, and its panel reads that line. Type a day in the **Data date** box and press `Enter`, and the line is a number.

{{#include ../../packages/gantt/guide/checks.md:resources}}
On this page, that line reads `needs resources`; the [50,000-task demo](/?sample=demo-50k) has none either. On the fit-out sample, every task but the milestones has a crew, so it reads `0`.

{{#include ../../packages/gantt/guide/checks.md:baseline}}
The page on setting a baseline is [Set a baseline, read what slipped](baseline.md). On this page, Execution index reads `none due` when none were due.

{{#include ../../packages/gantt/guide/checks.md:limits}}

16. **What is not built.** The fourteen lines cover twelve of DCMA's fourteen checks. The other two
    are on [What's coming](coming.md).

## Where it is checked

`apps/hosted/tests/checks-dialog.spec.ts` presses the button on a file with a hand-counted answer
and reads each line's name, number and sentence, the reason a flagged line gives and a `0` line does not, the `needs a data date` and `needs resources` lines, a file with one
unassigned task, and `no links`.
`packages/gantt/tests/checks-report.spec.ts` asserts the counts the page asks the engine for.
`crates/app/tests/checks.rs`, `checks_schedule.rs` and `checks_report.rs` check the counts
against hand-solved schedules, and the report's JSON.
