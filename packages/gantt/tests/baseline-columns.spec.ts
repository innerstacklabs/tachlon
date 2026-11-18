import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { openLedger } from "./support/moved-note";
import { expect, test } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// The variance columns (gs#703, story 3): while a baseline is set the task list gets Start, Finish and Float variance columns in
// working days, from the three cells Rust appends to each leaf row of `table_rows`. With no baseline the page builds none of them
// and a row has the cells it had before. The spec reads the DOM; it reads no pixels.

const cellsOf = (page: Page, row: number): Promise<string[]> =>
  page.evaluate(
    (r) => [...document.querySelectorAll<HTMLElement>(`#table-rows .row[data-row="${r}"] > span`)].map((cell) => cell.className),
    row,
  );

const text = (page: Page, row: number, column: string): Promise<string> =>
  page.evaluate(
    ([r, c]) => document.querySelector<HTMLElement>(`#table-rows .row[data-row="${r}"] > .${c}`)?.textContent ?? "missing",
    [row, column] as const,
  );

/** The first drawn leaf row with a duration of whole days, and that many days. */
const firstLeaf = (page: Page): Promise<{ row: number; task: number; days: number }> =>
  page.evaluate(() => {
    for (const el of document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")) {
      const task = Number(el.dataset.task);
      const days = /^(\d+)d$/.exec(window.gantt!.app.task_duration(task));
      if (days !== null && Number(days[1]) > 0) return { row: Number(el.dataset.row), task, days: Number(days[1]) };
    }
    throw new Error("no leaf row with a duration in days");
  });

