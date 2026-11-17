// The Check filter (gs#703): one choice in the Filters panel, after the Status choice, keeps the tasks one link check counts. What each
// check counts is checked in crates/app/tests/check_filter.rs; this checks the choice, the rows, the status and the row count.
//
// The file is `fixtures/links.xml`: the summary Phase over A -> C -> E and B -> D, so A and B have no predecessor, D and E no successor.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, openFilters, test } from "./support/gantt";

const LINKS = readFileSync(new URL("./fixtures/links.xml", import.meta.url));

/** The names of the rows the table shows now, in order. */
const names = async (page: Page): Promise<string[]> => (await page.locator("#table-rows .row .name").allTextContents()).map((t) => t.trim());

test("choosing Open starts keeps the tasks with nothing before them, and the Check choice follows the filter", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 50 });
  await page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: LINKS });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  const all = (await g.status()).rowCount;

  await openFilters(page);
  const check = page.locator("#check-filter");
  // It follows the Status choice in the panel.
  const ids = await page.locator("#filters-panel > *").evaluateAll((els) => els.map((el) => el.id));
  expect(ids.indexOf("check-filter")).toBe(ids.indexOf("status-filter") + 1);

  await check.selectOption("open-start");
  expect((await g.status()).filter).toBe("check");
  await expect.poll(() => names(page)).toEqual(["Phase", "A", "B"]);
  await expect(page.locator("#row-count")).toHaveText(`Check = Open starts · 3 of ${all} rows`);
  await expect(page.locator("#filters-button")).toHaveText("Filters · 1");

  await openFilters(page);
  await check.selectOption("open-end");
  await expect.poll(() => names(page)).toEqual(["Phase", "D", "E"]);

  await openFilters(page);
  await check.selectOption("missing-logic");
  await expect.poll(() => names(page)).toEqual(["Phase", "A", "B", "D", "E"]);

  // One filter at a time: Critical only replaces it, and the choice goes back to the empty entry.
  await openFilters(page);
  await page.locator("#critical-toggle").click();
  expect((await g.status()).filter).toBe("critical");
  await expect(check).toHaveValue("");

  await openFilters(page);
  await check.selectOption("open-start");
  expect((await g.status()).filter).toBe("check");
  await openFilters(page);
  await check.selectOption("");
  expect((await g.status()).filter).toBe("off");
  expect((await g.status()).rowCount).toBe(all);
  await expect(page.locator("#filters-button")).toHaveText("Filters");
});
