# Set a baseline, read what slipped
<!-- needs: chart -->

<!-- ANCHOR: goal -->
**Goal:** keep the plan as agreed, change the schedule for months, and read at any time what has slipped against that plan. The baseline is a saved copy of every task's dates, and everything here is the schedule set against it.
<!-- ANCHOR_END: goal -->

## Set the baseline
<!-- needs: call:setBaseline -->

<!-- ANCHOR: set -->
1. **Set the baseline.** Setting the baseline keeps the dates as they are now for every task. The chart confirms it, for example `Baseline set at data date 14 Oct: 59 rows`, or `Baseline set: 59 rows` when it was set with no data date, or came with a Project file. Setting it again keeps the dates as they are then in its place: the text names the data date it was set at, so after a data date move you can see that the second call took. Ctrl+Z takes the second one back, and one more Ctrl+Z the first: on a schedule that had no baseline that clears it, and on a file that came with its own it returns to that one. A Project file that carries its own Baseline 0 opens with the baseline already set. A P6 XER file opens with no baseline.
   <!-- control: setBaseline | keeps the dates as they are now as the baseline -->
<!-- ANCHOR_END: set -->

## Compare with a file
<!-- needs: call:setBaselineFromFile -->

<!-- ANCHOR: compare -->
Compare with last month's file instead. Give the chart the earlier XER or MS Project XML file of the same project, and it becomes the baseline: its tasks are matched to this schedule's by activity ID for a P6 file and by task UID for an MS Project file, and the chip names what slipped since, for example `vs baseline: 3 slipped · finish +2 days`. A file that is not a schedule, or has no task with an ID this schedule has, is refused with a message and nothing changes. Ctrl+Z puts the comparison that was there before back.
<!-- control: setBaselineFromFile | makes an earlier file of the same project the baseline, as one undo step -->
<!-- ANCHOR_END: compare -->

## Add new
<!-- needs: call:addNewToBaseline -->

<!-- ANCHOR: add -->
2. **Add new.** A task you insert after the baseline has no baseline dates. Adding the new tasks to the baseline gives each of those tasks its current dates as baseline; every other entry stays as it was, so what already slipped still reads as slipped. Ctrl+Z takes it back.
   <!-- control: addNewToBaseline | gives each task added since the baseline its current dates as baseline -->
<!-- ANCHOR_END: add -->

## Read the variance
<!-- needs: chart -->

<!-- ANCHOR: read -->
3. **Read the variance columns.** With a baseline set, the task list gets three columns after the others, **ΔStart**, **ΔFinish** and **ΔFloat**, in working days of the task's own calendar: `+5d` is five working days later (or, in ΔFloat, five more of float), `-2d` is two earlier, and a middot is no change. A later finish and less float read in the critical color, and no change reads in the muted color, so the eye goes to the rows that moved. A summary row reads the variance of its rolled-up span, and its ΔFloat stays blank. While you drag a bar the three columns are held empty, and they come back at the release. The **Variance** button hides them and shows them again; they stay hidden after an edit, and clearing the baseline takes the button away. The hover card of a task with a baseline entry also gives its baseline dates and variance.
4. **Read the comparison bars.** Each task bar has a thin gray bar under it at the baseline's dates. Edit a duration or drag a bar and the task bar moves while the gray bar stays, so the gap between them is the slip.
5. **Read the summary.** With a baseline set, a chip at the bottom right of the chart says how the schedule differs, for example `vs baseline: 3 slipped`, then `finish +2 days` when the project finish moved later and a critical clause when tasks joined or left the critical path, and `added` and `removed` counts when tasks came or went. A task counts as slipped only if it finishes later than its baseline and is live in both. The line follows each edit, undo and redo.
<!-- ANCHOR_END: read -->

## Keep only what slipped
<!-- needs: call:setFilter -->

<!-- ANCHOR: slipped -->
6. **Keep only what slipped.** Four filters appear once a baseline is set, and they work like the others in the page on finding your way around a big schedule, one at a time: Slipped (`Shift+S`) keeps the tasks that finish later than the baseline, Joined critical the tasks that are critical now and were not, Left critical the reverse, and Float lost the tasks with five or more working days less float than the baseline. A Project file's own baseline has no float, so Float lost cannot work with it; a baseline taken from a file carries float, so Float lost works with it. Clearing the baseline hides them.
   <!-- control: setFilter | keeps only the tasks that slipped, joined the critical path, left it or lost float since the baseline -->
<!-- ANCHOR_END: slipped -->

## Show moved
<!-- needs: chart, edit -->

<!-- ANCHOR: moved -->
7. **Show moved.** This is the other filter. **Show moved** (`Shift+M`) keeps what the last edit or drag moved, not what has slipped since the baseline; see the page on changing dates and seeing what moved. Use it right after a change to see its effect, and Slipped to see the total over the whole job.
<!-- ANCHOR_END: moved -->

## How to read what slipped
<!-- needs: call:setFilter, call:setBaseline, call:addNewToBaseline -->

<!-- ANCHOR: reading -->
Start with the chip: how many tasks slipped and how far the finish moved. Keep only the slipped tasks, and read ΔFinish down the list: the largest numbers are the tasks that slipped most. Where a task slipped and ΔFloat is negative, it ate float it had; where Joined critical lists it, the slip changed the critical path. Look at the gray bars to see the gap on the chart, and click a bar to see the driving path from the new dates. When you accept the new dates as the plan, setting the baseline again keeps them; adding the new tasks keeps the slips and only baselines what you added.
<!-- control: setFilter | keeps only the tasks that slipped -->
<!-- control: setBaseline | keeps the dates as they are now as the baseline -->
<!-- control: addNewToBaseline | baselines only the tasks added since the baseline -->
<!-- ANCHOR_END: reading -->
