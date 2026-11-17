import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { createServer, type ViteDevServer } from "vite";
import { expect, test, type Locator, type Page } from "@playwright/test";
import type { CriticalPathResult, ExplainResult, Gantt, HealthResult, RowResult, ViewState, WhatIfResult } from "../src/index";
import type { GanttHook } from "../src/test-hook";
import { chromePath } from "./support/knobs";
import { AXIS, ROW } from "./support/pixels";

// gs#1195: a host page drives find, the outline levels, row insert and delete, the data date and the baseline, and asks
// whether the chart is busy, through `Gantt` alone. The page below is such a host: it mounts two charts, A and B, keeps the
// `Gantt` each `mount` returned and installs the app's chrome (`@gantt/chrome`, whose find box the find test types in) on it, and calls nothing else; the spec reads the test hook only to see what happened and to find a
// bar to drag. B is there to show every call acts on its own chart. A holds the ledger (WBS summaries, a data date), loaded
// through `Gantt.load`. The chart's find and status keys reach the host as events (`on("find")`, `on("toggle-status")`).

/** What every busy refusal is shown with: `EditError::Busy`'s sentence. */
const BUSY = "Finish the drag first.";

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");

const HOST = `<!doctype html>
<html><head><meta charset="utf-8"><title>host</title></head>
<body style="margin: 0">
<div id="a" class="gantt" tabindex="0" style="width: 1200px; height: 420px"></div>
<div id="b" class="gantt" tabindex="0" style="width: 1200px; height: 300px"></div>
<script type="module">
  import { mount } from "/src/index.ts";
  import { installChrome } from "/@id/@gantt/chrome";
  const charts = {};
  for (const [key, n] of [["a", 40], ["b", 30]]) {
    const api = await mount(document.getElementById(key), { schedule: { demo: n }, forceWebgl: true });
    installChrome(api, document.getElementById(key));
    const heard = { find: 0, status: 0, changes: [] };
    api.on("find", () => { heard.find += 1; });
    api.on("toggle-status", () => { heard.status += 1; });
    api.on("change", (state) => heard.changes.push(state));
    charts[key] = { api, hook: window.gantt, heard };
  }
  window.charts = charts;
</script></body></html>`;

interface Kept {
  api: Gantt;
  hook: GanttHook;
  heard: { find: number; status: number; changes: ViewState[] };
}
type Key = "a" | "b";
declare global {
  interface Window {
    charts: Record<Key, Kept>;
  }
}

let server: ViteDevServer;
let base = "";

test.beforeAll(async () => {
  server = await createServer({
    root: fileURLToPath(new URL("..", import.meta.url)),
    logLevel: "silent",
    server: { host: "127.0.0.1", port: 0, strictPort: false },
  });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});

test.afterAll(async () => {
  await server.close();
});

test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });

/** Run `call` on chart `key`'s `Gantt` in the page and return what it returned. */
function host<T, A = undefined>(page: Page, key: Key, call: (api: Gantt, arg: A) => T, arg?: A): Promise<Awaited<T>> {
  return page.evaluate(
    ([k, body, a]) => {
      const run = new Function("api", "arg", `return (${body})(api, arg);`) as (api: Gantt, arg: unknown) => T;
      return run(window.charts[k as Key].api, a);
    },
    [key, call.toString(), arg] as const,
  ) as Promise<Awaited<T>>;
}

/** The task a row command answered with, or -1 when it refused. */
const taskOf = (r: RowResult | undefined): number => (r?.ok === true ? r.task : -1);

const state = (page: Page, key: Key): Promise<ViewState> => host(page, key, (api) => api.state());
const frames = (page: Page, key: Key): Promise<number> => page.evaluate((k) => window.charts[k].hook.status().frames, key);

async function nextFrame(page: Page, k = 4): Promise<void> {
  const target = (await frames(page, "a")) + k;
  await page.waitForFunction((t) => window.charts.a.hook.status().frames >= t, target);
}

const part = (page: Page, key: Key, name: string): Locator => page.locator(`#${key} [data-part="${name}"]`);

/** Open the host page, then load the ledger into A through `Gantt.load`. */
async function open(page: Page): Promise<void> {
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => window.charts?.a !== undefined && window.charts?.b !== undefined, undefined, { timeout: 180_000 });
  const loaded = await page.evaluate(async (text) => {
    const file = new File([new TextEncoder().encode(text)], "ledger.xer", { type: "application/octet-stream" });
    return window.charts.a.api.load({ file });
  }, LEDGER);
  expect(loaded).toEqual({ ok: true });
  await nextFrame(page, 6);
}

