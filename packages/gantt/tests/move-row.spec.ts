import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// Alt+Shift+Up and Alt+Shift+Down (gs#703) move the selected row among its siblings, its subtree with it; Ctrl+Z puts the order back.
// Chart state is read through window.gantt (the table's rows, outline levels, names), never pixels.

/** One visible row: its task, outline level and own name. */
interface Line {
  task: number;
  level: number;
  name: string;
}

const linesOf = (page: Page): Promise<Line[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    const rows = JSON.parse(w.app.table_rows(0, w.status().rowCount)) as number[][];
    return rows.map((r) => ({ task: r[0], level: w.app.task_outline_level(r[0]), name: w.app.task_name(r[0]).split(" / ").at(-1)! }));
  });

/** The row indexes of `at` and its siblings, in order. */
function siblingsOf(lines: Line[], at: number): number[] {
  const rows: number[] = [];
  let start = at;
  while (start > 0 && lines[start - 1].level >= lines[at].level) start -= 1;
  for (let k = start; k < lines.length && lines[k].level >= lines[at].level; k += 1) if (lines[k].level === lines[at].level) rows.push(k);
  return rows;
}

/** The row at `at` and the rows of its subtree. */
function subtreeOf(lines: Line[], at: number): Line[] {
  let end = at + 1;
  while (end < lines.length && lines[end].level > lines[at].level) end += 1;
  return lines.slice(at, end);
}

// Up moves the second of the siblings first; down moves the first second.
for (const [key, place] of [["Alt+Shift+ArrowUp", 1], ["Alt+Shift+ArrowDown", 0]] as const) {
  test(`${key} moves a summary among three or more siblings, with its subtree; Ctrl+Z restores the order`, async ({ g }) => {
    await g.open({ n: 2000 });
    await g.page.evaluate(() => window.gantt!.app.expand_all());
    const before = await linesOf(g.page);
    const at = before.findIndex((_, k) => subtreeOf(before, k).length > 1 && siblingsOf(before, k).length > 2 && siblingsOf(before, k).indexOf(k) === place);
    expect(at, "a summary with three or more siblings").toBeGreaterThanOrEqual(0);
    const names = siblingsOf(before, at).map((k) => before[k].name);
    await g.page.evaluate((t) => window.gantt!.app.select_task(t), before[at].task);
    await g.nextFrame(2);
    await g.page.keyboard.press(key);
    await g.nextFrame(2);
    const after = await linesOf(g.page);
    const now = after.findIndex((l) => l.task === before[at].task);
    expect(siblingsOf(after, now).map((k) => after[k].name), "the two swapped places").toEqual([names[1], names[0], ...names.slice(2)]);
    expect(subtreeOf(after, now), "its subtree moved with it").toEqual(subtreeOf(before, at));
    expect((await g.status()).selected?.task, "the selection follows the row").toBe(before[at].task);

    await g.page.keyboard.press("Control+z");
    await g.nextFrame(2);
    expect(await linesOf(g.page), "Ctrl+Z restores the order").toEqual(before);
  });
}

test("Alt+Shift+Up on the first row shows the refusal in the red line and changes nothing", async ({ g }) => {
  await g.open({ n: 2000 });
  const before = await linesOf(g.page);
  await g.page.evaluate((t) => window.gantt!.app.select_task(t), before[0].task);
  await g.nextFrame(2);
  const s0 = await g.status();

  await g.page.keyboard.press("Alt+Shift+ArrowUp");
  await expect(g.page.locator("#edit-error-text")).toHaveText("This row has no sibling above it.");
  expect((await g.status()).revision, "no undo step").toBe(s0.revision);
  expect(await linesOf(g.page), "no row moved").toEqual(before);
});
