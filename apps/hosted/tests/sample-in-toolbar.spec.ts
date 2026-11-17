import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// "Open…" is the one control for a file and a sample, so the hosted page has no row of its own above the chart and no second button
// in the toolbar (gs#858). Measured at 1280 x 1000.

test.use({ viewport: { width: 1280, height: 1000 } });

test("there is no host bar and no second button: the chart starts at the top of the page", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  await expect(page.locator(".host-bar"), "the header row is gone").toHaveCount(0);
  await expect(page.locator("#sample-button"), "there is no button besides Open…").toHaveCount(0);
  await expect(page.locator("#toolbar #open-button")).toHaveText("Open…");
  await expect(page.locator("#toolbar #sample-list"), "the list belongs to the toolbar").toHaveCount(1);
  await expect(page.locator("#toolbar #sample-status")).toHaveCount(1);
  const place = await page.evaluate(() => ({ chartTop: document.getElementById("app")!.getBoundingClientRect().top, chartHeight: document.getElementById("app")!.getBoundingClientRect().height, window: innerHeight }));
  expect(place.chartTop, "the chart's box starts at the top of the page").toBe(0);
  expect(place.chartHeight, "the chart takes the whole window").toBe(place.window);
  await expect(page.locator("#toolbar")).toHaveAttribute("title", /^Drawing with (WebGPU|WebGL2)$/);
  await expect(page.locator("#backend-label")).toHaveText(/^(WebGPU|WebGL2)$/);
});

test("Open… opens a menu with New schedule and Open a file… first, then the four samples, over the chart, and the toolbar keeps its one line", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  const measure = (): Promise<{ app: number; bar: number; overflow: number; below: boolean }> =>
    page.evaluate(() => {
      const bar = document.getElementById("toolbar")!;
      const list = document.getElementById("sample-list")!;
      const button = document.getElementById("open-button")!.getBoundingClientRect();
      return {
        app: document.getElementById("app")!.getBoundingClientRect().height,
        bar: bar.getBoundingClientRect().height,
        overflow: bar.scrollWidth - bar.clientWidth,
        below: list.getBoundingClientRect().top >= button.bottom,
      };
    });
  const before = await measure();
  await expect(page.locator("#open-button")).toHaveAttribute("aria-expanded", "false");
  await page.locator("#open-button").click();
  await expect(page.locator("#sample-list")).toBeVisible();
  await expect(page.locator("#open-button")).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator("#sample-list button")).toHaveText([
    /^New schedule/,
    /^Open a file…/,
    /^Compare with a file\.\.\./,
    /^Small project: office fit-out/,
    /^Medium: 5,000 activities/,
    /^Large: 50,000 activities/,
    /^Generated demo: 50,000 tasks/,
    /^Save/,
  ]);
  const after = await measure();
  expect(after.app, "opening the menu does not change the chart's box").toBe(before.app);
  expect(after.bar, "the toolbar stays one line").toBe(before.bar);
  expect(after.below, "the list hangs below the button").toBe(true);
  expect(after.overflow, "the strip does not overflow").toBeLessThanOrEqual(0);
  await page.keyboard.press("Escape");
  await expect(page.locator("#sample-list")).toBeHidden();
});

test("Open a file… in the menu opens the file chooser", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  await page.locator("#open-button").click();
  const chooser = page.waitForEvent("filechooser");
  await page.locator("#sample-list button[data-open-file]").click();
  expect((await chooser).isMultiple()).toBe(false);
  await expect(page.locator("#sample-list")).toBeHidden();
});
