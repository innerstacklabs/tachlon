// A cell the frozen Task block cuts is hidden (gs#957): with the table scrolled to any offset, no head cell and no body cell is visible
// while it straddles the frozen right edge, and a cell hidden this way shows again once it clears the edge.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => done())))));

/** Open the fit-out with the Progress columns on. */
async function opened(page: Page, open: () => Promise<void>): Promise<void> {
  await open();
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  if ((await page.locator("#table").getAttribute("data-progress")) !== "true") await page.click("#progress-toggle");
}

/** Scroll #table-body to `left`, wait for the frames, and list the head and body cells that straddle the frozen edge while visible. */
const straddlers = (page: Page, left: number) =>
  page.evaluate(async (to) => {
    document.getElementById("table-body")!.scrollLeft = to;
    await new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done())));
    const edge = document.querySelector("#table-head .cols > .name")!.getBoundingClientRect().right;
    const cut: string[] = [];
    let hidden = 0;
    for (const cell of document.querySelectorAll<HTMLElement>("#table-head .cols > *, #table-rows .row > *")) {
      if (cell.matches(".name, .code, .wbs")) continue;
      const box = cell.getBoundingClientRect();
      if (!(box.left < edge && edge < box.right)) continue;
      if (getComputedStyle(cell).opacity !== "0") cut.push(`${cell.parentElement!.matches("#table-head .cols") ? "head" : "row"} .${cell.classList[0]}`);
      else hidden += 1;
    }
    return { at: document.getElementById("table-body")!.scrollLeft, cut, hidden };
  }, left);

test("no head or body cell straddles the frozen edge while visible, at any scroll offset", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await opened(page, () => g.open({ n: 200 }));
  const width = await page.evaluate(() => document.getElementById("table-body")!.scrollWidth);
  expect(width).toBeGreaterThan(0);
  let hiddenSteps = 0;
  for (let left = 0; left <= width; left += 7) {
    const r = await straddlers(page, left);
    expect(r.cut, `visible cells cut by the frozen edge at scrollLeft ${r.at}`).toEqual([]);
    if (r.hidden > 0) hiddenSteps += 1;
  }
  // The scan lands a column on the edge at least once, so the hiding was exercised, and the columns come back at the ends.
  expect(hiddenSteps).toBeGreaterThan(0);
  await straddlers(page, 0);
  await frames(page);
  expect(await page.locator("#table").getAttribute("data-edge-cut")).toBeNull();
  await expect(page.locator("#table-head .cols > .start")).toHaveCSS("opacity", "1");
});
