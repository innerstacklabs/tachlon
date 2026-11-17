import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// A P6 file at 1280 px (gs#866): the toolbar's data date, file line and Find placeholder show in full, and the task list leaves the chart at least half the width.
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** How much room each piece of toolbar text lacks, in pixels (at most 0 when it shows whole). */
const shortOf = (page: Page) =>
  page.evaluate(() => {
    const context = document.createElement("canvas").getContext("2d")!;
    const need = (el: HTMLElement, text: string): number => {
      const style = getComputedStyle(el);
      context.font = `${style.fontSize} ${style.fontFamily}`;
      const pad = parseFloat(style.paddingLeft) + parseFloat(style.paddingRight) + parseFloat(style.borderLeftWidth) + parseFloat(style.borderRightWidth);
      return context.measureText(text).width + pad - el.getBoundingClientRect().width;
    };
    const byId = (id: string) => document.getElementById(id)!;
    const find = byId("find-input") as HTMLInputElement;
    return {
      dataDate: byId("data-date").scrollWidth - byId("data-date").clientWidth,
      file: byId("open-file").scrollWidth - byId("open-file").clientWidth,
      find: need(find, find.placeholder),
      chart: document.getElementById("timeline")!.getBoundingClientRect().width / window.innerWidth,
      dateText: byId("data-date").textContent,
    };
  });

test("at 1280 px a P6 file leaves the toolbar text whole and the chart half the width", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: LEDGER });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const seen = await shortOf(g.page);
  expect.soft(seen.dateText, "the data date is shown").not.toBe("");
  expect.soft(seen.dataDate, "the data date is cut by").toBeLessThanOrEqual(0);
  expect.soft(seen.file, "the file line is cut by").toBeLessThanOrEqual(0);
  expect.soft(seen.find, "the Find placeholder is cut by").toBeLessThanOrEqual(0);
  expect.soft(seen.chart, "the chart's share of the width").toBeGreaterThanOrEqual(0.5);
  // Critical only adds the filter's name to the row count; it reads whole too.
  await g.page.locator("#critical-toggle").evaluate((el) => (el as HTMLElement).click());
  await g.nextFrame(3);
  const count = await g.page.locator("#row-count").evaluate((el) => ({ text: el.textContent, cut: el.scrollWidth - el.clientWidth }));
  expect.soft(count.text, "the filter names itself in the row count").toMatch(/^Critical only · \d+ of \d+ activities$/);
  expect.soft(count.cut, "the row count is cut by").toBeLessThanOrEqual(0);
});
