import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// The hover card's Float row (gs#703): the float in working days of the task's calendar, then the same float in calendar days.

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

test("a_task_with_float_shows_it_in_calendar_days_too", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  // The first leaf row with a day or more of float, and what the card must read for it.
  const { row, task, expected } = await page.evaluate(() => {
    for (const el of document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")) {
      const task = Number(el.dataset.task);
      const calendar = window.gantt!.app.task_float_calendar_text(task);
      if (calendar !== "") return { row: Number(el.dataset.row), task, expected: `${(window.gantt!.app.task_details(task) as { floatText: string }).floatText} (${calendar})` };
    }
    throw new Error("no leaf row has a day of float");
  });
  expect(expected).toMatch(/ d \d+ h( \d+ min)? \(\d+ calendar days?\)$/);
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  await page.mouse.move(canvas.x + 1, y);
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3 && (await g.status()).hover?.task !== task; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
  }
  await expect(page.locator("#hover-card .details > *").filter({ hasText: /^Float$/ }).locator("xpath=following-sibling::*[1]")).toHaveText(expected);
});
