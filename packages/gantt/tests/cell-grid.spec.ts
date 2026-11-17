import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// Edit from the cell cursor (gs#421, story S3 of #300), with ruling R2 (Backspace opens the editor empty). The cursor is a click
// or an arrow in the list; Enter and F2 edit the cell with its old text, a printable key opens the editor holding it, Tab and
// Shift+Tab commit and move across, Enter commits and moves down, Escape discards, and the chart's letter keys are suspended
// while the cursor is in the grid. Assertions read chart state through window.gantt, never pixels.

const SIZE = 2000;

interface Row {
  row: number;
  task: number;
  critical: boolean;
}

/** The leaf rows among the first rows, in table order: the rows whose name and duration the editor accepts. */
const leafRows = (page: Page): Promise<Row[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    const rows = JSON.parse(w.app.table_rows(0, 24)) as [number, string, string, number | null, boolean, number][];
    return rows.flatMap((r, row) => (r[5] === 0 ? [{ row, task: r[0], critical: r[4] }] : []));
  });

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
const editor = (g: Gantt) => g.page.locator("#table-rows input.cell-edit");
/** Where the view is: the scroll and the pan. `c` moves both to the next critical task; it selects nothing. */
const view = (g: Gantt) => g.page.evaluate(() => [window.gantt!.status().scrollY, window.gantt!.app.x_of(0)]);
const label = (g: Gantt, task: number) => g.page.evaluate((t) => window.gantt!.app.task_label(t), task);
const duration = (g: Gantt, task: number) => g.page.evaluate((t) => window.gantt!.app.task_duration(t), task);

/** Click a cell to put the cursor on it, and wait for the frames that draw the mark. */
async function clickCell(g: Gantt, row: number, column: string): Promise<void> {
  await cell(g, row, column).click();
  await g.nextFrame(2);
}

test("typing 5d on a duration cell opens the editor holding it and Tab sets 5 days", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  const tasks = await visibleTasks(g.page);
  const before = await duration(g, leaf.task);
  const typed = before === "5d" ? "6d" : "5d";
  await clickCell(g, leaf.row, "dur");
  const revision = (await g.status()).revision;

  await g.page.keyboard.type(typed);
  await expect(editor(g), "the first key opened the editor").toHaveValue(typed);
  expect(await editor(g).getAttribute("data-field"), "on the duration").toBe("dur");
  expect(await duration(g, leaf.task), "nothing changes until the commit").toBe(before);
  expect((await g.status()).revision).toBe(revision);

  await g.page.keyboard.press("Tab");
  await g.nextFrame(2);
  expect(await duration(g, leaf.task), "Tab committed the typed duration").toBe(typed);
  expect((await g.status()).revision, "as one undo step").toBe(revision + 1);
  await expect(editor(g)).toHaveCount(0);
  expect(await marks(g.page), "and moved the cursor to the next row's name").toEqual([{ task: tasks[leaf.row + 1], column: "name" }]);

  await g.page.keyboard.press("Control+z");
  await g.nextFrame(2);
  expect(await duration(g, leaf.task), "one undo brings the old duration back").toBe(before);
});

test("the chart's letter keys are suspended while the cursor is in the grid, and return with Escape", async ({ g }) => {
  await g.open({ n: SIZE });
  const leaf = (await leafRows(g.page)).find((r) => !r.critical)!;
  const rowCount = (await g.status()).rowCount;
  // The float column cannot be edited: a letter key there has no editor to go to, so only the suspension keeps it off the chart.
  await clickCell(g, leaf.row, "float");
  expect((await g.status()).selected?.task).toBe(leaf.task);
  const viewBefore = await view(g);

  for (const key of ["c", "C", "L", "s", "1", "2", "0", "[", "]"]) await g.page.keyboard.press(key);
  await g.nextFrame(2);
  const after = await g.status();
  expect(after.selected?.task, "the same task is selected").toBe(leaf.task);
  expect(await view(g), "c jumped nowhere: the view did not move").toEqual(viewBefore);
  expect(after.rowCount, "1 and 2 collapsed nothing").toBe(rowCount);
  await expect(editor(g), "no editor opened on a cell that cannot be edited").toHaveCount(0);
  expect(await g.page.locator("#status-line").isHidden(), "s did not show the status line").toBe(true);

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  expect((await g.status()).selected, "Escape cleared the selection").toBeNull();
  await g.page.keyboard.press("c");
  await g.nextFrame(2);
  expect(await view(g), "and c is the chart's again: the view jumped to a critical task").not.toEqual(viewBefore);
  await g.page.keyboard.press("1");
  await g.nextFrame(2);
  expect((await g.status()).rowCount, "1 collapses to level 1 again").toBeLessThan(rowCount);
});

