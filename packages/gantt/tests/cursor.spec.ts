import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The cell cursor (gs#416, story S1 of #300): a click on a list cell selects its row and sets the column, the arrows move the
// cursor, Escape clears it. The row is the app's selection (`status().selected`); the column is the page's, read from the
// `data-cursor` mark on the cell. Every assertion reads chart state through window.gantt, never pixels. How the row moves (the
// first and last row, a collapsed summary, an edit and its undo, undo selecting and revealing its task) is checked on `Session`
// in crates/app/tests/nav.rs; this checks the keys, the clicks and the column.

// WebGL2 from the start: this file checks keys and clicks, not the GPU. A WebGPU device lost just after the page was ready reloaded
// it onto WebGL2 between a key and its check, and "End: the last column, same row" failed on main (gantt#1163).
test.use({ webgl: true });

const SIZE = 2000;

/** The task of every visible row, in table order. */
const visibleTasks = (page: Page): Promise<number[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    return (JSON.parse(w.app.table_rows(0, w.status().rowCount)) as number[][]).map((r) => r[0]);
  });

/** The cells that carry the cursor mark: the row's task and the cell's column class. */
const marks = (page: Page): Promise<{ task: number; column: string }[]> =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>('#table-rows [data-cursor="true"]')].map((cell) => ({
      task: Number(cell.closest<HTMLElement>(".row")!.dataset.task),
      column: cell.className,
    })),
  );

