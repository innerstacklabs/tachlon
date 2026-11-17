import type { Page } from "@playwright/test";
import { firstTopLevelSummary } from "./support/outline";
import { expect, test, type Gantt, openFilters } from "./support/gantt";

// Insert and Delete (gs#50 PR 4), and Mod+Enter and Mod+Backspace for the Mac (gs#418): the keys, the toolbar buttons, the refusal line, find re-run and the status line's
// task count. Every assertion reads chart state through window.gantt (status and the table's rows), never pixels.

const SIZE = 2000;
const ALERT = "#edit-error";
const FILTER_REFUSAL = "Clear the row filter or window to insert a row.";

type Row = [number, string, string, number | null, boolean, 0 | 1 | 2];

const rowsOf = (page: Page): Promise<Row[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    return JSON.parse(w.app.table_rows(0, w.status().rowCount)) as Row[];
  });

/** Select `task` by searching its outline path and stepping to the result that is that task. */
async function select(g: Gantt, task: number): Promise<void> {
  const ok = await g.page.evaluate((id) => {
    const app = window.gantt!.app;
    const found = app.find(app.task_details(id).outlinePath);
    let reached = false;
    for (let k = 0; k < found && !reached; k += 1) reached = app.find_go(k) === id;
    app.find("");
    return reached;
  }, task);
  expect(ok, `task ${task} was selected through find`).toBe(true);
  await g.nextFrame(2);
  expect((await g.status()).selected?.task).toBe(task);
}

/** The first leaf row at or after `from`: its row index and task. */
async function aLeaf(g: Gantt, from: number): Promise<{ row: number; task: number }> {
  const rows = await rowsOf(g.page);
  const row = rows.findIndex((r, at) => at >= from && r[5] === 0);
  expect(row, "a leaf row exists").toBeGreaterThanOrEqual(0);
  return { row, task: rows[row][0] };
}

const alertText = (g: Gantt) => g.page.locator("#edit-error-text");

// The engine side (where the row goes, its name and duration, the re-solve, what undo and redo
// restore, the refusals leaving the session unchanged) is in crates/app/tests/insert.rs and
// structure.rs. These tests check the buttons, alert line and find box that reach it; that each key (Insert, Delete,
// Mod+Enter, Mod+Backspace, the undo keys) reaches its command is checked once, in key-map.spec.ts.

test("Delete on a summary shows the refusal in the alert line", async ({ g }) => {
  await g.open({ n: SIZE });
  const s = await firstTopLevelSummary(g.page);
  await select(g, s.task!);
  await expect(g.page.locator(ALERT)).toBeHidden();
  await g.page.keyboard.press("Delete");
  await expect(g.page.locator(ALERT)).toBeVisible();
  await expect(g.page.locator(ALERT)).toHaveAttribute("role", "alert");
  await expect(alertText(g)).toContainText("is a summary with");
  await expect(alertText(g)).toContainText("Delete or move its sub-tasks first.");

  // The next successful edit clears the line.
  const leaf = await aLeaf(g, 6);
  await select(g, leaf.task);
  await g.page.keyboard.press("Insert");
  await expect(g.page.locator(ALERT)).toBeHidden();
});

test("Delete with nothing selected says so, and plain Backspace deletes no row", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.keyboard.press("Delete");
  await expect(alertText(g)).toHaveText("Select a row first.");
  await g.page.locator("#edit-error-close").click();
  await expect(g.page.locator(ALERT)).toBeHidden();

  const { task } = await aLeaf(g, 6);
  await select(g, task);
  const before = await g.status();
  await g.page.keyboard.press("Backspace");
  await g.nextFrame(2);
  const after = await g.status();
  expect(after.rowCount, "Backspace removed no row (gs#418, ruling R2)").toBe(before.rowCount);
  expect(after.revision, "and edited nothing").toBe(before.revision);
  expect((await rowsOf(g.page)).map((r) => r[0])).toContain(task);
  await expect(g.page.locator(ALERT), "and showed no refusal").toBeHidden();
});

test("Mod+Backspace deletes the selected row like Delete", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.keyboard.press("ControlOrMeta+Backspace");
  await expect(alertText(g), "the same refusal as Delete").toHaveText("Select a row first.");
  await g.page.locator("#edit-error-close").click();

  const { task } = await aLeaf(g, 6);
  await select(g, task);
  const before = await g.status();
  await g.page.keyboard.press("ControlOrMeta+Backspace");
  await g.nextFrame(2);
  expect((await g.status()).n).toBe(before.n - 1);
  expect((await rowsOf(g.page)).map((r) => r[0])).not.toContain(task);
});

