import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The frame loop draws while something changes and stops after (gantt#1335, `packages/gantt/src/loop.ts`). A call into the app wakes it, so
// `window.gantt.status()` would move the count it reads; the count here comes from the prototype's own `status`, which does not.

const frames = (page: Page): Promise<number> =>
  page.evaluate(() => {
    const app = window.gantt!.app;
    return (JSON.parse((Object.getPrototypeOf(app) as typeof app).status.call(app)) as { frames: number }).frames;
  });

test("an idle chart draws no frames, a wheel wakes it, and it goes quiet again", async ({ g, page }) => {
  await g.open({ frames: 5, timeout: 60_000 });
  await page.waitForTimeout(1500);
  const idle = await frames(page);
  await page.waitForTimeout(2000);
  expect(await frames(page), "frames drawn in two idle seconds").toBe(idle);

  const box = await g.box();
  await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.wheel(0, 400);
  await expect.poll(() => frames(page), { message: "the wheel drew frames" }).toBeGreaterThan(idle);
  await page.waitForTimeout(1500);
  const settled = await frames(page);
  await page.waitForTimeout(1500);
  expect(await frames(page), "frames drawn after the wheel went quiet").toBe(settled);
});

test("the loop keeps drawing while the moved note is shown, then stops", async ({ g, page }) => {
  await openLedger(g);
  await page.waitForTimeout(1000);
  await editDuration(page, "10d");
  await page.waitForTimeout(1500);
  const shown = await frames(page);
  await page.waitForTimeout(1000);
  expect(await frames(page), "frames drawn while the note holds").toBeGreaterThan(shown);
  await page.waitForTimeout(5000);
  const gone = await frames(page);
  await page.waitForTimeout(1500);
  expect(await frames(page), "frames drawn after the note faded").toBe(gone);
});

// The engine fires `gpulost` on the window when WebGPU loses its device, from the browser and not from a call (gantt#1335).
test("a lost GPU device wakes an idle chart, which draws a frame to see it", async ({ g, page }) => {
  await g.open({ frames: 5, timeout: 60_000 });
  await page.waitForTimeout(1500);
  const idle = await frames(page);
  await page.evaluate(() => window.dispatchEvent(new Event("gpulost")));
  await expect.poll(() => frames(page), { message: "the lost device drew a frame" }).toBeGreaterThan(idle);
});
