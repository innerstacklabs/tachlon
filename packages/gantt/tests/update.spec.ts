import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// gantt#2087: a host hands the chart new values for many tasks by key, in place: the selection, the scroll and the undo list stay as they were.
declare global {
  interface Window { upChart: Gantt; edits: unknown[]; mountUp(options: Record<string, unknown>): Promise<void> }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.edits = [];
  window.mountUp = async (options) => {
    window.upChart = await mount(document.getElementById("app"), { forceWebgl: true, ...options });
    window.upChart.on("edit", (change) => window.edits.push(change));
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

const tasks = (n: number, extra: object = {}) => Array.from({ length: n }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2, ...extra }));
async function mountHost(page: Page, options: Record<string, unknown>): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((o) => window.mountUp(o), options);
}
const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
const saved = (page: Page) => page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; name: string; duration: number }[] }).tasks);
const minutes = async (page: Page, keys: string[]) => { const all = await saved(page); return keys.map((key) => all.find((t) => t.key === key)!.duration); };
const keys = (from: number, to: number) => Array.from({ length: to - from }, (_, i) => `k${from + i}`);
const scrollTop = (page: Page) => page.evaluate(() => document.querySelector("#table-body")!.scrollTop);

test("update sets 20 tasks in place: selection, scroll and undo hold, no edit fires, the table shows the new durations", async ({ page }) => {
  await mountHost(page, { tasks: tasks(40) });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await page.evaluate(() => { window.edits.length = 0; });
  await expect.poll(async () => {
    await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 25 * document.querySelector("#table-rows .row")!.getBoundingClientRect().height; });
    await frames(page);
    return page.locator('#table-rows .row[data-row="30"]').count();
  }).toBe(1);
  await page.locator('#table-rows .row[data-row="30"]').click({ position: { x: 4, y: 4 } });
  await frames(page);
  const before = { selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) };
  expect(before.selected, "a task is selected").toBeDefined();
  expect(before.top, "the list is scrolled").toBeGreaterThan(0);
  const answer = await page.evaluate((ks) => window.upChart.update(ks.map((key, i) => ({ key, duration: 5, ...(i === 0 ? { snet: "2026-03-09" } : {}) }))), keys(0, 20));
  await frames(page);
  expect(answer).toEqual({ ok: true, updated: 20 });
  expect(await minutes(page, keys(0, 20)), "the 20 new durations (5 days of 480 minutes)").toEqual(Array(20).fill(2400));
  expect(await minutes(page, keys(20, 40)), "the other tasks keep theirs").toEqual(Array(20).fill(960));
  expect({ selected: await page.evaluate(() => window.upChart.state().selection?.task), top: await scrollTop(page) }, "selection and scroll are as read before the call").toEqual(before);
  expect(await page.evaluate(() => window.edits), "an update is not an edit").toEqual([]);
  await page.evaluate(() => { document.querySelector("#table-body")!.scrollTop = 0; });
  await frames(page);
  expect(await page.locator("#table-rows .row .dur").evaluateAll((cells) => cells.slice(0, 10).map((c) => c.textContent)), "the table shows the new durations").toEqual(Array(10).fill("5d"));
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  expect((await saved(page))[2]!.name, "undo undoes the rename").toBe("Task 2");
  expect(await minutes(page, keys(0, 20)), "and leaves the 20 durations").toEqual(Array(20).fill(2400));
});

test("a refused update changes no cell: an unknown key answers with its record and key, a value that is not an array is badRecords", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const before = JSON.stringify(await saved(page));
  const unknown = await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }, { key: "nope", duration: 1 }]));
  expect(unknown).toMatchObject({ ok: false, reason: "unknownKey", record: 1, key: "nope" });
  expect(await page.evaluate(() => window.upChart.update("k1" as never))).toMatchObject({ ok: false, reason: "badRecords" });
  expect(JSON.stringify(await saved(page)), "nothing changed, k1 included").toBe(before);
});

test("a call while a cell is being edited answers busy and changes nothing", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const before = JSON.stringify(await saved(page));
  await page.locator('#table-rows .row[data-row="1"] .dur').dblclick();
  await expect(page.locator("#table-rows input.cell-edit")).toBeVisible();
  expect(await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }]))).toMatchObject({ ok: false, reason: "busy" });
  expect(JSON.stringify(await saved(page))).toBe(before);
});

