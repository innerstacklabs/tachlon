import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Where the keyboard is (gantt#1152): in the chart, in the grid, in a cell editor or in the link editor. The cell cursor takes every
// printable key while it is in the grid on a row that is drawn; a row hidden under a collapsed summary or by a filter, or a focus
// that left the chart, must hand the letter keys back to the chart. Each test puts the cursor in the grid, ends that one way, and
// presses Shift+C (critical rows only) or Shift+L (the longest path): a chart command whose state the app reports.

test.use({ webgl: true });

const SIZE = 2000;
const critical = (page: Page): Promise<boolean> => page.evaluate(() => window.gantt!.app.critical_only());
const longest = (page: Page): Promise<boolean> => page.evaluate(() => window.gantt!.app.longest_path());

/** Click the name cell of table row 3, so the cursor is in the grid on a row that is drawn. */
async function enterGrid(g: Gantt): Promise<number> {
  await g.open({ n: SIZE });
  await g.page.locator('#table-rows .row[data-row="3"] > .name').click();
  await g.nextFrame(2);
  const selected = (await g.status()).selected;
  expect(selected, "the click selected a row").not.toBeNull();
  return selected!.task;
}

test("a row hidden under a collapsed summary gives the letter keys back to the chart", async ({ g }) => {
  const task = await enterGrid(g);
  // While the row is drawn the cursor keeps the keys: a letter edits the cell and no chart command runs.
  await g.page.keyboard.press("Shift+C");
  await g.nextFrame(2);
  await expect(g.page.locator("#table-rows input.cell-edit")).toHaveValue("C");
  expect(await critical(g.page), "the chart's Shift+C did not run").toBe(false);
  await g.page.keyboard.press("Escape");
  await expect(g.page.locator("#table-rows input.cell-edit")).toHaveCount(0);
  const summary = await g.page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(0, 1)) as number[][])[0][0]);
  expect(await g.page.evaluate((t) => window.gantt!.app.toggle_task(t), summary), "the first summary collapsed").toBe(true);
  await g.nextFrame(3);
  const after = await g.status();
  expect(after.selected?.task, "the selection stays on its task").toBe(task);
  expect(after.selected!.row, "the task's row is not among the visible rows").toBeGreaterThanOrEqual(after.rowCount);
  await g.page.keyboard.press("Shift+C");
  await g.nextFrame(3);
  expect(await critical(g.page), "Shift+C is the chart's, not typed into a cell nobody can see").toBe(true);
});

test("a row a filter hides gives the letter keys back to the chart", async ({ g }) => {
  const task = await enterGrid(g);
  expect(await g.page.evaluate(() => window.gantt!.app.set_critical_only(true))).toBe(true);
  await g.nextFrame(3);
  const tasks = await g.page.evaluate(() => {
    const w = window.gantt!;
    return (JSON.parse(w.app.table_rows(0, w.status().rowCount)) as number[][]).map((r) => r[0]);
  });
  expect(tasks, "the filter hides the selected task's row").not.toContain(task);
  await g.page.keyboard.press("Shift+L");
  await g.nextFrame(3);
  expect(await longest(g.page), "Shift+L is the chart's").toBe(true);
});

test("focus that leaves the chart takes the cursor out of the grid", async ({ g }) => {
  await enterGrid(g);
  await g.page.evaluate(() => {
    const outside = document.createElement("input");
    outside.id = "outside-box";
    document.body.append(outside);
    outside.focus();
  });
  await g.page.focus("#app");
  await g.page.keyboard.press("Shift+C");
  await g.nextFrame(3);
  expect(await critical(g.page), "back in the chart, Shift+C is the chart's").toBe(true);
});
