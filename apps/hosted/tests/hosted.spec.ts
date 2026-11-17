import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product";

// The hosted app's smoke test (gs#511 S3): the build opens, the chart gets ready and draws, the toolbar's tooltip says which
// backend draws, and a schedule file with two projects opens its chooser with no network request.

// The fixture is the one the chart's own picker spec uses: projects 1000 and 1001, 8 activities each.
const TWO_PROJECTS = readFileSync(new URL("../../../packages/gantt/tests/fixtures/two-projects.xer", import.meta.url));

test("the build opens, the toolbar names the backend, and a two-project file opens the chooser offline", async ({ page }) => {
  const logs: string[] = [];
  page.on("console", (message) => logs.push(`${message.type()}: ${message.text()}`));
  page.on("pageerror", (error) => logs.push(`pageerror: ${error.message}`));

  await page.goto("/");
  await expect(page).toHaveTitle(PRODUCT_NAME);
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, {
    timeout: 180_000,
  });
  await expect(page.locator("#table-rows .row").first()).toBeVisible();

  // The label is the chart's own answer, WebGPU or WebGL2 (the CI runner has no GPU, so it reads WebGL2 there).
  const label = page.locator("#backend-label");
  await expect(label).toHaveText(/^(WebGPU|WebGL2)$/);
  expect(await label.textContent()).toBe(await page.evaluate(() => window.gantt!.status().backend));

  // The chart is as big as the box the host page gives it: nothing is above it.
  const box = await page.evaluate(() => ({ root: document.getElementById("app")!.getBoundingClientRect().height, window: innerHeight }));
  expect(box.root).toBe(box.window);

  // Choosing a file reads it in the browser: the chooser opens and no request is made for it.
  const requests: string[] = [];
  page.on("request", (request) => requests.push(`${request.method()} ${request.url()}`));
  await page.setInputFiles("#open-input", { name: "two-projects.xer", mimeType: "application/octet-stream", buffer: TWO_PROJECTS });
  await expect(page.locator("#picker")).toBeVisible();
  await expect(page.locator("#picker-title")).toHaveText("two-projects.xer holds 2 projects");
  await expect(page.locator("#picker-list button")).toHaveCount(3);
  expect(requests, "opening a file makes no network request").toEqual([]);

  expect(logs.filter((line) => /^(error|pageerror)/.test(line)), "the page logs no error").toEqual([]);
});

// The tab shows the product's mark: the page's icon link holds an SVG inline (a data URI, so the page makes no request for it) and the browser can draw it (not the empty `data:,` it once named).
test("the page's icon link is an SVG data URI that loads", async ({ page }) => {
  await page.goto("/");
  const href = await page.locator('link[rel="icon"]').getAttribute("href");
  expect(href, "the icon link holds an SVG data URI").toMatch(/^data:image\/svg\+xml,./);
  await page.evaluate(async (src) => { const image = new Image(); image.src = src; await image.decode(); }, href!);
});
