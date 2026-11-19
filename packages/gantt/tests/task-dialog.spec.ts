import type { Page } from "@playwright/test";
import { fromTasks } from "../src/adapters/plain";
import { expect, test, type Gantt } from "./support/gantt";

// The task dialog (gantt#1875): Shift+Enter on the selected task opens its fields in one dialog. Save applies the changed ones as
// one undo step through `App.set_task_fields`; a refusal names the field and keeps the dialog open with nothing changed; Escape
// changes nothing. Real keys and clicks; the app is only the oracle for what the task holds.

// Keys and a dialog, not the GPU: a WebGPU device lost on a loaded runner must not reload the page under the test (gantt#1163).
test.use({ webgl: true });

const DIALOG = "#task-dialog";
const field = (page: Page, name: string) => page.locator(`${DIALOG} input[name=${name}]`);

/** The dialog is gone and the chart has focus again, so its keys (Ctrl+Z, F2) work. */
async function closed(g: Gantt): Promise<void> {
  await expect(g.page.locator(DIALOG)).toBeHidden();
  await expect(g.page.locator("#app")).toBeFocused();
}

/** The first leaf among the first rows, and what its name and duration hold now. */
async function leaf(g: Gantt): Promise<{ row: number; task: number }> {
  await g.open({ n: 200 });
  return g.page.evaluate(() => {
    const rows = JSON.parse(window.gantt!.app.table_rows(0, 24)) as [number, string, string, number | null, boolean, number][];
    const row = rows.findIndex((r) => r[5] === 0);
    return { row, task: rows[row][0] };
  });
}

const held = (page: Page, task: number) =>
  page.evaluate((t) => ({ name: window.gantt!.app.task_label(t), duration: window.gantt!.app.task_duration(t) }), task);

/** Click the name cell of `row` to select the task and put the cell cursor on it, then press Shift+Enter. */
async function openDialog(g: Gantt, row: number): Promise<void> {
  await g.page.locator(`#table-rows .row[data-row="${row}"] > .name`).click();
  await g.nextFrame(2);
  await g.page.keyboard.press("Shift+Enter");
  await expect(g.page.locator(DIALOG)).toBeVisible();
}

test("Shift+Enter edits two fields, Save applies both, and one Ctrl+Z reverts both", async ({ g }) => {
  const { row, task } = await leaf(g);
  const before = await held(g.page, task);
  await openDialog(g, row);
  await expect(field(g.page, "name")).toHaveValue(before.name);
  await expect(field(g.page, "duration")).toHaveValue(before.duration);

  await field(g.page, "name").fill("Renamed by the dialog");
  await field(g.page, "duration").fill("7d");
  await g.page.getByRole("button", { name: "Save" }).click();
  await closed(g);
  expect(await held(g.page, task)).toEqual({ name: "Renamed by the dialog", duration: "7d" });

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await held(g.page, task), "one undo puts back the name and the duration").toEqual(before);
});

test("a refused field keeps the dialog open, names the field and changes nothing", async ({ g }) => {
  const { row, task } = await leaf(g);
  const before = await held(g.page, task);
  await openDialog(g, row);

  await field(g.page, "name").fill("Never applied");
  await field(g.page, "duration").fill("not a duration");
  await g.page.getByRole("button", { name: "Save" }).click();
  await expect(g.page.locator(DIALOG)).toBeVisible();
  await expect(g.page.locator("#task-dialog-error")).toContainText("Duration:");
  expect(await held(g.page, task), "the name before the refused field is put back").toEqual(before);

  await field(g.page, "duration").fill("3d");
  await g.page.getByRole("button", { name: "Save" }).click();
  await closed(g);
  expect(await held(g.page, task)).toEqual({ name: "Never applied", duration: "3d" });
});

test("a date limit saves, undoes and refuses a bad day", async ({ g }) => {
  const { row, task } = await leaf(g);
  await openDialog(g, row);
  await field(g.page, "snlt").fill("2030-01-15");
  await g.page.getByRole("button", { name: "Save" }).click();
  await closed(g);
  expect(await g.page.evaluate((t) => window.gantt!.app.task_limit(t, "snlt"), task)).toBe("2030-01-15");
  await openDialog(g, row);
  await expect(field(g.page, "snlt"), "the dialog opens holding the saved day").toHaveValue("2030-01-15");
  await g.page.keyboard.press("Escape");
  await closed(g);

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await g.page.evaluate((t) => window.gantt!.app.task_limit(t, "snlt"), task)).toBe("");

  await openDialog(g, row);
  await field(g.page, "snlt").fill("not a day");
  await g.page.getByRole("button", { name: "Save" }).click();
  await expect(g.page.locator(DIALOG)).toBeVisible();
  await expect(g.page.locator("#task-dialog-error")).toContainText("Start no later than:");
});

test("Escape closes the dialog and changes nothing", async ({ g }) => {
  const { row, task } = await leaf(g);
  const before = await held(g.page, task);
  const revision = (await g.status()).revision;
  await openDialog(g, row);

  await field(g.page, "name").fill("Never applied");
  await g.page.keyboard.press("Escape");
  await closed(g);
  expect(await held(g.page, task)).toEqual(before);
  expect((await g.status()).revision).toBe(revision);
});

test("F2 still opens the cell editor after the dialog closes", async ({ g }) => {
  const { row } = await leaf(g);
  await openDialog(g, row);
  await g.page.keyboard.press("Escape");
  await closed(g);
  await g.page.keyboard.press("F2");
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeVisible();
  await expect(g.page.locator(DIALOG)).toBeHidden();
});

test("with IDs the dialog shows the Predecessors list, refuses a bad one with the reason, and Save makes the links", async ({ g }) => {
  await g.open({ n: 20 });
  const tasks = [1, 2, 3].map((id) => ({ id: String(id), name: `T${id}`, start: "2026-03-02", duration: 2 }));
  await g.page.evaluate((d) => window.gantt!.api!.load({ document: d }), fromTasks({ tasks, links: [{ from: "1", to: "3" }] }).document);
  await g.nextFrame(3);
  await openDialog(g, 2);
  await expect(field(g.page, "predecessors")).toHaveValue("1");
  const revision = (await g.status()).revision;
  await field(g.page, "predecessors").fill("9");
  await g.page.getByRole("button", { name: "Save" }).click();
  await expect(g.page.locator("#task-dialog-error")).toContainText("Predecessors:");
  expect((await g.status()).revision, "a refused list changes nothing").toBe(revision);
  await field(g.page, "predecessors").fill("1, 2SS+1d");
  await g.page.getByRole("button", { name: "Save" }).click();
  await closed(g);
  await expect(g.page.locator('#table-rows .row[data-row="2"] > .pred')).toHaveText("1, 2 SS+1d");
});

test("without Activity IDs the dialog has no Predecessors field", async ({ g }) => {
  await openDialog(g, (await leaf(g)).row);
  await expect(field(g.page, "predecessors")).toBeHidden();
});
