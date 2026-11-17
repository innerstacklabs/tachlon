// The DHTMLX Gantt side's hooks, for its community edition (MIT), read from the page
// `dhtmlx-setup.ts` installs (`window.gantt`, DHTMLX's global API) and from the page's own elements.
// Each runs in the page, so each is an arrow function that closes over nothing: Playwright sends its
// source text, and a method's text is not an expression. `ready` runs once a frame during load, so it
// reads only what is cheap: the count, one bar and one task.
//
// DHTMLX draws its rows and bars as DOM elements. The timeline's visible part is `.gantt_task
// .gantt_data_area`, below its scale; its own scrollbars (`.gantt_ver_scroll`, `.gantt_hor_scroll`)
// hold the scroll, which `gantt.getScrollState()` reports. The bars sit in `.gantt_bars_area`, each
// `.gantt_task_line` with its task's id (`data-task-id`, the dataset's: `dhtmlx-adapt.ts` keeps them)
// and a class for its type (`gantt_bar_task`, `gantt_milestone`, `gantt_project`). A bar's left in
// the bars area is `gantt.posFromDate(start_date)`.

import type { Side } from "../side.ts";

const dhtmlx: Side = {
  name: "dhtmlx",

  ready: (rows) => {
    const g = (window as any).gantt;
    if (g === undefined || (window as any).ganttParsedAt === undefined) return false;
    if (g.getTaskCount() !== rows || document.querySelector(".gantt_task .gantt_row, .gantt_grid .gantt_row") === null) return false;
    // And the frame shows a leaf's bar where its task's start date puts it: drawn from the data, not before.
    const bar = document.querySelector(".gantt_bars_area .gantt_task_line.gantt_bar_task") as HTMLElement | null;
    const id = bar?.getAttribute("data-task-id");
    const task = id == null || !g.isTaskExists(id) ? undefined : g.getTask(id);
    if (task?.start_date == null) return false;
    return Math.abs(bar!.offsetLeft - g.posFromDate(task.start_date)) <= 1;
  },

  scrollTarget: () => {
    // The rows of the timeline that show: its data area (below the scale), clipped to the timeline.
    // Wheel there scrolls the rows down and the time axis along.
    const g = (window as any).gantt;
    const cell = (document.querySelector(".gantt_task") as HTMLElement).getBoundingClientRect();
    const area = document.querySelector(".gantt_task .gantt_data_area") as HTMLElement;
    const box = area.getBoundingClientRect();
    const v = document.querySelector(".gantt_ver_scroll") as HTMLElement | null;
    const h = document.querySelector(".gantt_hor_scroll") as HTMLElement | null;
    const s = g.getScrollState();
    // The data area is as wide as the whole time axis and moves left as it scrolls: clip it to the timeline.
    const left = Math.max(box.left, cell.left);
    return {
      x: left,
      y: box.top,
      width: Math.min(box.right, cell.right) - left,
      height: Math.min(box.bottom, cell.bottom) - box.top,
      top: s.y,
      left: s.x,
      maxTop: v === null ? 0 : v.scrollHeight - v.clientHeight,
      maxLeft: h === null ? 0 : h.scrollWidth - h.clientWidth,
    };
  },

  // Every bar the DOM holds that is not a project's, wherever it lies. `data-task-id` is the dataset's id.
  // A `type: "project"` task's bar has `gantt_project` (and `gantt_bar_project`), a leaf's `gantt_bar_task`,
  // a milestone's `gantt_milestone` (a leaf too, as on the other sides).
  leafBars: () => {
    const bars = ([...document.querySelectorAll(".gantt_bars_area .gantt_task_line:not(.gantt_project)")] as HTMLElement[]).map((el) => {
      const b = el.getBoundingClientRect();
      const id = el.getAttribute("data-task-id")!;
      return { x: b.left, y: b.top, width: b.width, height: b.height, id, datasetId: id };
    });
    const sc = (window as any).gantt.getScale();
    return { bars, pxPerDay: sc.unit === "day" && sc.step === 1 ? sc.col_width : Number.NaN };
  },

  // The dragged task's start date in DHTMLX's store. The community edition does not schedule: the
  // drop (rounded to the scale) is the whole commit, done in the release's own handler.
  dragState: (target) => {
    const g = (window as any).gantt;
    return g.isTaskExists(target.id) ? String(g.getTask(target.id).start_date.getTime()) : "none";
  },

  backend: () => ((window as any).gantt ? "DOM" : null),

  // The page loads the package's published build (`codebase/dhtmlxgantt.js`, minified), nothing else.
  debugBuild: () => document.querySelector('script[src$="codebase/dhtmlxgantt.js"]') === null,

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

  inputBytes: async () => {
    const url = (window as any).ganttDataUrl;
    return typeof url === "string" ? (await (await fetch(url)).arrayBuffer()).byteLength : null;
  },

  rowsOnScreen: () => {
    const area = document.querySelector(".gantt_task .gantt_data_area") as HTMLElement;
    return Math.floor(area.clientHeight / (window as any).gantt.config.row_height);
  },

  // A sample of DHTMLX's own task store against the dates handed to it (`expected.json`, written by
  // `dhtmlx-adapt.ts`), to the minute: DHTMLX keeps a start and an end and works out the duration,
  // and that must not move either date. A milestone ends where it starts.
  spotCheck: async () => {
    const res = await fetch(`${(window as any).ganttDataDir}expected.json`).catch(() => null);
    if (res === null || !res.ok) return "no expected.json beside the page: run dhtmlx-adapt.ts for this dataset";
    const expected = (await res.json()) as { id: number | string; start: string; end: string }[];
    const g = (window as any).gantt;
    const z = (n: number) => String(n).padStart(2, "0");
    const wall = (d: Date | null | undefined) => (d == null ? "none" : `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}T${z(d.getHours())}:${z(d.getMinutes())}`);
    for (const e of expected) {
      const t = g.isTaskExists(e.id) ? g.getTask(e.id) : undefined;
      if (wall(t?.start_date) !== e.start || wall(t?.end_date) !== e.end) return `task ${e.id} is ${wall(t?.start_date)} to ${wall(t?.end_date)}, the schedule's is ${e.start} to ${e.end}`;
    }
    return expected.length === 0 ? "expected.json holds no tasks" : null;
  },

  // When `gantt.parse` returned: the dates taken, projects rolled up, the first view drawn.
  settled: () => (window as any).ganttParsedAt ?? null,
};

export default dhtmlx;
