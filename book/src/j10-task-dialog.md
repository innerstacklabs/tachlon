# Edit a task in its dialog
<!-- journey: J10 -->

{{#include ../../packages/gantt/guide/task-dialog.md:goal}}

## By hand

{{#include ../../packages/gantt/guide/task-dialog.md:steps}}

## Where it is checked

`packages/gantt/tests/task-dialog.spec.ts` asserts that `Shift+Enter` opens the dialog, that Save
applies two fields as one undo step, that a refused field keeps the dialog open and names the field,
that `Escape` changes nothing, and that `F2` still opens the cell editor.
`packages/gantt/tests/task-dialog-open.spec.ts` asserts that a double-click on a bar or on the ID
cell opens the dialog and that one on the name cell opens the cell editor.