test.beforeEach(async ({ page }) => {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await open(page);
});

test("find counts the matches, and findGo and findNext select them, in that chart only", async ({ page }) => {
  // Two new rows give a query with a known count: insertRow names each `New task`.
  const first = await host(page, "a", (api) => api.commands.insertRow());
  const second = await host(page, "a", (api) => api.commands.insertRow());
  expect(first).toMatchObject({ ok: true });
  expect(second).toMatchObject({ ok: true });
  const tasks = [first, second].map(taskOf);

  expect(await host(page, "a", (api) => api.commands.find("New task"))).toBe(2);
  expect(await host(page, "b", (api) => api.commands.find("New task")), "B has its own schedule and its own search").toBe(0);

  expect(await host(page, "a", (api) => api.commands.findGo(1))).toBe(true);
  expect((await state(page, "a")).selection?.task).toBe(tasks[1]);
  expect(await host(page, "a", (api) => api.commands.findNext()), "findNext wraps to the first match").toBe(0);
  expect((await state(page, "a")).selection?.task).toBe(tasks[0]);
  expect(await host(page, "a", (api) => api.commands.findNext())).toBe(1);
  expect((await state(page, "a")).selection?.task).toBe(tasks[1]);

  // Bad input is refused with false, never a throw.
  for (const bad of [2, -1, 0.5, Number.NaN]) {
    expect(await host(page, "a", (api, i) => api.commands.findGo(i), bad), `findGo(${bad})`).toBe(false);
  }
  expect(await host(page, "a", (api) => api.commands.find(42 as unknown as string))).toBe(false);
  expect(await host(page, "a", (api) => api.commands.find("no task is called this"))).toBe(0);
  expect(await host(page, "a", (api) => api.commands.findNext()), "no match to go to").toBe(false);
  expect((await state(page, "b")).selection, "B's selection is untouched").toBeNull();
});

test("collapseToLevel and expandAll change the rows, and refuse a level that is not a whole number from 1", async ({ page }) => {
  const rowsB = (await state(page, "b")).rowCount;
  const all = (await state(page, "a")).rowCount;
  expect(await host(page, "a", (api) => api.commands.collapseToLevel(1))).toBe(true);
  const one = (await state(page, "a")).rowCount;
  expect(one).toBeLessThan(all);
  expect(await host(page, "a", (api) => api.commands.collapseToLevel(2))).toBe(true);
  expect((await state(page, "a")).rowCount).toBeGreaterThan(one);
  for (const bad of [0, -1, 1.5, "1"]) {
    expect(await host(page, "a", (api, level) => api.commands.collapseToLevel(level as number), bad), `collapseToLevel(${String(bad)})`).toBe(false);
  }
  expect(await host(page, "a", (api) => api.commands.expandAll())).toBe(true);
  expect((await state(page, "a")).rowCount).toBe(all);
  expect((await state(page, "b")).rowCount, "B keeps its rows").toBe(rowsB);
});

test("insertRow and deleteRow answer with the task or a named refusal, never a throw", async ({ page }) => {
  const tasksB = (await state(page, "b")).taskCount;
  // Nothing is selected after a load: delete has nothing to act on.
  expect(await host(page, "a", (api) => api.commands.deleteRow())).toEqual({ ok: false, reason: "noSelection", message: "Select a row first." });
  const before = await state(page, "a");
  const inserted = await host(page, "a", (api) => api.commands.insertRow());
  expect(inserted?.ok).toBe(true);
  const task = taskOf(inserted);
  const after = await state(page, "a");
  expect(after.taskCount).toBe(before.taskCount + 1);
  expect(after.selection?.task, "the new task is selected").toBe(task);
  expect(await host(page, "a", (api) => api.commands.deleteRow())).toEqual({ ok: true, task });
  expect((await state(page, "a")).taskCount).toBe(before.taskCount);

  // A summary is refused, and so is an insert under a row filter.
  const summary = await page.evaluate(() => {
    const rows = JSON.parse(window.charts.a.hook.app.table_rows(0, 10)) as [number, string, string, number | null, boolean, number][];
    return rows.find((row) => row[5] !== 0)![0];
  });
  expect(await host(page, "a", (api, t) => api.commands.select(t), summary)).toBe(true);
  expect(await host(page, "a", (api) => api.commands.deleteRow())).toEqual({ ok: false, reason: "hasChildren", message: expect.stringContaining("Delete or move its sub-tasks first.") });
  expect(await host(page, "a", (api) => api.commands.setFilter("critical"))).toBe(true);
  expect(await host(page, "a", (api) => api.commands.insertRow())).toEqual({ ok: false, reason: "filterActive", message: "Clear the row filter or window to insert a row." });
  expect((await state(page, "a")).taskCount).toBe(before.taskCount);
  expect((await state(page, "b")).taskCount, "B is untouched").toBe(tasksB);
});

