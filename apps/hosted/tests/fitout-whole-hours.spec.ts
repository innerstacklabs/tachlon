import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The office fit-out opens on whole working hours (gs#901): remaining work rounds to the hour, so no summary shows hours or minutes
// in Dur, no date is off the hour, and no float has a fraction but a half day. And a Dur cell's text stays inside its cell.

test.use({ viewport: { width: 1280, height: 1000 } });

async function open(page: import("@playwright/test").Page): Promise<void> {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#table-rows .row").first()).toBeVisible();
}

test("the office fit-out opens with whole-hour dates, no hours or minutes in a summary's Dur, and half-day floats", async ({ page }) => {
  await open(page);
  const cells = await page.evaluate(() => {
    const text = (row: Element, cls: string): string => row.querySelector(`.${cls}`)?.textContent ?? "";
    return [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => ({
      name: text(row, "name"),
      summary: row.dataset["kind"] !== "0",
      dur: text(row, "dur"),
      start: text(row, "start"),
      finish: text(row, "finish"),
      float: text(row, "float"),
    }));
  });
  expect(cells.length, "rows are drawn").toBeGreaterThan(10);
  const summaries = cells.filter((cell) => cell.summary);
  expect(summaries.length, "summaries are drawn").toBeGreaterThan(0);
  expect(summaries.filter((cell) => /\d+h|\d+m/.test(cell.dur)).map((cell) => `${cell.name}: ${cell.dur}`), "summary Dur with hours or minutes").toEqual([]);
  const offHour = cells.flatMap((cell) => [cell.start, cell.finish].filter((date) => /:(?!00\b)\d\d/.test(date)).map((date) => `${cell.name}: ${date}`));
  expect(offHour, "Start or Finish off the hour").toEqual([]);
  const fractions = cells.filter((cell) => /\.\d/.test(cell.float) && !/\.5\D*$/.test(cell.float.replace(/[a-z\s]+$/i, ""))).map((cell) => `${cell.name}: ${cell.float}`);
  expect(fractions, "Float with a fraction other than .5").toEqual([]);
});

test("a Dur cell's text does not spill into Start at 1280 px", async ({ page }) => {
  await open(page);
  const worst = await page.evaluate(() => {
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")];
    return rows
      .map((row) => {
        const dur = row.querySelector<HTMLElement>(".dur")!;
        const start = row.querySelector<HTMLElement>(".start")!;
        const range = document.createRange();
        range.selectNodeContents(dur);
        return { text: dur.textContent, over: dur.scrollWidth - dur.clientWidth, spill: range.getBoundingClientRect().right - start.getBoundingClientRect().left };
      })
      .sort((a, b) => b.over - a.over)[0]!;
  });
  expect(worst.over, `"${worst.text}" is cut or overflows by`).toBeLessThanOrEqual(0);
  expect(worst.spill, `"${worst.text}" reaches into Start by`).toBeLessThanOrEqual(0);
});

test("a Dur longer than its column is clipped with an ellipsis, as the Task column is", async ({ page }) => {
  await open(page);
  const seen = await page.evaluate(() => {
    const dur = document.querySelector<HTMLElement>("#table-rows .row .dur")!;
    dur.textContent = "9999d 7h 59m";
    const style = getComputedStyle(dur);
    return { overflow: style.overflowX, ellipsis: style.textOverflow, box: dur.getBoundingClientRect().right, start: dur.parentElement!.querySelector(".start")!.getBoundingClientRect().left };
  });
  expect(seen.overflow, "the Dur cell clips").toBe("hidden");
  expect(seen.ellipsis, "the Dur cell ends a long text with an ellipsis").toBe("ellipsis");
  expect(seen.box, "the Dur cell ends before Start").toBeLessThanOrEqual(seen.start);
});
