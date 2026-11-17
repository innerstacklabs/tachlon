import { expect, test, type Gantt } from "./support/gantt";

// Row jumps (gs#703): with focus in the chart, Ctrl+Home and Ctrl+End (Cmd+Up and Cmd+Down on a Mac) select the first and last
// visible row, and PageUp and PageDown move the selection one page of rows, the rows that fit in the table.

const SIZE = 2000;
const MIDDLE = 1000;

const cell = (g: Gantt, row: number) => g.page.locator(`#table-rows .row[data-row="${row}"] > .dur`);
const selectedRow = async (g: Gantt) => {
  await g.nextFrame(2);
  return (await g.status()).selected?.row;
};
/** The rows that fit in the table body, as the key reads them. */
const page = (g: Gantt) =>
  g.page.evaluate(() => {
    const body = document.getElementById("table-body");
    const first = document.querySelector<HTMLElement>("#table-rows .row");
    return Math.floor((body?.clientHeight ?? 0) / (first?.getBoundingClientRect().height ?? 1));
  });

test("Ctrl+End selects the last row, Ctrl+Home the first, PageDown moves by a page", async ({ g }) => {
  await g.open({ n: SIZE });
  // A row in the middle is selected, so the app's clamp applies (with nothing selected the first row is chosen).
  const rowHeight = (await g.status()).rowHeight;
  await g.page.evaluate((top) => document.getElementById("table-body")?.scrollTo(0, top), MIDDLE * rowHeight);
  await g.nextFrame(3);
  await cell(g, MIDDLE).click();
  expect(await selectedRow(g)).toBe(MIDDLE);
  const rowCount = (await g.status()).rowCount;

  await g.page.keyboard.press("Control+End");
  expect(await selectedRow(g)).toBe(rowCount - 1);
  await expect(g.page.locator(`#table-rows .row[data-row="${rowCount - 1}"]`)).toBeInViewport();

  await g.page.keyboard.press("Control+Home");
  expect(await selectedRow(g)).toBe(0);

  const rows = await page(g);
  expect(rows).toBeGreaterThan(1);
  await g.page.keyboard.press("PageDown");
  expect(await selectedRow(g)).toBe(rows);
  await g.page.keyboard.press("PageUp");
  expect(await selectedRow(g)).toBe(0);
});

test("typing in the Find box leaves the selection where it was", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 5).click();
  await g.page.keyboard.press("Control+f");
  await g.page.keyboard.press("PageDown");
  await g.page.keyboard.press("Control+End");
  expect(await selectedRow(g)).toBe(5);
});

test("with a cell editor open the row keys leave the selection where it was", async ({ g }) => {
  await g.open({ n: SIZE });
  await cell(g, 5).click();
  await g.page.keyboard.press("F2");
  const editor = g.page.locator("#table-rows input.cell-edit");
  await expect(editor).toBeVisible();

  for (const key of ["Home", "End", "PageUp", "PageDown", "Control+Home", "Control+End", "Meta+ArrowUp", "Meta+ArrowDown"]) {
    await g.page.keyboard.press(key);
    await expect(editor, `${key} leaves the editor open`).toBeVisible();
    expect(await selectedRow(g), `${key} leaves the selection`).toBe(5);
  }
});
