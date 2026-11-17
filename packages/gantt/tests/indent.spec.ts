import type { Page } from "@playwright/test";
import { firstTopLevelSummary } from "./support/outline";
import { expect, test, type Gantt } from "./support/gantt";

// Indent and outdent keys (gs#422, story S5b of #300): Alt+Shift+Right indents the selected row under the row above it,
// Alt+Shift+Left outdents it and adopts the siblings below it (ruling R1). Playwright on Linux proves the handler; it does not
// prove the MacBook keyboard (Option+Shift+Right), which is pressed once by hand. Every assertion reads chart state through
// window.gantt (status, the table's rows, outline levels), never pixels.

const SIZE = 2000;
const ALERT = "#edit-error";

/** One visible row: its task, its outline level and its kind (leaf 0, expanded summary 1, collapsed summary 2). */
interface Line {
  task: number;
  level: number;
  kind: number;
}

const linesOf = (page: Page): Promise<Line[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    const rows = JSON.parse(w.app.table_rows(0, w.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    return rows.map((r) => ({ task: r[0], level: w.app.task_outline_level(r[0]), kind: r[5] }));
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

/** The index of the first row after `at` whose level is at most `level`: the end of the subtree of a row at `level`. */
const subtreeEnd = (lines: Line[], at: number, level: number): number => {
  let end = at + 1;
  while (end < lines.length && lines[end].level > level) end += 1;
  return end;
};

/** The row after `at` with the same level and the same parent, if the outline holds one. */
const nextSibling = (lines: Line[], at: number): number | null => {
  const end = subtreeEnd(lines, at, lines[at].level);
  return end < lines.length && lines[end].level === lines[at].level ? end : null;
};

/** The row before `at` with the same level and the same parent, if the outline holds one. */
const previousSibling = (lines: Line[], at: number): number | null => {
  for (let k = at - 1; k >= 0; k -= 1) {
    if (lines[k].level < lines[at].level) return null;
    if (lines[k].level === lines[at].level) return k;
  }
  return null;
};

/** A nested row with a following sibling that an outdent may adopt: a summary when there is one (so it has no link to refuse), else a leaf with no links. */
async function adopter(g: Gantt, lines: Line[]): Promise<number> {
  const candidates = lines.map((line, at) => ({ line, at })).filter(({ line, at }) => line.level >= 2 && nextSibling(lines, at) !== null);
  const linkFree = await g.page.evaluate(
    (tasks) => tasks.map((t) => {
      const d = window.gantt!.app.task_details(t);
      return d.predecessorCount + d.successorCount === 0;
    }),
    candidates.map(({ line }) => line.task),
  );
  const usable = candidates.filter(({ line }, k) => line.kind !== 0 || linkFree[k]);
  expect(usable.length, "a nested row with a following sibling").toBeGreaterThan(0);
  return (usable.find(({ line }) => line.kind === 1) ?? usable[0]).at;
}

test("Alt+Shift+Right indents a row under the row above; undo and redo follow", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.expand_all());
  const before = await linesOf(g.page);
  // A row whose previous sibling is a summary, so the indent cannot turn a leaf with a link into a summary.
  const at = before.findIndex((_, k) => {
    const p = previousSibling(before, k);
    return p !== null && before[p].kind === 1;
  });
  expect(at, "a row under a summary sibling").toBeGreaterThan(0);
  const parent = previousSibling(before, at)!;
  const task = before[at].task;
  await select(g, task);
  const s0 = await g.status();

  await g.page.keyboard.press("Alt+Shift+ArrowRight");
  await g.nextFrame(2);
  const after = await linesOf(g.page);
  const s1 = await g.status();
  expect(after.map((l) => l.task), "no row moved: it is already where the last child goes").toEqual(before.map((l) => l.task));
  expect(after[at].level, "one level deeper").toBe(before[at].level + 1);
  expect(subtreeEnd(after, parent, after[parent].level), "the summary above now holds the row and its subtree").toBe(subtreeEnd(before, at, before[at].level));
  expect(after[parent].kind, "the new parent shows open").toBe(1);
  expect(s1.selected?.task, "the selection follows the task").toBe(task);
  expect(s1.revision, "an undo step was recorded").toBeGreaterThan(s0.revision);
  await expect(g.page.locator(ALERT)).toBeHidden();

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await linesOf(g.page), "Ctrl+Z restores every level").toEqual(before);
  expect((await g.status()).selected?.task).toBe(task);

  await g.page.keyboard.press("Control+Shift+z");
  await g.nextFrame(2);
  expect(await linesOf(g.page), "Ctrl+Shift+Z indents again").toEqual(after);
  expect((await g.status()).selected?.task).toBe(task);
});

