import { expect, test, type Page } from "@playwright/test";
import { trackLogs, pageErrors, waitReady, waitSampleSettled } from "./support/page";

// A dragged bar lands on a working-day start of its calendar, not at the minute the pointer let go (gs#863). On the
// office fit-out, dragging "Glazing and joinery fabrication" about 100 px later (three weeks, at the opening zoom) must put its start at 08:00 on a working
// day, and the Procurement summary's duration must stay whole days (before the fix the Design summary read "45d 7h 35m").
// The task is one that has not started: the MS Project reader applies actuals (gs#860), and a started or finished task is
// fixed at them and does not move. The sample's remaining work is whole days (gs#901), so the task opens at 08:00 and does not
// start mid-day by itself. The drag distance is worked out from the measured opening time so the pointer lets go at about
// 15:00, in the middle of an afternoon working period (the file's calendar is 08:00-12:00 and 13:00-17:00): the raw start the
// pointer asks for is mid-day, and only the day-start snap puts it on 08:00.

const TASK = "Glazing and joinery fabrication";
const SUMMARY = "Procurement";
/** Three weeks of calendar time. */
const DRAG_DAYS = 21 * 24 * 60;
/** The time of day, in minutes after midnight, the pointer should let go at: 15:00, the middle of 13:00-17:00. */
const RELEASE_AT = 15 * 60;

/** The text of one cell of the table row named `name`. */
const cell = (page: Page, name: string, cls: string): Promise<string> =>
  page.evaluate(
    ([wanted, column]) => {
      for (const row of document.querySelectorAll<HTMLElement>("#table-rows .row")) {
        if (row.querySelector(".name")?.textContent?.trim() === wanted) return row.querySelector(`.${column}`)?.textContent ?? "";
      }
      throw new Error(`no row named ${wanted}`);
    },
    [name, cls],
  );

/** The centre of the task's bar in CSS pixels of the page, from the bar the last frame drew. */
async function barCentre(page: Page, name: string): Promise<{ x: number; y: number }> {
  const task = await page.evaluate((wanted) => {
    for (const row of document.querySelectorAll<HTMLElement>("#table-rows .row")) {
      if (row.querySelector(".name")?.textContent?.trim() === wanted) return Number(row.dataset.task);
    }
    throw new Error(`no row named ${wanted}`);
  }, name);
  const rect = await page.evaluate((id) => {
    const found = (window.gantt!.app.test_bar_rects() as { task: number; x: number; y: number; width: number; height: number }[]).find(
      (r) => r.task === id,
    );
    return found === undefined ? null : { ...found, dpr: window.devicePixelRatio };
  }, task);
  expect(rect, `${name} has a bar on screen`).not.toBeNull();
  const canvas = await page.locator("#timeline").boundingBox();
  if (canvas === null || rect === null) throw new Error("no canvas box");
  return {
    x: canvas.x + (rect.x + rect.width / 2) / rect.dpr,
    y: canvas.y + (rect.y + rect.height / 2) / rect.dpr,
  };
}

// The office fit-out opens with a saved baseline set (gs#703); this spec tests the no-baseline layout, so it clears the baseline once.
test("a dragged task lands on 08:00 of a working day and its summary keeps whole days", async ({ page }) => {
  const logs = trackLogs(page);
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.locator("#baseline-clear").click();
  await expect(page.locator("#baseline-clear")).toBeHidden();
  const before = { start: await cell(page, TASK, "start"), summary: await cell(page, SUMMARY, "dur") };

  const at = await barCentre(page, TASK);
  const opened = /(\d{2}):(\d{2})$/.exec(before.start);
  expect(opened, "the start reads as a time").not.toBeNull();
  const openedAt = Number(opened![1]) * 60 + Number(opened![2]);
  const pxPerMinute = await page.evaluate(() => window.gantt!.status().pxPerMinute);
  const dragPx = Math.round((DRAG_DAYS + RELEASE_AT - openedAt) * pxPerMinute);
  // Where the pointer lets go, as a time of day: inside the afternoon period, clear of its edges by more than a pixel.
  const releasedAt = openedAt + dragPx / pxPerMinute - DRAG_DAYS;
  expect(releasedAt, "the pointer lets go inside 13:00-17:00").toBeGreaterThan(13 * 60 + 5);
  expect(releasedAt, "the pointer lets go inside 13:00-17:00").toBeLessThan(17 * 60 - 5);
  expect(dragPx, "about 100 px").toBeGreaterThan(80);
  expect(dragPx, "about 100 px").toBeLessThan(130);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x + dragPx / 2, at.y, { steps: 5 });
  await page.mouse.move(at.x + dragPx, at.y, { steps: 5 });
  await page.mouse.up();
  const frames = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((from) => window.gantt!.status().frames >= from + 3, frames);

  const start = await cell(page, TASK, "start");
  expect(start, "the drag moved the task").not.toBe(before.start);
  expect(start, "the new start is a working-day start").toMatch(/ 08:00$/);
  expect(await cell(page, TASK, "finish"), "the task is whole days long").toMatch(/ 17:00$/);
  const summary = await cell(page, SUMMARY, "dur");
  expect(summary, "the Procurement summary has no hours or minutes part").toMatch(/^\d+d$/);
  expect(pageErrors(logs)).toEqual([]);
});
