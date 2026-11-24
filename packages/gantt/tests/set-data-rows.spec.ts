import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// gantt#2087: `setData` adds the tasks its rows hold and the chart does not, in place: the selection, the scroll and the undo list stay, and the add is not an edit.
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

const tasks = Array.from({ length: 20 }, (_, i) => ({ id: `t${i}`, key: `k${i}`, name: `Task ${i}`, start: "2026-03-02", duration: 2 }));
const added = [
  { id: "n1", key: "n1", name: "Added one", start: "2026-03-09", duration: 3, parent: "t4" },
  { id: "n2", key: "n2", name: "Added two", start: "2026-03-10", duration: 1 },
  { id: "n3", key: "n3", name: "Added three", start: "2026-03-11", duration: 1 },
];
/** The rows with the three added ones: `n1` under `t4`, `n3` at the top level after `t4` (a sibling that has a child), `n2` at the top level after `t9`. */
const rows = (second: object = {}) => [...tasks.slice(0, 5), added[2]!, ...tasks.slice(5, 10), { ...added[1], ...second }, ...tasks.slice(10), added[0]!];

const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
const saved = (page: Page) => page.evaluate(() => (window.upChart.toDocument() as { tasks: { key?: string; name: string; duration: number; parent: number | null }[] }).tasks);
const table = (page: Page) => page.evaluate(() => ({ rows: window.gantt!.status().rowCount, height: document.getElementById("table-body")!.scrollHeight, top: document.getElementById("table-body")!.scrollTop, selected: window.upChart.state().selection?.task }));

async function open(page: Page): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((o) => window.mountUp(o), { tasks });
  await page.locator('#table-rows .row[data-row="2"] .name').dblclick();
  await page.locator("#table-rows input.cell-edit").fill("Renamed");
  await page.keyboard.press("Enter");
  await page.waitForFunction(() => window.edits.length > 0);
  await page.evaluate(() => { window.edits.length = 0; document.getElementById("table-body")!.scrollTop = 1000; });
  await frames(page);
  await page.locator("#table-rows .row").nth(8).click({ position: { x: 4, y: 4 } });
  await frames(page);
}

test("setData adds the rows its data holds under their parents and after their previous sibling: selection, scroll and the earlier rename's undo hold, the table follows, and it is no edit", async ({ page }) => {
  await open(page);
  const before = await table(page);
  expect(before.top, "the table is scrolled").toBeGreaterThan(0);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows())).toEqual({ ok: true, updated: 3 });
  await frames(page);
  const after = await table(page);
  expect([after.rows, after.height, after.top, after.selected], "the table has the three rows more, and the scroll and the selection are as they were").toEqual([before.rows + 3, before.height + 72, before.top, before.selected]);
  const now = await saved(page);
  expect(now.map((task) => task.key).filter((key) => key === "n1" || key === "n2" || key === "n3" || key === "k4" || key === "k5" || key === "k9" || key === "k10"), "n1 first under k4, n3 after k4 and its child, n2 after k9").toEqual(["k4", "n1", "n3", "k5", "k9", "n2", "k10"]);
  expect(now.find((task) => task.key === "n1"), "the values and the parent as given").toMatchObject({ name: "Added one", duration: 1440, parent: now.findIndex((task) => task.key === "k4") });
  expect(now.find((task) => task.key === "n2"), "the values as given, at the top level").toMatchObject({ name: "Added two", duration: 480, parent: null });
  expect(await page.evaluate(() => window.edits), "an add is not an edit").toEqual([]);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows()), "the same rows again").toEqual({ ok: true, updated: 0 });
  expect(await page.evaluate(() => window.upChart.commands.undo())).toBe(true);
  const undone = await saved(page);
  expect([undone.find((task) => task.key === "k2")!.name, undone.length], "the one Undo is the person's rename; the added rows stay").toEqual(["Task 2", 23]);
});

test("a bad record on the second add keeps the first and corrected rows add the rest; a key the chart already holds refuses the whole call", async ({ page }) => {
  await open(page);
  const first = await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows().map((task) => (task.key === "n1" ? { ...task, progress: 150 } : task)));
  expect(first, "the first add refused, nothing set before it: the message says nothing changed").toMatchObject({ ok: false, key: "n1", message: expect.stringMatching(/^Nothing changed: [^]*$/) });
  expect((first as { message: string }).message).not.toContain("in the chart");
  const answer = await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows({ progress: 150 }));
  expect(answer).toMatchObject({ ok: false, reason: "badPercent", key: "n2", message: expect.stringContaining("Partly applied") });
  expect((await saved(page)).map((task) => task.key).filter((key) => key?.startsWith("n")), "the adds before it stay").toEqual(["n1", "n3"]);
  expect(await page.evaluate((next) => window.upChart.setData({ tasks: next }), rows({ progress: 50 })), "the rest is added").toEqual({ ok: true, updated: 1 });
  expect((await saved(page)).map((task) => task.key).filter((key) => key?.startsWith("n"))).toEqual(["n1", "n3", "n2"]);
  await page.evaluate(() => window.gantt!.app.add_task(JSON.stringify({ key: "held", name: "Held" }), null, null, false));
  const next = [...rows({ progress: 50 }), { id: "m1", key: "m1", name: "New" }, { id: "held", key: "held", name: "Held" }];
  expect(await page.evaluate((all) => window.upChart.setData({ tasks: all }), next), "a row the chart holds is not an add").toMatchObject({ ok: false, reason: "rowsChanged", message: expect.stringContaining("Nothing changed") });
  expect((await saved(page)).some((task) => task.key === "m1"), "m1 was not added either").toBe(false);
});
