// Typing into the progress cells (gs#703): Actual start, Actual finish and Remaining take a day or a duration through the
// table's cell editor, one undo step each. Real mouse and keyboard events; the app is only the oracle for what it holds.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// An MS Project file whose tasks carry no actuals: two summaries, each with leaves of eight working hours.
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));

const row = (page: Page, name: string): Locator => page.locator("#table-rows .row", { has: page.locator(".name", { hasText: name }) });
const taskOf = async (r: Locator): Promise<number> => Number(await r.getAttribute("data-task"));
const held = (page: Page, task: number) =>
  page.evaluate((t) => ({ start: window.gantt!.app.task_actual_start(t), finish: window.gantt!.app.task_actual_finish(t), remaining: window.gantt!.app.task_remaining(t) }), task);
const stamp = (page: Page) => page.evaluate(() => window.gantt!.status().stamp);

async function openFile(g: Gantt): Promise<void> {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.click("#progress-toggle");
  await page.waitForFunction(() => document.querySelector("#table-rows .row .remaining") !== null);
}

/**
 * Double-click the `cls` cell of `r` with the mouse, as a person does. The table scrolls sideways to bring the cell into view, and
 * the press goes to the cell's column on the row's middle: an empty cell has no height, so Playwright cannot target it itself.
 */
async function open(page: Page, r: Locator, cls: string): Promise<void> {
  // The rows are drawn again on the frame after an edit; a press before that would land on rows about to be replaced.
  await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => done())))));
  const at = await r.evaluate((rowEl, name) => {
    const body = document.getElementById("table-body")!;
    const cell = rowEl.querySelector(`:scope > .${name}`)!;
    const edge = body.getBoundingClientRect().right;
    if (cell.getBoundingClientRect().right > edge) body.scrollLeft += cell.getBoundingClientRect().right - edge + 8;
    const box = cell.getBoundingClientRect();
    const rowBox = rowEl.getBoundingClientRect();
    return { x: box.left + box.width / 2, y: rowBox.top + rowBox.height / 2 };
  }, cls);
  await page.mouse.dblclick(at.x, at.y);
}

/** Double-click the `cls` cell of `r`, replace the editor's text and press `key`. */
async function type(page: Page, r: Locator, cls: string, text: string, key: string): Promise<void> {
  await open(page, r, cls);
  const input = r.locator(`.${cls} input.cell-edit`);
  await expect(input).toBeVisible();
  await input.fill(text);
  await input.press(key);
}

test("typing_an_actual_start_sets_it_and_ctrl_z_clears_it", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  expect(await held(page, task)).toEqual({ start: "", finish: "", remaining: "" });
  const before = await stamp(page);

  await type(page, child, "actual-start", "2026-10-05", "Enter");
  await expect(child.locator(".actual-start")).toHaveText("2026-10-05 00:00");
  expect((await held(page, task)).start).toBe("2026-10-05 00:00");
  expect(await stamp(page), "the edit bumps the stamp").toBeGreaterThan(before);
  expect(await page.locator("#table-rows input.cell-edit").count(), "the editor closed").toBe(0);

  await page.keyboard.press("Control+z");
  await expect(child.locator(".actual-start")).toHaveText("");
  expect(await held(page, task)).toEqual({ start: "", finish: "", remaining: "" });
});

test("the_editor_opens_on_the_day_and_an_unchanged_commit_makes_no_edit", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  await type(page, child, "actual-start", "2026-10-05", "Enter");
  const settled = await stamp(page);

  await open(page, child, "actual-start");
  const input = child.locator(".actual-start input.cell-edit");
  await expect(input, "the day, not the day and the time").toHaveValue("2026-10-05");
  await input.press("Enter");
  await expect(input).toHaveCount(0);
  expect(await stamp(page), "an unchanged commit is no edit").toBe(settled);
  await page.keyboard.press("Control+z");
  await expect(child.locator(".actual-start"), "the one undo step is the typed start").toHaveText("");
});

test("remaining_and_the_actual_finish_keep_the_other_fields_and_one_undo_puts_each_back", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  await type(page, child, "actual-start", "2026-10-05", "Enter");
  await type(page, child, "remaining", "4h", "Enter");
  await expect(child.locator(".remaining")).toHaveText("4h");
  await expect(child.locator(".percent")).toHaveText("50%");
  await type(page, child, "actual-finish", "2026-10-06", "Enter");
  expect(await held(page, task)).toEqual({ start: "2026-10-05 00:00", finish: "2026-10-06 00:00", remaining: "0" });

  await page.keyboard.press("Control+z");
  expect(await held(page, task)).toEqual({ start: "2026-10-05 00:00", finish: "", remaining: "4h" });
  await page.keyboard.press("Control+z");
  expect(await held(page, task)).toEqual({ start: "2026-10-05 00:00", finish: "", remaining: "1d" });
});

