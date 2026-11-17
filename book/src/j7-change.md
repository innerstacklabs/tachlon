# Change dates and see what moved
<!-- journey: J7 -->

**Goal:** make a change and see its effect on the schedule. A one-line summary of what moved shows
at the top right of the chart for a few seconds, and a **Show moved** button below it, off the time axis, keeps only the
tasks that change moved.

<figure class="shot">
<img src="media/j7-change.webp" alt="The demo schedule just after a bar was dragged: a note at the top right of the chart reads 328 tasks moved, finish +9 days, with the Show moved button below it">
<figcaption>After a drag, the note at the top right says what moved; Show moved keeps only those tasks.</figcaption>
</figure>

## By hand

1. **Drag a bar.** Drag a task's bar to move it. The drag sets a start-no-earlier-than and
   recomputes only the tasks downstream of it, so the critical path updates on every step. The
   rows hold still during the drag and refresh on release. A dragged task lands on a working-day
   start of its calendar. A task with an actual finish (from a P6 or MS Project file) is fixed at
   its actual dates and does not follow a drag. On release a chip says why, for example `Pour slab
   is finished: its dates are its actuals. Change Actual finish in the table to move it.`
2. **Edit a duration.** Double-click a duration cell and type `5d`. Dependent bars move and the
   float of the predecessors updates. Someone can stretch or shrink a task by dragging the right end of its bar, and one undo puts it back.
3. **Add, remove or change a link.** See [Build a schedule from the keyboard](j6-build.md). A link edit,
   including a change of its type or lag,
   shows the same note as a duration edit, and so does undoing or redoing it. No task is the
   edited one, so the count includes every task that moved, the link's later task among them.
   A link that moves no early date shows no note.
4. **Read the result.** After an edit, a link change or a drag that moved other tasks, a note at the top right of
   the chart says what moved, for example `8 tasks moved · critical 2 joined, 1 left`;
   after a data date move it starts with `data date +7 days ·`, and when the project finish moves
   a `finish` clause comes before the critical one. It stays for 4 seconds,
   then fades over 0.4 seconds (with "reduce motion" on in your system, it disappears at once, with
   no fade). A change that moves nothing, or opening a file, hides it. Bars that became critical
   turn red, and with "Critical only" on (`Shift+C`) the rows follow the new schedule. Click a bar to see the driving path from the new
   dates.
5. **Show what moved.** While a report exists, a **Show moved** button sits below the note, off the time axis. It
   stays after the note fades, until the next change. Press it and the task list keeps only the
   tasks the change moved and the task you edited, with their summaries (the header names it
   **Moved**). It is one filter at a time like the others in [Find your way around a big
   schedule](j2-navigate.md), so it replaces Critical only, Longest path, Slipped or an activity
   code, and choosing one of those replaces it. It follows the changes you make: each new report,
   including the one an undo or a bar drag release makes, moves the filter to that report's rows,
   so it always shows what the latest change moved. A change that moves nothing makes no report
   and ends the filter.
6. **Undo.** Ctrl+Z. A whole bar drag is one undo step.

## Where it is checked

`packages/gantt/tests/hover-drag.spec.ts` runs the hover card, bar drag, undo and redo journey.
`packages/gantt/tests/moved-note.spec.ts` checks the note: its text, the 4 second hold, the fade, the hold while the pointer is over it, and where it sits.
`packages/gantt/tests/moved-note-reduced.spec.ts` checks it with "reduce motion" on (full at 3.9 seconds, gone at 4.05, no fade) and after a drag release (the bars do not slide).
`crates/app/tests/filter_moved.rs` checks the Show moved filter, and `moved-note.spec.ts` its button.
`crates/app/tests/duration_cone.rs` and `crates/engine/tests/cone.rs` compare the recompute from
a moved or changed task with a full `compute()`.
