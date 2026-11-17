// Windows forced colors (gantt#703): the browser replaces every background, so the toolbar's "on" states and the unsaved dot must show as a
// border (chrome.css, the forced-colors block). Playwright's `forcedColors: "active"` forces backgrounds the way Windows does, so the specs read
// computed styles: the background no longer tells the state, and the border width does. WebGL2 from the start: this file checks the toolbar.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";

test.use({ webgl: true });

interface Look {
  background: string;
  border: number;
  borderColor: string;
}

const look = (page: Page, selector: string): Promise<Look> =>
  page.locator(selector).evaluate((el) => {
    const style = getComputedStyle(el);
    return { background: style.backgroundColor, border: Number.parseFloat(style.borderTopWidth), borderColor: style.borderTopColor };
  });

test("in forced colors a pressed button, Filters with a filter on and an open Columns menu are told from idle by their border", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  await page.emulateMedia({ forcedColors: "active" });
  const idle = {
    progress: await look(page, "#progress-toggle"),
    filters: await look(page, "#filters-button"),
    columns: await look(page, "#columns-button"),
  };

  await page.locator("#progress-toggle").click();
  await expect(page.locator("#progress-toggle")).toHaveAttribute("aria-pressed", "true");
  await openFilters(page);
  await page.locator("#critical-toggle").click();
  await expect(page.locator("#filters-button")).toHaveAttribute("data-on", "true");
  await page.keyboard.press("Escape");
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-button")).toHaveAttribute("aria-expanded", "true");

  for (const [name, selector] of [["progress", "#progress-toggle"], ["filters", "#filters-button"], ["columns", "#columns-button"]] as const) {
    const on = await look(page, selector);
    expect.soft(on.background, `${name}: forced colors leave the background the same on or off`).toBe(idle[name].background);
    expect.soft(on.border, `${name}: on has a thicker border than idle`).toBeGreaterThan(idle[name].border);
  }
});

test("in forced colors the unsaved dot has a border the same size as its box, so it shows", async ({ g }) => {
  const { page } = g;
  page.on("dialog", (dialog) => void dialog.accept());
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", {
    name: "one-project.xer",
    mimeType: "application/octet-stream",
    buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)),
  });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Edited");
  await page.keyboard.press("Enter");
  await g.nextFrame(2);
  await page.emulateMedia({ forcedColors: "active" });

  const mark = page.locator("#unsaved-mark");
  await expect(mark).toBeVisible();
  const dot = await look(page, "#unsaved-mark");
  expect(dot.border, "the ring is 4 px, so it fills the 8 px dot").toBeGreaterThanOrEqual(4);
  expect(dot.borderColor, "the ring's colour is not the forced background").not.toBe(dot.background);
  const box = await mark.boundingBox();
  expect(box?.width, "the border does not make the dot bigger").toBe(8);
});
