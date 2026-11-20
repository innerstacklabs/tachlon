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

// The last column (#155): at 800 px the Predecessors track must stay whole beside the frozen Task block when the table is scrolled to the end.
test("medium-5k at 800 px: the last column is whole beside the frozen block at the end of the scroll", async ({ page }) => {
  await page.setViewportSize({ width: 800, height: 800 });
  await page.goto("/?sample=medium-5k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await page.waitForFunction(() => document.querySelectorAll("#table-rows .row .finish").length > 5);
  await page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    body.scrollLeft = body.scrollWidth;
  });
  await page.waitForFunction(() => document.getElementById("table-head")!.scrollLeft === document.getElementById("table-body")!.scrollLeft);
  const seen = await page.evaluate(() => {
    const cells = [...document.querySelectorAll<HTMLElement>("#table-head .cols > *")].filter((cell) => getComputedStyle(cell).display !== "none");
    const last = cells.reduce((a, b) => (b.getBoundingClientRect().right > a.getBoundingClientRect().right ? b : a));
    const box = last.getBoundingClientRect();
    return {
      last: last.className,
      left: box.left,
      right: box.right,
      opacity: getComputedStyle(last).opacity,
      frozenRight: document.querySelector("#table-head .cols > .name")!.getBoundingClientRect().right,
      paneRight: document.getElementById("table")!.getBoundingClientRect().right,
    };
  });
  expect(seen.opacity, `the ${seen.last} head is not hidden as a cut column`).toBe("1");
  // 16 px is a classic vertical scrollbar: the scrolling width is the pane's less its scrollbar, so equality here hides the column there.
  expect(seen.left - seen.frozenRight, `the ${seen.last} head clears the frozen block by a scrollbar's width`).toBeGreaterThanOrEqual(16);
  expect(seen.right, `the ${seen.last} head is inside the pane`).toBeLessThanOrEqual(seen.paneRight);
});