test("the data date reads and sets through Gantt, and each change is one change event", async ({ page }) => {
  await nextFrame(page);
  const heard = (): Promise<number> => page.evaluate(() => window.charts.a.heard.changes.length);
  const count = await heard();
  expect(await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"))).toEqual({ ok: true });
  expect((await state(page, "a")).dataDate).toBe("2024-02-05");
  await nextFrame(page);
  expect(await heard()).toBe(count + 1);
  expect((await page.evaluate(() => window.charts.a.heard.changes.at(-1)!)).dataDate).toBe("2024-02-05");

  for (const bad of ["2024-02-30", "5 Feb", 20240205]) {
    expect(await host(page, "a", (api, day) => api.commands.setDataDate(day as string), bad), `setDataDate(${bad})`).toEqual({ ok: false, reason: "badDate", message: "Write the data date as YYYY-MM-DD." });
  }
  expect((await state(page, "a")).dataDate, "a refused date changes nothing").toBe("2024-02-05");
  expect(await host(page, "a", (api) => api.commands.setDataDate(null))).toEqual({ ok: true });
  expect((await state(page, "a")).dataDate).toBeNull();
  expect((await state(page, "b")).dataDate, "the demo in B has none").toBeNull();
});

test("the baseline sets, takes new tasks and clears through Gantt, and state() reads its rows and its data date", async ({ page }) => {
  expect((await state(page, "a")).baseline).toBeNull();
  expect(await host(page, "a", (api) => api.commands.clearBaseline())).toEqual({ ok: false, reason: "noBaseline", message: "There is no baseline to clear." });
  expect(await host(page, "a", (api) => api.commands.addNewToBaseline())).toEqual({ ok: false, reason: "noBaseline", message: "There is no baseline to clear." });

  await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"));
  expect(await host(page, "a", (api) => api.commands.setBaseline())).toEqual({ ok: true });
  const set = await state(page, "a");
  expect(set.baseline).toEqual({ rows: set.taskCount, dataDate: "2024-02-05", dayText: "5 Feb", slipped: 0, added: 0 });
  expect(await host(page, "a", (api) => api.filterInputs().baselineHasFloat)).toBe(true);

  expect((await host(page, "a", (api) => api.commands.insertRow()))?.ok).toBe(true);
  expect((await state(page, "a")).baseline?.rows, "a new task is not in the baseline").toBe(set.taskCount);
  expect(await host(page, "a", (api) => api.commands.addNewToBaseline())).toEqual({ ok: true });
  expect((await state(page, "a")).baseline?.rows).toBe(set.taskCount + 1);

  expect(await host(page, "a", (api) => api.commands.clearBaseline())).toEqual({ ok: true });
  expect((await state(page, "a")).baseline).toBeNull();
  expect((await state(page, "b")).baseline).toBeNull();
});

test("state().baseline tells a host how many tasks the baseline lacks and the day it was set, as the chart writes it", async ({ page }) => {
  await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"));
  await host(page, "a", (api) => api.commands.setBaseline());
  const set = (await state(page, "a")).baseline;
  expect(set?.added, "nothing was added since").toBe(0);
  expect(set?.dayText, "the day as the Baseline button's title writes it").toBe("5 Feb");
  expect(set?.dataDate, "dataDate stays the form setDataDate takes").toBe("2024-02-05");

  await host(page, "a", (api) => api.commands.insertRow());
  await host(page, "a", (api) => api.commands.insertRow());
  const grown = (await state(page, "a")).baseline;
  expect(grown?.added, "two tasks the baseline has no entry for").toBe(2);
  expect(grown?.rows).toBe(set?.rows);
  expect(grown?.dayText).toBe("5 Feb");

  await host(page, "a", (api) => api.commands.addNewToBaseline());
  const taken = (await state(page, "a")).baseline;
  expect(taken?.added, "Add new took them in").toBe(0);
  expect(taken?.rows).toBe((set?.rows ?? 0) + 2);
  expect(await host(page, "a", (api) => Object.isFrozen(api.state().baseline)), "the object stays frozen").toBe(true);
});

test("data-baseline on the chart's root changes when a baseline is set, added to and cleared through Gantt.commands", async ({ page }) => {
  const root = page.locator("#a");
  await expect(root).not.toHaveAttribute("data-baseline", /.*/);
  await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"));
  await host(page, "a", (api) => api.commands.setBaseline());
  const rows = (await state(page, "a")).taskCount;
  await expect(root).toHaveAttribute("data-baseline", `Baseline set at data date 5 Feb: ${rows} rows`);
  await expect(part(page, "a", "table-head").locator(".var-start")).toHaveCount(1);

  await host(page, "a", (api) => api.commands.insertRow());
  await host(page, "a", (api) => api.commands.addNewToBaseline());
  await expect(root).toHaveAttribute("data-baseline", `Baseline set at data date 5 Feb: ${rows + 1} rows`);

  await host(page, "a", (api) => api.commands.clearBaseline());
  await expect(root).not.toHaveAttribute("data-baseline", /.*/);
  await expect(part(page, "a", "table-head").locator(".var-start")).toHaveCount(0);
});

test("busy() is true during a cell edit and a bar drag, and the commands refuse then", async ({ page }) => {
  expect(await host(page, "a", (api) => api.busy())).toBe(false);

  // A cell edit: a double click on a task's name opens its editor.
  await part(page, "a", "table-rows").locator(".row .name").nth(3).dblclick();
  await expect(part(page, "a", "table-rows").locator("input")).toHaveCount(1);
  expect(await host(page, "a", (api) => api.busy())).toBe(true);
  expect(await host(page, "b", (api) => api.busy()), "B is not busy").toBe(false);
  expect(await host(page, "a", (api) => api.commands.insertRow())).toEqual({ ok: false, reason: "busy", message: BUSY });
  expect(await host(page, "a", (api) => api.commands.setBaseline())).toEqual({ ok: false, reason: "busy", message: BUSY });
  expect(await host(page, "a", (api) => api.commands.setDataDate(api.state().dataDate)), "the day shown is refused too").toEqual({ ok: false, reason: "busy", message: BUSY });
  expect(await host(page, "a", (api) => api.commands.collapseToLevel(1))).toBe(false);
  await page.keyboard.press("Escape");
  await expect(part(page, "a", "table-rows").locator("input")).toHaveCount(0);
  expect(await host(page, "a", (api) => api.busy())).toBe(false);

  // A bar drag: press on a leaf's bar and move past the threshold.
  const box = (await part(page, "a", "timeline").boundingBox())!;
  const rows = (await state(page, "a")).rowCount;
  let pressed = false;
  for (let row = 1; row < Math.min(rows, 8) && !pressed; row++) {
    const y = box.y + AXIS + row * ROW + ROW / 2;
    const startX = await page.evaluate((r) => {
      const app = window.charts.a.hook.app;
      const start = app.row_start(r);
      return start === undefined ? null : app.x_of(start);
    }, row);
    if (startX === null || startX < 0 || startX > box.width - 60) continue;
    for (let dx = 4; dx <= 40 && !pressed; dx += 4) {
      await page.mouse.move(box.x + startX + dx, y);
      await nextFrame(page, 2);
      const hover = await page.evaluate(() => window.charts.a.hook.status().hover);
      if (hover !== null && hover.summary !== true) {
        await page.mouse.down();
        for (let k = 1; k <= 6; k++) await page.mouse.move(box.x + startX + dx + 12 * k, y);
        await nextFrame(page, 3);
        pressed = (await page.evaluate(() => window.charts.a.hook.status().drag?.active)) === true;
        if (!pressed) await page.mouse.up();
      }
    }
  }
  expect(pressed, "a bar drag is on").toBe(true);
  expect(await host(page, "a", (api) => api.busy())).toBe(true);
  expect(await host(page, "a", (api) => api.commands.deleteRow())).toMatchObject({ ok: false, reason: "busy", message: BUSY });
  expect(await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"))).toEqual({ ok: false, reason: "busy", message: BUSY });
  expect(await host(page, "a", (api) => api.commands.expandAll())).toBe(false);
  await page.mouse.up();
  await nextFrame(page, 3);
  expect(await host(page, "a", (api) => api.busy())).toBe(false);
});

test("the chart's find and status keys reach the host as events, in the chart that has focus", async ({ page }) => {
  const heard = (key: Key): Promise<{ find: number; status: number }> =>
    page.evaluate((k) => ({ find: window.charts[k].heard.find, status: window.charts[k].heard.status }), key);
  await page.locator("#a").focus();
  await page.keyboard.press("Control+f");
  expect(await heard("a")).toEqual({ find: 1, status: 0 });
  // The chart's own find box hears the same event and takes the focus; give it back to the chart for the next key.
  await page.locator("#a").focus();
  await page.keyboard.press("s");
  expect(await heard("a")).toEqual({ find: 1, status: 1 });
  expect(await heard("b"), "B heard nothing").toEqual({ find: 0, status: 0 });
  await page.locator("#b").focus();
  await page.keyboard.press("s");
  expect(await heard("b")).toEqual({ find: 0, status: 1 });
});

test("every command does nothing and returns undefined after destroy", async ({ page }) => {
  const after = await page.evaluate(() => {
    const api = window.charts.b.api;
    api.destroy();
    const c = api.commands;
    const results = [c.find("x"), c.findNext(), c.findGo(0), c.collapseToLevel(1), c.expandAll(), c.insertRow(), c.deleteRow(), c.setDataDate(null), c.setBaseline(), c.clearBaseline(), c.addNewToBaseline()];
    return results.map((r) => r === undefined);
  });
  expect(after).toEqual(Array(11).fill(true));
  expect(await page.evaluate(() => { try { window.charts.b.api.busy(); return "returned"; } catch { return "threw"; } })).toBe("threw");
});

/** Undo once through `Gantt.commands`. */
const undo = (page: Page): Promise<boolean | undefined> => host(page, "a", (api) => api.commands.undo());

/** The first leaf row's task in A. */
const firstLeaf = (page: Page): Promise<number> =>
  page.evaluate(() => {
    const rows = JSON.parse(window.charts.a.hook.app.table_rows(0, 20)) as [number, string, string, number | null, boolean, number][];
    return rows.find((row) => row[5] === 0)![0];
  });

test("each edit command is one undo step, and undo puts the schedule back", async ({ page }) => {
  const start = await state(page, "a");
  expect(await undo(page), "a load leaves no history").toBe(false);

  expect((await host(page, "a", (api) => api.commands.insertRow()))?.ok).toBe(true);
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).taskCount, "undo removes the inserted task").toBe(start.taskCount);
  expect(await undo(page), "insertRow was one step").toBe(false);

  expect(await host(page, "a", (api, t) => api.commands.select(t), await firstLeaf(page))).toBe(true);
  expect((await host(page, "a", (api) => api.commands.deleteRow()))?.ok).toBe(true);
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).taskCount, "undo restores the deleted task").toBe(start.taskCount);
  expect(await undo(page), "deleteRow was one step").toBe(false);

  expect(await host(page, "a", (api) => api.commands.setDataDate("2024-02-05"))).toEqual({ ok: true });
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).dataDate).toBe(start.dataDate);
  expect(await undo(page), "setDataDate was one step").toBe(false);

  expect(await host(page, "a", (api) => api.commands.setBaseline())).toEqual({ ok: true });
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).baseline).toBeNull();
  expect(await undo(page), "setBaseline was one step").toBe(false);

  await host(page, "a", (api) => api.commands.setBaseline());
  expect(await host(page, "a", (api) => api.commands.clearBaseline())).toEqual({ ok: true });
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).baseline?.rows, "undo puts the cleared baseline back").toBe(start.taskCount);
  expect(await undo(page)).toBe(true);
  expect(await undo(page), "clearBaseline was one step").toBe(false);

  await host(page, "a", (api) => api.commands.setBaseline());
  await host(page, "a", (api) => api.commands.insertRow());
  expect(await host(page, "a", (api) => api.commands.addNewToBaseline())).toEqual({ ok: true });
  expect((await state(page, "a")).baseline?.rows).toBe(start.taskCount + 1);
  expect(await undo(page)).toBe(true);
  expect((await state(page, "a")).baseline?.rows, "addNewToBaseline was one step").toBe(start.taskCount);
  expect((await state(page, "a")).taskCount, "and left the inserted task").toBe(start.taskCount + 1);
});

