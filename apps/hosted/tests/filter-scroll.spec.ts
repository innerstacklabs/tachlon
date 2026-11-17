import { expect, test, type Page } from "@playwright/test";

// gs#1015: turning on a filter whose rows are all outside the chart window scrolls the chart to the first visible row's
// start day, keeping the zoom, once per filter change. A filter whose rows are already in view leaves the chart alone.
// The sample is the generated demo (`?sample=demo-50k`), which opens on 5 Jan 2026. The hosted catalog has no 2,000-row
// sample and every sample's Near critical rows start where the chart opens, so the first case pans the chart away
// (`go_to_date`, the Find box's date jump) and then turns the filter on: the rows are there and no bar is in view.

test.use({ viewport: { width: 1280, height: 800 } });

async function open(page: Page): Promise<void> {
  await page.goto("/?sample=demo-50k");
  await page.waitForFunction(() => window.gantt?.ready === true && window.gantt.status().frames >= 10, undefined, { timeout: 180_000 });
}

/** Press a filter in the Filters menu and close the menu. */
async function press(page: Page, id: string): Promise<void> {
  await page.click("#filters-button");
  await page.click(`#${id}`);
  await page.keyboard.press("Escape");
}

interface View {
  t: [number, number];
  px: number;
  inView: number;
  rows: number;
}

/** The view after 120 more frames, so a scroll that is still running has landed. */
async function settled(page: Page): Promise<View> {
  const from = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((n) => window.gantt!.status().frames >= n + 120, from);
  return page.evaluate(() => {
    const { app, status } = window.gantt!;
    const s = status();
    const bars = app.test_bar_rects() as { x: number; width: number }[];
    const dpr = window.devicePixelRatio;
    const inView = bars.filter((b) => b.x + b.width > 0 && b.x < s.surface[0] * dpr).length;
    return { t: s.t, px: s.pxPerMinute, inView, rows: s.rowCount };
  });
}

/** Pan the chart to a day long after every bar, then wait for it to land. */
async function panAway(page: Page): Promise<View> {
  expect(await page.evaluate(() => window.gantt!.app.go_to_date("2040-01-01"))).toBe(true);
  const away = await settled(page);
  expect(away.inView, "the chart is panned past every bar").toBe(0);
  return away;
}

test("a filter whose rows are off screen scrolls the chart to the first of them and keeps the zoom", async ({ page }) => {
  await open(page);
  const away = await panAway(page);
  await press(page, "near-critical-toggle");
  const after = await settled(page);
  expect(after.rows, "the filter keeps some rows, not all").toBeGreaterThan(0);
  expect(after.rows).toBeLessThan(await page.evaluate(() => window.gantt!.status().n));
  expect(after.inView, "at least one bar is in the chart viewport").toBeGreaterThan(0);
  expect(after.px, "the zoom is kept").toBe(away.px);
  const start = await page.evaluate(() => window.gantt!.app.row_start(0)!);
  expect(start, "the first row's start is inside the window").toBeGreaterThan(after.t[0]);
  expect(start).toBeLessThan(after.t[1]);
});

test("one scroll per filter change: panning away afterwards stays away", async ({ page }) => {
  await open(page);
  await panAway(page);
  await press(page, "near-critical-toggle");
  expect((await settled(page)).inView).toBeGreaterThan(0);
  expect(await page.evaluate(() => window.gantt!.app.go_to_date("2040-01-01"))).toBe(true);
  expect((await settled(page)).inView, "the chart is not pulled back until the filter changes again").toBe(0);
});

test("a filter whose rows are already in view does not move the chart", async ({ page }) => {
  await open(page);
  const before = await settled(page);
  expect(before.inView, "the unfiltered chart shows bars").toBeGreaterThan(0);
  await press(page, "near-critical-toggle");
  const after = await settled(page);
  expect(after.rows).toBeLessThan(before.rows);
  expect(after.inView, "the filtered rows are in view").toBeGreaterThan(0);
  expect(after.t, "the window did not move").toEqual(before.t);
});
