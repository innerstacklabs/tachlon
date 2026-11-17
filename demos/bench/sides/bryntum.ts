// The Bryntum side's hooks, read from the page `bryntum-setup.ts` installs (`window.gantt`, the
// trial's Gantt, through its public API) and from the page's own elements. Each runs in the page, so
// each is an arrow function that closes over nothing: Playwright sends its source text, and a
// method's text is not an expression. `ready` runs once a frame during load, so it reads only what is
// cheap: two flags, a count, one element and one record.
//
// One side for both variants of the page (`bryntum-page.js`): with `?dates` the tasks come with the
// dates Bryntum's engine wrote back on a first pass; `build` names which, for the table.

import type { Side } from "../side.ts";

const bryntum: Side = {
  name: "bryntum",

  ready: (rows) => {
    const w = window as any;
    const g = w.gantt;
    // Without dates given, the engine must have placed every task (the page notes the initial
    // commit's `dataReady`; Bryntum draws before it has), as ours has when it reports ready. With
    // dates given the frame may come first; `settled` then holds the input phases until it has.
    if (g === undefined || g.taskStore.count !== rows || (w.ganttDated !== true && w.ganttScheduledAt === undefined)) return false;
    // And the frame shows a task bar where its task's start date puts it: drawn after the dates, not before.
    const wrap = g.subGrids.normal.element.querySelector(".b-gantt-task-wrap:not(.b-gantt-task-parent)") as HTMLElement | null;
    if (wrap === null || g.element.querySelector(".b-grid-row") === null) return false;
    const id = wrap.dataset.taskId!;
    const task = g.taskStore.getById(id) ?? g.taskStore.getById(Number(id));
    const canvas = wrap.closest(".b-sch-foreground-canvas") as HTMLElement | null;
    if (task?.startDate == null || canvas === null) return false;
    const left = wrap.getBoundingClientRect().left - canvas.getBoundingClientRect().left;
    return Math.abs(left - g.getCoordinateFromDate(task.startDate)) <= 1;
  },

  scrollTarget: () => {
    const g = (window as any).gantt;
    // The rows of the time axis that show: the time axis column (as tall as every row) clipped to
    // the body that scrolls them. Wheel there scrolls the rows down and the time axis along.
    const view = (g.subGrids.normal.element as HTMLElement).getBoundingClientRect();
    const body = (g.scrollable.element as HTMLElement).getBoundingClientRect();
    const top = Math.max(view.top, body.top);
    return {
      x: view.left,
      y: top,
      width: view.width,
      height: Math.min(view.bottom, body.bottom) - top,
      top: g.scrollable.y,
      left: g.subGrids.normal.scrollable.x,
      maxTop: g.scrollable.maxY,
      maxLeft: g.subGrids.normal.scrollable.maxX,
    };
  },

  // Every task bar the DOM holds (not a summary's), wherever it lies. The store's ids are the dataset's (`bryntum-adapt.ts`).
  leafBars: () => {
    const g = (window as any).gantt;
    const wraps = [...g.subGrids.normal.element.querySelectorAll(".b-gantt-task-wrap:not(.b-gantt-task-parent)")] as HTMLElement[];
    const bars = wraps.flatMap((wrap) => {
      const b = wrap.querySelector(".b-gantt-task")?.getBoundingClientRect();
      return b === undefined ? [] : [{ x: b.left, y: b.top, width: b.width, height: b.height, id: wrap.dataset.taskId!, datasetId: wrap.dataset.taskId! }];
    });
    const d0 = new Date(g.timeAxis.startDate);
    const d1 = new Date(d0);
    d1.setDate(d1.getDate() + 1);
    return { bars, pxPerDay: g.getCoordinateFromDate(d1) - g.getCoordinateFromDate(d0) };
  },

  // The dragged task's start date once Bryntum's engine is idle. The record takes its new date at the
  // drop, before the engine has rescheduled what follows it (1.3 to 4.3 s at 50,000 tasks, headless),
  // so while the engine is busy this gives the date it last gave when idle: the drag counts as
  // committed only when the whole schedule is.
  dragState: (target) => {
    const w = window as any;
    const store = w.gantt.taskStore;
    const task = store.getById(target.id) ?? store.getById(Number(target.id));
    const seen: Record<string, string> = (w.ganttDragSeen ??= {});
    if (w.gantt.isEngineReady === true || seen[target.id!] === undefined) seen[target.id!] = String(task?.startDate?.getTime());
    return seen[target.id!];
  },

  // Bryntum draws its rows and bars as DOM elements, not on a canvas.
  backend: () => ((window as any).gantt ? "DOM" : null),

  // The trial is not Bryntum's production build: its README says "Trial bundles are not minified".
  debugBuild: () => /trial build/.test(document.querySelector('meta[name="bench-build"]')?.getAttribute("content") ?? ""),

  build: () => {
    const label = document.querySelector('meta[name="bench-build"]')?.getAttribute("content") ?? null;
    return label === null ? null : (window as any).ganttDated === true ? `${label}, dates given` : label;
  },

  // When the engine's first calculation was done (the initial commit's `dataReady`).
  settled: () => (window as any).ganttScheduledAt ?? null,

  // The dataset the cell's files were made from, as `bryntum-adapt.ts` wrote it into them.
  datasetSha256: () => (window as any).ganttDatasetSha256 ?? null,

  rowsOnScreen: () => {
    const g = (window as any).gantt;
    return Math.floor((g.scrollable.element as HTMLElement).clientHeight / g.rowHeight);
  },

  // A sample of the tasks' dates against our engine's for the same file (`expected.json`, written by
  // `bryntum-dates.ts`), to the minute, one minute either way (docs/MEASURING.md, "The Bryntum side").
  spotCheck: async () => {
    // The Community tier's engine has no working calendar and keeps no start under a predecessor, so
    // its dates are not ours; its own store is checked instead: the first 64 finish-to-start links hold (a successor starts after its predecessor ends).
    if (/Community/.test(document.querySelector('meta[name="bench-build"]')?.getAttribute("content") ?? "")) {
      const links = ((window as any).gantt.dependencyStore.records as any[]).filter((d) => d.type === 2).slice(0, 64);
      for (const d of links) if (d.fromTask?.endDate == null || d.toTask?.startDate == null || d.toTask.startDate < d.fromTask.endDate) return `link ${d.id} is not scheduled: ${d.fromTask?.endDate} to ${d.toTask?.startDate}`;
      return links.length === 0 ? "no finish-to-start link to check" : null;
    }
    const res = await fetch(`${(window as any).ganttDataDir}expected.json`).catch(() => null);
    if (res === null || !res.ok) return "no expected.json beside the page: run bryntum-dates.ts for this dataset";
    const expected = (await res.json()) as { id: number | string; start: string; end: string }[];
    const store = (window as any).gantt.taskStore;
    const wall = (d: Date | null | undefined) => {
      if (d == null) return Number.NaN;
      return Date.UTC(d.getFullYear(), d.getMonth(), d.getDate(), d.getHours(), d.getMinutes()) / 60_000;
    };
    const minutes = (s: string) => Date.parse(`${s}:00Z`) / 60_000;
    for (const e of expected) {
      const t = store.getById(e.id) ?? store.getById(Number(e.id));
      const off = [wall(t?.startDate) - minutes(e.start), wall(t?.endDate) - minutes(e.end)];
      if (!off.every((m) => Math.abs(m) <= 1)) return `task ${e.id} is ${t?.startDate} to ${t?.endDate}, the schedule's is ${e.start} to ${e.end}`;
    }
    return expected.length === 0 ? "expected.json holds no tasks" : null;
  },
};

export default bryntum;