test("setDataDate(state().dataDate) keeps an 08:00 data date, recomputes nothing and records no step", async ({ page }) => {
  const hook = (): Promise<{ dataDate: number | null; revision: number; stamp: number }> =>
    page.evaluate(() => {
      const s = window.charts.a.hook.status();
      return { dataDate: s.dataDate, revision: s.revision, stamp: s.stamp };
    });
  const day = (await state(page, "a")).dataDate;
  expect(day, "the ledger's data date is 2024-01-10 08:00").toBe("2024-01-10");
  const before = await hook();
  expect(await host(page, "a", (api, d) => api.commands.setDataDate(d), day)).toEqual({ ok: true });
  expect(await hook(), "the data date, the schedule and the stamp are as they were").toEqual(before);
  expect(await undo(page), "no undo step").toBe(false);
  await expect(part(page, "a", "unsaved-mark"), "not marked unsaved").toBeHidden();
});

test("the chrome's find box searches through the host's find, goes only to its own matches, and the host's count follows an edit", async ({ page }) => {
  const tasks = [];
  for (let k = 0; k < 2; k++) tasks.push(taskOf(await host(page, "a", (api) => api.commands.insertRow())));
  expect(await host(page, "a", (api) => api.commands.find("New task"))).toBe(2);

  // The find box (apps/chrome) searches for another task's name through `commands.find`, so the search is now the box's.
  const leaf = await firstLeaf(page);
  const name = await page.evaluate((t) => window.charts.a.hook.app.task_label(t), leaf);
  await part(page, "a", "find-input").fill(name);
  await nextFrame(page);
  const selected = async (): Promise<number | undefined> => (await state(page, "a")).selection?.task;
  expect(await selected(), "the box went to its first match").not.toBe(tasks[0]);
  expect(await host(page, "a", (api) => api.commands.findGo(0))).toBe(true);
  expect(await page.evaluate((t) => window.charts.a.hook.app.task_name(t!), await selected()), "findGo goes to the box's search").toContain(name);

  // The host searches again; Enter in the box runs the box's query again first, so it goes only to a box match.
  expect(await host(page, "a", (api) => api.commands.find("New task"))).toBe(2);
  await part(page, "a", "find-input").press("Enter");
  const boxPick = await selected();
  expect(await page.evaluate((t) => window.charts.a.hook.app.task_name(t!), boxPick), "Enter goes to a box match").toContain(name);
  expect(await host(page, "a", (api) => api.commands.find("New task"))).toBe(2);
  expect(await host(page, "a", (api) => api.commands.findNext())).toBe(0);
  expect(await selected(), "findNext goes to the host's first match").toBe(tasks[0]);

  // A structure edit changes the matches: the host's count follows it.
  expect(await host(page, "a", (api) => api.commands.deleteRow())).toEqual({ ok: true, task: tasks[0] });
  expect(await host(page, "a", (api) => api.commands.findNext()), "one match is left, and findNext goes to it").toBe(0);
  expect(await selected()).toBe(tasks[1]);
  expect(await host(page, "a", (api) => api.commands.findGo(1)), "the deleted match is gone").toBe(false);
});

