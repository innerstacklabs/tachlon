# Build a schedule from the keyboard
<!-- journey: J6 -->

{{#include ../../packages/gantt/guide/build.md:goal}}

To start from nothing, choose **New schedule**,
the first item of the Open menu: it gives a schedule with no tasks that starts on this week's Monday, and asks first
when the open schedule has unsaved changes. Until it has a task, the task list says "No tasks yet. Press Insert, or
Mod+Enter, to add the first one."

<figure class="shot">
<img src="media/j6-build.webp" alt="The demo schedule just after Insert: a new row named New task, one day long, sits below the selected row in the task list, with its bar on the chart">
<figcaption>Insert adds a one-day task below the selected row and selects it.</figcaption>
</figure>

## By hand

{{#include ../../packages/gantt/guide/build.md:steps}}

## With the toolbar

{{#include ../../packages/gantt/guide/build.md:insert-button}}

{{#include ../../packages/gantt/guide/build.md:delete-button}}

On this page, the toolbar's **Insert** and **Delete** buttons are those two buttons.

## Where it is checked

`packages/gantt/tests/insert-delete.spec.ts`, `indent.spec.ts`, `table-edit.spec.ts`, `cursor.spec.ts`,
`range.spec.ts`, `cell-grid.spec.ts`, `links.spec.ts`, `link-escape.spec.ts`, `link-edit.spec.ts` and `predecessors-column.spec.ts`. The Rust side checks each edit against a full `compute()` of the whole
schedule (`crates/app/tests/duration_cone.rs`).
