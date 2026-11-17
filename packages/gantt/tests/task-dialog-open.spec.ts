import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// A double-click opens the task dialog (gantt#1875): on a bar, and on a list cell with no editor of its own (the ID cell). A cell
// that has an editor still opens it. Real double-clicks; the app is only the oracle for the task's name.

// Clicks and a dialog, not the GPU: a WebGPU device lost on a loaded runner must not reload the page under the test (gantt#1163).
test.use({ webgl: true });

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const DIALOG = "#task-dialog";

/** The first leaf bar: its task, table row and middle in page CSS pixels. */
async function leafBar(g: Gantt): Promise<{ task: number; row: number; x: number; y: number }> {
  const box = await g.box();
  const status = await g.status();
  const bar = await g.page.evaluate(() => {
    const rows = JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as number[][];
    const row = rows.findIndex((r) => r[5] === 0);
    const task = rows[row][0];
    const rect = (window.gantt!.app.test_bar_rects() as { task: number; x: number; width: number }[]).find((r) => r.task === task)!;
    return { task, row, x: rect.x / window.devicePixelRatio, width: rect.width / window.devicePixelRatio };
  });
  return { task: bar.task, row: bar.row, x: box.x + bar.x + bar.width / 2, y: box.y + AXIS + bar.row * ROW - status.scrollY + ROW / 2 };
}

const name = (g: Gantt, task: number): Promise<string> => g.page.evaluate((t) => window.gantt!.app.task_label(t), task);

test("a double-click on a bar opens the dialog for that task", async ({ g }) => {
  await g.open({ n: 2000 });
  const bar = await leafBar(g);
  await g.page.mouse.move(bar.x, bar.y);
  await g.nextFrame(2);
  await g.page.mouse.dblclick(bar.x, bar.y);
  await expect(g.page.locator(DIALOG)).toBeVisible();
  await expect(g.page.locator(`${DIALOG} input[name=name]`)).toHaveValue(await name(g, bar.task));
  expect((await g.status()).gesture, "the press ended with the click").toBe("idle");
});

test("a double-click on the ID cell opens the dialog, and on the name cell still opens the cell editor", async ({ g }) => {
  await g.open({ n: 200 });
  await g.page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const row = g.page.locator("#table-rows .row").first();
  await expect(row.locator(".code")).toBeVisible();
  const task = Number(await row.getAttribute("data-task"));

  await row.locator(".name").dblclick();
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeVisible();
  await expect(g.page.locator(DIALOG)).toBeHidden();
  await g.page.keyboard.press("Escape");
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeHidden();

  await row.locator(".code").dblclick();
  await expect(g.page.locator(DIALOG)).toBeVisible();
  await expect(g.page.locator(`${DIALOG} input[name=name]`)).toHaveValue(await name(g, task));
  await expect(g.page.locator("#table-rows input.cell-edit")).toBeHidden();
});
