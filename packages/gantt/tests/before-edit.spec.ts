import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { BeforeEditChange, ChangeSet, Fields, Gantt, GanttEvents } from "../src/index";
import { chromePath } from "./support/knobs";
import { fromTasks } from "../src/adapters/plain";

declare global {
  interface Window { beChart: Gantt<Fields>; asked: BeforeEditChange[]; sets: ChangeSet[]; answers: unknown[]; errors: GanttEvents["error"][]; mountBe(code: string, options?: Record<string, unknown>): Promise<unknown> }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.asked = [], window.errors = [], window.sets = [];
  window.mountBe = async (code, options = {}) => {
    const answer = (0, eval)("(" + code + ")");
    window.beChart = await mount(document.getElementById("app"), { schedule: { demo: 30 }, forceWebgl: true, ...options, ...(options.recordSaves ? { save: async (set) => void window.sets.push(set) } : {}), beforeEdit: (change) => (window.asked.push(change), answer(change)) });
    window.beChart.on("error", (error) => window.errors.push(error));
  };
  window.ready = true;
</script></body></html>`;
let server: ViteDevServer;
let base = "";
test.beforeAll(async () => {
  server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0, strictPort: false } });
  await server.listen();
  base = server.resolvedUrls!.local[0]!;
});
test.afterAll(async () => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });

async function mountHost(page: Page, code: string, options: Record<string, unknown> = {}): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate(([c, o]) => window.mountBe(c as string, o as Record<string, unknown>).then(() => undefined), [code, options]);
}
const doc = (page: Page): Promise<string> => page.evaluate(() => JSON.stringify(window.beChart.toDocument()));
const asked = (page: Page, kind: string): Promise<BeforeEditChange[]> => page.evaluate((k) => window.asked.filter((change) => change.kind === k), kind);
const alert = (page: Page) => page.locator("#table-rows .cell-error");
async function typeDuration(page: Page): Promise<void> {
  await page.locator("#table-rows .row").nth(6).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("17");
  await page.keyboard.press("Enter");
}

test("a function that refuses a typed duration shows its reason and leaves the schedule as it was, and one that allows lets it through", async ({ page }) => {
  await mountHost(page, `(change) => (change.kind === "setDuration" ? "locked by the planner" : undefined)`);
  const before = await doc(page);
  await typeDuration(page);
  await expect(alert(page)).toHaveText("locked by the planner");
  expect(await doc(page), "the schedule is as it was").toBe(before);
  expect(await asked(page, "setDuration")).toMatchObject([{ source: "typed", changes: [{ record: "task", op: "update", set: { duration: 17 } }] }]);
  await mountHost(page, `() => true`);
  await typeDuration(page);
  expect(await doc(page), "an allowed edit applies").not.toBe(before);
});

async function dragBar(page: Page): Promise<void> {
  const bar = await page.evaluate(() => {
    const { x, y, width, height } = (window.gantt!.app.test_bar_rects() as Record<string, number>[])[2]!;
    const box = document.getElementById("timeline")!.getBoundingClientRect();
    return { x: box.x + (x! + width! / 2) / devicePixelRatio, y: box.y + (y! + height! / 2) / devicePixelRatio };
  });
  await page.mouse.move(bar.x, bar.y);
  await page.mouse.down();
  await page.mouse.move(bar.x + 60, bar.y, { steps: 6 });
}

test("a drag asks once, when it starts, and not again while it moves or when it ends", async ({ page }) => {
  await mountHost(page, `() => undefined`);
  await dragBar(page);
  expect((await asked(page, "drag")).length, "one call while the bar moves").toBe(1);
  await page.mouse.up();
  expect(await asked(page, "drag"), "still one call after the drag ends").toMatchObject([{ source: "drag", tasks: [expect.any(Number)] }]);
});

test("a function that throws, answers with a promise or calls the chart refuses the edit with the failed sentence and fires one error event", async ({ page }) => {
  for (const code of [`() => { throw new Error("boom"); }`, `async () => undefined`, `() => { window.beChart.toDocument(); }`]) {
    await mountHost(page, code);
    const [before, rows] = [await doc(page), await page.evaluate(() => window.beChart.state().rowCount)];
    await typeDuration(page);
    await expect(alert(page), code).toHaveText(/function failed/);
    expect(await doc(page), `${code}: the schedule is as it was`).toBe(before);
    expect(await page.evaluate(() => window.beChart.state().rowCount), `${code}: no row was added`).toBe(rows);
    expect(await page.evaluate(() => window.errors.map(({ source, fatal }) => `${source} ${fatal}`)), code).toEqual(["beforeEdit false"]);
  }
});

test("a command refused by a function that throws or answers with a promise answers beforeEditFailed", async ({ page }) => {
  for (const code of [`() => { throw new Error("boom"); }`, `async () => undefined`]) {
    await mountHost(page, code);
    expect(await page.evaluate(() => window.beChart.commands.insertRow()), code).toMatchObject({ ok: false, reason: "beforeEditFailed", message: expect.stringMatching(/function failed/) });
  }
});

test("a command called from inside the function returns busy and does nothing, so the edit goes on", async ({ page }) => {
  await mountHost(page, `() => { window.answers = [window.beChart.commands.insertRow()]; }`);
  const rows = await page.evaluate(() => window.beChart.state().rowCount);
  await typeDuration(page);
  await expect(page.locator("#table-rows .row").nth(6).locator(".dur")).toHaveText(/17/);
  expect(await page.evaluate(() => window.answers), "the command answers busy").toMatchObject([{ ok: false, reason: "busy" }]);
  expect(await page.evaluate(() => [window.beChart.state().rowCount, window.errors.length]), "no row was added and no error fired").toEqual([rows, 0]);
});

test("a command called from inside the function on a drag throws like any call, so the drag is refused with one error event", async ({ page }) => {
  await mountHost(page, `() => { window.beChart.commands.insertRow(); }`);
  const [before, rows] = [await doc(page), await page.evaluate(() => window.beChart.state().rowCount)];
  await dragBar(page);
  await page.mouse.up();
  expect(await page.evaluate(() => window.errors.map(({ source, fatal }) => `${source} ${fatal}`))).toEqual(["beforeEdit false"]);
  expect(await doc(page), "the schedule is as it was").toBe(before);
  expect(await page.evaluate(() => window.beChart.state().rowCount), "no row was added").toBe(rows);
});

test("the function is still asked after the switch to WebGL2", async ({ page }) => {
  await mountHost(page, `() => "no"`, { forceWebgl: false });
  test.skip(await page.evaluate(() => window.beChart.backend() !== "WebGPU"), "this browser has no WebGPU to lose");
  await page.evaluate(() => window.gantt!.failNextFrame(false));
  await page.waitForFunction(() => { try { return window.beChart.backend() === "WebGL2"; } catch { return false; } }, undefined, { timeout: 60_000 });
  await typeDuration(page);
  await expect(alert(page)).toHaveText("no");
});

test("a chart mounted with hours and dateForm day tells the edit event and the before-edit call in them", async ({ page }) => {
  const tasks = [{ id: "a", key: "k-a", name: "Dig", start: "2026-03-02", duration: 2 }, { id: "b", key: "k-b", name: "Pour", start: "2026-03-02", duration: 3 }];
  await mountHost(page, `() => undefined`, { schedule: undefined, tasks, durationUnit: "hours", dateForm: "day", recordSaves: true });
  const heard = () => page.evaluate(() => (window as unknown as { heard: unknown[] }).heard);
  await page.evaluate(() => { const w = window as unknown as { heard: unknown[] }; w.heard = []; window.beChart.on("edit", (edit) => w.heard.push(edit.commits[0]!.changes[0])); });
  await page.locator("#table-rows .row").nth(0).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("17");
  await page.keyboard.press("Enter");
  await page.locator("#table-rows .row").nth(1).locator(".start").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("2026-03-04");
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await heard()).length).toBe(2);
  const duration = { record: "task", key: "k-a", set: { duration: 136 }, was: { duration: 2 } };
  const start = { record: "task", key: "k-b", set: { constraint: { type: "snet", date: "2026-03-04" } } };
  expect((await asked(page, "setDuration"))[0]!.changes, "17 days of an 8-hour calendar is 136 hours; the 2 the host gave comes back as 2").toMatchObject([duration]);
  expect((await asked(page, "move"))[0]!.changes).toMatchObject([start]);
  expect(await heard()).toMatchObject([duration, start]);
  await expect.poll(() => page.evaluate(() => window.sets), "a save for each commit, in hours and days too, with no stamp").toMatchObject([{ changes: [duration] }, { changes: [start] }]);
  expect(await page.evaluate(() => JSON.stringify(window.sets))).not.toContain("dayMinutes");
});

test("after a load, a before-edit call and the edit event measure a keyless row with the loaded schedule's day", async ({ page }) => {
  await mountHost(page, `() => undefined`, { schedule: undefined, tasks: [{ id: "a", key: "k-a", name: "Dig", start: "2026-03-02", duration: 2 }], durationUnit: "hours" });
  const rows = [{ id: "a", name: "Dig", start: "2026-03-02", duration: 2 }, { id: "b", name: "Pour", start: "2026-03-02", duration: 2, calendar: "short" }];
  const calendars = [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, { key: "short", workDays: [1, 2, 3, 4, 5], hoursPerDay: 7.5 }];
  const { document } = fromTasks({ tasks: rows, calendars, durationUnit: "hours" });
  await page.evaluate(() => { const w = window as unknown as { opened: number; heard: unknown[] }; [w.opened, w.heard] = [0, []]; window.beChart.on("open", () => (w.opened += 1)); window.beChart.on("edit", (edit) => w.heard.push(edit.commits[0]!.changes[0])); });
  await page.evaluate((loaded) => window.beChart.load({ document: loaded }), document);
  await page.waitForFunction(() => (window as unknown as { opened: number }).opened === 1);
  await page.locator("#table-rows .row").nth(1).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("2");
  await page.keyboard.press("Enter");
  await expect.poll(() => page.evaluate(() => (window as unknown as { heard: unknown[] }).heard.length)).toBe(1);
  const duration = { record: "task", key: null, set: { duration: 15 }, was: { duration: 2 } };
  expect((await asked(page, "setDuration"))[0]!.changes, "2 days of a 7.5-hour calendar, not of the schedule mounted first").toMatchObject([duration]);
  expect(await page.evaluate(() => (window as unknown as { heard: unknown[] }).heard)).toMatchObject([duration]);
});

test("a typed predecessor with a lag is measured by its successor's calendar in the before-edit call, as in the edit event", async ({ page }) => {
  const calendars = [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, { key: "short", workDays: [1, 2, 3, 4, 5], hoursPerDay: 7.5 }];
  const tasks = [{ id: "1", key: "k1", name: "Dig", start: "2026-03-02", duration: 2 }, { id: "2", key: "k2", name: "Pour", start: "2026-03-02", duration: 2, calendar: "short" }];
  await mountHost(page, `() => undefined`, { schedule: undefined, tasks, calendars, durationUnit: "hours" });
  await page.evaluate(() => { const w = window as unknown as { heard: { record: string }[][] }; w.heard = []; window.beChart.on("edit", (edit) => w.heard.push(edit.commits[0]!.changes)); });
  await page.evaluate(() => window.gantt!.app.set_predecessors_text(1, "1FS+2d"));
  await expect.poll(() => page.evaluate(() => (window as unknown as { heard: unknown[] }).heard.length)).toBe(1);
  const made = { record: "link", op: "create", set: { lag: 15 } };
  expect((await asked(page, "group"))[0]!.changes.find((c) => c.record === "link"), "2 days of the 7.5-hour successor, not of the first task's 8").toMatchObject(made);
  expect((await page.evaluate(() => (window as unknown as { heard: { record: string }[][] }).heard))[0]!.find((c) => c.record === "link")).toMatchObject(made);
});

test("a lag updated at a link made since the last commit, in the same tick, is measured by its own day in the before-edit call and the edit event", async ({ page }) => {
  const calendars = [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, { key: "short", workDays: [1, 2, 3, 4, 5], hoursPerDay: 7.5 }];
  const tasks = [{ id: "1", key: "k1", name: "Dig", start: "2026-03-02", duration: 2 }, { id: "2", key: "k2", name: "Pour", start: "2026-03-02", duration: 2, calendar: "short" }];
  await mountHost(page, `() => undefined`, { schedule: undefined, tasks, calendars, durationUnit: "hours" });
  await page.evaluate(() => { const w = window as unknown as { heard: { record: string }[][] }; w.heard = []; window.beChart.on("edit", (edit) => w.heard.push(...edit.commits.map((commit) => commit.changes))); });
  await page.evaluate(() => { window.gantt!.app.set_predecessors_text(1, "1FS+2d"); window.gantt!.app.set_predecessors_text(1, "1FS+3d"); });
  await expect.poll(() => page.evaluate(() => (window as unknown as { heard: unknown[] }).heard.length)).toBe(2);
  const later = { record: "link", op: "update", set: { lag: 22.5 }, was: { lag: 15 } };
  const linkOf = (changes: readonly { record: string }[]) => changes.find((c) => c.record === "link");
  expect(linkOf((await page.evaluate(() => window.asked)).at(-1)!.changes), "3 days of the 7.5-hour successor, though the link is not in an edit event yet").toMatchObject(later);
  expect(linkOf((await page.evaluate(() => (window as unknown as { heard: { record: string }[][] }).heard))[1]!)).toMatchObject(later);
});

test("a chart mounted with a field map takes the host's names in its rows, update and setData, and tells them in the edit event, the before-edit call and the save", async ({ page }) => {
  const tasks = [{ id: "a", title: "Dig", plannedStart: "2026-03-02", days: 2 }, { id: "b", title: "Pour", plannedStart: "2026-03-02", days: 3 }];
  await mountHost(page, `() => undefined`, { schedule: undefined, tasks, fields: { tasks: { name: "title", start: "plannedStart", duration: "days", key: "id" } }, recordSaves: true });
  const heard = () => page.evaluate(() => (window as unknown as { heard: unknown[] }).heard);
  await page.evaluate(() => { const w = window as unknown as { heard: unknown[] }; w.heard = []; window.beChart.on("edit", (edit) => w.heard.push(edit.commits[0]!.changes[0])); });
  await page.locator("#table-rows .row").nth(0).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("17");
  await page.keyboard.press("Enter");
  await expect.poll(async () => (await heard()).length).toBe(1);
  const change = { record: "task", key: "a", set: { days: 17 }, was: { days: 2 } };
  expect((await asked(page, "setDuration"))[0]!.changes).toMatchObject([change]);
  expect(await heard()).toMatchObject([change]);
  await expect.poll(() => page.evaluate(() => window.sets)).toMatchObject([{ changes: [change] }]);
  expect(await page.evaluate(() => window.beChart.update([{ id: "b", title: "Pour 2", days: 4 }]))).toEqual({ ok: true, updated: 1 });
  expect(await page.evaluate(() => window.beChart.setData({ tasks: [{ id: "a", title: "Dig 2", plannedStart: "2026-03-02", days: 17 }, { id: "b", title: "Pour 2", plannedStart: "2026-03-02", days: 4 }] }))).toEqual({ ok: true, updated: 2 });
  expect(await page.evaluate(() => window.beChart.snapshot().tasks.map(({ name, duration }) => [name, duration])), "snapshot keeps the chart's names").toEqual([["Dig 2", 17], ["Pour 2", 4]]);
});