test("a duration is counted in the unit the chart mounted with: hours, minutes, and days of the task's own calendar", async ({ page }) => {
  const set = (options: Record<string, unknown>, duration: number, key = "k0") => mountHost(page, options).then(() => page.evaluate(([k, d]) => window.upChart.update([{ key: k as string, duration: d as number }]), [key, duration])).then(() => minutes(page, [key]));
  expect(await set({ tasks: tasks(2), durationUnit: "hours" }, 12)).toEqual([720]);
  expect(await set({ tasks: tasks(2), durationUnit: "minutes" }, 89.6)).toEqual([90]);
  const ten = { key: "ten", workDays: [1, 2, 3, 4, 5], hoursPerDay: 10 };
  const calendars = [{ key: "std", workDays: [1, 2, 3, 4, 5], hoursPerDay: 8 }, ten];
  expect(await set({ tasks: [{ ...tasks(1)[0]!, calendar: "ten" }], calendars }, 2)).toEqual([1200]);
  expect(await set({ tasks: tasks(1), calendars }, 2), "a task on the first calendar counts its 8-hour day").toEqual([960]);
  const document = await page.evaluate(() => window.upChart.toDocument());
  expect(await set({ document, durationUnit: "day" }, 3), "a unit that is not one of the three counts days, as the reader does").toEqual([1440]);
});

const shown = (page: Page) => page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-task] > span")].map((c) => ({ text: c.textContent, marked: c.dataset.changed === "true", motion: getComputedStyle(c).animationName })));

test("an update highlights the cells in view whose shown value changed and no others, and the mark goes after its timer", async ({ page }) => {
  await mountHost(page, { tasks: tasks(40) });
  const before = await shown(page);
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 5 }))), keys(0, 20));
  await expect(page.locator("#table-rows [data-changed]").first()).toBeAttached();
  const after = await shown(page);
  expect(after.map((c) => c.marked), "a cell is marked when its text differs from the one shown before").toEqual(after.map((c, i) => c.text !== before[i]!.text));
  expect(after.filter((c) => c.marked).length, "some changed").toBeGreaterThan(0);
  expect(after.filter((c) => !c.marked).length, "some did not").toBeGreaterThan(0);
  expect(after.find((c) => c.marked)!.motion, "with motion the mark fades").toBe("gantt-changed");
  await expect(page.locator("#table-rows [data-changed]")).toHaveCount(0, { timeout: 4000 });
});

test("an update that sends the values already shown marks nothing, and with reduced motion the mark has no animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await mountHost(page, { tasks: tasks(40) });
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 2 }))), keys(0, 20));
  await frames(page);
  expect((await shown(page)).filter((c) => c.marked), "the same values change no cell").toEqual([]);
  await page.evaluate((ks) => window.upChart.update(ks.map((key) => ({ key, duration: 5 }))), keys(0, 20));
  await expect(page.locator("#table-rows [data-changed]").first()).toBeAttached();
  const marked = (await shown(page)).filter((c) => c.marked).map((c) => c.motion);
  expect([marked.length > 0, marked.every((a) => a === "none")], "marked, with no animation").toEqual([true, true]);
});

test("a pending key the update wrote is no longer pending, and a refused update marks and clears nothing", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  const pending = () => page.locator("#table-rows .row[data-pending]").evaluateAll((rows) => rows.map((r) => (r as HTMLElement).dataset.row));
  await page.evaluate(() => window.upChart.setPending(["k1", "k2"]));
  await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }, { key: "nope", duration: 1 }]));
  await frames(page);
  expect(await pending(), "a refused update leaves both pending").toEqual(["1", "2"]);
  expect(await shown(page).then((cells) => cells.filter((c) => c.marked)), "and marks nothing").toEqual([]);
  await page.evaluate(() => window.upChart.update([{ key: "k1", duration: 9 }]));
  await frames(page);
  expect(await pending(), "only the task written leaves").toEqual(["2"]);
});

test("an update followed by a load before the next frame highlights nothing in the new schedule", async ({ page }) => {
  await mountHost(page, { tasks: tasks(10) });
  await page.evaluate(async (ks) => {
    const document = window.upChart.toDocument() as { tasks: { duration: number }[] };
    window.upChart.update(ks.map((key) => ({ key, duration: 5 })));
    await window.upChart.load({ document: { ...document, tasks: document.tasks.map((t) => ({ ...t, duration: 9 })) } });
  }, keys(0, 10));
  await frames(page);
  expect(await page.locator("#table-rows [data-changed]").count(), "the new schedule's cells are not marked").toBe(0);
});
