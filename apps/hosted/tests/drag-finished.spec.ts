import { expect, test, type Page } from "@playwright/test";
import { trackLogs, pageErrors, waitReady, waitSampleSettled } from "./support/page";

// A drag on a finished task says why it does not move (gs#895). On the office fit-out, "Space planning and layout options" (uid 102)
// has an actual finish, so its dates are its actuals. Dragging its bar several days later moves nothing, and the release shows the
// note in the alert line, with the task's name.

const TASK = "Space planning and layout options";
/** What the chip says (gs#895). */
const NOTE = `${TASK} is finished: its dates are its actuals. Change Actual finish in the table to move it.`;

/** Every row's name, start and finish as the table shows them. */
const dates = (page: Page): Promise<string[]> =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map(
      (row) => `${row.querySelector(".name")?.textContent?.trim()}|${row.querySelector(".start")?.textContent}|${row.querySelector(".finish")?.textContent}`,
    ),
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
  return { x: canvas.x + (rect.x + rect.width / 2) / rect.dpr, y: canvas.y + (rect.y + rect.height / 2) / rect.dpr };
}

/** Drag `name`'s bar five days later with the pointer, and return the rows before, plus what the alert line shows before the release. */
async function dragFive(page: Page, name: string): Promise<{ before: string[]; midDrag: string | null }> {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const before = await dates(page);
  const at = await barCentre(page, name);
  const pxPerMinute = await page.evaluate(() => window.gantt!.status().pxPerMinute);
  const dragPx = Math.round(5 * 24 * 60 * pxPerMinute);
  await page.mouse.move(at.x, at.y);
  await page.mouse.down();
  await page.mouse.move(at.x + dragPx / 2, at.y, { steps: 5 });
  await page.mouse.move(at.x + dragPx, at.y, { steps: 5 });
  const frames = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((from) => window.gantt!.status().frames >= from + 3, frames);
  const line = page.locator("#edit-error");
  const midDrag = (await line.isVisible()) ? await page.locator("#edit-error-text").textContent() : null;
  await page.mouse.up();
  const after = await page.evaluate(() => window.gantt!.status().frames);
  await page.waitForFunction((from) => window.gantt!.status().frames >= from + 3, after);
  return { before, midDrag };
}

test("a drag on an in-progress task moves nothing; the note says it has started in the calm chip, not the red banner", async ({ page }) => {
  const logs = trackLogs(page);
  const name = "Detailed design and specification";
  const { before } = await dragFive(page, name);
  const chip = page.locator("#drag-note");
  await expect(chip).toBeVisible();
  await expect(chip).toHaveText(
    `${name} has started: its start is its actual start. Change Remaining or % complete in the table to move its finish.`,
  );
  await expect(page.locator("#edit-error"), "the red banner stays hidden").toBeHidden();
  expect(await chip.evaluate((el) => el.scrollWidth <= el.clientWidth && el.scrollHeight <= el.clientHeight), "the whole text shows, none cut off").toBe(true);
  await expect(page.locator("#table-head span", { hasText: /^Task$/ }), "the header row is visible").toBeVisible();
  expect(await dates(page), "no task's dates changed").toEqual(before);
  expect(pageErrors(logs)).toEqual([]);
});

test("a drag on a finished task moves nothing; the release shows the note in the calm chip, not the red banner, and the next press clears it", async ({ page }) => {
  const logs = trackLogs(page);
  const { before, midDrag } = await dragFive(page, TASK);
  expect(midDrag, "nothing is said before the release").toBeNull();

  const chip = page.locator("#drag-note");
  await expect(chip).toBeVisible();
  await expect(chip).toHaveText(NOTE);
  await expect(page.locator("#edit-error"), "the red banner stays hidden").toBeHidden();
  // The office fit-out opens with Progress on: Dur, Start and Finish follow the progress columns, past the pane's right edge until the table
  // is scrolled (gs#974). The chip must not cover a head the pane shows, so the check is on the heads inside the pane.
  const headers = await page.evaluate(() => {
    const pane = document.getElementById("table-head")!.getBoundingClientRect();
    return [...document.querySelectorAll<HTMLElement>("#table-head span")]
      .filter((el) => ["Task", "Dur", "Start", "Finish"].includes(el.textContent?.trim() ?? "") && el.getBoundingClientRect().right <= pane.right)
      .map((el) => {
        const r = el.getBoundingClientRect();
        const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2);
        return { text: el.textContent?.trim(), covered: top === null || !el.contains(top) };
      });
  });
  expect(headers.map((h) => h.text), "the Task head is in the pane").toContain("Task");
  expect(headers.filter((h) => h.covered), "no header is covered").toEqual([]);
  expect(await dates(page), "no task's dates changed").toEqual(before);

  // The next pointer down clears it (a press on empty chart space).
  const canvas = await page.locator("#timeline").boundingBox();
  if (canvas === null) throw new Error("no canvas box");
  await page.mouse.move(canvas.x + canvas.width - 40, canvas.y + canvas.height - 120);
  await page.mouse.down();
  await expect(chip).toBeHidden();
  await page.mouse.up();
  expect(pageErrors(logs)).toEqual([]);
});
