import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Paste (gs#703, P4): Ctrl+V with focus in the chart adds one row per line of plain text. The spec dispatches a `paste`
// ClipboardEvent carrying a DataTransfer on the focused chart, so it needs no clipboard permission.

const paste = (page: Page, text: string): Promise<boolean> =>
  page.evaluate((text) => {
    const data = new DataTransfer();
    data.setData("text/plain", text);
    const event = new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true });
    (document.activeElement ?? document.body).dispatchEvent(event);
    return event.defaultPrevented;
  }, text);

/** Every row's task, in table order. */
const tasks = (page: Page): Promise<number[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    return (JSON.parse(w.app.table_rows(0, w.status().rowCount)) as [number][]).map((row) => row[0]);
  });

const nameOf = (page: Page, task: number): Promise<string> => page.evaluate((id) => window.gantt!.app.task_name(id), task);

const open = async (g: Gantt): Promise<number> => {
  await g.open({ n: 2000 });
  await g.page.locator("#app").focus();
  return (await g.status()).rowCount;
};

test("pasting three lines adds three rows, selects and shows the first, and one Ctrl+Z removes them", async ({ g }) => {
  const before = await open(g);
  expect(await paste(g.page, "Pour slab\r\n\n  Cure slab \nStrip forms\n")).toBe(true);
  await g.nextFrame(3);
  const after = await tasks(g.page);
  expect(after).toHaveLength(before + 3);
  expect(await Promise.all(after.slice(-3).map((task) => nameOf(g.page, task)))).toEqual(["Pour slab", "Cure slab", "Strip forms"]);
  expect((await g.status()).selected?.task, "the first new row is selected").toBe(after[before]);
  await expect(g.page.locator(`#table-rows .row[data-task="${after[before]}"]`), "the table scrolled to it").toBeVisible();

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await tasks(g.page)).toHaveLength(before);
});

test("text with a tab, or no name, is left to the browser", async ({ g }) => {
  const before = await open(g);
  expect(await paste(g.page, "Task\tDur\nPour slab\t3d")).toBe(false);
  expect(await paste(g.page, " \n\n")).toBe(false);
  expect(await tasks(g.page)).toHaveLength(before);
});

test("more than 500 names are refused in the red line and add nothing", async ({ g }) => {
  const before = await open(g);
  await paste(g.page, Array.from({ length: 501 }, (_, i) => `Row ${i}`).join("\n"));
  await expect(g.page.locator("#edit-error-text")).toHaveText("Paste at most 500 rows at a time.");
  expect(await tasks(g.page)).toHaveLength(before);
});
