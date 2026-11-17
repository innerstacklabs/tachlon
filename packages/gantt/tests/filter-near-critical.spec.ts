import { expect, openFilters, test } from "./support/gantt";

// The Near critical filter (gs#703): the button sits after Longest path in the Filters panel; pressing it keeps the tasks with at
// most ten working days of float (critical ones included) and the summaries above them. The float arithmetic is checked in
// crates/app/tests/filter_near_critical.rs; this checks the button, the status and the count.
const SIZE = 2000;

test("Near critical sets the filter to near, drops rows and counts in the Filters button", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = (await g.status()).rowCount;
  await openFilters(g.page);
  const near = g.page.locator("#near-critical-toggle");
  await expect(near).toHaveAttribute("aria-pressed", "false");
  // It follows Longest path in the panel.
  const ids = await g.page.locator("#filters-panel button").evaluateAll((els) => els.map((el) => el.id));
  expect(ids.indexOf("near-critical-toggle")).toBe(ids.indexOf("longest-toggle") + 1);

  await near.click();
  const on = await g.status();
  expect(on.filter).toBe("near");
  expect(on.rowCount).toBeLessThan(all);
  await expect(near).toHaveAttribute("aria-pressed", "true");
  await expect(g.page.locator("#filters-button")).toHaveText("Filters · 1");

  // One filter at a time: Critical only replaces it, and it holds at least the critical rows.
  await g.page.locator("#critical-toggle").click();
  const critical = await g.status();
  expect(critical.filter).toBe("critical");
  expect(critical.rowCount).toBeLessThanOrEqual(on.rowCount);
  await expect(near).toHaveAttribute("aria-pressed", "false");

  await near.click();
  await near.click();
  expect((await g.status()).filter).toBe("off");
  expect((await g.status()).rowCount).toBe(all);
});