test("a click on the timeline puts the cursor away and the letter keys return", async ({ g }) => {
  await g.open({ n: SIZE });
  const leaf = (await leafRows(g.page)).find((r) => r.row < 15)!;
  const rowCount = (await g.status()).rowCount;
  await clickCell(g, leaf.row, "name");
  await g.page.keyboard.press("1");
  await g.nextFrame(2);
  expect((await g.status()).rowCount, "in the grid, 1 collapses nothing").toBe(rowCount);

  const box = await g.box();
  const start = await g.page.evaluate((r) => window.gantt!.app.row_start(r), leaf.row);
  const x = await g.page.evaluate((s) => window.gantt!.app.x_of(s!), start);
  const scrollY = (await g.status()).scrollY;
  await g.page.mouse.click(box.x + x + 3, box.y + AXIS + leaf.row * ROW - scrollY + ROW / 2);
  await g.nextFrame(2);
  expect((await g.status()).selected?.task, "the click selected the bar's task: the cursor still shows").toBe(leaf.task);
  await g.page.keyboard.press("1");
  await g.nextFrame(2);
  expect((await g.status()).rowCount, "1 collapses to level 1 again").toBeLessThan(rowCount);
});

test("a printable key on a name opens the editor holding it, not the chart's command; Escape leaves the old name", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  const name = await label(g, leaf.task);
  await clickCell(g, leaf.row, "name");
  const revision = (await g.status()).revision;
  const viewBefore = await view(g);

  await g.page.keyboard.press("c");
  await expect(editor(g), "c opened the editor holding c").toHaveValue("c");
  expect((await g.status()).selected?.task, "c did not move the selection").toBe(leaf.task);
  expect(await view(g), "or the view").toEqual(viewBefore);
  await g.page.keyboard.type("hello");
  await expect(editor(g)).toHaveValue("chello");

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(editor(g)).toHaveCount(0);
  expect(await label(g, leaf.task), "Escape left the old name").toBe(name);
  expect(await cell(g, leaf.row, "name").textContent(), "and the cell shows it").toContain(name);
  expect((await g.status()).revision).toBe(revision);
  expect((await g.status()).selected?.task, "Escape in the editor leaves the cursor where it was").toBe(leaf.task);
  expect(await marks(g.page)).toEqual([{ task: leaf.task, column: "name" }]);
});

test("a refused duration keeps the editor open on Tab and moves nothing", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  const before = await duration(g, leaf.task);
  await clickCell(g, leaf.row, "dur");
  const revision = (await g.status()).revision;

  await g.page.keyboard.type("abc");
  await g.page.keyboard.press("Tab");
  await g.nextFrame(2);
  await expect(editor(g), "the editor is still open").toHaveValue("abc");
  await expect(editor(g)).toHaveClass(/invalid/);
  await expect(g.page.locator("#table-rows .cell-error"), "with the reason").not.toHaveText("");
  expect(await g.page.evaluate(() => document.activeElement?.className), "and holds focus").toBe("cell-edit invalid");
  expect(await duration(g, leaf.task)).toBe(before);
  expect((await g.status()).revision).toBe(revision);
  expect((await g.status()).selected?.task, "the cursor did not move").toBe(leaf.task);
  expect(await marks(g.page)).toEqual([{ task: leaf.task, column: "dur" }]);

  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(editor(g)).toHaveCount(0);
  expect(await duration(g, leaf.task), "Escape discards what was typed").toBe(before);
});

