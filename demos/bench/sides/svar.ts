// The SVAR Gantt side's hooks, read from the page `svar-setup.ts` installs (`window.gantt`, SVAR's
// API from its `init` callback: `getState()` and `getTask(id)`) and from the page's own elements.
// Each runs in the page, so each is an arrow function that closes over nothing: Playwright sends its
// source text, and a method's text is not an expression. `ready` runs once a frame during load, so it
// reads only what is cheap: the state, one bar and one task.
//
// SVAR draws its rows and bars as DOM elements. The whole Gantt (`.wx-gantt`) scrolls the rows; the
// chart (`.wx-chart`) scrolls the time axis; the bars sit in `.wx-bars`, each `.wx-bar` with its
// task's id (`data-id`, the dataset's: `svar-adapt.ts` keeps them; SVAR writes a string id as
// `":" + id`, but the benchmark's ids are numbers, written as they are) and a class for its type
// (`wx-task`, `wx-milestone`, `wx-summary`). On SVAR's default day scale a bar starts at its start
// date's day: (days from the scale's start) times the width of a day (`_scales.lengthUnitWidth`).

import type { Side } from "../side.ts";

const svar: Side = {
  name: "svar",

  ready: (rows) => {
    const api = (window as any).gantt;
    if (api === undefined) return false;
    const s = api.getState();
    if (s._tasks?.length !== rows || document.querySelector(".wx-gantt .wx-row") === null) return false;
    // And the frame shows a leaf's bar where its task's start date puts it: drawn from the data, not before.
    const bar = document.querySelector(".wx-bars .wx-bar.wx-task") as HTMLElement | null;
    const task = bar === null ? undefined : (api.getTask(bar.dataset.id) ?? api.getTask(Number(bar.dataset.id)));
    if (task?.start == null || s._scales?.lengthUnit !== "day") return false;
    const day = (d: Date) => Date.UTC(d.getFullYear(), d.getMonth(), d.getDate()) / 86_400_000;
    const at = (day(task.start) - day(s._scales.start)) * s._scales.lengthUnitWidth;
    const left = bar!.getBoundingClientRect().left - bar!.parentElement!.getBoundingClientRect().left;
    return Math.abs(left - at) <= 1;
  },

  scrollTarget: () => {
    // The rows of the chart that show: the chart below its scale (`.wx-scale`), clipped to the Gantt. Wheel there
    // scrolls the rows (the Gantt) down and the time axis (the chart) along.
    const gantt = document.querySelector(".wx-gantt") as HTMLElement;
    const chart = gantt.querySelector(".wx-chart") as HTMLElement;
    const box = chart.getBoundingClientRect();
    const view = gantt.getBoundingClientRect();
    const scale = (chart.querySelector(".wx-scale") as HTMLElement).getBoundingClientRect();
    const top = Math.max(box.top, view.top, scale.bottom);
    return {
      x: box.left,
      y: top,
      width: chart.clientWidth,
      height: Math.min(box.top + chart.clientHeight, view.bottom) - top,
      top: gantt.scrollTop,
      left: chart.scrollLeft,
      maxTop: gantt.scrollHeight - gantt.clientHeight,
      maxLeft: chart.scrollWidth - chart.clientWidth,
    };
  },

  // Every bar the DOM holds that is not a summary's, wherever it lies. `data-id` is the dataset's
  // numeric id as it is (SVAR would prefix a string id with ":").
  leafBars: () => {
    const bars = ([...document.querySelectorAll(".wx-bars .wx-bar:not(.wx-summary)")] as HTMLElement[]).map((el) => {
      const b = el.getBoundingClientRect();
      return { x: b.left, y: b.top, width: b.width, height: b.height, id: el.dataset.id!, datasetId: el.dataset.id! };
    });
    const sc = (window as any).gantt.getState()._scales;
    return { bars, pxPerDay: sc.lengthUnit === "day" ? sc.lengthUnitWidth : Number.NaN };
  },

  // The dragged task's start date. SVAR's MIT edition does not schedule, so the drop's `update-task`
  // is the whole commit: the task store takes the new date then.
  dragState: (target) => {
    const api = (window as any).gantt;
    const task = api.getTask(target.id) ?? api.getTask(Number(target.id));
    return String(task?.start?.getTime());
  },

  backend: () => ((window as any).gantt ? "DOM" : null),

  // The bundle `svar-setup.ts` builds is a production one (React's included); the page notes which.
  debugBuild: () => (window as any).ganttProductionBuild !== true,

  build: () => document.querySelector('meta[name="bench-build"]')?.getAttribute("content") ?? null,

  // The dataset hash inside the file the page fetched, only when that file (fetched again) hashes
  // to the SHA-256 written beside it and holds the hash the page took out at load.
  datasetSha256: async () => {
    const w = window as any;
    if (typeof w.ganttDataUrl !== "string" || typeof w.ganttDatasetSha256 !== "string") return null;
    const [file, side] = await Promise.all([fetch(w.ganttDataUrl), fetch(`${w.ganttDataUrl}.sha256`)]);
    if (!file.ok || !side.ok) return null;
    const bytes = await file.arrayBuffer();
    const hash = [...new Uint8Array(await crypto.subtle.digest("SHA-256", bytes))].map((b) => b.toString(16).padStart(2, "0")).join("");
    const named = (await side.text()).trim().split(/\s+/)[0];
    const inside = JSON.parse(new TextDecoder().decode(bytes)).datasetSha256;
    return hash === named && inside === w.ganttDatasetSha256 ? inside : null;
  },

  rowsOnScreen: () => {
    const chart = document.querySelector(".wx-gantt .wx-chart") as HTMLElement;
    const scale = (chart.querySelector(".wx-scale") as HTMLElement).getBoundingClientRect().height;
    return Math.floor((chart.clientHeight - scale) / (window as any).gantt.getState().cellHeight);
  },

  // A sample of SVAR's own task store against the dates handed to it (`expected.json`, written by
  // `svar-adapt.ts`), to the minute: SVAR keeps a start and end and works out the duration, and
  // that must not move either date. A milestone has its start only.
  spotCheck: async () => {
    const res = await fetch(`${(window as any).ganttDataDir}expected.json`).catch(() => null);
    if (res === null || !res.ok) return "no expected.json beside the page: run svar-adapt.ts for this dataset";
    const expected = (await res.json()) as { id: number | string; start: string; end: string }[];
    const api = (window as any).gantt;
    const z = (n: number) => String(n).padStart(2, "0");
    const wall = (d: Date | null | undefined) => (d == null ? "none" : `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}T${z(d.getHours())}:${z(d.getMinutes())}`);
    for (const e of expected) {
      const t = api.getTask(e.id);
      const end = t?.type === "milestone" ? wall(t.start) : wall(t?.end);
      if (wall(t?.start) !== e.start || end !== e.end) return `task ${e.id} is ${wall(t?.start)} to ${end}, the schedule's is ${e.start} to ${e.end}`;
    }
    return expected.length === 0 ? "expected.json holds no tasks" : null;
  },
};

export default svar;
