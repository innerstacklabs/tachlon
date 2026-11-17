import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// Progress at 1280 px (gs#974): the office fit-out carries actual dates and progress, so it opens with Progress on. As it opens the chart
// keeps at least half the width and % complete sits right after Task, inside the table pane, with no scroll and no divider drag.
test.use({ viewport: { width: 1280, height: 800 } });

test("the office fit-out opens with Progress on and % complete inside the table pane at 1280 px, the chart at half", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await page.waitForFunction(() => document.querySelector("#table-rows .row .percent") !== null);
  const seen = await page.evaluate(() => {
    const table = document.getElementById("table")!.getBoundingClientRect();
    return {
      chart: window.innerWidth - table.right,
      tableRight: table.right,
      percentRight: document.querySelector("#table-head .percent")!.getBoundingClientRect().right,
      scrollLeft: document.getElementById("table-body")!.scrollLeft,
    };
  });
  expect(seen.chart, "the chart keeps half the width as the file opens").toBeGreaterThanOrEqual(640);
  expect(seen.scrollLeft, "the table is not scrolled").toBe(0);
  expect(seen.percentRight, "the % complete head is inside the table pane as the file opens").toBeLessThanOrEqual(seen.tableRight);
});
