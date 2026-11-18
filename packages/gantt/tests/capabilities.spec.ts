import { createServer, type ViteDevServer } from "vite";
import { fileURLToPath } from "node:url";
import { expect, test, type Page } from "@playwright/test";
import type { Gantt } from "../src/index";
import { chromePath } from "./support/knobs";

// The host's capability profile (`mount`'s `capabilities`, `Gantt.setCapabilities`): a kind of change the host turned off is refused on the page, with the sentence that says so, and the schedule stays as it was.
declare global {
  interface Window {
    capChart: Gantt;
    mountCaps(options: Record<string, unknown>): Promise<unknown>;
  }
}
const HOST = `<!doctype html><html><body style="margin: 0"><div id="app" class="gantt" tabindex="0" style="width: 1000px; height: 500px"></div><script type="module">
  import { mount } from "/src/index.ts";
  window.mountCaps = async (options) => (window.capChart = await mount(document.getElementById("app"), { schedule: { demo: 30 }, forceWebgl: true, ...options }));
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

async function openHost(page: Page): Promise<void> {
  await page.route(`${base}host.html`, (route) => route.fulfill({ contentType: "text/html", body: HOST }));
  await page.goto(`${base}host.html`);
  await page.waitForFunction(() => (window as unknown as { ready?: boolean }).ready === true, undefined, { timeout: 120_000 });
}
const mountCaps = async (page: Page, options: Record<string, unknown>): Promise<void> => {
  await openHost(page);
  await page.evaluate((o) => window.mountCaps(o).then(() => undefined), options);
};
const doc = (page: Page): Promise<string> => page.evaluate(() => JSON.stringify(window.capChart.toDocument()));
async function typeDuration(page: Page): Promise<void> {
  await page.locator("#table-rows .row").nth(6).locator(".dur").dblclick();
  await page.locator("#table-rows input.cell-edit").fill("17");
  await page.keyboard.press("Enter");
}

test("a profile that turns off dates refuses a typed duration with its sentence, and setCapabilities(undefined) lets it through", async ({ page }) => {
  await mountCaps(page, { capabilities: { editable: { dates: false } } });
  const before = await doc(page);
  await typeDuration(page);
  await expect(page.locator("#table-rows .cell-error")).toHaveText("The host does not allow changing dates and durations.");
  expect(await doc(page), "the schedule is as it was").toBe(before);
  await page.evaluate(() => window.capChart.setCapabilities(undefined));
  await typeDuration(page);
  await expect(page.locator("#table-rows .row").nth(6).locator(".dur")).toHaveText(/17/);
  expect(await doc(page), "an allowed edit applies").not.toBe(before);
});

test("a profile that turns off bar moves refuses a bar drag and says so in the drag note", async ({ page }) => {
  await mountCaps(page, { capabilities: { gestures: { barMove: false } } });
  const before = await doc(page);
  const bar = await page.evaluate(() => {
    const { x, y, width, height } = (window.gantt!.app.test_bar_rects() as Record<string, number>[])[2]!;
    const box = document.getElementById("timeline")!.getBoundingClientRect();
    return { x: box.x + (x! + width! / 2) / devicePixelRatio, y: box.y + (y! + height! / 2) / devicePixelRatio };
  });
  await page.mouse.move(bar.x, bar.y);
  await page.mouse.down();
  await page.mouse.move(bar.x + 60, bar.y, { steps: 6 });
  await page.mouse.up();
  await expect(page.locator("#drag-note")).toHaveText("The host does not allow moving a bar.");
  expect(await doc(page), "the schedule is as it was").toBe(before);
});

test("a profile with a member the chart does not have rejects mount", async ({ page }) => {
  await openHost(page);
  await expect(page.evaluate(() => window.mountCaps({ capabilities: { gestures: { drag: false } } }))).rejects.toThrow(/gestures\.drag: unknown key/);
});