test("Delete with a range of several rows selected says it works on one row and deletes nothing", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 6);
  await select(g, task);
  await g.page.keyboard.press("Shift+ArrowDown");
  await g.nextFrame(2);
  const before = await g.status();
  await g.page.keyboard.press("Delete");
  await expect(alertText(g)).toHaveText("Delete works on one row. Select a single row to delete it.");
  const after = await g.status();
  expect(after.n, "no task was deleted").toBe(before.n);
  expect((await rowsOf(g.page)).map((r) => r[0])).toContain(task);
});

test("Alt or Shift with Enter or Backspace does not insert or delete", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 6);
  await select(g, task);
  const before = await g.status();
  for (const key of ["Alt+Enter", "ControlOrMeta+Alt+Enter", "ControlOrMeta+Shift+Enter", "ControlOrMeta+Alt+Backspace", "ControlOrMeta+Shift+Backspace", "Enter"]) {
    await g.page.keyboard.press(key);
  }
  await g.nextFrame(2);
  const after = await g.status();
  expect(after.rowCount, "no row was added or removed").toBe(before.rowCount);
  expect(after.revision, "nothing was edited").toBe(before.revision);
});

test("Insert with Critical only on shows the filter refusal", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 0);
  await select(g, task);
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await g.nextFrame(2);
  const filtered = await g.status();
  expect(filtered.filter).toBe("critical");

  await g.page.locator("#app").focus();
  await g.page.keyboard.press("Insert");
  await expect(alertText(g)).toHaveText(FILTER_REFUSAL);

  // The button is refused the same way, and with the filter off the insert goes through.
  await g.page.locator("#insert-row").click();
  await expect(alertText(g)).toHaveText(FILTER_REFUSAL);
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await g.page.locator("#app").focus();
  if ((await g.status()).selected === null) await select(g, task);
  await g.page.locator("#insert-row").click();
  await g.nextFrame(2);
  await expect(g.page.locator(ALERT)).toBeHidden();
  expect((await g.status()).n).toBe(filtered.n + 1);
});

test("Delete, Insert, Mod+Enter and Mod+Backspace typed inside a cell editor change no row", async ({ g }) => {
  await g.open({ n: SIZE });
  const { row, task } = await aLeaf(g, 6);
  await select(g, task);
  const before = await g.status();
  await g.page.dblclick(`#table-rows .row[data-row="${row}"] .name`);
  const input = g.page.locator("#table-rows input.cell-edit");
  await expect(input).toBeVisible();
  for (const key of ["Delete", "Backspace", "Insert", "ControlOrMeta+Enter", "ControlOrMeta+Backspace"]) await g.page.keyboard.press(key);
  const after = await g.status();
  expect(after.rowCount, "no row was added or removed").toBe(before.rowCount);
  expect(after.n).toBe(before.n);
  expect(after.revision, "nothing was edited").toBe(before.revision);
  await expect(input, "the editor is still open").toBeVisible();
  await g.page.keyboard.press("Escape");
  // The same keys in the find box.
  await g.page.locator("#find-input").focus();
  for (const key of ["Delete", "Backspace", "Insert", "ControlOrMeta+Enter", "ControlOrMeta+Backspace"]) await g.page.keyboard.press(key);
  expect((await g.status()).n).toBe(before.n);
});

test("the buttons act on the selected row like the keys, and the status line follows the task count", async ({ g }) => {
  await g.open({ n: SIZE });
  const { row, task } = await aLeaf(g, 6);
  const fmt = new Intl.NumberFormat("en-US");
  await g.page.keyboard.press("s");
  const line = g.page.locator("#status-line");
  await expect(line).toBeVisible();
  await expect(line).toContainText(`${fmt.format(SIZE)} tasks`);

  await g.page.locator("#delete-row").click();
  await expect(alertText(g), "no selection: refused").toHaveText("Select a row first.");

  await select(g, task);
  await g.page.locator("#insert-row").click();
  await g.nextFrame(2);
  const inserted = await g.status();
  expect(inserted.n).toBe(SIZE + 1);
  expect(inserted.selected?.row).toBe(row + 1);
  await expect(g.page.locator(ALERT), "the success cleared the earlier refusal").toBeHidden();
  await expect(line).toContainText(`${fmt.format(SIZE + 1)} tasks`);

  await g.page.locator("#delete-row").click();
  await g.nextFrame(2);
  const deleted = await g.status();
  expect(deleted.n).toBe(SIZE);
  expect(deleted.rowCount).toBe(inserted.rowCount - 1);
  await expect(line).toContainText(`${fmt.format(SIZE)} tasks`);
});

