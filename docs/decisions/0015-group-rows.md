# Group rows

Status: accepted. Decided by karpathy, 2026-11-15 (gantt#1875, devteam-1 decision 1389). The shape is the same as the group feature of Bryntum and DHTMLX, and reversible: nothing is saved in a document.

## Context

`docs/PRODUCT.md` ("A7. Row filters and grouping") says grouping by an activity code makes "group rows that are not tasks", which strains decision 0001: `Timeline::order` assumes every row is a task. It asks for a decision record before the rendering rows. `crate::group::group_by_code` already returns one `Group` per value of a code type, and its docs and tests assume a different shape from the one below (see Consequences).

## Decision

1. **A grouped list is flat.** One header per value of the grouping code type, and under it the leaves that carry that value, in outline order. A summary task is not listed and has no header or summary of its own: no bar, no rolled-up dates. A collapsed summary hides no leaf while the list is grouped. A leaf with no value of the type goes under a header that is drawn as an empty-value marker, not as the literal text "(none)", so a real code value spelled "(none)" is its own group; that header is last, and the other headers are in order of value.
2. **A task with two values sits under its first.** First means first in the task's own list of activity codes (`TaskMeta::activity_codes`, file order) among those of the grouping type. A task appears once, so the row count is the leaf count.
3. **Filters apply first, then the grouping.** The groups are made from the rows the filters keep. A value whose leaves are all filtered out has no header, and the empty-value header is shown only when a kept leaf has no value.
4. **A header is a row of the view, not a task.** It has no `TaskId`, no schedule entry and no bar. How the row order carries it is for the rendering row to settle; `Timeline::order` stays a list of tasks.
5. **Insert and reparent are refused while the list is grouped**, as `run_insert` and `reparent_target` (indent, outdent, move) are refused while `Session::filtering` is true. In a flat list the row above is not the outline sibling, so a place or a parent chosen from the list would be a guess. Each refusal is an `EditError` that says so in plain words. Grouping adds no other refusal.
6. **The choice is view state, not saved.** The grouping is not in a saved document (decision 0010), an export, or the undo history. A new session is ungrouped. Collapse state is kept and returns when the grouping is turned off.

## Consequences

- **`group.rs` changes in the row after this one.** Today `group_by_code` puts a leaf with two values in both groups, leaves out a leaf with no value, and lists the summaries that contain a leaf. Points 1 and 2 reverse all three; the changed tests are that row's, and the rows after it build on its result.
- A rendering row says what it costs at 50,000 tasks: a frame-time read on a GPU machine for anything drawn per frame, and the rebuild time beside the 12.5 ms that decision 0001 measures at 100,000 tasks.
- `Session::set_group_by` takes a median 14.57 ms at 50,000 tasks (gantt#2024), against the 12.5 ms that decision 0001 measures for a collapse at 100,000 tasks. It runs once per click and stays inside a frame, so karpathy accepted it (2026-11-15).
- A user who wants an outline with groups does not get one. That is a later decision, and this record is amended first.
- A saved layout (PRODUCT.md, the Layouts row) would carry the grouping; that row amends point 6 first.
- Grouping by WBS (PRODUCT.md, J2) stays undecided; this record covers an activity code type only.