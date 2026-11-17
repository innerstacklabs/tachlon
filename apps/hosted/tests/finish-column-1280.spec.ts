import { expect, test } from "@playwright/test";
import { readSamplesJson } from "./samples-file";
import { waitReady, waitSampleSettled } from "./support/page";

// The Finish column at 1280 px (gs#880): the medium-5k sample, the one the demo opens for scale, shows every Finish cell in full inside the
// table's visible box without a sideways scroll, and a task name is cut only when it is longer than the width the table states for the
// Task column (the --gantt-task-fit token, or --gantt-task-min when none is set).
test.use({ viewport: { width: 1280, height: 800 } });

test("medium-5k at 1280 px: the Finish column is whole and names are cut only past the stated width", async ({ page }) => {
  expect(readSamplesJson<{ samples: { id: string }[] }>().samples.map((sample) => sample.id)).toContain("medium-5k");
  await page.goto("/?sample=medium-5k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelectorAll("#table-rows .row .finish").length > 5);
  // A P6 file opens with Progress on, which puts Finish after the progress columns, to be reached by scrolling (gs#974). This spec holds the
  // Progress-off layout: Finish whole in the visible box with no sideways scroll.
  await page.click("#progress-toggle");
  await expect(page.locator("#table")).not.toHaveAttribute("data-progress", /.*/);
  await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
  const seen = await page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    const box = body.getBoundingClientRect();
    const visibleRight = box.left + body.clientWidth;
    const finish = [...document.querySelectorAll<HTMLElement>("#table-rows .row .finish")];
    const head = [...document.querySelectorAll<HTMLElement>("#table-head .cols > span")].find((cell) => cell.textContent === "Finish")!;
    const names = [...document.querySelectorAll<HTMLElement>("#table-rows .row .name")];
    const probe = document.createElement("div");
    probe.style.cssText = "position:absolute;visibility:hidden;width:var(--gantt-task-fit, var(--gantt-task-min))";
    document.getElementById("table")!.append(probe);
    const stated = probe.getBoundingClientRect().width;
    probe.remove();
    const textWidth = (cell: HTMLElement): number => {
      const range = document.createRange();
      range.selectNodeContents(cell);
      return range.getBoundingClientRect().width;
    };
    return {
      scrollLeft: body.scrollLeft,
      finishCut: finish.filter((cell) => cell.scrollWidth > cell.clientWidth).length,
      finishOutside: finish.filter((cell) => cell.getBoundingClientRect().right > visibleRight + 0.5).length,
      headOutside: head.getBoundingClientRect().right - visibleRight,
      count: finish.length,
      stated,
      // A name cut although its text (indent included) fits inside the stated width.
      earlyCut: names.filter((cell) => cell.scrollWidth > cell.clientWidth && cell.getBoundingClientRect().width >= stated - 0.5 && textWidth(cell) + parseFloat(getComputedStyle(cell).paddingLeft) <= stated - 0.5).length,
      narrowName: names.filter((cell) => cell.getBoundingClientRect().width < stated - 0.5).length,
    };
  });
  expect(seen.count, "Finish cells are drawn").toBeGreaterThan(5);
  expect(seen.scrollLeft, "the table is at rest, not scrolled sideways").toBe(0);
  expect(seen.finishCut, "Finish cells cut").toBe(0);
  expect(seen.finishOutside, "Finish cells outside the table's visible box").toBe(0);
  expect(seen.headOutside, "the Finish head's right edge past the visible box by").toBeLessThanOrEqual(0.5);
  expect(seen.narrowName, "names narrower than the stated width").toBe(0);
  expect(seen.earlyCut, "names cut although they fit the stated width").toBe(0);
});

// A P6 file opens with Progress on (gs#974): % complete is whole and right after Task, the other columns follow it and the table scrolls to
// them, so Finish need not be whole. The chart keeps at least half the width.
test("medium-5k at 1280 px with Progress on: % complete is whole right after Task and the chart keeps half the width", async ({ page }) => {
  await page.goto("/?sample=medium-5k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelectorAll("#table-rows .row .percent").length > 5);
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  const seen = await page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    const table = document.getElementById("table")!.getBoundingClientRect();
    const head = (cls: string) => document.querySelector(`#table-head .${cls}`)!.getBoundingClientRect();
    const cells = [...document.querySelectorAll<HTMLElement>("#table-rows .row .percent")];
    return {
      scrollLeft: body.scrollLeft,
      visibleRight: body.getBoundingClientRect().left + body.clientWidth,
      chart: window.innerWidth - table.right,
      percentLeft: head("percent").left,
      percentRight: head("percent").right,
      taskRight: head("name").right,
      cellsOutside: cells.filter((cell) => cell.getBoundingClientRect().right > body.getBoundingClientRect().left + body.clientWidth + 0.5).length,
      cellsCut: cells.filter((cell) => cell.scrollWidth > cell.clientWidth).length,
    };
  });
  expect(seen.scrollLeft, "the table is at rest").toBe(0);
  expect(seen.chart, "the chart keeps half the width").toBeGreaterThanOrEqual(640);
  expect(seen.percentRight, "the % complete head is inside the visible box").toBeLessThanOrEqual(seen.visibleRight + 0.5);
  expect(seen.percentLeft, "% complete sits right after Task").toBeCloseTo(seen.taskRight + 6, 0);
  expect(seen.cellsOutside, "% complete cells outside the visible box").toBe(0);
  expect(seen.cellsCut, "% complete cells cut").toBe(0);
});