test("a +5d edit reads +5d in the Finish variance of the moved row; no baseline, no columns; the cursor skips them", async ({ page, g }) => {
  await g.open({ n: 200 });
  const table = page.locator("#table");
  const { row, task, days } = await firstLeaf(page);
  const before = await cellsOf(page, row);
  expect(before, "five cells, as before").toHaveLength(5);
  await expect(table).not.toHaveAttribute("data-variance", /.*/);
  await expect(page.locator("#table-rows .var-finish")).toHaveCount(0);
  await expect(page.locator("#table-head .var-finish")).toHaveCount(0);

  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await expect(table).toHaveAttribute("data-variance", "true");
  await expect(page.locator("#table-head .var-finish")).toBeVisible();
  expect(await cellsOf(page, row), "three variance cells after Float").toEqual([...before, "var-start zero", "var-finish zero", "var-float zero"]);
  expect(await text(page, row, "var-finish"), "no change is a middot, not a 0").toBe("\u00b7");
  await expect(page.locator(`#table-rows .row[data-row="${row}"] > .var-finish`)).toHaveClass(/zero/);

  await page.evaluate(([t, d]) => window.gantt!.app.set_duration(t!, `${d}d`), [task, days + 5] as const);
  await g.nextFrame(3);
  expect(await text(page, row, "var-finish")).toBe("+5d");
  expect(await text(page, row, "var-start")).toBe("\u00b7");
  await expect(page.locator(`#table-rows .row[data-row="${row}"] > .var-finish`)).toHaveClass(/late/);
  await expect(page.locator(`#table-rows .row[data-row="${row}"] > .var-start`)).not.toHaveClass(/late/);

  // No change is muted and a later finish is critical-coloured: the two cells read in different colours.
  const colour = (column: string): Promise<string> =>
    page.evaluate(
      ([r, c]) => getComputedStyle(document.querySelector(`#table-rows .row[data-row="${r}"] > .${c}`)!).color,
      [row, column] as const,
    );
  const critical = await page.evaluate(() => getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-crit-text"));
  expect(await colour("var-finish"), "a later finish reads in the critical colour").not.toBe(await colour("var-start"));
  expect(critical, "the token is defined").not.toBe("");

  // The cursor's columns are Task to Float: ArrowRight from Float stays on Float.
  await page.locator(`#table-rows .row[data-row="${row}"] > .float`).click();
  await g.nextFrame(2);
  await page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  const marked = await page.evaluate(() => [...document.querySelectorAll<HTMLElement>('#table-rows [data-cursor="true"]')].map((c) => c.className));
  expect(marked, "the cursor stays on Float").toEqual(["float"]);

  await page.locator("#baseline-clear").click();
  await g.nextFrame(3);
  await expect(table).not.toHaveAttribute("data-variance", /.*/);
  expect(await cellsOf(page, row), "the cells are back to what they were").toEqual(before);
});

test("with an ID column a row keeps its cells without a baseline, and has three more with one", async ({ page, g }) => {
  test.setTimeout(120_000);
  await openLedger(g);
  const { row } = await firstLeaf(page);
  // The ledger has activity IDs and WBS codes: the five cells every row has, an ID cell, a Predecessors cell, a WBS cell, and four of progress if it has any.
  const has = await page.evaluate(() => ({ codes: window.gantt!.app.has_codes(), wbs: window.gantt!.app.has_wbs_codes(), progress: window.gantt!.app.has_progress() }));
  expect(has.codes, "the ledger has an ID column").toBe(true);
  const wanted = 5 + 2 + (has.wbs ? 1 : 0) + (has.progress ? 4 : 0);
  expect(await cellsOf(page, row), "the cells a row had before").toHaveLength(wanted);

  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  expect(await cellsOf(page, row)).toHaveLength(wanted + 3);
  await page.locator("#baseline-clear").click();
  await g.nextFrame(3);
  expect(await cellsOf(page, row)).toHaveLength(wanted);
});

test("less float reads in the critical colour; the columns are held while a bar is dragged and come back at the release", async ({ page, g }) => {
  await g.open({ n: 200 });
  const found = await page.evaluate(() => {
    for (const el of document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")) {
      const task = Number(el.dataset.task);
      const days = /^(\d+)d$/.exec(window.gantt!.app.task_duration(task));
      const float = parseFloat(window.gantt!.app.task_float(task));
      if (days !== null && float >= 2) return { row: Number(el.dataset.row), task, days: Number(days[1]) };
    }
    return null;
  });
  expect(found, "a leaf with at least two days of float").not.toBeNull();
  const { row, task, days } = found!;
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await expect(page.locator(`#table-rows .row[data-row="${row}"] > .var-float`)).toHaveClass(/zero/);

  await page.evaluate(([t, d]) => window.gantt!.app.set_duration(t!, `${d}d`), [task, days + 1] as const);
  await g.nextFrame(3);
  expect(await text(page, row, "var-float")).toBe("-1d");
  const lost = page.locator(`#table-rows .row[data-row="${row}"] > .var-float`);
  await expect(lost).toHaveClass(/lost/);
  const colour = (column: string): Promise<string> =>
    page.evaluate(
      ([r, c]) => getComputedStyle(document.querySelector(`#table-rows .row[data-row="${r}"] > .${c}`)!).color,
      [row, column] as const,
    );
  expect(await colour("var-float"), "lost float is drawn as a later finish is").toBe(await colour("var-finish"));
  expect(await text(page, row, "var-finish")).toBe("+1d");
  await expect(page.locator(`#table-rows .row[data-row="${row}"] > .var-finish`)).toHaveClass(/late/);

  // Held during a drag: the rows carry no variance cells while a bar is dragged, and the release brings them back.
  const box = await g.box();
  const y = box.y + AXIS + row * ROW + ROW / 2;
  const startX = await page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row);
  let pressX: number | null = null;
  for (let dx = 4; dx <= 40 && pressX === null; dx += 4) {
    await page.mouse.move(box.x + startX + dx, y);
    await g.nextFrame(2);
    if ((await g.status()).hover?.task === task) pressX = box.x + startX + dx;
  }
  expect(pressX, "a bar of the task is under the pointer").not.toBeNull();
  await page.mouse.down();
  await page.mouse.move(pressX! + 4, y);
  for (let k = 1; k <= 8; k++) await page.mouse.move(pressX! + 4 + 15 * k, y);
  await g.nextFrame(3);
  expect((await g.status()).drag?.active, "the drag is on").toBe(true);
  await expect(page.locator("#table-rows .var-finish")).toHaveCount(0);
  await expect(page.locator("#table")).toHaveAttribute("data-variance", "true");
  await page.mouse.up();
  await g.nextFrame(3);
  expect(await page.locator("#table-rows .row .var-finish").count(), "the cells are back at the release").toBeGreaterThan(0);
});

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** The cells (`[cell class, head class]`) whose left edge is not under their head's, row by row. */
const misplaced = (page: Page, pairs: [string, string][]): Promise<string[]> =>
  page.evaluate((list) => {
    const bad: string[] = [];
    for (const row of document.querySelectorAll<HTMLElement>("#table-rows .row")) {
      for (const [cls, head] of list) {
        const cell = row.querySelector<HTMLElement>(`:scope > .${cls}`);
        const at = document.querySelector<HTMLElement>(`#table-head .cols > .${head}`)?.getBoundingClientRect().left ?? NaN;
        if (cell !== null && !(Math.abs(cell.getBoundingClientRect().left - at) <= 1.5)) bad.push(`${cls} in row ${row.dataset.row}`);
      }
    }
    return bad;
  }, pairs);

