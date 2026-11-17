import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Duration and Remaining (gs#703): a leaf's card shows Duration after Finish, and Remaining after it once the task
// has started; a summary shows neither, and a task not started shows no Remaining. The spec reads the card's DOM.

const labels = (page: Page): Promise<string[]> =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>("#hover-card .details > :nth-child(odd)")].map((el) => el.textContent ?? ""));

const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate(
    (name) => {
      const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
      const at = cells.findIndex((el) => el.textContent === name);
      return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
    },
    label,
  );

/** The first drawn row of `kind` (0 for a leaf, anything else for a summary): its row and task. */
const firstRow = (page: Page, leaf: boolean): Promise<{ row: number; task: number }> =>
  page.evaluate((wantLeaf) => {
    const el = document.querySelector<HTMLElement>(wantLeaf ? "#table-rows .row[data-kind='0']" : "#table-rows .row:not([data-kind='0'])");
    if (el === null) throw new Error("no such row");
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  }, leaf);

/** Move the pointer along the row until the card is for `task`. */
async function hoverTask(g: Gantt, row: number, task: number): Promise<void> {
  const { page } = g;
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  await page.mouse.move(canvas.x + 1, y);
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.task === task) {
      await expect(page.locator("#hover-card")).toBeVisible();
      return;
    }
  }
  throw new Error("no bar of the task under the pointer");
}

test("the_card_of_a_leaf_shows_its_duration_and_a_started_task_its_remaining", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 200 });
  const { row, task } = await firstRow(page, true);
  const duration = await page.evaluate((t) => window.gantt!.app.task_duration(t), task);
  expect(duration, "the table's Dur text").not.toBe("");

  await hoverTask(g, row, task);
  const before = await labels(page);
  expect(before).toContain("Duration");
  expect(before.indexOf("Duration"), "after Finish").toBe(before.indexOf("Finish") + 1);
  expect(await valueOf(page, "Duration")).toBe(duration);
  expect(before, "not started: no Remaining").not.toContain("Remaining");

  // Start it on its own scheduled day, so the bar stays where the sweep looks for it.
  await page.evaluate((t) => {
    const { start } = window.gantt!.app.task_details(t) as { start: string };
    window.gantt!.app.set_progress_field(t, "actual_start", start.slice(0, 10));
  }, task);
  await g.nextFrame(3);
  await hoverTask(g, row, task);
  const remaining = await page.evaluate((t) => window.gantt!.app.task_remaining(t), task);
  expect(remaining, "a started task holds a Remaining").not.toBe("");
  const after = await labels(page);
  expect(after.indexOf("Remaining"), "after Duration").toBe(after.indexOf("Duration") + 1);
  expect(await valueOf(page, "Remaining")).toBe(remaining);
});
