import { readFileSync } from "node:fs";
import { expect, openFilters, test } from "./support/gantt";

// The Start constraint filter (gs#703 K5): one button in the Filters panel keeps the tasks with a real start constraint. The rule
// is checked in crates/app/tests/constraint_filter.rs; this checks the button, the rows, the status and the count on the ledger file,
// where Activities 13, 26 and 39 carry a constraint above the import floor and the other tasks hold only the floor. Activity 26 has
// an actual start, which pins its early start (decision 0008), so it no longer holds one: the filter keeps 13 and 39.

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

test("the Start constraint button keeps the tasks with a constraint and drops those at their floor", async ({ page, g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const all = await g.status();

  await openFilters(page);
  const button = page.locator("#start-constraint-toggle");
  await expect(button).toHaveAttribute("aria-pressed", "false");
  await button.click();

  const on = await g.status();
  expect(on.filter).toBe("constraint");
  // The two leaves that still hold a constraint, and the summaries above them; every other task is at its floor or has started.
  expect(on.keptLeaves).toBe(2);
  expect(on.rowCount).toBeLessThan(all.rowCount);
  await expect(button).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#filters-button")).toHaveText("Filters · 1");
  const leaves = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")].map((r) => [...r.children].map((cell) => cell.textContent ?? "").find((text) => /^Activity \d+$/.test(text))),
  );
  expect(leaves).toEqual(["Activity 13", "Activity 39"]);

  // Pressed again, it turns the filter off and every row comes back.
  await button.click();
  expect((await g.status()).filter).toBe("off");
  expect((await g.status()).rowCount).toBe(all.rowCount);
  await expect(page.locator("#filters-button")).toHaveText("Filters");
});