test("a drag with a baseline set keeps the progress cells under their heads, and the variance cells come back at the release", async ({ page, g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, null, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  const same = (...names: string[]): [string, string][] => names.map((name) => [name, name]);
  const progress = same("actual-start", "actual-finish", "remaining", "percent");
  const variance = same("var-start", "var-finish", "var-float");
  expect(await misplaced(page, [...variance, ...progress]), "at rest every cell is under its head").toEqual([]);
  expect(await page.locator("#table-rows .row:first-child > span").count(), "a row has its variance and progress cells").toBeGreaterThan(11);

  // Press a bar that can be dragged: a leaf with no actual start. Some bars are fixed at their actuals, so try the leaves in turn.
  const candidates = await page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")]
      .filter((el) => window.gantt!.app.task_actual_start(Number(el.dataset.task)) === "")
      .map((el) => ({ row: Number(el.dataset.row), task: Number(el.dataset.task) })),
  );
  const box = await g.box();
  let dragging = false;
  for (const { row, task } of candidates) {
    const y = box.y + AXIS + row * ROW + ROW / 2;
    const startX = await page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row);
    for (let dx = 4; dx <= 40 && !dragging; dx += 4) {
      await page.mouse.move(box.x + startX + dx, y);
      await g.nextFrame(2);
      if ((await g.status()).hover?.task !== task) continue;
      await page.mouse.down();
      await page.mouse.move(box.x + startX + dx + 4, y);
      for (let k = 1; k <= 8; k++) await page.mouse.move(box.x + startX + dx + 4 + 15 * k, y);
      await g.nextFrame(3);
      dragging = (await g.status()).drag?.active === true;
      if (!dragging) await page.mouse.up();
    }
    if (dragging) break;
  }
  expect(dragging, "a bar of a task with no actuals can be dragged").toBe(true);

  await expect(page.locator("#table-rows .var-finish")).toHaveCount(0);
  expect(await page.locator("#table-rows .var-hold").count(), "one empty cell holds the place of the three").toBeGreaterThan(0);
  expect(await misplaced(page, progress), "mid-drag the progress cells stay under their heads").toEqual([]);
  expect(await misplaced(page, [["var-hold", "var-start"]]), "the held cell starts where ΔStart does").toEqual([]);

  await page.mouse.up();
  await g.nextFrame(3);
  expect(await page.locator("#table-rows .var-hold").count(), "no held cell after the release").toBe(0);
  expect(await misplaced(page, [...variance, ...progress]), "after the release every cell is under its head").toEqual([]);
  expect(await page.locator("#table-rows .row .var-finish").count()).toBeGreaterThan(0);
});

