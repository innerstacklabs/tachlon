// gs#974: with Progress on, a fade at the table pane's right edge says more columns are to the right. It shows while the pane's scrollLeft is
// below its maximum, is gone at the maximum, and is never there with Progress off.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

/** Whether the table draws the fade at its right edge. */
const hint = (page: Page) =>
  page.evaluate(
    () =>
      new Promise<boolean>((done) =>
        requestAnimationFrame(() =>
          requestAnimationFrame(() => {
            const after = getComputedStyle(document.getElementById("table")!, "::after");
            done(after.content !== "none" && after.content !== "normal" && after.backgroundImage.includes("gradient"));
          }),
        ),
      ),
  );

const scrollTo = (page: Page, to: "start" | "end") =>
  page.evaluate((where) => {
    const body = document.getElementById("table-body")!;
    body.scrollLeft = where === "start" ? 0 : body.scrollWidth;
  }, to);

test("the table hints at columns to the right while Progress is on and the pane is not at its right end", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  // Progress off: nothing to the right to hint at, whatever the scroll.
  if ((await page.locator("#table").getAttribute("data-progress")) === "true") await page.click("#progress-toggle");
  await scrollTo(page, "start");
  expect(await hint(page), "Progress off, at the start").toBe(false);

  // Progress on, at the start: the Progress columns are out of view.
  await page.click("#progress-toggle");
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await scrollTo(page, "start");
  const room = await page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    return body.scrollWidth - body.clientWidth;
  });
  expect(room, "the Progress columns need more than the pane").toBeGreaterThan(0);
  expect(await hint(page), "Progress on, scrollLeft 0").toBe(true);

  // Part way along: still more to the right.
  await page.evaluate((half) => (document.getElementById("table-body")!.scrollLeft = half), Math.floor(room / 2));
  expect(await hint(page), "Progress on, part way").toBe(true);

  // At the maximum: nothing more to the right.
  await scrollTo(page, "end");
  expect(await hint(page), "Progress on, at the maximum").toBe(false);

  // Back off: the fade is gone from the start too.
  await scrollTo(page, "start");
  await page.click("#progress-toggle");
  await expect(page.locator("#table")).not.toHaveAttribute("data-progress", "true");
  expect(await hint(page), "Progress off again").toBe(false);
});
