# Change dates and see what moved
<!-- needs: chart, edit -->

<!-- ANCHOR: goal -->
**Goal:** make a change and see its effect on the schedule. A one-line summary of what moved shows
at the top right of the chart for a few seconds, and a **Show moved** button below it, off the time axis, keeps only the
tasks that change moved.
<!-- ANCHOR_END: goal -->

## Steps

<!-- ANCHOR: steps -->
1. **Drag a bar.** Drag a task's bar to move it. The drag sets a start-no-earlier-than and
   recomputes only the tasks downstream of it, so the critical path updates on every step. During
   the drag the list's dates follow the bar; the variance columns wait until you release. A dragged task lands on a working-day
   start of its calendar. A task with an actual start or finish, from a file or typed in, does not follow a drag.
   After a drag of a finished task a chip says why on release: `Framing is finished: its dates are its actuals. Change Actual finish in the
   table to move it.` A task that has started but not finished refuses a move with its own chip, `Framing has started: its start is its
   actual start. Change Remaining or % complete in the table to move its finish.`, and a stretch with `Framing has started: change
   Remaining or % complete in the table to move its finish.`
2. **Edit a duration.** Double-click a duration cell and type `5d`. Dependent bars move and the
   float of the predecessors updates. Drag the right end of a bar to stretch or shrink a task, and one undo puts it back.
3. **Add, remove or change a link.** The page on building a schedule from the keyboard says how. A link edit,
   including a change of its type or lag,
   shows the same note as a duration edit, and so does undoing or redoing it. No task is the
   edited one, so the count includes every task that moved, the link's later task among them.
   A link that moves no early date shows no note.
4. **Read the result.** After an edit, a link change or a drag that moved other tasks, a note at the top right of
   the chart says what moved, for example `8 tasks moved · critical 2 joined, 1 left`.
   After a data date move the note starts with `data date +7 days ·`, and when the project finish moves
   a `finish` clause comes before the critical one. It stays for 4 seconds,
   then fades over 0.4 seconds (with "reduce motion" on in your system, it disappears at once, with
   no fade). A change that moves nothing, or opening a file, hides it. Bars that became critical
   turn red, and with "Critical only" on (`Shift+C`) the rows follow the new schedule. Click a bar to see the driving path from the new
   dates.
5. **Show what moved.** While a report exists, a **Show moved** button sits below the note, off the time axis. It
   stays after the note fades, until the next change. Press it and the task list keeps only the
   tasks the change moved and the task you edited, with their summaries. It is one filter at a time like the other row filters, so it replaces Critical only, Longest path, Slipped or an activity
   code, and choosing one of those replaces it. It follows the changes you make: each new report,
   including the one an undo or a bar drag release makes, moves the filter to that report's rows,
   so it always shows what the latest change moved. A change that moves nothing makes no report
   and ends the filter.
6. **Undo.** Ctrl+Z. A whole bar drag is one undo step.
<!-- ANCHOR_END: steps -->
