import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The in-cell text editor is named by its field and its task (gs#703), so a screen reader says what is being edited.
// Assertions read the accessible name through the role query, and the task's label through window.gantt.

const SIZE = 2000;

/** The first leaf row among the first rows: its name and duration accept the editor. */
const firstLeaf = (page: Page): Promise<{ row: number; task: number }> =>
  page.evaluate(() => {
    const rows = JSON.parse(window.gantt!.app.table_rows(0, 24)) as [number, string, string, number | null, boolean, number][];
    const row = rows.findIndex((r) => r[5] === 0);
    return { row, task: rows[row][0] };
  });

const label = (g: Gantt, task: number) => g.page.evaluate((t) => window.gantt!.app.task_label(t), task);

/** Click a cell to put the cursor on it, then open its editor with Enter. */
async function openEditor(g: Gantt, row: number, column: string): Promise<void> {
  await g.page.locator(`#table-rows .row[data-row="${row}"] > .${column}`).click();
  await g.nextFrame(2);
  await g.page.keyboard.press("Enter");
}

test("Enter on a name cell opens a box named Task name and the task", async ({ g }) => {
  await g.open({ n: SIZE });
  const leaf = await firstLeaf(g.page);
  await openEditor(g, leaf.row, "name");
  const box = g.page.getByRole("textbox", { name: /Task name/ });
  await expect(box).toBeVisible();
  await expect(box).toHaveAttribute("aria-label", `Task name: ${await label(g, leaf.task)}`);
});

test("Enter on a duration cell opens a box named Duration and the task", async ({ g }) => {
  await g.open({ n: SIZE });
  const leaf = await firstLeaf(g.page);
  await openEditor(g, leaf.row, "dur");
  const box = g.page.getByRole("textbox", { name: /Duration/ });
  await expect(box).toBeVisible();
  await expect(box).toHaveAttribute("aria-label", `Duration: ${await label(g, leaf.task)}`);
});
