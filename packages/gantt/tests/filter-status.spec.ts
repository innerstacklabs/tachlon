// The Status filter (gs#703): one choice in the Filters panel, after the activity code group, keeps the tasks in one state. The
// state rule is checked in crates/app/tests/status_filter.rs; this checks the choice, the rows, the status and the count.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";

// An MS Project file whose tasks carry no actuals: two summaries, each with leaves of eight working hours.
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));

const row = (page: Page, name: string): Locator => page.locator("#table-rows .row", { has: page.locator(".name", { hasText: name }) });

/** Double-click the Actual start cell of `r`, type `text` and press Enter, as the progress specs do. */
async function startOn(page: Page, r: Locator, text: string): Promise<void> {
  await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => done())))));
  const at = await r.evaluate((rowEl) => {
    const body = document.getElementById("table-body")!;
    const cell = rowEl.querySelector(":scope > .actual-start")!;
    const edge = body.getBoundingClientRect().right;
    if (cell.getBoundingClientRect().right > edge) body.scrollLeft += cell.getBoundingClientRect().right - edge + 8;
    const box = cell.getBoundingClientRect();
    const rowBox = rowEl.getBoundingClientRect();
    return { x: box.left + box.width / 2, y: rowBox.top + rowBox.height / 2 };
  });
  await page.mouse.dblclick(at.x, at.y);
  const input = r.locator(".actual-start input.cell-edit");
  await expect(input).toBeVisible();
  await input.fill(text);
  await input.press("Enter");
  await expect(r.locator(".actual-start")).not.toHaveText("");
}

test("choosing In progress narrows the rows to the started tasks and the Filters button shows one active", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.click("#progress-toggle");
  await page.waitForFunction(() => document.querySelector("#table-rows .row .actual-start") !== null);
  const all = (await g.status()).rowCount;
  await startOn(page, row(page, "First child"), "2026-10-05");

  await openFilters(page);
  const status = page.locator("#status-filter");
  // It follows the activity code group in the panel.
  const ids = await page.locator("#filters-panel > *").evaluateAll((els) => els.map((el) => el.id));
  expect(ids.indexOf("status-filter")).toBe(ids.indexOf("code-filter") + 1);

  await status.selectOption("in-progress");
  const on = await g.status();
  expect(on.filter).toBe("status");
  // The started task and the summaries above it.
  expect(on.rowCount).toBeGreaterThan(1);
  expect(on.rowCount).toBeLessThan(all);
  await expect(row(page, "First child")).toBeVisible();
  await expect(page.locator("#filters-button")).toHaveText("Filters · 1");

  // One filter at a time: Critical only replaces it, and the choice goes back to the empty entry.
  await openFilters(page);
  await page.locator("#critical-toggle").click();
  expect((await g.status()).filter).toBe("critical");
  await expect(status).toHaveValue("");

  await openFilters(page);
  await status.selectOption("not-started");
  expect((await g.status()).filter).toBe("status");
  await expect(row(page, "First child")).toHaveCount(0);

  await openFilters(page);
  await status.selectOption("");
  expect((await g.status()).filter).toBe("off");
  expect((await g.status()).rowCount).toBe(all);
  await expect(page.locator("#filters-button")).toHaveText("Filters");
});
