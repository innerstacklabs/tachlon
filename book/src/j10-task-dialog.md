# Edit a task in its dialog
<!-- journey: J10 -->

**Goal:** change several of a task's fields in one place, and undo the whole change in one step. The
task dialog holds a task's name, duration, start, finish, percent complete, actual dates, remaining
work, start no later than, finish no earlier than and finish no later than together.

## By hand

1. **Open it.** Click a task in the list to select it and press `Shift+Enter`, or double-click its
   bar, or double-click a list cell that has no editor of its own (the ID or float cell). The
   dialog is titled `Edit task:` and the task's name, and shows the task's fields as the list
   writes them: **Name**, **Duration**, **Start**, **Finish**, **Percent complete**,
   **Actual start**, **Actual finish**, **Remaining**, **Start no later than**, **Finish no earlier
   than** and **Finish no later than**. Double-clicking a name, duration, start,
   finish or progress cell still opens that cell's own editor, and `F2` still does too. The dialog
   does not open while a drag is under way.
2. **Change the fields.** Type in any of them. A date is written `YYYY-MM-DD`.
3. **Save.** Click **Save**, or press `Enter` in a field. The fields you changed are applied
   together as one edit, and a single `Ctrl+Z` puts all of them back. A field you did not touch
   is not sent.
4. **A refused field.** When the chart refuses a value, the dialog stays open and says why, in a
   line that starts with the field's name, for example `Duration:` for a duration it cannot read.
   Nothing has changed, not even the fields before it. Correct the value and save again.
5. **Leave without a change.** **Cancel** or `Escape` closes the dialog and changes nothing. After
   any of the three ways out, the chart has focus again, so its keys work.

## Where it is checked

`packages/gantt/tests/task-dialog.spec.ts` asserts that `Shift+Enter` opens the dialog, that Save
applies two fields as one undo step, that a refused field keeps the dialog open and names the field,
that `Escape` changes nothing, and that `F2` still opens the cell editor.
`packages/gantt/tests/task-dialog-open.spec.ts` asserts that a double-click on a bar or on the ID
cell opens the dialog and that one on the name cell opens the cell editor.
