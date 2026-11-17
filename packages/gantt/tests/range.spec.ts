import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Range selection (gs#423, story S2 of #300): Shift+Down, Shift+click and Shift+Space reach the app's range, and the table tints
// its rows. The range is the app's (`status().range` is `[first, last, extent]` as table rows); the tint is the `data-range` mark
// on the drawn rows. How the range moves, ends and follows a collapse is checked on `Session` in crates/app/tests/range.rs; this
// checks the keys, the click and the tint.

const SIZE = 2000;

/** The rows that carry the range tint, in table order. */
const tinted = (page: Page): Promise<number[]> =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('#table-rows .row[data-range="true"]')].map((row) => Number(row.dataset.row)),
  );

const cell = (g: Gantt, row: number, column = "dur") => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

const press = async (g: Gantt, key: string, times = 1): Promise<void> => {
  for (let i = 0; i < times; i++) await g.page.keyboard.press(key);
  await g.nextFrame(2);
};

test("Shift+Down moves the range's other end and tints its rows", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 3).click();
  await g.nextFrame(2);
  expect((await g.status()).range, "a click is a selection, not a range").toBeNull();

  await press(g, "Shift+ArrowDown", 2);
  expect((await g.status()).range, "two steps down: the range is rows 3 to 5, the extent on 5").toEqual([3, 5, 5]);
  expect(await tinted(g.page), "the table tints exactly those rows").toEqual([3, 4, 5]);
});

test("Shift+click extends the range to the clicked row", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 2).click();
  await g.nextFrame(2);

  await cell(g, 6, "finish").click({ modifiers: ["Shift"] });
  await g.nextFrame(2);
  expect((await g.status()).range, "rows 2 to 6, the extent on the clicked row").toEqual([2, 6, 6]);
  expect(await tinted(g.page)).toEqual([2, 3, 4, 5, 6]);
});

test("Shift+Space makes the selected row a range of its own row", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 3).click();
  await g.nextFrame(2);

  await press(g, "Shift+Space");
  expect((await g.status()).range, "one row").toEqual([3, 3, 3]);
  expect(await tinted(g.page)).toEqual([3]);
});
