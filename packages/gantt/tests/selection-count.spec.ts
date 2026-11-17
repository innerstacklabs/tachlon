import { expect, test, type Gantt } from "./support/gantt";
import { firstTopLevelSummary } from "./support/outline";

// The toolbar says how many rows a range holds (gs#703): with two or more rows selected, `#selection-count` after the row
// count reads `5 rows selected`. It goes with Escape or with a single selection. It is a polite live region, and it is a
// separate span from `#row-count`, which the toolbar writes.

const SIZE = 2000;

const cell = (g: Gantt, row: number, column = "dur") => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);
const count = (g: Gantt) => g.page.locator("#selection-count");

test("a range says how many rows it holds", async ({ g }) => {
  await g.open({ n: SIZE });
  await expect(count(g), "a polite live region").toHaveAttribute("aria-live", "polite");
  await expect(count(g), "no selection, no text").toHaveText("");

  await cell(g, 3).click();
  for (let i = 0; i < 4; i++) await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  expect((await g.status()).range, "rows 3 to 7").toEqual([3, 7, 7]);
  await expect(count(g)).toHaveText("5 rows selected");

  await g.page.keyboard.press("Shift+ArrowUp");
  await g.nextFrame(2);
  await expect(count(g), "the count follows the range").toHaveText("4 rows selected");

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  expect((await g.status()).range).toBeNull();
  await expect(count(g), "Escape empties it").toHaveText("");

  await cell(g, 3).click();
  await g.nextFrame(2);
  await expect(count(g), "a single selection shows nothing").toHaveText("");
});

test("a live range shrunk to one row shows nothing", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 3).click();
  for (let i = 0; i < 2; i++) await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  await expect(count(g)).toHaveText("3 rows selected");

  for (let i = 0; i < 2; i++) await g.page.keyboard.press("Shift+ArrowUp");
  await g.nextFrame(2);
  const range = (await g.status()).range;
  expect(range === null || range[1] - range[0] + 1 < 2, "the head met the anchor: at most one row").toBe(true);
  await expect(count(g), "one row is a single selection").toHaveText("");
});

test("a collapsed summary inside the range counts as one row", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.expand_all());
  await g.nextFrame(2);
  const summary = await firstTopLevelSummary(g.page);
  expect(summary.hasSummary, "the demo has a top-level summary").toBe(true);
  await g.page.evaluate((t) => window.gantt!.app.toggle_task(t), summary.task!);
  await g.nextFrame(2);
  const at = summary.r0!;
  await cell(g, at, "name").click({ position: { x: 90, y: 6 } });
  for (let i = 0; i < 2; i++) await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  expect((await g.status()).range, "the collapsed summary and two rows after it").toEqual([at, at + 2, at + 2]);
  await expect(count(g), "the hidden children are not counted").toHaveText("3 rows selected");
});
