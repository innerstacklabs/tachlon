import { fileURLToPath } from "node:url";
import { createServer } from "vite";
import { expect, type Page, test } from "@playwright/test";
import type { MountOptions } from "../src/index";
import { openHeadless } from "../src/headless";
import { chromePath } from "./support/knobs";
// tl#1079 M4: a host mounts plain JSON, three tasks and a link, drives it through `Gantt.commands`.
const day = (id: number, name: string, start: string) => ({ id, name, start, duration: 3 });
const json: MountOptions = { tasks: [day(1, "Dig", "2026-11-02"), day(2, "Pour", "2026-11-05"), day(3, "Cure", "2026-11-10")], links: [{ from: 1, to: 2 }] };
const HOST = `<div id="g" class="gantt" style="width: 1000px; height: 400px"></div><script type="module">
  import { mount } from "/src/index.ts"; window.api = await mount(document.getElementById("g"), { ...${JSON.stringify(json)}, forceWebgl: true });</script>`;
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader"] } });
test("mount({ tasks, links }) shows three rows that scroll, zoom and select", async ({ page }) => {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const base = (await server.listen()).resolvedUrls!.local[0]!;
  const js = (expr: string): Promise<unknown> => page.evaluate(expr);
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.api !== undefined", undefined, { timeout: 180_000 });
  expect(await js(`[api.state().rowCount, api.state().taskCount]`)).toEqual([3, 3]);
  const [px, from] = (await js("[gantt.status().pxPerMinute, gantt.status().from, api.commands.zoomIn(), api.commands.select(2)]")) as number[];
  await expect.poll(() => js("[gantt.status().pxPerMinute > " + px + ", api.state().selection?.task, api.commands.scrollTo({ date: '2027-06-01' })]")).toEqual([true, 2, true]);
  await expect.poll(() => js("gantt.status().from")).not.toBe(from);
  await server.close();
});
const CHARTS = `${["a", "b", "c"].map((id) => `<div id="${id}" class="gantt" style="width: 500px; height: 200px"></div>`).join("")}<script type="module">
  import { mount } from "/src/index.ts"; window.json = ${JSON.stringify(json)};
  window.start = (id, options) => mount(document.getElementById(id), { ...options, forceWebgl: true }); window.ready = true;</script>`;
// Runs one script on a page that holds three empty chart roots, and gives back what it returns.
async function onCharts(page: Page, script: string): Promise<unknown> {
  const server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", server: { host: "127.0.0.1", port: 0 } });
  const base = (await server.listen()).resolvedUrls!.local[0]!;
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: CHARTS }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction("window.ready === true", undefined, { timeout: 180_000 });
  const result = await page.evaluate(`(async () => { ${script} })()`);
  await server.close();
  return result;
}
test("mount({ document }) draws the tasks of the document another chart's toDocument returned", async ({ page }) => {
  expect(await onCharts(page, `const a = await start("a", json); const b = await start("b", { document: a.toDocument() }); return [b.state().taskCount, JSON.stringify(b.snapshot().tasks) === JSON.stringify(a.snapshot().tasks)];`)).toEqual([3, true]);
});
test("mount with no schedule, tasks or document starts with no tasks and no error, and load({ document }) then draws", async ({ page }) => {
  expect(await onCharts(page, `const document = (await start("a", json)).toDocument(); const errors = []; const b = await start("b", {}); b.on("error", (e) => errors.push(e)); const empty = b.state().taskCount;
    const loaded = await b.load({ document }); await new Promise((done) => requestAnimationFrame(() => requestAnimationFrame(done))); return [empty, errors.length, loaded.ok, b.state().taskCount];`)).toEqual([0, 0, true, 3]);
});
test("mount with a schedule and a document draws the schedule", async ({ page }) => {
  expect(await onCharts(page, `const document = (await start("a", json)).toDocument(); const both = await start("b", { schedule: { demo: 12 }, document }); const demo = await start("c", { schedule: { demo: 12 } }); return [both.state().taskCount === demo.state().taskCount, both.state().taskCount !== 3];`)).toEqual([true, true]);
});
// decision 0018: `options` on mount reach `fromTasks`, so a started task's actual start is its late start under `startedLateStart: "project"`.
test("mount({ tasks, options }) shows a started task's actual start as its late start under startedLateStart project", async ({ page }) => {
  const late = (options: string) => `const tasks = [{ id: 1, name: "Dig", start: "2026-11-02", duration: 3, actualStart: "2026-11-02" }, { id: 2, name: "Pour", start: "2026-11-02", duration: 10 }]; await start("a", { tasks, ${options} }); return gantt.app.task_details(0).lateStart;`;
  const [p6, project] = [await onCharts(page, late("")), await onCharts(page, late('options: { startedLateStart: "project" }'))];
  expect(project, "the actual start is the late start").toMatch(/^2026-11-02/);
  expect(p6, "without the option the late start is the engine's own").not.toBe(project);
});
// gantt#2132: `mustFinishBy` on mount reaches `fromTasks`, so the late dates and float equal the headless results for the same data (the three-task case of gantt#2074); a date out of range rejects `mount`.
test("mount({ tasks, links, mustFinishBy }) shows the late dates and float the headless call gives", async ({ page }) => {
  const data = { tasks: ["a", "b", "c"].map((id) => ({ id, name: id, start: "2026-03-02", duration: 5 })), links: [{ from: "a", to: "b" }, { from: "b", to: "c" }], mustFinishBy: "2026-03-18T17:00" };
  const headless = openHeadless(data).results().map(({ lateStart, lateFinish, float }) => ({ lateStart, lateFinish, float }));
  expect(headless.map((t) => t.float), "the date is in force").toEqual([-2, -2, -2]);
  expect(await onCharts(page, `await start("a", ${JSON.stringify(data)}); return [0, 1, 2].map((i) => { const d = gantt.app.task_details(i); return { lateStart: d.lateStart.replace(" ", "T"), lateFinish: d.lateFinish.replace(" ", "T"), float: d.floatDays }; });`)).toEqual(headless);
  expect(await onCharts(page, `try { await start("a", { ...${JSON.stringify(data)}, mustFinishBy: "9999-12-31" }); } catch (e) { return [e.stage, e.message]; }`)).toEqual(["schedule", expect.stringMatching(/must_finish_by: \d+ is out of range/)]);
});
// tl#2081: `calendars` on mount reach `fromTasks`, so a holiday on a ten-hour calendar sets the same finish as the headless call (the default calendar would end it at 17:00 a day earlier).
test("mount({ tasks, calendars }) shows the finish the headless call gives for the same calendars", async ({ page }) => {
  const data = { tasks: [{ id: "a", name: "a", start: "2027-01-04", duration: 3, calendar: "ten" }], calendars: [{ key: "ten", workDays: [1, 2, 3, 4, 5], hoursPerDay: 10, exceptions: [{ date: "2027-01-05", working: false }] }] };
  expect(openHeadless(data).results()[0]!.finish).toBe("2027-01-07T18:00");
  expect(await onCharts(page, `await start("a", ${JSON.stringify(data)}); return gantt.app.task_details(0).lateFinish.replace(" ", "T");`)).toBe("2027-01-07T18:00");
});
