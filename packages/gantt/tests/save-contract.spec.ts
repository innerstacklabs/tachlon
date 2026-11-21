import { createServer, type ViteDevServer } from "vite";
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import { chromePath } from "./support/knobs";

const MODULE = `import { mount } from "/src/index.ts";
import { liveSave } from "/src/save-queue.ts";
const log = [], errs = [], seen = new Set(); let key = 0, uuid = 0, held;
Object.defineProperty(crypto, "randomUUID", { value: () => "00000000-0000-4000-8000-" + String(++uuid).padStart(12, "0") });
const names = ["createTask", "updateTask", "deleteTask", "createLink", "updateLink", "deleteLink", "updateSchedule"];
const handlers = Object.fromEntries(names.map(name => [name, async (...args) => { const context = args.pop(), set = context.changeSet; if (!seen.has(set.seq)) { seen.add(set.seq); log.push("set seq=" + set.seq + " source=" + set.source + " n=" + set.changes.length); } log.push(name + " " + JSON.stringify([...args, context.was ?? null, context.once])); }]));
const tasks = ["a", "b", "c"].map((key, i) => ({ key, id: String(i + 1), name: "Task " + i, start: "2026-11-02", duration: 2 }));
Object.assign(window, { log, errs, installMissing: () => { held = Object.fromEntries(names.filter(name => name !== "createLink").map(name => [name, handlers[name]])); }, start: async (live = false) => { window.chart = await mount(document.getElementById("app"), { tasks, forceWebgl: true, newKey: () => "k" + ++key, beforeEdit: arg => { if (!live) log.push("before " + JSON.stringify(arg)); }, save: live ? liveSave(() => held) : handlers }); window.chart.on("error", error => errs.push(error)); } });
window.ready = true;`;
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">import "/tests/save-contract-host.ts";</script></body></html>`;
const host = { name: "save-contract-host", resolveId: (id: string) => id === "/tests/save-contract-host.ts" ? id : null, load: (id: string) => id === "/tests/save-contract-host.ts" ? MODULE : null };
let server: ViteDevServer;
test.beforeAll(async () => { server = await createServer({ root: fileURLToPath(new URL("..", import.meta.url)), logLevel: "silent", plugins: [host], server: { host: "127.0.0.1", port: 0 } }); await server.listen(); });
test.afterAll(() => server.close());
test.use({ launchOptions: { executablePath: chromePath(), args: ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"] } });
const row = (page: Page, n: number) => page.locator(`#table-rows .row[data-row="${n}"]`);
async function open(page: Page, live = false): Promise<void> {
  const base = server.resolvedUrls!.local[0]!; await page.emulateMedia({ reducedMotion: "reduce" });
  await page.route(`${base}host.html`, route => route.fulfill({ contentType: "text/html", body: HOST })); await page.goto(`${base}host.html`);
  await page.waitForFunction("window.ready === true", undefined, { timeout: 120_000 }); await page.evaluate(`start(${live})`); await row(page, 2).waitFor({ timeout: 60_000 });
}
const frames = (page: Page) => page.evaluate(() => new Promise<void>(resolve => requestAnimationFrame(() => requestAnimationFrame(() => resolve()))));
const edit = async (page: Page, n: number, value: string): Promise<void> => { await row(page, n).locator(".dur").dblclick(); await page.locator("#table-rows input.cell-edit").fill(value); await page.keyboard.press("Enter"); };
async function step(page: Page, action: () => Promise<unknown>): Promise<void> {
  const before = await page.evaluate<number>("log.length"); await action(); await expect.poll(() => page.evaluate<number>("log.length")).toBeGreaterThan(before);
  let previous = -1, stable = 0; await expect.poll(async () => { const n = await page.evaluate<number>("log.length"); stable = n === previous ? stable + 1 : 0; previous = n; return stable; }, { intervals: [100] }).toBeGreaterThanOrEqual(2);
  await expect.poll(() => page.evaluate("chart.state().unsavedCommits")).toBe(0);
}
const bar = (page: Page, task: number) => page.evaluate((task) => { const r = (window.gantt!.app.test_bar_rects() as { task: number; x: number; y: number; width: number; height: number }[]).find(r => r.task === task)!; const box = document.getElementById("timeline")!.getBoundingClientRect(), style = getComputedStyle(document.getElementById("app")!); return { x: box.x + (r.x + r.width / 2) / devicePixelRatio, y: box.y + (r.y + r.height / 2) / devicePixelRatio, terminal: box.x + (r.x + r.width) / devicePixelRatio + parseFloat(style.getPropertyValue("--gantt-terminal-gap")) + parseFloat(style.getPropertyValue("--gantt-terminal-size")) / 2 }; }, task);
async function link(page: Page): Promise<void> {
  await frames(page); const a = await bar(page, 0), b = await bar(page, 1); await page.mouse.move(a.x, a.y); await frames(page);
  await page.mouse.move(a.terminal, a.y); await page.mouse.down(); await page.mouse.move(b.x, b.y, { steps: 8 }); await frames(page); await page.mouse.up(); await frames(page);
}

test("what each save handler and the before-edit function receive is byte for byte what main sent", async ({ page }) => {
  await open(page);
  await step(page, async () => { await row(page, 2).locator(".finish").click(); await frames(page); await page.keyboard.press("Enter"); await page.keyboard.press("Tab"); await frames(page); await page.keyboard.press("Enter"); await page.locator("#table-rows input.cell-edit").fill("1, 2SS+1d"); await page.keyboard.press("Enter"); });
  await step(page, () => page.keyboard.press("Control+z")); await step(page, () => page.keyboard.press("Control+Shift+z"));
  await step(page, () => page.evaluate(() => { const data = new DataTransfer(); data.setData("text/plain", "Pour slab\nCure slab"); (document.activeElement ?? document.body).dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true })); }));
  await step(page, () => page.keyboard.press("Control+z"));
  for (let n = 0; n < 2; n++) await step(page, async () => { await row(page, 0).locator(".name").click(); await page.keyboard.press("Delete"); });
  for (let n = 0; n < 2; n++) await step(page, () => page.keyboard.press("Control+z"));
  await step(page, () => page.evaluate("chart.commands.setDataDate('2026-11-09')")); await step(page, async () => { await page.locator("#app").focus(); await page.keyboard.press("Control+z"); });
  await step(page, async () => { await row(page, 0).locator(".name").click(); await frames(page); await page.keyboard.press("Shift+Enter"); await page.locator('#task-dialog input[name=name]').fill("Renamed task"); await page.getByRole("button", { name: "Save", exact: true }).click(); });
  await step(page, () => link(page));
  await step(page, async () => { await frames(page); const a = await bar(page, 0); await page.mouse.move(a.x, a.y); await page.mouse.down(); await page.mouse.move(a.x + 160, a.y, { steps: 8 }); await frames(page); await page.mouse.up(); });
  const log = await page.evaluate<string[]>("log"); expect(log.length).toBeGreaterThan(0);
  expect([...new Set(log.map(line => line.split(" ")[0]))]).toEqual(expect.arrayContaining(["createTask", "updateTask", "deleteTask", "createLink", "deleteLink", "updateSchedule"]));
  const fixture = new URL("./fixtures/save-contract-main.jsonl", import.meta.url);
  if (process.env.PIN_WRITE === "1") { expect(process.env.PIN_SHA).toMatch(/^[0-9a-f]{40}$/); writeFileSync(fixture, [`main ${process.env.PIN_SHA}`, ...log].map(line => JSON.stringify(line)).join("\n") + "\n"); }
  else { const lines = readFileSync(fixture, "utf8").split("\n"); if (lines.at(-1) === "") lines.pop(); const [main, ...expected] = lines.map(line => JSON.parse(line)); expect(main).toMatch(/^main /); expect(log).toEqual(expected); }
});

test("a refused action loses no redo", async ({ page }) => {
  await open(page, true); await edit(page, 0, "9"); await expect(row(page, 0).locator(".dur")).toHaveText("9d"); await page.keyboard.press("Control+z"); await expect(row(page, 0).locator(".dur")).toHaveText("2d");
  expect(await page.evaluate("chart.commands.redo()"), "the edit is available to redo").toBe(true); await expect(row(page, 0).locator(".dur")).toHaveText("9d"); await page.keyboard.press("Control+z"); await expect(row(page, 0).locator(".dur")).toHaveText("2d");
  await expect.poll(() => page.evaluate("chart.state().unsavedCommits")).toBe(0); await page.evaluate("installMissing()"); await link(page);
  await expect(page.locator("#drag-note")).toHaveText("This page cannot save new links: it gave no createLink.");
  expect(await page.evaluate("log")).toEqual([]); expect(await page.evaluate("errs.filter(error => error.message.includes('the save handlers have no'))")).toEqual([]);
  expect(await page.evaluate("chart.commands.redo()"), "the refused release preserves redo").toBe(true); await expect(row(page, 0).locator(".dur")).toHaveText("9d");
});
