# Moving from Bryntum Gantt

`fromBryntum` reads the JSON that a Bryntum Gantt project loads (its tasks and dependencies) into the
document (decision 0010), which `Gantt.load({ document })` takes. It is plain TypeScript: no wasm, no
network, no DOM. It runs in the browser or in Node.

```ts
import { fromBryntum, mount } from "@gantt/chart";

const gantt = await mount(root, { schedule: { demo: 1 } });
const bryntum = await (await fetch("/api/gantt-load")).json(); // what your Bryntum project loads
const read = fromBryntum(bryntum);
if (!read.ok) throw new Error(read.error);
const loaded = await gantt.load({ document: read.document });
if (!loaded.ok) throw new Error(loaded.error);
console.log(read.leftOut); // one line for each thing the adapter could not carry
```

`fromBryntum` never throws. Data with no task list returns `{ ok: false, error }`. A document that
`load` refuses (for example, dependencies that form a loop) comes back from `load` as
`{ ok: false, error }`, and the chart keeps the schedule it had.

## What it reads

| Bryntum | Document |
|---|---|
| `tasks` (`{ rows }` or an array; `tasksData` too) | `tasks`, in outline order |
| `children` (nested) or `parentId` (flat) | `parent` and `outline_level`; a task with children is a summary row |
| `name` | `name` |
| `id` | `code`, as text, so a row can be traced back to its Bryntum task |
| `duration` and `durationUnit` | `duration` in working minutes: a day is 480, a week 2,400, a month 9,600; an hour is 60 |
| `startDate` | `snet` (the earliest start) when the task has no predecessor, or `manuallyScheduled` is true |
| `constraintType: "startnoearlierthan"` with `constraintDate` | `snet` |
| `constraintType: "startnolaterthan"` with `constraintDate` | `limit.snlt` (the latest start) |
| `constraintType: "finishnoearlierthan"` with `constraintDate` | `limit.fnet` (the earliest finish) |
| `constraintType: "finishnolaterthan"` with `constraintDate` | `limit.fnlt` (the latest finish) |
| `constraintType: "muststarton"` or `"mustfinishon"` with `constraintDate` | `limit.pin_start` or `limit.pin_finish`: the task is held to that date, and no `snet` comes from it |
| `constraintType: "aslateaspossible"`, with or without `constraintDate` | `limit.alap`: the task starts as late as the schedule allows, and no `snet` comes from its date |
| `percentDone` | `progress` |
| `dependencies` (`{ rows }` or an array; `dependenciesData` too) | `links` |
| dependency `type` 0, 1, 2, 3 | `SS`, `SF`, `FS`, `FF` (no `type` is `FS`) |
| dependency `lag` and `lagUnit` | `lag` in working minutes (negative for a lead) |

Dates are read as written: a time zone or offset in the text is ignored. The origin of the document is
the Monday on or before the earliest `startDate` or `constraintDate`. The chart computes every other
start and finish from the durations and links, so a task that has a predecessor starts when the
predecessor lets it, not on the date the Bryntum data stored.

## What it leaves out

Each item below that appears in the data is listed in `leftOut`, and the same lines are the document's
`notes`, which the chart shows under its import-notes button.

- **Calendars.** Every task follows one calendar: Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00.
  Project and task calendars, holidays and exceptions are not read. A day of duration is 480 working
  minutes whatever `hoursPerDay` says.
- **Resources and assignments.**
- **Baselines.**
- **Constraints of any other type, and a dated type above with no `constraintDate`.** The task keeps its
  duration and links.
- **Dependencies that touch a parent task.** The document links tasks that have no children. A link
  to or from a parent is dropped.
- **Dependencies to a task that is not in the data, from a task to itself, and repeats of the same
  pair.** The first link of a pair is kept.
- **Actual dates and remaining work.** `percentDone` is kept as the percentage the chart shows; the
  document gets no actual start or finish.
- **Effort, scheduling modes, rollups, custom fields, styling and anything else not named above.**
  These are not listed one by one.

A task with a `duration` or `durationUnit` the adapter cannot read is read with duration 0, and a
line says so. A task whose parent is not in the data is read as a top-level task.
