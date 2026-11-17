import { readFileSync } from "node:fs";
import { expect, openFilters, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The Filters menu (gs#808): the filters sit behind one toolbar button that carries the count of filters on. The controls in the
// panel are the old ones (same ids, same handlers), so what they do to the chart is checked once here and in filters.spec.ts.
const SIZE = 2000;

// The ledger fixture with one activity code type and two values, so the code choices exist.
const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const CODED = Buffer.from(
  LEDGER.replace(
    /%E\s*$/,
    "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n%R\t7000\tPhase\n" +
      "%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n%R\t7100\t7000\tCivil\n%R\t7101\t7000\tMEP\n%E\n",
  ),
);

test("the button reads Filters with nothing on and Filters · 2 with two on", async ({ g }) => {
  await g.open({ n: SIZE });
  const button = g.page.locator("#filters-button");
  await expect(button).toHaveText("Filters");
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await expect(button).toHaveText("Filters · 1");
  await g.page.locator("#lookahead-toggle").click();
  await expect(button).toHaveText("Filters · 2");
  await g.page.locator("#critical-toggle").click();
  await g.page.locator("#lookahead-toggle").click();
  await expect(button).toHaveText("Filters");
});

test("Enter and Space on the button open the panel and put focus on its first item", async ({ g }) => {
  await g.open({ n: SIZE });
  const panel = g.page.locator("#filters-panel");
  await expect(panel).toBeHidden();
  for (const key of ["Enter", " "]) {
    await g.page.focus("#filters-button");
    await g.page.keyboard.press(key);
    await expect(panel, `${JSON.stringify(key)} opens it`).toBeVisible();
    await expect(g.page.locator("#filters-button")).toHaveAttribute("aria-expanded", "true");
    await expect(g.page.locator("#critical-toggle")).toBeFocused();
    await g.page.keyboard.press("Escape");
    await expect(panel).toBeHidden();
  }
});

test("the arrow keys move between the items", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.focus("#filters-button");
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#critical-toggle")).toBeFocused();
  await g.page.keyboard.press("ArrowDown");
  await expect(g.page.locator("#longest-toggle")).toBeFocused();
  await g.page.keyboard.press("ArrowDown");
  await expect(g.page.locator("#near-critical-toggle")).toBeFocused();
  await g.page.keyboard.press("ArrowDown");
  // Slipped needs a baseline and Show moved needs a move report: neither is offered, so the next item is Start constraint (the code choice and Status are not walked).
  await expect(g.page.locator("#start-constraint-toggle")).toBeFocused();
  await g.page.keyboard.press("ArrowDown");
  await expect(g.page.locator("#lookahead-toggle")).toBeFocused();
  await g.page.keyboard.press("ArrowDown");
  await expect(g.page.locator("#critical-toggle"), "the walk wraps round").toBeFocused();
  await g.page.keyboard.press("ArrowUp");
  await expect(g.page.locator("#lookahead-toggle")).toBeFocused();
  await g.page.keyboard.press("End");
  await expect(g.page.locator("#lookahead-toggle")).toBeFocused();
  await g.page.keyboard.press("Home");
  await expect(g.page.locator("#critical-toggle")).toBeFocused();
  // Nothing the menu took reached the chart's keys.
  expect((await g.status()).filter).toBe("off");
});

test("Escape closes the panel and focus returns to the button, and a press outside closes it too", async ({ g }) => {
  await g.open({ n: SIZE });
  const panel = g.page.locator("#filters-panel");
  await g.page.focus("#filters-button");
  await g.page.keyboard.press("Enter");
  await g.page.keyboard.press("ArrowDown");
  await expect(g.page.locator("#longest-toggle")).toBeFocused();
  await g.page.keyboard.press("Escape");
  await expect(panel).toBeHidden();
  await expect(g.page.locator("#filters-button")).toBeFocused();
  await expect(g.page.locator("#filters-button")).toHaveAttribute("aria-expanded", "false");

  await g.page.locator("#filters-button").click();
  await expect(panel).toBeVisible();
  await g.page.locator("#timeline").click({ position: { x: 20, y: 200 } });
  await expect(panel).toBeHidden();
});

test("a filter turned on in the menu changes the chart as the old control did, and the line still names it", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = await g.status();
  const count = g.page.locator("#row-count");
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await expect(g.page.locator("#critical-toggle")).toHaveAttribute("aria-pressed", "true");
  await g.nextFrame(3);
  const critical = await g.status();
  expect(critical.filter).toBe("critical");
  expect(critical.rowCount).toBeLessThan(all.rowCount);
  await expect(count).toContainText("Critical only");

  // Longest path replaces it (one row filter at a time), and Lookahead adds to it.
  await g.page.locator("#longest-toggle").click();
  await g.page.locator("#lookahead-toggle").click();
  await g.nextFrame(3);
  const both = await g.status();
  expect(both.filter).toBe("longest");
  expect(both.lookahead.on).toBe(true);
  await expect(count).toContainText("Longest path + Lookahead");
  await expect(g.page.locator("#filters-button")).toHaveText("Filters · 2");
});