test("Enter and F2 edit with the old text; Enter commits and moves down; Tab and Shift+Tab move across", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  const tasks = await visibleTasks(g.page);
  const name = await label(g, leaf.task);
  await clickCell(g, leaf.row, "name");

  await g.page.keyboard.press("F2");
  await expect(editor(g), "F2 opens the editor with the old name").toHaveValue(name);
  await g.page.keyboard.press("Escape");
  await g.page.keyboard.press("Enter");
  await expect(editor(g), "Enter does too").toHaveValue(name);
  await g.page.keyboard.press("Control+a");
  await g.page.keyboard.type("Typed name");
  const revision = (await g.status()).revision;
  await g.page.keyboard.press("Enter");
  await g.nextFrame(2);
  expect(await label(g, leaf.task), "Enter committed the name").toBe("Typed name");
  expect((await g.status()).revision, "as one undo step").toBe(revision + 1);
  expect(await marks(g.page), "and moved the cursor down, in the same column").toEqual([{ task: tasks[leaf.row + 1], column: "name" }]);

  // Unchanged text: Tab and Shift+Tab only move. name -> dur, dur -> the next row's name.
  const next = tasks[leaf.row + 1];
  await g.page.keyboard.press("F2");
  await g.page.keyboard.press("Tab");
  await g.nextFrame(2);
  expect(await marks(g.page), "Tab from a name goes to the duration of the same row").toEqual([{ task: next, column: "dur" }]);
  await g.page.keyboard.press("F2");
  await g.page.keyboard.press("Tab");
  await g.nextFrame(2);
  expect(await marks(g.page), "Tab from a duration goes to the next row's name").toEqual([{ task: tasks[leaf.row + 2], column: "name" }]);
  await g.page.keyboard.press("F2");
  await g.page.keyboard.press("Shift+Tab");
  await g.nextFrame(2);
  expect(await marks(g.page), "Shift+Tab from a name goes to the previous row's duration").toEqual([{ task: next, column: "dur" }]);
  await g.page.keyboard.press("Enter");
  await g.page.keyboard.press("Shift+Tab");
  await g.nextFrame(2);
  expect(await marks(g.page), "Shift+Tab from a duration goes to the name of the same row").toEqual([{ task: next, column: "name" }]);
  expect((await g.status()).revision, "none of those changed the schedule").toBe(revision + 1);
});

test("Backspace on a cell with the cursor opens the editor empty, and Escape restores the old value (R2)", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  const name = await label(g, leaf.task);
  const before = await duration(g, leaf.task);
  const state = await g.status();

  await clickCell(g, leaf.row, "name");
  await g.page.keyboard.press("Backspace");
  await expect(editor(g), "the editor is empty").toHaveValue("");
  expect((await g.status()).rowCount, "Backspace deleted no row").toBe(state.rowCount);
  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await expect(editor(g)).toHaveCount(0);
  expect(await label(g, leaf.task), "Escape restored the old name").toBe(name);
  expect(await cell(g, leaf.row, "name").textContent()).toContain(name);

  await g.page.keyboard.press("ArrowRight");
  await g.page.keyboard.press("Backspace");
  await expect(editor(g), "on a duration too").toHaveValue("");
  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);
  expect(await duration(g, leaf.task)).toBe(before);
  expect((await g.status()).revision, "nothing was an edit").toBe(state.revision);
  expect((await g.status()).rowCount).toBe(state.rowCount);
});

/**
 * The cursor drops a key that waits for its row more than 1 s (performance.now), so on a busy machine a real frame can outlast the wait and the
 * key is gone. These specs open on the fake clock, so every requestAnimationFrame is the clock's (a real frame already pending when the clock is
 * installed would never wake the loop), then freeze it and draw the frames by hand: the wait is counted in frames, never in wall time.
 */
const openOnFakeClock = async (g: Gantt): Promise<void> => {
  await g.page.clock.install();
  await g.open({ n: SIZE });
};
const frozen = async (g: Gantt): Promise<(frames: number) => Promise<void>> => {
  // 30 s ahead: a loaded machine can spend more than a second between the install and this call, and a time in the past is refused.
  await g.page.clock.pauseAt(Date.now() + 30_000);
  // A frame that scrolls the table is not finished until the browser has delivered the scroll event: the next frame would put the scroll back.
  const scrolled = () => g.page.evaluate(() => Math.abs(document.querySelector<HTMLElement>("[data-part=table-body]")!.scrollTop - window.gantt!.status().scrollY) < 1);
  return async (frames) => {
    for (let i = 0; i < frames; i++) {
      await g.page.clock.runFor(16);
      await expect.poll(scrolled, "the table's scroll event reached the chart").toBe(true);
    }
  };
};