test("findNext continues from the task it went to after an insert before it, with no skip and no repeat", async ({ page }) => {
  const tasks = [];
  for (let k = 0; k < 3; k++) tasks.push(taskOf(await host(page, "a", (api) => api.commands.insertRow())));
  expect(await host(page, "a", (api) => api.commands.find("New task"))).toBe(3);
  expect(await host(page, "a", (api) => api.commands.findGo(1))).toBe(true);
  // A match inserted before the one gone to moves it from index 1 to 2.
  expect(await host(page, "a", (api, t) => api.commands.select(t), tasks[0])).toBe(true);
  expect(await host(page, "a", (api) => api.commands.insertRow())).toMatchObject({ ok: true });
  expect(await host(page, "a", (api) => api.commands.findNext()), "the match after the task gone to").toBe(3);
  expect((await state(page, "a")).selection?.task).toBe(tasks[2]);
});

test("whatIf says which tasks a week's slip would move, and changes nothing", async ({ page }) => {
  // B is the demo (nothing started); A holds the ledger, whose first leaf has started.
  const leaf = (key: Key): Promise<number> => page.evaluate((k) => (JSON.parse(window.charts[k].hook.app.table_rows(0, 20)) as number[][]).find((row) => row[5] === 0)![0]!, key);
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.b.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const ask = (tasks: unknown, key: Key = "b"): Promise<WhatIfResult | undefined> => host(page, key, (api, t) => api.commands.whatIf(t as number[]), tasks);
  const before = { marks: await marks(), state: await state(page, "b") };
  const week = await ask([await leaf("b")]);
  if (week?.ok !== true) throw new Error(`a week's slip was refused: ${JSON.stringify(week)}`);
  const first = week.moved[0]!;
  expect(first.startNow > first.startWas && first.finishNow > first.finishWas, `${JSON.stringify(first)} moves later`).toBe(true);
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before.marks);
  expect(await state(page, "b"), "the view state is as it was").toEqual(before.state);
  expect(await host(page, "b", (api) => api.commands.undo()), "no undo step").toBe(false);
  expect(await ask([await leaf("b")]), "the same ask answers the same").toEqual(week);
  expect(await ask([await leaf("a")], "a"), "a started task").toMatchObject({ ok: false, reason: "hasActuals" });
  expect(await ask([1.5]), "not a task number").toMatchObject({ ok: false, reason: "unknownTask" });
  const hole = await host(page, "b", (api) => { const sparse = new Array<number>(2); sparse[1] = 5; return api.commands.whatIf(sparse); });
  expect(hole, "a hole is no task, never task 0").toMatchObject({ ok: false, reason: "unknownTask" });
});