test("Alt+Shift+Right under a collapsed row opens it", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.expand_all());
  const before = await linesOf(g.page);
  const at = before.findIndex((_, k) => {
    const p = previousSibling(before, k);
    return p !== null && before[p].kind === 1;
  });
  expect(at).toBeGreaterThan(0);
  const parent = previousSibling(before, at)!;
  await g.page.evaluate((t) => window.gantt!.app.toggle_task(t), before[parent].task);
  await g.nextFrame(2);
  expect((await linesOf(g.page)).find((l) => l.task === before[parent].task)?.kind, "the summary is collapsed").toBe(2);
  await select(g, before[at].task);

  await g.page.keyboard.press("Alt+Shift+ArrowRight");
  await g.nextFrame(2);
  const after = await linesOf(g.page);
  expect(after.find((l) => l.task === before[parent].task)?.kind, "the new parent opened").toBe(1);
  expect(after.find((l) => l.task === before[at].task)?.level).toBe(before[at].level + 1);
});

test("Alt+Shift+Left outdents a row and adopts the siblings below it; undo hands them back", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.expand_all());
  const before = await linesOf(g.page);
  const at = await adopter(g, before);
  const task = before[at].task;
  const level = before[at].level;
  const parentEnd = subtreeEnd(before, at, level - 1);
  const followers = before.slice(subtreeEnd(before, at, level), parentEnd).filter((l) => l.level === level);
  expect(followers.length, "siblings below the row").toBeGreaterThan(0);
  expect(subtreeEnd(before, at, level), "the siblings are not under the row yet").toBeLessThan(parentEnd);
  await select(g, task);
  const s0 = await g.status();

  await g.page.keyboard.press("Alt+Shift+ArrowLeft");
  await g.nextFrame(2);
  const after = await linesOf(g.page);
  const s1 = await g.status();
  expect(after.map((l) => l.task), "preorder is unchanged: the row goes just after its parent's remaining children").toEqual(before.map((l) => l.task));
  expect(after[at].level, "one level up").toBe(level - 1);
  expect(subtreeEnd(after, at, level - 1), "the row now holds everything to the end of its old parent").toBe(parentEnd);
  for (const f of followers) {
    const row = after.findIndex((l) => l.task === f.task);
    expect(after[row].level, `sibling ${f.task} keeps its level and so is now a child of the row`).toBe(f.level);
    expect(row, `sibling ${f.task} sits inside the row's subtree`).toBeLessThan(subtreeEnd(after, at, level - 1));
  }
  expect(after[at].kind === 1 || after[at].kind === 2, "the row is a summary now").toBe(true);
  expect(s1.selected?.task, "the selection follows the task").toBe(task);
  expect(s1.revision, "an undo step was recorded").toBeGreaterThan(s0.revision);
  await expect(g.page.locator(ALERT)).toBeHidden();

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await linesOf(g.page), "Ctrl+Z gives the siblings back and restores every level").toEqual(before);
  expect((await g.status()).selected?.task).toBe(task);

  await g.page.keyboard.press("Control+Shift+z");
  await g.nextFrame(2);
  expect(await linesOf(g.page), "Ctrl+Shift+Z outdents and adopts again").toEqual(after);
  expect((await g.status()).selected?.task).toBe(task);
});

test("Alt+Shift+Left does not also collapse the summary", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.expand_all());
  const s = await firstTopLevelSummary(g.page);
  expect(s.hasSummary && s.task !== undefined).toBe(true);
  await select(g, s.task!);
  const rows0 = (await g.status()).rowCount;
  const kind = async (): Promise<number | undefined> => (await linesOf(g.page)).find((l) => l.task === s.task)?.kind;
  expect(await kind(), "an expanded summary").toBe(1);

  await g.page.keyboard.press("Alt+Shift+ArrowLeft");
  await g.nextFrame(2);
  expect(await kind(), "still expanded: the shifted key is not Alt+Left").toBe(1);
  expect((await g.status()).rowCount).toBe(rows0);
  await expect(g.page.locator("#edit-error-text")).toHaveText("This row is already at the top level.");

  // The control: the plain key still collapses the same row.
  await g.page.keyboard.press("Alt+ArrowLeft");
  await g.nextFrame(2);
  expect(await kind(), "Alt+Left still collapses").toBe(2);
});

test("Alt+Shift+Right on the first row shows the refusal in the red line and changes nothing", async ({ g }) => {
  await g.open({ n: SIZE });
  const before = await linesOf(g.page);
  await select(g, before[0].task);
  const s0 = await g.status();
  await expect(g.page.locator(ALERT)).toBeHidden();

  await g.page.keyboard.press("Alt+Shift+ArrowRight");
  await g.nextFrame(2);
  await expect(g.page.locator(ALERT)).toBeVisible();
  await expect(g.page.locator("#edit-error-text")).toHaveText("This row has no sibling above it.");
  const s1 = await g.status();
  expect(s1.revision, "no undo step").toBe(s0.revision);
  expect(s1.selected?.task).toBe(before[0].task);
  expect(await linesOf(g.page), "no row changed").toEqual(before);

  // The next successful edit clears the line.
  await g.page.keyboard.press("Insert");
  await g.nextFrame(2);
  await expect(g.page.locator(ALERT)).toBeHidden();
});