test("a_refused_entry_keeps_the_editor_open_with_the_reason_and_changes_nothing", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  await type(page, child, "actual-start", "2026-10-05", "Enter");
  const settled = await stamp(page);

  await type(page, child, "actual-finish", "2026-10-01", "Enter");
  await expect(child.locator(".actual-finish input.cell-edit.invalid")).toBeVisible();
  await expect(child.locator(".cell-error")).toHaveText("The actual finish is before the actual start.");
  await child.locator(".actual-finish input").fill("later");
  await expect(child.locator(".cell-error"), "typing clears the reason").toHaveCount(0);
  await child.locator(".actual-finish input").press("Enter");
  await expect(child.locator(".cell-error")).toHaveText("Write the date as YYYY-MM-DD.");
  await child.locator(".actual-finish input").press("Escape");
  expect(await stamp(page)).toBe(settled);
  expect((await held(page, task)).finish).toBe("");

  const summary = row(page, "First summary");
  await type(page, summary, "actual-start", "2026-10-05", "Enter");
  await expect(summary.locator(".cell-error")).toBeVisible();
  await summary.locator(".actual-start input").press("Escape");
  expect(await stamp(page)).toBe(settled);
});

test("tab_moves_to_the_next_progress_cell_and_enter_on_the_cursor_opens_it", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  await type(page, child, "actual-start", "2026-10-05", "Tab");
  await expect(child.locator(".actual-finish")).toHaveAttribute("data-cursor", "true");
  await page.keyboard.press("Enter");
  await expect(child.locator(".actual-finish input.cell-edit")).toBeVisible();
  await child.locator(".actual-finish input").fill("2026-10-06");
  await child.locator(".actual-finish input").press("Tab");
  await expect(child.locator(".remaining")).toHaveAttribute("data-cursor", "true");
  expect((await held(page, task)).finish).toBe("2026-10-06 00:00");
});

test("a_commit_of_the_same_day_keeps_an_actual_start_that_has_a_time_of_day", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  // A start at 13:00, as an imported file holds one; the editor cannot type a time of day.
  await page.evaluate((t) => window.gantt!.app.set_progress_field(t, "actual_start", "2027-03-10 13:00"), task);
  await expect(child.locator(".actual-start")).toHaveText("2027-03-10 13:00");
  const settled = await stamp(page);

  await open(page, child, "actual-start");
  const input = child.locator(".actual-start input.cell-edit");
  await expect(input).toHaveValue("2027-03-10");
  await input.press("Enter");
  await expect(input).toHaveCount(0);
  expect((await held(page, task)).start, "the time of day stays").toBe("2027-03-10 13:00");
  expect(await stamp(page), "an unchanged commit is no edit").toBe(settled);
});

test("typing_100_in_percent_complete_finishes_the_task_and_ctrl_z_puts_it_back", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const task = await taskOf(child);
  await type(page, child, "actual-start", "2026-10-05", "Enter");
  const started = await held(page, task);

  await type(page, child, "percent", "75%", "Enter");
  await expect(child.locator(".percent")).toHaveText("75%");
  await expect(child.locator(".remaining")).toHaveText("2h");

  await type(page, child, "percent", "100", "Enter");
  await expect(child.locator(".percent")).toHaveText("100%");
  expect((await held(page, task)).finish, "100 finishes the task").not.toBe("");
  expect((await held(page, task)).remaining).toBe("0");

  await type(page, child, "percent", "101", "Enter");
  await expect(child.locator(".percent input.cell-edit.invalid")).toBeVisible();
  await expect(child.locator(".cell-error")).toHaveText("Write the percent complete as a whole number from 0 to 100, such as 75 or 75%.");
  await child.locator(".percent input").press("Escape");

  await page.keyboard.press("Control+z");
  await page.keyboard.press("Control+z");
  expect(await held(page, task), "undo puts all three fields back").toEqual(started);
});

test("tab_and_shift_tab_walk_the_progress_cells_and_wrap_to_the_next_and_previous_row", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const first = row(page, "First child");
  const second = row(page, "Second child");
  const cursorAt = async (r: Locator, cls: string) => {
    await expect(r.locator(`.${cls}`)).toHaveAttribute("data-cursor", "true");
    await expect(page.locator("#table-rows [data-cursor='true']"), "the cursor is on one cell").toHaveCount(1);
  };

  // Unchanged text: Tab and Shift+Tab only move.
  await open(page, first, "percent");
  await expect(first.locator(".percent input.cell-edit")).toBeVisible();
  await page.keyboard.press("Tab");
  await cursorAt(second, "actual-start");
  await page.keyboard.press("Enter");
  await expect(second.locator(".actual-start input.cell-edit")).toBeVisible();

  await page.keyboard.press("Shift+Tab");
  await cursorAt(first, "percent");

  await open(page, first, "actual-finish");
  await expect(first.locator(".actual-finish input.cell-edit")).toBeVisible();
  await page.keyboard.press("Shift+Tab");
  await cursorAt(first, "actual-start");

  await open(page, first, "remaining");
  await expect(first.locator(".remaining input.cell-edit")).toBeVisible();
  await page.keyboard.press("Shift+Tab");
  await cursorAt(first, "actual-finish");
});
