# Update progress against a data date
<!-- needs: chart, edit -->

<!-- ANCHOR: goal -->
**Goal:** record the progress of each task, move the data date forward, and see the schedule follow. The progress cells hold the actuals; the data date says where "now" is.
<!-- ANCHOR_END: goal -->

## Show the progress cells
<!-- needs: chart -->

<!-- ANCHOR: show -->
1. **Show the progress cells.** The **Progress** button in the toolbar adds four cells to the task
   list, right after Task, in this order: **% complete**, **Actual start**, **Actual finish**, **Remaining**. A file
   that carries actuals opens with them showing; on a file with none they start off, and the
   button turns them on, empty. Press it again to take them away. The button shows whether they
   are on.
<!-- ANCHOR_END: show -->

## Type an actual
<!-- needs: chart, edit -->

<!-- ANCHOR: type -->
2. **Type an actual.** Double-click a progress cell, or put the cell cursor on it and press
   `Enter`. Actual start and Actual finish take a day written `YYYY-MM-DD`; Remaining takes a
   duration such as `4h` or `2d`; % complete takes a whole number from 0 to 100, with or without
   the `%` sign, such as `75` or `75%`. `Enter` commits and `Escape` discards. `Tab` commits and
   walks Actual start, Actual finish, Remaining, % complete, so from Remaining it goes back to % complete, the cell nearest
   Task, and from % complete to Actual start on the next row; `Shift+Tab` walks the other way, to the previous row's % complete
   from Actual start. Each commit is one undo step (`Ctrl+Z`). Committing the text a cell already
   shows adds no undo step, and an actual start that has a time of day keeps it when you commit the same
   day.
3. **How the cells depend on each other.** Setting Remaining on a started task sets its %
   complete from the task's duration: a task of eight working hours with `4h` remaining reads
   `50%`. Setting % complete sets Remaining, so `75%` on that task reads `2h`. A % complete from 1 to 99 on a task with no actual start also gives it one, its early start (with a data date set, a % complete that would put that start after the data date is refused); from then it is started and fixed there (step 7). A `0` on a task with no actual start leaves it unstarted, with no actual start and no remaining. A % complete of
   `100` finishes the task: it gets an actual finish and `0` remaining, and one `Ctrl+Z` puts the
   three cells back as they were. Setting an Actual finish also sets Remaining to `0`.
4. **A refused entry.** A bad entry leaves the editor open with the reason under it, and changes
   nothing:

   | Entry | Message |
   |---|---|
   | An actual finish before the actual start | "The actual finish is before the actual start." |
   | Text that is not a day | "Write the date as YYYY-MM-DD." |
   | A % complete outside 0 to 100 | "Write the percent complete as a whole number from 0 to 100, such as 75 or 75%." |
   | An actual start or finish after the data date, or a % complete that would set one | "An actual date cannot be after the data date. Move the data date first, or write a date on or before it." |

   An actual the task already holds passes, so its remaining can still be edited.
   Typing clears the reason, and `Escape` leaves the task as it was. A summary row takes no
   progress entry, because its figures come from the tasks under it.
<!-- ANCHOR_END: type -->

## Read a summary
<!-- needs: chart -->

<!-- ANCHOR: summary -->
5. **Read a summary.** A summary row's % complete is the duration-weighted share of the leaves
   under it: with two leaves of eight hours and one of them finished, the summary reads `50%`. A
   summary with nothing started under it shows nothing.
<!-- ANCHOR_END: summary -->

## Move the data date
<!-- needs: edit, call:setDataDate -->

<!-- ANCHOR: move -->
6. **Move the data date.** The data date is a line on the chart. Give the chart a day written `YYYY-MM-DD` to move the data date to it, or no day to clear it. Text that is not a date is refused ("Write the data
   date as YYYY-MM-DD."), and nothing changes.
   <!-- control: setDataDate | moves the data date to 00:00 on a day written YYYY-MM-DD, or clears it -->
<!-- ANCHOR_END: move -->

<!-- ANCHOR: moves -->
7. **What changes when it moves.** The line moves to 00:00 on the day. Work that has not started
   cannot start before the data date, so every such bar that sat before it moves right to the
   new date, and the tasks after it follow. Started and finished tasks are fixed at their actuals
   and do not move. The whole move is one undo step:
   `Ctrl+Z` puts the date, the line and every bar back, and `Ctrl+Shift+Z` repeats it. Clearing
   the data date lets the work it held back start earlier again. After a move that shifts other
   tasks, the note at the top right of the chart starts with `data date +7 days ·` (see the page on changing dates and seeing what moved).
<!-- ANCHOR_END: moves -->

## Go to the data date
<!-- needs: chart -->

<!-- ANCHOR: goto -->
8. **Go to the data date.** With the chart focused and the cell cursor not in the list, `t` pans
   the chart to the data date, keeping the zoom. With no data date it does nothing.
<!-- ANCHOR_END: goto -->

## Filter by status
<!-- needs: call:setFilter -->

<!-- ANCHOR: status -->
9. **Filter by status.** A status filter keeps only the tasks that are *In progress*, *Not started* or *Complete*. It is one filter at a time like the other row filters.
   <!-- control: setFilter | keeps only the tasks of one status: In progress, Not started or Complete -->
<!-- ANCHOR_END: status -->
