# Find your way around a big schedule
<!-- needs: chart -->

<!-- ANCHOR: goal -->
**Goal:** in a large schedule, move around, and find any task, in seconds.
<!-- ANCHOR_END: goal -->

## Zoom, pan and collapse
<!-- needs: chart -->

<!-- ANCHOR: moving -->
1. **Zoom and pan.** Ctrl+wheel or a trackpad pinch zooms at the cursor, from years down to hours. Drag pans both axes. The plain wheel scrolls rows. The **Fit** button fits the project.
2. **Collapse.** Click a summary row's chevron to hide or show the rows beneath it. Press `1` to `9` to collapse every summary to that outline level, `0` to expand all. Alt+Left collapses the selected summary (on any other row it selects the parent); Alt+Right expands it one level, and `*` expands it and every summary below it.
<!-- ANCHOR_END: moving -->

## Filter by key
<!-- needs: chart -->

<!-- ANCHOR: filter-keys -->
3. **Filter.** `Shift+C` shows only critical tasks (zero or negative total float, and not finished), with their summaries as context. `Shift+L` shows the longest path. `c` jumps to the next critical task.
<!-- ANCHOR_END: filter-keys -->

## Filter by activity code, by what moved, or by a date window
<!-- needs: call:setFilter -->

<!-- ANCHOR: filters -->
4. **Filter by activity code, or by what moved.** A file with P6 activity codes (an XER file) also offers an activity code filter: choose a code type, then one of its values, and only the tasks that carry that value stay, with the summaries that contain them. Choosing another code type clears the filter until you choose a value. An MS Project file has no activity codes and offers no such choice. A filter for what moved keeps only the tasks the last edit moved (see the page on changing dates and seeing what moved); each new report moves it to that report's rows, and a change that makes no report ends it. These filters are modes of one setting, so turning one on turns the others off, and clearing the baseline turns Slipped off.
5. **Look ahead.** A Lookahead keeps only the tasks active in a date window, from a start date for a number of weeks. It combines with the filters above.
   <!-- control: setFilter | keeps only the tasks of one filter, such as an activity code or what moved, or of a Lookahead window -->
<!-- ANCHOR_END: filters -->

## Find
<!-- needs: call:find, call:findGo -->

<!-- ANCHOR: find -->
6. **Find.** A find box selects the first task whose name, activity ID or WBS code contains the typed text (a name is matched with its ancestors' names); an activity ID that is exactly the typed text is selected first. Enter goes to the next match and Shift+Enter to the previous one.
   <!-- control: find | counts the tasks whose name, activity ID or WBS code contains the text -->
   <!-- control: findGo | selects the match at one position in that list, which Enter and Shift+Enter step through -->
   <!-- shot: navigate-find | the find box holds a name and counts the matches, and the matching task is selected in the task list and on the chart -->
<!-- ANCHOR_END: find -->

## Read the chart
<!-- needs: chart -->

<!-- ANCHOR: read -->
7. **Follow the driving path.** Click a bar to select it. The chain of driving predecessors and the driven successors stay in the accent color; every other bar and arrow dims. Escape clears the selection. Selecting a row by click or arrow key brings its bar into view. Three terms name what these views show:
   - **Critical** is a task with zero or negative total float that is not finished.
   - Longest path is the tasks found by tracing back from every task that finishes at the project finish, along links that drive. It can leave out a critical task, for example one whose successor starts on a start-no-earlier-than date.
   - Driving path is the chain of driving predecessors behind one selected task: the tasks whose links set its start, then theirs in turn.
8. **Read a task.** Hover a bar for a card with its activity ID, status, dates, actuals, float, baseline dates with their variance, and its predecessor and successor counts, with each link's type and lag.
9. **Resize the list.** Drag the divider between the task list and the chart to give either one more room (the list keeps at least 420 px and the chart at least 320 px). Double-click the divider to put the width back; with focus on it, Left and Right move it 16 px. In a right-to-left chart, dragging the list divider left widens the list, and `Left` moves the divider left.
10. **Hide a column.** The **Columns** button (an icon of three columns, next to Progress) opens a panel with one checkbox each for Start, Finish, Float and Free float, ticked while the column shows. Free float is unticked when the chart opens: tick it to add a **Free float** column beside Float, how long a task can run late before the next task has to wait, empty for a summary and a finished task. Press one to hide that column; the Task column takes its room, and the cell cursor passes over it. Press it again to bring the column back. Escape closes the panel. Task and Dur stay, and an edit does not bring a hidden column back. Each of Dur, Start, Finish, Float and Free float also has an up and a down button in the panel. Press one to move that column up or down the list: the table puts it left or right of its neighbor, and the arrow keys walk the cells in the new order. The order lasts until you reload the page.
<!-- ANCHOR_END: read -->

## Copy the list
<!-- needs: chart -->

<!-- ANCHOR: copy -->
`Ctrl+A` then `Ctrl+C` copies the visible rows as tab-separated text, which a spreadsheet pastes into cells. A filter or a collapsed summary leaves its rows out. The columns are ID (when the file has IDs), WBS (when a task has a WBS code), Task, Dur, Start, Finish and Float, then the variance columns while a baseline is set, then Actual start, Actual finish, Remaining and % complete when the file has progress.
<!-- ANCHOR_END: copy -->

## Export the list
<!-- needs: call:exportCsv, call:exportXlsx, call:copyList -->

<!-- ANCHOR: export -->
Exporting the list as CSV downloads the visible rows as `<name>.csv` and says `Exported 41 rows to <name>.csv`. A filter or a collapsed summary leaves its rows out (the note then reads `Exported 41 of 53 rows to <name>.csv`). The columns are the ones `Ctrl+A` then `Ctrl+C` copies. Copying the list puts the same rows on the clipboard as tab-separated text, for a tablet with no keyboard (it says `Copied 41 rows`). Exporting the list as Excel downloads the same rows as `<name>.xlsx` and says `Exported 41 rows to <name>.xlsx`.
<!-- control: exportCsv | downloads the visible rows as a CSV file -->
<!-- control: exportXlsx | downloads the visible rows as an Excel file -->
<!-- control: copyList | puts the visible rows on the clipboard as tab-separated text -->
<!-- ANCHOR_END: export -->