test("whatIfLonger says which tasks days more on one task would move, and changes nothing", async ({ page }) => {
  const leaf = (key: Key): Promise<number> => page.evaluate((k) => (JSON.parse(window.charts[k].hook.app.table_rows(0, 20)) as number[][]).find((row) => row[5] === 0)![0]!, key);
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.b.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const ask = (task: unknown, days: unknown, key: Key = "b"): Promise<WhatIfResult | undefined> => host(page, key, (api, [t, d]) => api.commands.whatIfLonger(t as number, d as number), [task, days]);
  const before = await marks();
  const task = await leaf("b");
  const longer = await ask(task, 5);
  if (longer?.ok !== true) throw new Error(`days longer was refused: ${JSON.stringify(longer)}`);
  const first = longer.moved.find((m) => m.task === task)!;
  expect(first.startNow === first.startWas && first.finishNow > first.finishWas, `${JSON.stringify(first)} keeps its start and ends later`).toBe(true);
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before);
  expect(await host(page, "b", (api) => api.commands.undo()), "no undo step").toBe(false);
  expect(await ask(await leaf("a"), 5, "a"), "a started task").toMatchObject({ ok: false, reason: "hasActuals" });
  expect(await ask(1.5, 5), "not a task number").toMatchObject({ ok: false, reason: "unknownTask" });
  expect(await ask(task, 1.5), "not a whole number of days").toMatchObject({ ok: false, reason: "unknownTask" });
});

