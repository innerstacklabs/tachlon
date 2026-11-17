import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// Read-only mode (gantt#2085): no edit path changes the schedule (keys, cell editor, task dialog, bar drag, paste, undo) and a click still selects a row.
declare global {
  interface Window {
    roChart: Gantt;
    mountRo(options: Record<string, unknown>): Promise<unknown>;
  }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.mountRo = async (options) => (window.roChart = await mount(document.getElementById("app"), { schedule: { demo: 30 }, ...options }));
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

async function mountHost(page: Page, options: Record<string, unknown>): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
  await page.evaluate((o) => window.mountRo(o).then(() => undefined), options);
}
const doc = (page: Page): Promise<string> => page.evaluate(() => JSON.stringify(window.roChart.toDocument()));
const cell = (page: Page) => page.locator("#table-rows .row").nth(6).locator(".dur");
const READ_ONLY = /read-only/;

test("read-only refuses every edit path and says why, and a click still selects a row", async ({ page }) => {
  await mountHost(page, { forceWebgl: true, readOnly: true });
  const before = await doc(page);
  const alert = page.locator("#edit-error-text");
  await cell(page).click();
  expect(await page.evaluate(() => window.gantt!.status().selected?.task), "a click selects").toBeDefined();
  for (const key of ["Delete", "Insert", "Alt+Shift+ArrowRight", "Control+z", "Shift+Enter"]) {
    await page.keyboard.press(key);
    await expect(alert, key).toHaveText(READ_ONLY);
    await page.locator("#edit-error-close").click();
  }
  expect(await page.evaluate(() => (document.getElementById("task-dialog") as HTMLDialogElement).open), "the dialog stays shut").toBe(false);
  await cell(page).dblclick();
  await expect(alert, "the cell editor").toHaveText(READ_ONLY);
  await expect(page.locator("#table-rows input.cell-edit")).toHaveCount(0);
  await page.evaluate(() => {
    const data = new DataTransfer();
    data.setData("text/plain", "Pour slab");
    document.getElementById("app")!.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  });
  const bar = await page.evaluate(() => {
    const { x, y, width, height } = (window.gantt!.app.test_bar_rects() as Record<string, number>[])[2]!;
    const box = document.getElementById("timeline")!.getBoundingClientRect();
    return { x: box.x + (x! + width! / 2) / devicePixelRatio, y: box.y + (y! + height! / 2) / devicePixelRatio };
  });
  await page.mouse.move(bar.x, bar.y);
  await page.mouse.down();
  await page.mouse.move(bar.x + 60, bar.y, { steps: 6 });
  await page.mouse.up();
  await expect(page.locator("#drag-note"), "the drag note").toHaveText(READ_ONLY);
  expect(await doc(page), "the schedule is as it was").toBe(before);
});

test("setReadOnly(false) lets a typed duration through, and setReadOnly(true) refuses it again", async ({ page }) => {
  await mountHost(page, { forceWebgl: true, readOnly: true });
  expect(await page.evaluate(() => window.roChart.readOnly())).toBe(true);
  const before = await doc(page);
  await page.evaluate(() => window.roChart.setReadOnly(false));
  await cell(page).dblclick();
  await page.locator("#table-rows input.cell-edit").fill("17");
  await page.keyboard.press("Enter");
  expect(await doc(page), "the typed duration changed the schedule").not.toBe(before);
  await page.evaluate(() => window.roChart.setReadOnly(true));
  await cell(page).dblclick();
  await expect(page.locator("#table-rows input.cell-edit")).toHaveCount(0);
});

test("read-only survives the switch to WebGL2", async ({ page }) => {
  await mountHost(page, { readOnly: true });
  test.skip(await page.evaluate(() => window.roChart.backend() !== "WebGPU"), "this browser has no WebGPU to lose");
  await page.evaluate(() => window.gantt!.failNextFrame(false));
  await page.waitForFunction(() => { try { return window.roChart.backend() === "WebGL2"; } catch { return false; } }, undefined, { timeout: 60_000 });
  expect(await page.evaluate(() => window.roChart.readOnly())).toBe(true);
  await cell(page).dblclick();
  await expect(page.locator("#edit-error-text")).toHaveText(READ_ONLY);
});