const cell = (g: Gantt, row: number, column: string) => g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`);

test("a click on a cell selects its row and puts the cursor on that column", async ({ g }) => {
  await g.open({ n: SIZE });
  const tasks = await visibleTasks(g.page);

  await cell(g, 3, "dur").click();
  await g.nextFrame(2);
  expect((await g.status()).selected?.task, "status().selected is the clicked row's task").toBe(tasks[3]);
  expect(await marks(g.page), "one cursor mark, on the duration cell").toEqual([{ task: tasks[3], column: "dur" }]);

  await cell(g, 5, "finish").click();
  await g.nextFrame(2);
  expect((await g.status()).selected?.task).toBe(tasks[5]);
  expect(await marks(g.page), "the mark moved with the click").toEqual([{ task: tasks[5], column: "finish" }]);
});

test("the arrows move the cursor one visible row or one column; Escape clears it", async ({ g }) => {
  await g.open({ n: SIZE });
  const tasks = await visibleTasks(g.page);
  await cell(g, 3, "name").click();
  await g.nextFrame(2);

  await g.page.keyboard.press("ArrowDown");
  await g.nextFrame(2);
  expect((await g.status()).selected?.task, "ArrowDown selects the next row").toBe(tasks[4]);
  expect(await marks(g.page), "the column stays").toEqual([{ task: tasks[4], column: "name" }]);

  await g.page.keyboard.press("ArrowUp");
  await g.page.keyboard.press("ArrowUp");
  await g.nextFrame(2);
  expect((await g.status()).selected?.task, "two ArrowUp select the row two above").toBe(tasks[2]);

  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  expect(await marks(g.page), "ArrowRight moves to the next column, on the same row").toEqual([{ task: tasks[2], column: "dur" }]);
  await g.page.keyboard.press("ArrowLeft");
  await g.page.keyboard.press("ArrowLeft");
  await g.page.keyboard.press("ArrowLeft");
  await g.nextFrame(2);
  const left = await marks(g.page);
  expect(left.length, "one mark").toBe(1);
  expect(left[0].column, "ArrowLeft stops at the first column the table has (the ID column when it shows)").toMatch(/^(code|name)$/);

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  expect((await g.status()).selected, "Escape clears the selection").toBeNull();
  expect(await marks(g.page), "and the cursor").toEqual([]);
});

test("the cursor survives an edit and its undo", async ({ g }) => {
  await g.open({ n: SIZE });
  const tasks = await visibleTasks(g.page);
  await cell(g, 4, "dur").click();
  await g.nextFrame(2);
  const before = await g.status();
  expect(before.selected?.task).toBe(tasks[4]);
  const name = await g.page.evaluate((t) => window.gantt!.app.task_label(t), tasks[4]);

  await g.page.evaluate((t) => window.gantt!.app.rename_task(t, "Cursor survives"), tasks[4]);
  await g.nextFrame(3);
  expect((await g.status()).revision, "the rename was an undo step").toBeGreaterThan(before.revision);
  await g.page.keyboard.press("Control+z");
  await g.nextFrame(3);
  expect(await g.page.evaluate((t) => window.gantt!.app.task_label(t), tasks[4]), "the undo ran: the old name is back").toBe(name);
  expect(await marks(g.page), "the cursor is on the same cell").toEqual([{ task: tasks[4], column: "dur" }]);
});

test("the arrows belong to the cell editor while a cell is edited", async ({ g }) => {
  await g.open({ n: SIZE });
  const tasks = await visibleTasks(g.page);
  await cell(g, 4, "name").dblclick();
  await g.nextFrame(2);
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeVisible();

  await g.page.keyboard.press("ArrowDown");
  await g.nextFrame(2);
  expect((await g.status()).selected?.task, "ArrowDown in the editor does not move the cursor").toBe(tasks[4]);
});

test("ArrowRight from the ID column reaches the WBS cell and then the task name", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.setInputFiles("#open-input", {
    name: "ledger-timeline.xer",
    mimeType: "application/octet-stream",
    buffer: readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url)),
  });
  await g.page.waitForFunction(() => document.getElementById("table")?.dataset.wbs === "true", undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  await cell(g, 1, "code").click();
  await g.nextFrame(2);
  expect(await marks(g.page), "the click puts the cursor on the ID cell").toMatchObject([{ column: "code" }]);

  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  expect(await marks(g.page), "ArrowRight from the ID reaches the WBS cell").toMatchObject([{ column: "wbs" }]);

  await g.page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  expect(await marks(g.page), "the next ArrowRight reaches the task name").toMatchObject([{ column: "name" }]);

  await g.page.keyboard.press("ArrowLeft");
  await g.nextFrame(2);
  expect(await marks(g.page), "ArrowLeft goes back to the WBS cell").toMatchObject([{ column: "wbs" }]);
});

// Home and End (gs#703): the cursor goes to the first and last column the table has for this file, on the same row. The first
// column is `code` while the file has IDs and `name` while it has none; the last is `float`, or `percent` while the table shows the
// progress columns (the XER fixture has actuals, so it does).
const FILES = [
  { label: "a file with IDs", path: "./fixtures/one-project.xer", first: "code" },
  { label: "a file with no IDs", path: "../../../crates/import/tests/fixtures/summary.xml", first: "name" },
];
for (const file of FILES) {
  test(`Home and End put the cursor on the first and last column of the same row (${file.label})`, async ({ g }) => {
    await g.open({ n: 300 });
    const buffer = readFileSync(new URL(file.path, import.meta.url));
    await g.page.setInputFiles("#open-input", { name: file.path.split("/").pop()!, mimeType: "application/octet-stream", buffer });
    await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
    await g.nextFrame(3);
    await g.page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
    const tasks = await visibleTasks(g.page);

    await cell(g, 2, "dur").click();
    await g.nextFrame(2);
    await g.page.keyboard.press("End");
    await g.nextFrame(2);
    const progress = await g.page.evaluate(() => document.getElementById("table")!.dataset.progress === "true");
    expect(progress, "the progress columns show for the XER file only").toBe(file.path.endsWith(".xer"));
    expect(await marks(g.page), "End: the last column, same row").toEqual([{ task: tasks[2], column: progress ? "percent" : "float" }]);
    await g.page.keyboard.press("Home");
    await g.nextFrame(2);
    expect(await marks(g.page), "Home: the first column, same row").toEqual([{ task: tasks[2], column: file.first }]);
    expect((await g.status()).selected?.task, "the row stays").toBe(tasks[2]);
  });
}
