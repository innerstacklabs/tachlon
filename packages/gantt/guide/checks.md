# Check a schedule
<!-- needs: call:checks -->

<!-- ANCHOR: goal -->
**Goal:** before you trust a schedule's dates or send it on, count the things that make a schedule unreliable: tasks tied to nothing, odd links, and tasks that are very long or very loose. The checks give fifteen lines. They do not say pass or fail; you decide what a count means for your schedule.
<!-- ANCHOR_END: goal -->
<!-- control: checks | reports one count for each of the fifteen checks -->
<!-- shot: checks-lines | the checks listed over the chart, each line with a check's name, its count and one sentence on what it counts -->

## Missing logic

<!-- ANCHOR: missing -->
5. **Missing logic.** Unfinished tasks with nothing before them or nothing after them, summary tasks not counted. An unlinked task floats free of the schedule: if its predecessor slips, nothing moves it. Open starts counts the same tasks that have nothing before them, and Open ends those that have nothing after them, so you can see which side is loose; a task with no links at all is in both, and counts once in Missing logic.
<!-- ANCHOR_END: missing -->

## Show the tasks a check counts
<!-- needs: call:checks, call:setFilter -->

<!-- ANCHOR: show -->
A check with a count above 0 can keep only the tasks it counts: Missing logic, Open starts, Open ends, High float, High duration, Hard constraints, Negative float, Invalid dates, Missing resources and Missed (Invalid dates keeps leaf tasks only).
<!-- control: setFilter | keeps only the tasks that one check counts -->
<!-- ANCHOR_END: show -->

## Leads and lags

<!-- ANCHOR: leads -->
6. **Leads and Lags.** Leads are links with a negative lag, so the next task starts before the one it follows has finished; Lags are links with a positive lag, a wait between the two. A lead makes the dates depend on an overlap nobody can see on the bar, and a lag hides a task inside a link.
<!-- ANCHOR_END: leads -->

## Remove a lead or a lag
<!-- needs: call:checks, edit -->

<!-- ANCHOR: lead-fix -->
   To remove one, select the link, press `Enter` and type the type alone, such as `FS`, or add a task for the wait. The box takes `FS-1d` and `FS+2d`.
   <!-- control: checks | counts the links that carry a lead or a lag -->
<!-- ANCHOR_END: lead-fix -->

## Finish to start

<!-- ANCHOR: share -->
7. **Finish to start.** The share of links that finish one task and start the next, such as 8 of 10; a file with no links has no share. A low share means many start-to-start and finish-to-finish links, which are harder to follow. Look at the links that are not finish to start and ask whether each one is needed.
<!-- ANCHOR_END: share -->

## Float, duration and constraints

<!-- ANCHOR: counts -->
8. **High float.** Unfinished tasks that can slip more than 44 working days before they push the finish, counted in days of the task's own calendar. A very loose task is often missing a successor: check what it feeds.
9. **High duration.** Tasks longer than 44 working days. A duration of `100d` may be a typing slip for `10d`; if it is real, split the task into steps that can be tracked.
10. **Hard constraints and Negative float.** Hard constraints counts the unfinished tasks a fixed date holds: a start no later than, a finish no later than, or a must start on or must finish on date. A task held to a date ignores what its predecessors do, so each one is a place the logic no longer drives the schedule. Negative float counts the unfinished tasks that cannot meet a late date: a date that holds a task is earlier than the date its predecessors allow, so its float is below `0` and the task is critical. Both skip summary tasks and read `0` on a file with no such date. A link into a task that a pin holds before its predecessor allows is one the pin breaks; the chart draws it two pixels wide in the held color.
<!-- ANCHOR_END: counts -->

## Invalid dates

<!-- ANCHOR: invalid -->
11. **Invalid dates.** Tasks with an actual start or finish after the data date. A schedule cannot hold work done in the future: either the data date is too early and needs a later day, or an actual was typed wrong.
<!-- ANCHOR_END: invalid -->

## No data date
<!-- needs: call:checks, call:setDataDate -->

<!-- ANCHOR: no-date -->
12. **No data date.** With no data date set, Invalid dates has no number and never reads `0`, because a `0` would look like a clean check that never ran. Give the chart a data date and the line is a number.
    <!-- control: setDataDate | gives the chart a data date written YYYY-MM-DD -->
<!-- ANCHOR_END: no-date -->

## Missing resources

<!-- ANCHOR: resources -->
13. **Missing resources.** Unfinished tasks with a duration and no resource, summary tasks and milestones not counted. A task nobody is assigned to carries no labor or cost. When the file lists no resource at all, the line has no number and never reads `0`, for the same reason as the data date. A P6 XER file reads that way for now, because the XER reader does not read its assignments yet.
<!-- ANCHOR_END: resources -->

## Missed and Execution index

<!-- ANCHOR: baseline -->
14. **Missed and Execution index.** Both read the baseline (see the page on setting a baseline) against the data date. Missed counts the tasks the baseline had finishing by the data date that finished later or have not finished yet; it reads `0` with no baseline. Execution index is the tasks finished over the tasks the baseline had finishing by the data date, such as `0.80`. When none were due, with no baseline or no data date, it has no number, never `0`.
<!-- ANCHOR_END: baseline -->

## How high is high

<!-- ANCHOR: limits -->
15. **How high is high.** The checks show no limit. The limits in DCMA's 14-point assessment guidance are 5% of tasks for missing logic, high float, high duration, hard constraints and missed tasks, 5% of links for lags, at least 90% of links finish to start, zero allowed for leads, negative float and invalid dates, and an execution index of at least 0.95. They are guidance, not rules. Work out the share yourself from the count and the number of tasks or links, and treat a count a little over a limit as a prompt to look.
<!-- ANCHOR_END: limits -->