test("a key typed before the selected row is drawn is not lost", async ({ g }) => {
  await openOnFakeClock(g);
  const [leaf] = await leafRows(g.page);
  const tasks = await visibleTasks(g.page);
  const far = tasks[1500];
  await clickCell(g, leaf.row, "name");
  const draw = await frozen(g);

  // Select a row far below and press the key in the same task: the table has not scrolled or drawn that row yet.
  await g.page.evaluate((t) => {
    window.gantt!.app.select_task(t);
    document.querySelector(".gantt")!.dispatchEvent(new KeyboardEvent("keydown", { key: "x", bubbles: true, cancelable: true }));
  }, far);
  await draw(10);
  await expect(editor(g), "the editor opened on the far row once it was drawn").toHaveValue("x");
  expect(await g.page.evaluate(() => Number(document.activeElement?.closest<HTMLElement>(".row")?.dataset.task))).toBe(far);
});

/**
 * Select `task` and press `keys` on the chart in the same browser task, before the table has scrolled to or drawn that row. Returns,
 * for each key, whether the page left it alone (true) or took it with preventDefault (false).
 */
const pressBeforeDrawn = (page: Page, task: number, keys: string[]): Promise<boolean[]> =>
  page.evaluate(
    ([t, list]) => {
      window.gantt!.app.select_task(t as number);
      const root = document.querySelector(".gantt")!;
      return (list as string[]).map((key) =>
        root.dispatchEvent(new KeyboardEvent("keydown", { key: key.replace("Shift+", ""), shiftKey: key.startsWith("Shift+"), bubbles: true, cancelable: true })),
      );
    },
    [task, keys] as const,
  );

test("keys typed before the selected row is drawn are replayed in order: Backspace deletes, Enter commits", async ({ g }) => {
  await openOnFakeClock(g);
  const [leaf] = await leafRows(g.page);
  const tasks = await visibleTasks(g.page);
  const far = tasks[1500];
  await clickCell(g, leaf.row, "name");
  const draw = await frozen(g);

  await pressBeforeDrawn(g.page, far, ["x", "Backspace", "y"]);
  await draw(10);
  await expect(editor(g), "x, Backspace, y typed in order leave y").toHaveValue("y");
  await g.page.keyboard.press("Escape");
  await draw(2);

  const revision = (await g.status()).revision;
  const near = tasks[100];
  await pressBeforeDrawn(g.page, near, ["N", "e", "w", "Enter"]);
  await draw(10);
  await expect(editor(g), "Enter committed: no editor is left open").toHaveCount(0);
  expect(await label(g, near), "the name typed before the editor opened was committed").toBe("New");
  expect((await g.status()).revision, "as one undo step").toBe(revision + 1);
  expect(await marks(g.page), "and the cursor moved down").toEqual([{ task: tasks[101], column: "name" }]);
});

test("a Tab typed before the selected row is drawn is taken, commits, and moves across", async ({ g }) => {
  await openOnFakeClock(g);
  const [leaf] = await leafRows(g.page);
  const tasks = await visibleTasks(g.page);
  const far = tasks[1500];
  await clickCell(g, leaf.row, "name");
  const draw = await frozen(g);

  const left = await pressBeforeDrawn(g.page, far, ["x", "Tab"]);
  expect(left, "the page took x and Tab: the browser moved no focus").toEqual([false, false]);
  await draw(10);
  await expect(editor(g)).toHaveCount(0);
  expect(await label(g, far), "Tab committed the typed name").toBe("x");
  expect(await marks(g.page), "and moved to the duration of the same row").toEqual([{ task: far, column: "dur" }]);
});

test("ending the selection leaves the grid: a later selection does not bring the editing keys back", async ({ g }) => {
  await g.open({ n: SIZE });
  const leaf = (await leafRows(g.page)).find((r) => !r.critical)!;
  const rowCount = (await g.status()).rowCount;
  await clickCell(g, leaf.row, "name");

  // Opening another schedule clears the selection; a find result then selects a task. Neither is a click or an arrow.
  await g.page.evaluate(() => window.gantt!.app.clear_selection());
  await g.nextFrame(2);
  expect((await g.status()).selected).toBeNull();
  await g.page.evaluate((t) => window.gantt!.app.select_task(t), leaf.task);
  await g.nextFrame(2);
  await g.page.keyboard.press("c");
  await g.nextFrame(2);
  await expect(editor(g), "c did not open the editor: the cursor is not in the grid").toHaveCount(0);
  await g.page.keyboard.press("1");
  await g.nextFrame(2);
  expect((await g.status()).rowCount, "1 is the chart's again").toBeLessThan(rowCount);

  await g.page.keyboard.press("ArrowDown");
  await g.nextFrame(2);
  await g.page.keyboard.press("c");
  await expect(editor(g), "an arrow puts the cursor back in the grid").toHaveValue("c");
});