test("find re-runs after an insert", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 6);
  await g.page.locator("#find-input").fill("New task");
  await expect(g.page.locator("#find-count")).toHaveText("no matches");
  await select(g, task);
  await g.page.locator("#insert-row").click();
  await expect(g.page.locator("#find-count")).toHaveText("1 of 1");
  expect((await g.status()).selected?.task).toBe(await g.page.evaluate(() => window.gantt!.app.find_go(0)));
});

/** A query that matches the leaf's ancestor (so also the leaf and its siblings) but whose first match is not the leaf. */
async function queryAbove(g: Gantt, task: number): Promise<string> {
  const rows = await rowsOf(g.page);
  const query = await g.page.evaluate((id) => window.gantt!.app.task_details(id).outlinePath, rows[0][0]);
  const inside = await g.page.evaluate(([id, q]) => window.gantt!.app.task_details(id as number).outlinePath.toLowerCase().includes((q as string).toLowerCase()), [task, query]);
  expect(inside, "the query matches the leaf, so an edit moves the match count").toBe(true);
  expect(rows[0][0], "the first match is the first row, not the edited row").not.toBe(task);
  return query;
}

/** The count the find box should read after an edit: the first match is current, so "1 of N". */
async function countAfter(g: Gantt, query: string): Promise<string> {
  const fmt = new Intl.NumberFormat("en-US");
  const found = await g.page.evaluate((q) => window.gantt!.app.find(q), query);
  expect(found).toBeGreaterThan(0);
  return `1 of ${fmt.format(found)}`;
}

test("Insert with a find query open keeps the new task selected and recounts the matches", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 6);
  const query = await queryAbove(g, task);
  await g.page.locator("#find-input").fill(query);
  const before = await g.page.locator("#find-count").textContent();
  await select(g, task);
  const rowsBefore = await rowsOf(g.page);
  await g.page.locator("#insert-row").click();
  await g.nextFrame(2);
  const rows = await rowsOf(g.page);
  const added = rows.map((r) => r[0]).find((id) => !rowsBefore.some((b) => b[0] === id))!;
  expect(added, "a row was added").toBeDefined();
  expect((await g.status()).selected?.task, "the new task stays selected").toBe(added);
  const expected = await countAfter(g, query);
  await expect(g.page.locator("#find-count"), "the count follows the new row").toHaveText(expected);
  expect(expected).not.toBe(before);
});

test("Delete with a find query open selects the next row's task and recounts the matches", async ({ g }) => {
  await g.open({ n: SIZE });
  const { task } = await aLeaf(g, 6);
  const query = await queryAbove(g, task);
  await g.page.locator("#find-input").fill(query);
  const before = await g.page.locator("#find-count").textContent();
  await select(g, task);
  const rowsBefore = await rowsOf(g.page);
  const at = rowsBefore.findIndex((r) => r[0] === task);
  await g.page.locator("#delete-row").click();
  await g.nextFrame(2);
  expect((await rowsOf(g.page)).map((r) => r[0]), "the task is gone").not.toContain(task);
  expect((await g.status()).selected?.task, "the next row's task is selected").toBe(rowsBefore[at + 1][0]);
  const expected = await countAfter(g, query);
  await expect(g.page.locator("#find-count"), "the count follows the deleted row").toHaveText(expected);
  expect(expected).not.toBe(before);
});

/** Select a leaf (by a click on its `column` cell, or through find), press Insert, type a name and press Enter: the new task's name afterwards. */
async function nameAfterInsert(g: Gantt, column?: string): Promise<string> {
  await g.open({ n: SIZE });
  const leaf = await aLeaf(g, 6);
  if (column === undefined) await select(g, leaf.task);
  else await g.page.locator(`#table-rows .row[data-row="${leaf.row}"] > .${column}`).click();
  await g.page.keyboard.press("Insert");
  await g.nextFrame(2);
  const added = (await g.status()).selected!.task;
  await g.page.keyboard.type("Pour footings");
  await g.page.keyboard.press("Enter");
  await g.nextFrame(2);
  return g.page.evaluate((id) => window.gantt!.app.task_details(id).name, added);
}

test("Insert, then typing, names the new row and Enter keeps it", async ({ g }) => {
  expect(await nameAfterInsert(g)).toBe("Pour footings");
});

test("Insert with the cursor on the duration cell still puts the typing in the new row's name", async ({ g }) => {
  expect(await nameAfterInsert(g, "dur")).toBe("Pour footings");
});
