// The Bryntum side's behaviour without the trial: its hooks run against a fake Bryntum-like page
// (a `window.gantt` and the few elements the hooks read), and its page script runs against a fake
// `gantt.module.js`. What each checks is what the hooks and the page do, not their source text.

import assert from "node:assert/strict";
import { copyFileSync, mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import bryntum from "./bryntum.ts";

const here = dirname(fileURLToPath(import.meta.url));
const g = globalThis as any;

type Rect = { left: number; top: number; width: number; height: number };
/** An element with a box, data attributes and the children a selector finds. */
class El {
  dataset: Record<string, string> = {};
  rect: Rect;
  found: Record<string, El[]>;
  up: Record<string, El>;
  constructor(rect: Rect, found: Record<string, El[]> = {}, up: Record<string, El> = {}) {
    this.rect = rect;
    this.found = found;
    this.up = up;
  }
  getBoundingClientRect() {
    const { left, top, width, height } = this.rect;
    return { left, top, width, height, x: left, y: top, right: left + width, bottom: top + height };
  }
  get clientHeight() {
    return this.rect.height;
  }
  querySelector(sel: string) {
    return this.found[sel]?.[0] ?? null;
  }
  querySelectorAll(sel: string) {
    return this.found[sel] ?? [];
  }
  closest(sel: string) {
    return this.up[sel] ?? null;
  }
}

const WRAP = ".b-gantt-task-wrap:not(.b-gantt-task-parent)";
const DAY = new Date(2026, 2, 2, 8);

/** A fake page: one task bar for task 1 at `barLeft` px into the time axis, whose start date puts it at 100 px. */
function page(o: { barLeft?: number; count?: number; meta?: string } = {}) {
  const canvas = new El({ left: 400, top: 0, width: 2000, height: 45000 });
  const bar = new El({ left: 400 + (o.barLeft ?? 100), top: 120, width: 40, height: 20 });
  const wrap = new El({ left: 400 + (o.barLeft ?? 100), top: 115, width: 40, height: 30 }, { ".b-gantt-task": [bar] }, { ".b-sch-foreground-canvas": canvas });
  wrap.dataset.taskId = "1";
  const normal = new El({ left: 400, top: 60, width: 880, height: 45000 }, { [WRAP]: [wrap] });
  const body = new El({ left: 0, top: 60, width: 1280, height: 720 });
  const task = { startDate: DAY, endDate: new Date(2026, 2, 3, 16) };
  const gantt = {
    taskStore: { count: o.count ?? 1000, getById: (id: unknown) => (String(id) === "1" ? task : undefined) },
    subGrids: { normal: { element: normal, scrollable: { x: 0, maxX: 61000 } } },
    element: new El({ left: 0, top: 0, width: 1280, height: 800 }, { ".b-grid-row": [new El({ left: 0, top: 60, width: 1280, height: 45 })] }),
    scrollable: { element: body, y: 0, maxY: 44264 },
    rowHeight: 45,
    isEngineReady: true,
    timeAxis: { startDate: new Date(2026, 2, 1) },
    getCoordinateFromDate: (d: Date) => (d.getTime() === DAY.getTime() ? 100 : ((d.getTime() - new Date(2026, 2, 1).getTime()) / 864e5) * 20),
  };
  const meta = o.meta === undefined ? null : { getAttribute: () => o.meta };
  g.window = g;
  g.document = { querySelector: (sel: string) => (sel.startsWith("meta") ? meta : null) };
  g.gantt = gantt;
  return { gantt, task };
}

const SAVED = ["window", "document", "gantt", "fetch", "location", "ganttDated", "ganttScheduledAt", "ganttDatasetSha256", "ganttDataDir", "ganttDragSeen"];
const before = Object.fromEntries(SAVED.map((k) => [k, g[k]]));
afterEach(() => {
  for (const k of SAVED) {
    if (before[k] === undefined) delete g[k];
    else g[k] = before[k];
  }
});

test("ready: the computed variant waits for the engine's first calculation and a bar where its date puts it", () => {
  page();
  assert.equal(bryntum.ready(1000), false, "before the initial commit's dataReady");
  g.ganttScheduledAt = 1234;
  assert.equal(bryntum.ready(1000), true);
  assert.equal(bryntum.ready(999), false, "another row count");
  page({ barLeft: 102 });
  g.ganttScheduledAt = 1234;
  assert.equal(bryntum.ready(1000), false, "a bar 2 px from where its start date puts it: drawn before the dates");
});

test("ready: the dated variant may be ready before the engine, and settled says when the engine was done", () => {
  page();
  g.ganttDated = true;
  assert.equal(bryntum.ready(1000), true);
  assert.equal(bryntum.settled!(), null, "the engine has not finished");
  g.ganttScheduledAt = 2500.5;
  assert.equal(bryntum.settled!(), 2500.5);
});

test("the drag state is the task's start date as of the last idle engine; bars, box, build and dataset come from the page", () => {
  const { gantt, task } = page({ meta: "Bryntum 7.3.8 trial build" });
  assert.deepEqual(bryntum.leafBars(), { bars: [{ x: 500, y: 120, width: 40, height: 20, id: "1", datasetId: "1" }], pxPerDay: 20 }, "the task bar's box, not its wrap's");
  const t = { x: 520, y: 130, pxPerDay: 20, id: "1" };
  const before = bryntum.dragState(t);
  assert.equal(before, String(DAY.getTime()));
  task.startDate = new Date(2026, 2, 12, 8);
  gantt.isEngineReady = false;
  assert.equal(bryntum.dragState(t), before, "the record moved, but the engine is still placing what follows it");
  gantt.isEngineReady = true;
  assert.equal(bryntum.dragState(t), String(task.startDate.getTime()), "committed once the engine is idle");

  assert.deepEqual(bryntum.scrollTarget(), { x: 400, y: 60, width: 880, height: 720, top: 0, left: 0, maxTop: 44264, maxLeft: 61000 }, "the time axis clipped to the body");
  assert.equal(bryntum.rowsOnScreen!(), 16);
  assert.equal(bryntum.debugBuild!(), true);
  assert.equal(bryntum.build!(), "Bryntum 7.3.8 trial build");
  g.ganttDated = true;
  assert.equal(bryntum.build!(), "Bryntum 7.3.8 trial build, dates given");
  g.ganttDatasetSha256 = "ab".repeat(32);
  assert.equal(bryntum.datasetSha256!(), "ab".repeat(32));
  page({ meta: "Bryntum 7.3.8" });
  assert.equal(bryntum.debugBuild!(), false, "a build that is no trial");
});

test("the spot check passes the schedule's dates, one minute either way, and names a task that is off or a missing file", async () => {
  page();
  g.ganttDataDir = "cells/tree4-1000-1/";
  let asked = "";
  const serve = (body: unknown, ok = true) => (url: string) => {
    asked = url;
    return Promise.resolve({ ok, json: async () => body });
  };
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:01", end: "2026-03-03T16:00" }]);
  assert.equal(await bryntum.spotCheck!(), null);
  assert.equal(asked, "cells/tree4-1000-1/expected.json");
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:02", end: "2026-03-03T16:00" }]);
  assert.match((await bryntum.spotCheck!())!, /^task 1 is .*, the schedule's is 2026-03-02T08:02 to 2026-03-03T16:00$/);
  g.fetch = serve(null, false);
  assert.match((await bryntum.spotCheck!())!, /no expected\.json/);
});

/** Run the page script against a fake Bryntum module; what it did, in order. */
async function runPage(search: string) {
  const dir = mkdtempSync(join(tmpdir(), "bench-page-"));
  mkdirSync(join(dir, "node_modules", "@bryntum", "gantt"), { recursive: true });
  writeFileSync(join(dir, "node_modules", "@bryntum", "gantt", "package.json"), '{"name":"@bryntum/gantt","type":"module","main":"gantt.module.js"}\n');
  writeFileSync(join(dir, "package.json"), '{"type":"module"}\n');
  writeFileSync(
    join(dir, "node_modules", "@bryntum", "gantt", "gantt.module.js"),
    `const log = (e) => globalThis.pageLog.push(e);
export class Gantt { constructor(c) { log({ gantt: c }); this.project = { destroy: () => log("old project destroyed") }; } set project(p) { if (this._p) log("project set"); this._p = p; } get project() { return this._p; } }
export class ProjectModel { constructor(c) { log({ project: c }); this.config = c; } }
`,
  );
  copyFileSync(join(here, "bryntum-page.js"), join(dir, "page.js"));
  g.pageLog = [];
  g.window = g;
  g.location = { search, href: "http://side.test/" };
  g.fetch = (url: URL) => {
    g.pageLog.push({ fetch: url.href });
    return Promise.resolve({ json: async () => ({ datasetSha256: "ef".repeat(32), startDate: "2026-03-01T08:00:00", tasks: [] }) });
  };
  performance.clearMarks();
  const real = performance.mark.bind(performance);
  (performance as any).mark = (name: string) => {
    g.pageLog.push({ mark: name });
    return real(name);
  };
  try {
    await import(`${pathToFileURL(join(dir, "page.js")).href}?${encodeURIComponent(search)}`);
  } finally {
    (performance as any).mark = real;
  }
  return g.pageLog as any[];
}

test("the page: library ready, the empty Gantt, then data start, the cell's file, and one new project without the hash", async () => {
  const log = await runPage("?cell=tree4-1000-1&dates");
  const at = (pred: (e: any) => boolean) => log.findIndex(pred);
  const order = [
    at((e) => e.mark === "library-ready"),
    at((e) => e.gantt !== undefined),
    at((e) => e.mark === "data-start"),
    at((e) => e.fetch !== undefined),
    at((e) => e === "old project destroyed"),
    at((e) => e.project !== undefined),
    at((e) => e === "project set"),
  ];
  assert.ok(order.every((i, k) => i >= 0 && (k === 0 || i > order[k - 1])), `the order: ${JSON.stringify(log.map((e) => (typeof e === "string" ? e : Object.keys(e)[0])))}`);
  assert.equal(log[order[1]].gantt.project, undefined, "the Gantt is made empty, before the data");
  assert.equal(log[order[3]].fetch, "http://side.test/cells/tree4-1000-1/bryntum-dated.json");
  const config = log[order[5]].project;
  assert.equal(config.datasetSha256, undefined, "Bryntum never sees the hash");
  assert.deepEqual([config.taskStore, config.dependencyStore, config.startDate], [{ useRawData: true }, { useRawData: true }, "2026-03-01T08:00:00"]);
  assert.deepEqual([g.ganttDatasetSha256, g.ganttDated, g.ganttDataDir], ["ef".repeat(32), true, "cells/tree4-1000-1/"]);
  assert.equal(g.ganttScheduledAt, undefined);
  config.listeners.dataReady({ isInitialCommit: false });
  assert.equal(g.ganttScheduledAt, undefined, "a later commit is not the first calculation");
  config.listeners.dataReady({ isInitialCommit: true });
  assert.equal(typeof g.ganttScheduledAt, "number");

  const plain = await runPage("?cell=chain-1000-7");
  assert.ok(plain.some((e) => e.fetch === "http://side.test/cells/chain-1000-7/bryntum.json"), "without dates: constraints and links only");
  assert.equal(g.ganttDated, false);
});
