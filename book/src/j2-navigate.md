# Find your way around a big schedule
<!-- journey: J2 -->

**Goal:** in a large schedule, move around, and find any task, in seconds. Try it on the [50,000-task demo schedule](/?sample=demo-50k).

<figure class="shot">
<img src="media/j2-navigate.webp" alt="The 50,000-task demo schedule after a find: the Find box in the toolbar holds a name and reads 2 of 200, and the matching task is selected in the task list and on the chart">
<figcaption>Find a task by name: the box in the toolbar counts the matches, and Enter steps to the next one.</figcaption>
</figure>

## By hand

1. **Zoom and pan.** Ctrl+wheel or a trackpad pinch zooms at the cursor, from years down to
   hours. Drag pans both axes. The plain wheel scrolls rows. The **Fit** button fits the project.
2. **Collapse.** Click a summary row's chevron to hide or show the rows beneath it. Press `1` to
   `9` to collapse every summary to that outline level, `0` to expand all. Alt+Left collapses the
   selected summary (on any other row it selects the parent); Alt+Right expands it one level, and `*`
   expands it and every summary below it.
3. **Filter.** The **Filters** button in the toolbar opens the Filters panel, which holds the
   filters and the Lookahead. `Shift+C` shows only critical tasks (zero or negative total float, and not finished),
   with their summaries as context. `Shift+L` shows the longest path. Once a baseline is set (the
   toolbar's **Baseline** button keeps the dates as they are now), a **Slipped** button appears in
   the Filters panel and keeps only the tasks that finish later than the baseline, with their
   summaries; a task added since the baseline is not kept.
4. **Filter by activity code, or by what moved.** A file with P6 activity codes (an XER
   file) also gets an **Activity code** choice in the Filters panel: choose a code type, then one of
   its values, and only the tasks that carry that value stay, with the summaries that contain them.
   A **Clear** button in the panel appears while it is on and removes the activity-code
   restriction; a Lookahead window that is on, and any summaries you collapsed, stay as they are.
   Choosing another code type clears the filter until you choose a value. An MS Project file, or
   the demo, has no activity codes and shows no such choice. The **Show moved** button is in the
   Filters panel from the start, disabled until an edit moves other tasks. Then it keeps only the
   tasks that edit moved ([Change dates and see what moved](j7-change.md)); each new report moves
   it to that report's rows, and a change that
   makes no report ends it. These filters are modes of one setting, so turning one on turns the
   others off, and clearing the baseline turns Slipped off. When filters are on, the Filters button
   shows how many, for example `Filters · 2` with a filter and the Lookahead on. `c` jumps to the
   next critical task.
5. **Look ahead.** In the Filters panel, the **Lookahead** toggle, with its date box and weeks box, keeps
   only the tasks active in a date window. It combines with the filters above.
6. **Find.** Ctrl+F (Cmd+F) goes to the find box. Typing selects the first task whose name, activity ID
   or WBS code contains the text (a name is matched with its ancestors' names); an activity ID that is exactly the typed text is selected first. Enter goes to the next match and Shift+Enter to the previous one.
7. **Follow the driving path.** Click a bar to select it. The chain of driving predecessors and
   the driven successors stay in the accent color; every other bar and arrow dims. Escape clears
   the selection. Selecting a row by click or arrow key brings its bar into view.
   Three terms name what these views show:
   - **Critical** is a task with zero or negative total float that is not finished.
   - **Longest path** is the tasks found by tracing back from every task that finishes at the
     project finish, along links that drive. It can leave out a critical task, for example one whose
     successor starts on a start-no-earlier-than date.
   - **Driving path** is the chain of driving predecessors behind one selected task: the tasks whose
     links set its start, then theirs in turn.
8. **Read a task.** Hover a bar for a card with its activity ID, status, dates, actuals, float,
   baseline dates with their variance, and its predecessor and successor counts, with each
   link's type and lag.
9. **Resize the list.** Drag the divider between the task list and the chart to give either one
   more room (the list keeps at least 420 px and the chart at least 320 px). Double-click the
   divider to put the width back; with focus on it, Left and Right move it 16 px. In a right-to-left chart, dragging the list divider left widens the list, and `Left` moves the divider left.
10. **Hide a column.** The **Columns** button in the toolbar (an icon of three columns, next to Progress) opens a panel with one checkbox each for Start,
   Finish, Float and Free float, ticked while the column shows. Free float is unticked when the chart opens:
   tick it to add an **FF** column, how long a task can run late before the next task has to wait, empty for
   a summary and a finished task. Press one to hide that column; the Task column takes
   its room, and the cell cursor passes over it. Press it again to bring the column back. Escape closes
   the panel. Task and Dur stay, and an edit does not bring a hidden column back.
   Each of Dur, Start, Finish, Float and Free float also has an up and a down button in the panel. Press one to move that
   column up or down the list: the table puts it left or right of its neighbor, and the arrow keys walk the
   cells in the new order. The order lasts until you reload the page.

The toolbar shows the active filters and the visible row count, for example
`Critical only · 21 of 50,000 activities` (Critical only, Near critical and Longest path count
the activities they keep, not the rows, which also hold the summaries above them). The number after "of" counts
every activity in the schedule, summaries included: the [office fit-out sample](/?sample=office-fitout), with 50 activities
under 9 summaries, reads `Critical only · 23 of 59 activities`. With an activity code on, it names the code, for example
`Phase = Civil · 14 of 41 rows`. With only Slipped on, it counts the slipped tasks, for example
`46 slipped`, not the rows. With Longest path on, a line at the chart's bottom left names the path: its
first and last activity, how many activities it holds, and the calendar days from the first start to
the project finish, for example `Longest path: Excavate to Handover · 14 activities · 212 days`.

To take the list into a spreadsheet, open the hosted page's **Open…** menu and choose **Export the list as CSV**.
It downloads the visible rows as `<name>.csv` and says `Exported 41 rows to <name>.csv`. `Ctrl+A` then `Ctrl+C`
copies the same rows as tab-separated text, which a spreadsheet pastes into cells; so does **Copy the list** in the same menu,
for a tablet with no keyboard (it says `Copied 41 rows`). A filter or a collapsed
summary leaves its rows out of both (the note then reads `Exported 41 of 53 rows to <name>.csv`). The columns are
ID (when the file has IDs), WBS (when a task has a WBS code), Task, Dur, Start, Finish and Float, then the
variance columns while a baseline is set, then Actual start, Actual finish, Remaining and % complete when the file has progress.
**Export the list as Excel**, under the CSV item, downloads the same rows as `<name>.xlsx` and says `Exported 41 rows to <name>.xlsx`.

## Where it is checked

`packages/gantt/tests/zoom-pan.spec.ts`, `outline.spec.ts`, `filters.spec.ts`, `lookahead.spec.ts`,
`find.spec.ts` and `selection.spec.ts` assert each of these through the `GanttHook` test hook;
`baseline.spec.ts` asserts the Slipped button, and `columns-menu.spec.ts` the Columns menu.
