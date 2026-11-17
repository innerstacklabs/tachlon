// The Bryntum side's page script: our code, copied beside the trial by `bryntum-setup.ts` (as page.js).
//
// It loads one cell's schedule the way Bryntum's own big data set example does
// (https://bryntum.com/products/gantt/examples/bigdataset/): the Gantt is made first, with three
// columns (name, start date, duration) and no column lines, and the schedule then goes in as one
// new ProjectModel built from the whole data at once, with `useRawData` on the task and dependency
// stores ("speeds record creation up a bit by not cloning the raw data objects"); the example's
// approach A. Everything else is Bryntum's default, so the time axis spans the project and every
// row is shown. The Gantt is `window.gantt` for the side's hooks.
//
// `?cell=<shape>-<size>-<seed>` names the schedule: `cells/<cell>/` of this directory, written by
// `bryntum-dates.ts`. Two variants:
// - no `dates`: `bryntum.json`, constraints and links only; Bryntum's engine places every task.
// - `?dates`: `bryntum-dated.json`, the same tasks with the dates Bryntum's engine wrote back on a
//   first pass, as its console advises above 1,000 tasks ("Consider providing both `startDate` and
//   `endDate` for all tasks") and as a real app loads a saved project.
//
// The marks the driver splits load at (`demos/bench/side.ts`), placed as ours are: `library-ready`
// once the library has run (ours: the wasm instantiated); `data-start` once the component is made
// and before the schedule's fetch (ours: mounted, then the file fetched and opened). The project's
// first `dataReady` (its engine has placed every task and written the dates back) is
// `ganttScheduledAt`, which the side's `settled` reports.

import { Gantt, ProjectModel } from "@bryntum/gantt";

performance.mark("library-ready");
const params = new URLSearchParams(location.search);
const dir = `cells/${params.get("cell")}/`;
window.ganttDataDir = dir;
window.ganttDated = params.has("dates");

window.gantt = new Gantt({
  appendTo: "container",
  emptyText: "",
  columns: [
    { type: "name", field: "name", text: "Name", width: 200 },
    { type: "startdate", text: "Start date" },
    { type: "duration", text: "Duration" },
  ],
  columnLines: false,
});

performance.mark("data-start");
const file = window.ganttDated ? "bryntum-dated.json" : "bryntum.json";
const { datasetSha256, ...data } = await (await fetch(new URL(dir + file, location.href))).json();
window.ganttDatasetSha256 = datasetSha256;

window.gantt.project?.destroy();
window.gantt.project = new ProjectModel({
  taskStore: { useRawData: true },
  dependencyStore: { useRawData: true },
  ...data,
  // Bryntum draws before its engine has placed the tasks (`delayCalculation`), and its
  // `isEngineReady` holds before that first calculation starts; `dataReady` of the initial commit
  // is when the engine has placed every task and written the dates back (the event the example
  // waits on too).
  listeners: {
    dataReady({ isInitialCommit }) {
      if ((isInitialCommit ?? true) && window.ganttScheduledAt === undefined) window.ganttScheduledAt = performance.now();
    },
  },
});
