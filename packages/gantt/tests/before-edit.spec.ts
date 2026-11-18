import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { BeforeEditChange, Gantt, GanttEvents } from "../src/index";
import { chromePath } from "./support/knobs";

declare global {
  interface Window { beChart: Gantt; asked: BeforeEditChange[]; answers: unknown[]; errors: GanttEvents["error"][]; mountBe(code: string, options?: Record<string, unknown>): Promise<unknown> }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.asked = [], window.errors = [];
  window.mountBe = async (code, options = {}) => {
    const answer = (0, eval)("(" + code + ")");
    window.beChart = await mount(document.getElementById("app"), { schedule: { demo: 30 }, forceWebgl: true, ...options, beforeEdit: (change) => (window.asked.push(change), answer(change)) });
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
  expect(await asked(page, "setDuration")).toMatchObject([{ source: "typed", changes: [{ record: "task", op: "update", field: "duration", new: 17 }] }]);
  await mountHost(page, `() => true`);
  await typeDuration(page);
  expect(await doc(page), "an allowed edit applies").not.toBe(before);
});

test("a drag asks once, when it starts, and not again while it moves or when it ends", async ({ page }) => {
  await mountHost(page, `() => undefined`);
  const bar = await page.evaluate(() => {
    const { x, y, width, height } = (window.gantt!.app.test_bar_rects() as Record<string, number>[])[2]!;
    const box = document.getElementById("timeline")!.getBoundingClientRect();
    return { x: box.x + (x! + width! / 2) / devicePixelRatio, y: box.y + (y! + height! / 2) / devicePixelRatio };
  });
  await page.mouse.move(bar.x, bar.y);
  await page.mouse.down();
  await page.mouse.move(bar.x + 60, bar.y, { steps: 6 });
  expect((await asked(page, "drag")).length, "one call while the bar moves").toBe(1);
  await page.mouse.up();
  expect(await asked(page, "drag"), "still one call after the drag ends").toMatchObject([{ source: "drag", tasks: [expect.any(Number)] }]);
});

test("a function that throws, answers with a promise or calls the chart refuses the edit with the default sentence and fires one error event", async ({ page }) => {
  for (const code of [`() => { throw new Error("boom"); }`, `async () => undefined`, `() => { window.beChart.toDocument(); }`]) {
    await mountHost(page, code);
    const [before, rows] = [await doc(page), await page.evaluate(() => window.beChart.state().rowCount)];
    await typeDuration(page);
    await expect(alert(page), code).toHaveText(/not allowed/);
    expect(await doc(page), `${code}: the schedule is as it was`).toBe(before);
    expect(await page.evaluate(() => window.beChart.state().rowCount), `${code}: no row was added`).toBe(rows);
    expect(await page.evaluate(() => window.errors.map(({ source, fatal }) => `${source} ${fatal}`)), code).toEqual(["beforeEdit false"]);
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

test("the function is still asked after the switch to WebGL2", async ({ page }) => {
  await mountHost(page, `() => "no"`, { forceWebgl: false });
  test.skip(await page.evaluate(() => window.beChart.backend() !== "WebGPU"), "this browser has no WebGPU to lose");
  await page.evaluate(() => window.gantt!.failNextFrame(false));
  await page.waitForFunction(() => { try { return window.beChart.backend() === "WebGL2"; } catch { return false; } }, undefined, { timeout: 60_000 });
  await typeDuration(page);
  await expect(alert(page)).toHaveText("no");
});