test("explain lists the chain behind a task as the selection's back path does, and changes nothing", async ({ page }) => {
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.b.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const ask = (task: unknown): Promise<ExplainResult | undefined> => host(page, "b", (api, t) => api.commands.explain(t as number), task);
  const last = await page.evaluate(() => window.charts.b.hook.app.test_finishing_task()!);
  const before = await marks();
  const why = await ask(last);
  if (why?.ok !== true) throw new Error(`explain was refused: ${JSON.stringify(why)}`);
  expect(why.chain.length, "the project's last task has tasks driving it").toBeGreaterThan(0);
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before);
  expect(await host(page, "b", (api) => api.commands.undo()), "no undo step").toBe(false);
  expect(await ask(last), "the same ask answers the same").toEqual(why);
  expect(await host(page, "b", (api, t) => api.commands.select(t), last)).toBe(true);
  expect(why.chain.length, "the selection's back path holds the same tasks").toBe((await state(page, "b")).selection?.backCount);
  expect(await ask(1.5), "not a task number").toMatchObject({ ok: false, reason: "unknownTask" });
  expect(await ask(1e9), "no such task").toMatchObject({ ok: false, reason: "unknownTask" });
});

test("health lists the tasks behind each count checks reports, and changes nothing", async ({ page }) => {
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.a.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const before = await marks();
  const answer = await host(page, "a", (api): HealthResult | undefined => api.commands.health());
  if (answer?.ok !== true) throw new Error(`health was refused: ${JSON.stringify(answer)}`);
  const report = (await host(page, "a", (api) => api.checks())) as unknown as Record<string, number | null>;
  const keys = ["missingLogic", "openStarts", "openEnds", "highFloat", "highDuration", "invalidDates", "missingResources", "missed", "hardConstraints", "negativeFloat"] as const;
  for (const key of keys) {
    const count = answer.counts[key];
    expect(count?.count ?? null, `${key}: the total is the report's`).toBe(report[key]);
    expect(count?.tasks.length ?? 0, `${key}: at most 50 tasks, all of them when fewer`).toBe(Math.min(count?.count ?? 0, 50));
  }
  const open = keys.map((key) => answer.counts[key]).find((count) => (count?.count ?? 0) > 0);
  expect(open, "the ledger has a count with tasks behind it").toBeDefined();
  expect(open?.tasks[0], "a task carries its number, activity ID and name").toMatchObject({ task: expect.any(Number), id: expect.any(String), name: expect.stringMatching(/\S/) });
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before);
  expect(await host(page, "a", (api) => api.commands.undo()), "no undo step").toBe(false);
});