test("Escape closes the panel from either activity code choice, and focus returns to the button", async ({ g }) => {
  await g.open({ n: 300 });
  await g.page.setInputFiles("#open-input", { name: "coded.xer", mimeType: "application/octet-stream", buffer: CODED });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const panel = g.page.locator("#filters-panel");
  for (const id of ["#code-type", "#code-value"]) {
    await openFilters(g.page);
    await g.page.selectOption("#code-type", "Phase");
    await g.page.focus(id);
    await g.page.keyboard.press("Escape");
    await expect(panel, `Escape on ${id}`).toBeHidden();
    await expect(g.page.locator("#filters-button")).toBeFocused();
  }
  expect((await g.status()).filter, "Escape changed no filter").toBe("off");
});

test("Tab out of the panel closes it", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.focus("#filters-button");
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#filters-panel")).toBeVisible();
  await g.page.focus("#insert-row");
  await expect(g.page.locator("#filters-panel")).toBeHidden();
});

test("Show moved is off until an edit moved tasks, then filters to the moved rows, and the count and the line name it", async ({ g }) => {
  await openLedger(g);
  const all = (await g.status()).rowCount;
  const item = g.page.locator("#moved-filter");
  await openFilters(g.page);
  await expect(item, "no move report yet").toBeDisabled();
  await g.page.keyboard.press("Escape");

  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  await openFilters(g.page);
  await expect(item).toBeEnabled();
  await expect(item).toHaveAttribute("aria-pressed", "false");
  await item.click();
  await g.nextFrame(3);
  const moved = await g.status();
  expect(moved.filter).toBe("moved");
  expect(moved.rowCount).toBeLessThan(all);
  await expect(item).toHaveAttribute("aria-pressed", "true");
  await expect(g.page.locator("#filters-button")).toHaveText("Filters · 1");
  await expect(g.page.locator("#row-count")).toContainText("Moved");

  // Another filter replaces it, and the item shows that at once.
  await g.page.locator("#critical-toggle").click();
  await expect(item).toHaveAttribute("aria-pressed", "false");
  await g.page.locator("#critical-toggle").click();
  await item.click();
  await item.click();
  expect((await g.status()).filter, "a second press turns it off").toBe("off");
});

test("a filter key turns Show moved off in the open panel: Shift+C and Shift+L", async ({ g }) => {
  await openLedger(g);
  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  // The edit leaves the cell cursor in the grid, which takes the plain keys; a press on the timeline puts it away.
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
  await openFilters(g.page);
  const item = g.page.locator("#moved-filter");
  for (const [key, filter] of [["Shift+C", "critical"], ["Shift+L", "longest"]] as const) {
    await item.click();
    await expect(item).toHaveAttribute("aria-pressed", "true");
    await g.page.keyboard.press(key);
    await g.page.waitForFunction((want) => window.gantt!.status().filter === want, filter);
    await expect(item, `${key} replaced Show moved`).toHaveAttribute("aria-pressed", "false");
    await expect(g.page.locator("#filters-panel"), "the panel stays open").toBeVisible();
  }
});

test("the panel opens with a hint line that says one filter at a time, and the arrow walk never lands on it", async ({ g }) => {
  await g.open({ n: SIZE });
  await openFilters(g.page);
  const panel = g.page.locator("#filters-panel");
  const hint = panel.locator("p", { hasText: "One filter at a time; Lookahead adds to it" });
  await expect(hint).toBeVisible();
  // It is the first thing in the panel, above the first filter.
  expect(await panel.evaluate((el) => el.firstElementChild?.tagName)).toBe("P");
  await g.page.focus("#critical-toggle");
  for (let i = 0; i < 6; i++) {
    await g.page.keyboard.press("ArrowDown");
    expect(await g.page.evaluate(() => document.activeElement?.tagName), "the walk stays on controls").not.toBe("P");
    expect(await g.page.evaluate(() => document.activeElement?.closest("p") === null)).toBe(true);
  }
  await g.page.keyboard.press("ArrowUp");
  await g.page.keyboard.press("Home");
  await expect(g.page.locator("#critical-toggle")).toBeFocused();
});

test("Clear filters turns the filter and Lookahead off and lists every row; it is last in the walk and hidden with nothing on", async ({ g }) => {
  await g.open({ n: SIZE });
  const clear = g.page.locator("#filters-clear");
  const full = (await g.status()).rowCount;
  await openFilters(g.page);
  await expect(clear, "nothing is on, so there is nothing to clear").toBeHidden();
  await g.page.locator("#critical-toggle").click();
  await g.page.locator("#lookahead-toggle").click();
  await expect(g.page.locator("#filters-button")).toHaveText("Filters · 2");
  await expect.poll(async () => (await g.status()).rowCount).toBeLessThan(full);
  await expect(clear).toBeVisible();
  await g.page.keyboard.press("End");
  await expect(clear, "the walk ends on it").toBeFocused();
  await g.page.keyboard.press("Enter");
  await expect(g.page.locator("#filters-button")).toHaveText("Filters");
  await expect(clear).toBeHidden();
  await expect.poll(async () => (await g.status()).rowCount).toBe(full);
  const s = await g.status();
  expect(s.filter).toBe("off");
  expect(s.lookahead.on).toBe(false);
  await expect(g.page.locator("#critical-toggle"), "focus stays in the panel").toBeFocused();
});