test("a focused toolbar button keeps Enter and Space but not the chart's letter keys while the cursor is in the grid", async ({ g }) => {
  await g.open({ n: SIZE });
  const [leaf] = await leafRows(g.page);
  await clickCell(g, leaf.row, "name");
  const viewBefore = await view(g);

  await g.page.locator("#insert-row").focus();
  await g.page.keyboard.press("c");
  await expect(editor(g), "c from a focused button opens the editor").toHaveValue("c");
  expect(await view(g), "and does not run the chart's c").toEqual(viewBefore);
  await g.page.keyboard.press("Escape");
  await g.nextFrame(2);

  let rowCount = (await g.status()).rowCount;
  await g.page.locator("#insert-row").focus();
  await g.page.keyboard.press("Enter");
  await g.nextFrame(2);
  await expect(editor(g), "Enter on a focused button activates it, not the editor").toHaveCount(0);
  expect((await g.status()).rowCount, "Enter inserted a row").toBe(rowCount + 1);

  rowCount += 1;
  await g.page.locator("#insert-row").focus();
  await g.page.keyboard.press(" ");
  await g.nextFrame(2);
  await expect(editor(g)).toHaveCount(0);
  expect((await g.status()).rowCount, "Space inserted another").toBe(rowCount + 1);
});

/** A leaf row far down the table whose duration is not `5d`, with the task of the row after it. */
const farLeaf = (page: Page): Promise<{ task: number; next: number }> =>
  page.evaluate(() => {
    const w = window.gantt!;
    const rows = JSON.parse(w.app.table_rows(1400, 1440)) as [number, string, string, number | null, boolean, number][];
    const at = rows.findIndex((r, i) => r[5] === 0 && w.app.task_duration(r[0]) !== "5d" && i + 1 < rows.length);
    return { task: rows[at][0], next: rows[at + 1][0] };
  });

test("queued cell commits replay in order: x, Tab, 5, d, Tab, y commits x and 5d and leaves y in the next cell", async ({ g }) => {
  await openOnFakeClock(g);
  const [leaf] = await leafRows(g.page);
  const far = await farLeaf(g.page);
  await clickCell(g, leaf.row, "name");
  const revision = (await g.status()).revision;
  const draw = await frozen(g);

  await pressBeforeDrawn(g.page, far.task, ["x", "Tab", "5", "d", "Tab", "y"]);
  await draw(10);
  await expect(editor(g), "y is typed into the third cell").toHaveValue("y");
  expect(await g.page.evaluate(() => Number(document.activeElement?.closest<HTMLElement>(".row")?.dataset.task)), "in the next row").toBe(far.next);
  expect(await label(g, far.task), "the first cell committed x").toBe("x");
  expect(await duration(g, far.task), "the second cell committed 5d").toBe("5d");
  expect((await g.status()).revision, "as two undo steps").toBe(revision + 2);
});

test("a Shift+Tab in the middle of a queue moves back and each cell commits in order", async ({ g }) => {
  await openOnFakeClock(g);
  const [leaf] = await leafRows(g.page);
  const far = await farLeaf(g.page);
  await clickCell(g, leaf.row, "name");
  const revision = (await g.status()).revision;
  const draw = await frozen(g);

  await pressBeforeDrawn(g.page, far.task, ["x", "Tab", "5", "d", "Shift+Tab", "z"]);
  await draw(10);
  await expect(editor(g), "z is typed into the name again").toHaveValue("z");
  expect(await editor(g).getAttribute("data-field")).toBe("name");
  expect(await g.page.evaluate(() => Number(document.activeElement?.closest<HTMLElement>(".row")?.dataset.task)), "on the same row").toBe(far.task);
  expect(await label(g, far.task), "x was committed before z").toBe("x");
  expect(await duration(g, far.task), "5d was committed").toBe("5d");
  expect((await g.status()).revision, "as two undo steps").toBe(revision + 2);
});