test("a summary row reads the variance of its rolled-up span, and its Float variance stays blank", async ({ page, g }) => {
  await g.open({ n: 200 });
  const found = await page.evaluate(() => {
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row")];
    for (const [i, el] of rows.entries()) {
      if (el.dataset.kind !== "1") continue;
      const level = window.gantt!.app.task_outline_level(Number(el.dataset.task));
      for (const next of rows.slice(i + 1)) {
        if (window.gantt!.app.task_outline_level(Number(next.dataset.task)) <= level) break;
        const task = Number(next.dataset.task);
        const days = /^(\d+)d$/.exec(window.gantt!.app.task_duration(task));
        if (next.dataset.kind === "0" && days !== null) return { summary: Number(el.dataset.row), task, days: Number(days[1]) };
      }
    }
    return null;
  });
  expect(found, "an expanded summary with a leaf of whole days in view").not.toBeNull();
  const { summary, task, days } = found!;
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  expect(await text(page, summary, "var-finish"), "no change is a middot").toBe("·");

  await page.evaluate(([t, d]) => window.gantt!.app.set_duration(t!, `${d}d`), [task, days + 400] as const);
  await g.nextFrame(3);
  expect(await text(page, summary, "var-finish"), "the summary's finish moved with its last leaf").toMatch(/^\+\d+d$/);
  await expect(page.locator(`#table-rows .row[data-row="${summary}"] > .var-finish`)).toHaveClass(/late/);
  expect(await text(page, summary, "var-float"), "a summary has no float").toBe("");
});

test("the_toggle_hides_and_shows_the_columns_and_they_stay_hidden_after_an_edit", async ({ page, g }) => {
  await g.open({ n: 200 });
  const table = page.locator("#table");
  const toggle = page.locator("#variance-toggle");
  const { row, task, days } = await firstLeaf(page);
  const before = await cellsOf(page, row);

  // No baseline: no button, and no columns.
  await expect(toggle).toBeHidden();

  // A baseline brings the button and the columns, on by default.
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#table-head .var-finish")).toBeVisible();
  expect(await cellsOf(page, row)).toHaveLength(before.length + 3);

  // Hide: the head cells and the row cells go.
  await toggle.click();
  await g.nextFrame(3);
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("#table-head .var-finish")).toBeHidden();
  await expect(page.locator("#table-rows .var-finish")).toHaveCount(0);
  expect(await cellsOf(page, row), "the row has the cells it had without a baseline").toEqual(before);

  // An edit does not bring them back.
  await page.evaluate(([t, d]) => window.gantt!.app.set_duration(t!, `${d}d`), [task, days + 5] as const);
  await g.nextFrame(3);
  await expect(toggle).toBeVisible();
  await expect(toggle).toHaveAttribute("aria-pressed", "false");
  await expect(page.locator("#table-head .var-finish")).toBeHidden();
  await expect(page.locator("#table-rows .var-finish")).toHaveCount(0);

  // Show: the moved row reads its variance again.
  await toggle.click();
  await g.nextFrame(3);
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#table-head .var-finish")).toBeVisible();
  expect(await text(page, row, "var-finish")).toBe("+5d");

  // Clearing the baseline takes the button away; the next baseline brings the columns back.
  await toggle.click();
  await g.nextFrame(2);
  await page.locator("#baseline-clear").click();
  await g.nextFrame(3);
  await expect(toggle).toBeHidden();
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  await expect(toggle).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#table-head .var-finish")).toBeVisible();
  expect(await cellsOf(page, row)).toHaveLength(before.length + 3);
  await expect(table).toHaveAttribute("data-variance", "true");
});