test("propose holds changes apart from the schedule until accept applies them as one undo step, or reject drops them", async ({ page }) => {
  const task = await page.evaluate(() => (JSON.parse(window.charts.b.hook.app.table_rows(0, 20)) as number[][]).find((row) => row[5] === 0)![0]!);
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.b.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const lengthen = (): Promise<unknown> => host(page, "b", (api, t) => api.commands.propose({ kind: "duration", task: t, days: 9 }), task);
  const before = await marks();
  expect(await lengthen(), "held, with what it would move").toMatchObject({ ok: true });
  expect(await host(page, "b", (api, t) => api.commands.propose({ kind: "move", task: t, start: "soon" }), task), "not a date").toMatchObject({ ok: false, reason: "badDate" });
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before);
  expect(await host(page, "b", (api) => api.commands.undo()), "no undo step").toBe(false);
  expect(await host(page, "b", (api) => api.commands.acceptProposal())).toEqual({ ok: true, changes: 1 });
  expect(await host(page, "b", (api) => api.commands.undo()), "one undo step").toBe(true);
  expect(await host(page, "b", (api) => api.commands.undo()), "and only one").toBe(false);
  expect(await host(page, "b", (api) => api.commands.acceptProposal()), "none held").toMatchObject({ ok: false, reason: "noProposal" });
  await lengthen();
  expect(await host(page, "b", (api) => api.commands.rejectProposal())).toBe(true);
  expect(await host(page, "b", (api) => api.commands.rejectProposal()), "nothing left to drop").toBe(false);
});

test("acceptProposed applies one held change as one undo step and rejectProposed drops one, the others staying held", async ({ page }) => {
  const [a, b] = await page.evaluate(() => (JSON.parse(window.charts.b.hook.app.table_rows(0, 20)) as number[][]).filter((row) => row[5] === 0).map((row) => row[0]!));
  const one = (call: "acceptProposed" | "rejectProposed", kind: "move" | "duration", task: number): Promise<unknown> => host(page, "b", (api, k) => api.commands[k.call]({ kind: k.kind, task: k.task }), { call, kind, task });
  for (const task of [a!, b!]) await host(page, "b", (api, t) => api.commands.propose({ kind: "duration", task: t, days: 9 }), task);
  expect(await one("rejectProposed", "move", a!), "no move of it is held").toBe(false);
  expect(await one("acceptProposed", "duration", a!)).toEqual({ ok: true, changes: 1 });
  expect(await host(page, "b", (api) => api.commands.undo()), "one undo step").toBe(true);
  expect(await host(page, "b", (api) => api.commands.undo()), "and only one").toBe(false);
  expect(await one("acceptProposed", "duration", a!), "already accepted").toMatchObject({ ok: false, reason: "noProposal" });
  expect(await one("rejectProposed", "duration", b!), "the other is still held").toBe(true);
  expect(await host(page, "b", (api) => api.commands.acceptProposal()), "none left").toMatchObject({ ok: false, reason: "noProposal" });
});

test("criticalPath lists the critical tasks and the longest path, and changes nothing", async ({ page }) => {
  const marks = (): Promise<{ revision: number; stamp: number }> => page.evaluate(() => { const s = window.charts.a.hook.status(); return { revision: s.revision, stamp: s.stamp }; });
  const before = await marks();
  const ask = (): Promise<CriticalPathResult | undefined> => host(page, "a", (api) => api.commands.criticalPath());
  const answer = await ask();
  if (answer?.ok !== true) throw new Error(`criticalPath was refused: ${JSON.stringify(answer)}`);
  const { critical, longest } = answer;
  expect(critical.count, "the ledger has critical tasks").toBeGreaterThan(0);
  expect(critical.tasks.length, "at most 50 tasks, all of them when fewer").toBe(Math.min(critical.count, 50));
  expect(longest.count, "the longest path holds tasks, no more than the critical set").toBeGreaterThan(0);
  expect(longest.count).toBeLessThanOrEqual(critical.count);
  expect(critical.tasks[0], "a task carries its number, activity ID and name").toMatchObject({ task: expect.any(Number), id: expect.any(String), name: expect.stringMatching(/\S/) });
  expect(await ask(), "the same ask answers the same").toEqual(answer);
  expect(await marks(), "no edit: the revision and the stamp are as they were").toEqual(before);
  expect(await host(page, "a", (api) => api.commands.undo()), "no undo step").toBe(false);
});
