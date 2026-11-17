import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// Moving the data date from the toolbar (gs#703, story 5). The box beside Baseline takes a day written YYYY-MM-DD; Enter moves
// the data date, the line and every unstarted bar that sat before it, and Ctrl+Z (the chart has focus again) puts all of it
// back. The oracle is TypeScript from the rule: work that has not started starts no earlier than the data date. The demo has no
// data date, so the first move also draws the line. The Rust side, with hand-solved dates, is crates/app/tests/data_date_edit.rs.

const SIZE = 200;

type Row = [task: number, start: string, finish: string, float: number | null, critical: boolean, kind: number];

/** The demo's origin is Monday 2026-01-05: minutes from it to 00:00 on `day`. */
const minutesFromOrigin = (day: string): number => (Date.parse(`${day}T00:00:00Z`) - Date.parse("2026-01-05T00:00:00Z")) / 60_000;

/** `day` (YYYY-MM-DD) and `days` later. */
const plusDays = (day: string, days: number): string => new Date(Date.parse(`${day}T00:00:00Z`) + days * 86_400_000).toISOString().slice(0, 10);

const rows = (page: Page): Promise<Row[]> =>
  page.evaluate(() => JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as Row[]);

/** The first leaf with nothing before it (so only its own start and the data date bound it), and its start text. */
async function firstFree(page: Page): Promise<{ task: number; start: string }> {
  const all = await rows(page);
  const found = await page.evaluate((leaves) => {
    const app = window.gantt!.app;
    return leaves.find((task) => app.task_details(task).predecessorCount === 0) ?? null;
  }, all.filter((row) => row[5] === 0).map((row) => row[0]));
  expect(found, "the demo has a leaf with no predecessor").not.toBeNull();
  const row = all.find((r) => r[0] === found)!;
  return { task: row[0], start: row[1] };
}

const startOf = async (page: Page, task: number): Promise<string> => (await rows(page)).find((row) => row[0] === task)![1];


/** The drawn x of the bar of `task`, in physical pixels, or null when it is not on screen. */
const barX = (page: Page, task: number): Promise<number | null> =>
  page.evaluate((id) => {
    const rect = (window.gantt!.app.test_bar_rects() as { task: number; x: number }[]).find((r) => r.task === id);
    return rect === undefined ? null : rect.x;
  }, task);

/** Move the data date to `day` the way a person does: with no data date the box is on the toolbar; with one, click the date to swap the box in. */
async function typeDate(page: Page, day: string): Promise<void> {
  const box = page.locator("#data-date-input");
  if (await box.isVisible()) await box.click();
  else {
    await page.locator("#data-date").click();
    await expect(box, "the click swaps the box in, focused").toBeFocused();
  }
  await box.fill(day);
  await box.press("Enter");
}

test("typing a date moves the line and the unstarted bars", async ({ page, g }) => {
  // No slide: the bars are at their new places on the first frame, so the reads below are not mid-glide.
  await page.emulateMedia({ reducedMotion: "reduce" });
  await g.open({ n: SIZE });
  const box = page.locator("#data-date-input");
  const readout = page.locator("#data-date");
  const free = await firstFree(page);
  const day1 = plusDays(free.start.slice(0, 10), 7);
  const day2 = plusDays(day1, 7);
  const x0 = await barX(page, free.task);
  expect((await g.status()).dataDate, "the demo has no data date").toBeNull();
  await expect(box, "no data date: the box is on the toolbar").toBeVisible();
  await expect(readout).toBeHidden();

  await typeDate(page, day1);
  await g.nextFrame(3);
  let s = await g.status();
  expect(s.dataDate, "the data date is 00:00 on the day").toBe(minutesFromOrigin(day1));
  expect(s.dataDateX, "the line is on the chart").not.toBeNull();
  const start1 = await startOf(page, free.task);
  expect(start1 >= `${day1} 00:00`, `${free.task} starts ${start1}, no earlier than the data date`).toBe(true);
  expect(start1, "the unstarted bar moved").not.toBe(free.start);
  const x1 = await barX(page, free.task);
  if (x0 !== null && x1 !== null) expect(x1, "the drawn bar moved right").toBeGreaterThan(x0);
  await expect(readout, "a data date: the readout takes the box's place").toHaveText(`Data date ${day1} 00:00`);
  await expect(box).toBeHidden();
  await expect(box, "the chart has the keys again").not.toBeFocused();
  const line1 = s.dataDateX!;

  await typeDate(page, day2);
  await g.nextFrame(3);
  s = await g.status();
  expect(s.dataDate).toBe(minutesFromOrigin(day2));
  expect(s.dataDateX!, "the line moved right").toBeGreaterThan(line1);
  const start2 = await startOf(page, free.task);
  expect(start2 >= `${day2} 00:00`, `${free.task} starts ${start2}`).toBe(true);
  expect(start2 > start1).toBe(true);

  // Ctrl+Z (the chart has focus again after Enter) puts back the line and the bar, one step at a time.
  await page.keyboard.press("Control+z");
  await g.nextFrame(3);
  s = await g.status();
  expect(s.dataDate).toBe(minutesFromOrigin(day1));
  expect(s.dataDateX).toBe(line1);
  expect(await startOf(page, free.task)).toBe(start1);
  await expect(readout).toHaveText(`Data date ${day1} 00:00`);

  await page.keyboard.press("Control+z");
  await g.nextFrame(3);
  s = await g.status();
  expect(s.dataDate, "back to no data date").toBeNull();
  expect(s.dataDateX).toBeNull();
  expect(await startOf(page, free.task)).toBe(free.start);
  expect(await barX(page, free.task)).toBe(x0);
  await expect(readout).toBeHidden();
  await expect(box, "and the box is back on the toolbar, empty").toBeVisible();
  await expect(box).toHaveValue("");

  await page.keyboard.press("Control+Shift+z");
  await g.nextFrame(3);
  expect((await g.status()).dataDate, "redo repeats the move").toBe(minutesFromOrigin(day1));
  expect(await startOf(page, free.task)).toBe(start1);
});

test("text that is not a date is refused in the alert line, and Escape puts the date back", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const box = page.locator("#data-date-input");
  const readout = page.locator("#data-date");
  const line = page.locator("#edit-error");
  const free = await firstFree(page);
  const day = plusDays(free.start.slice(0, 10), 7);
  await typeDate(page, day);
  await g.nextFrame(3);
  const before = await g.status();

  // Enter on the date as it stands changes nothing, and is not an undo step.
  await readout.click();
  await expect(box).toHaveValue(day);
  await box.press("Enter");
  await expect(readout).toBeVisible();
  expect((await g.status()).stamp, "no edit").toBe(before.stamp);

  await readout.click();
  for (const bad of ["2026-02-30", "next week"]) {
    await box.fill(bad);
    await box.press("Enter");
    await g.nextFrame(2);
    await expect(line, `"${bad}" is refused`).toBeVisible();
    await expect(line).toContainText("Write the data date as YYYY-MM-DD.");
    await expect(box).toHaveAttribute("aria-invalid", "true");
    await expect(box, "the box keeps focus and the text").toBeFocused();
    const s = await g.status();
    expect(s.dataDate, "nothing changed").toBe(before.dataDate);
    expect(s.stamp).toBe(before.stamp);
  }

  await box.press("Escape");
  await expect(box).toBeHidden();
  await expect(readout).toHaveText(`Data date ${day} 00:00`);
  await expect(line).toBeHidden();
  expect((await g.status()).dataDate).toBe(before.dataDate);
  // The next time the box opens it holds the date, not the refused text, and is not marked.
  await readout.click();
  await expect(box).toHaveValue(day);
  await expect(box).not.toHaveAttribute("aria-invalid", "true");
});

test("the toolbar writes the data date only when it changes", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const free = await firstFree(page);
  const day = plusDays(free.start.slice(0, 10), 7);
  await page.evaluate(() => {
    const seen = { writes: 0 };
    (window as unknown as { dateWrites: typeof seen }).dateWrites = seen;
    const watch = new MutationObserver((records) => {
      seen.writes += records.length;
    });
    const options = { attributes: true, characterData: true, childList: true, subtree: true };
    watch.observe(document.getElementById("data-date")!, options);
    watch.observe(document.getElementById("data-date-input")!, options);
  });
  const writes = (): Promise<number> =>
    page.evaluate(async () => {
      await new Promise((done) => setTimeout(done, 0));
      return (window as unknown as { dateWrites: { writes: number } }).dateWrites.writes;
    });
  await g.nextFrame(30);
  expect(await writes(), "no write across 30 idle frames").toBe(0);

  await typeDate(page, day);
  await g.nextFrame(5);
  const afterMove = await writes();
  expect(afterMove, "the move wrote").toBeGreaterThan(0);
  await g.nextFrame(30);
  expect(await writes(), "and then no more").toBe(afterMove);
});

// A name cell is selected, the readout takes focus, and Enter or Space opens the date box. The same key must not bubble to the
// chart's cursor, which would open the selected cell's editor and pull focus off the date box (its blur then closes it).
for (const key of ["Enter", " "]) {
  test(`${key === " " ? "Space" : key} on the focused readout opens the date box and no cell editor`, async ({ page, g }) => {
    await g.open({ n: SIZE });
    const box = page.locator("#data-date-input");
    const readout = page.locator("#data-date");
    const editor = page.locator("#table-rows input.cell-edit");
    // Put the cursor on a name cell, then give the readout keyboard focus the way Tab would.
    if (!(await readout.isVisible())) await typeDate(page, plusDays((await firstFree(page)).start.slice(0, 10), 7));
    await g.nextFrame(2);
    await page.locator("#table-rows .row > .name").first().click();
    await g.nextFrame(2);
    expect((await g.status()).selected, "a cell is selected").not.toBeNull();
    await expect(editor).toHaveCount(0);
    await readout.focus();
    await expect(readout).toBeFocused();

    await readout.press(key);
    await g.nextFrame(3);
    await expect(box, "the date box is open").toBeVisible();
    await expect(box, "and has the focus").toBeFocused();
    await expect(editor, "no cell editor opened").toHaveCount(0);
  });
}

test("an empty box plus Enter clears the data date, and one undo puts it back", async ({ page, g }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await g.open({ n: SIZE });
  const box = page.locator("#data-date-input");
  const readout = page.locator("#data-date");
  const free = await firstFree(page);
  const day = plusDays(free.start.slice(0, 10), 7);
  const unset = await g.status();
  await typeDate(page, day);
  await g.nextFrame(3);
  const set = await g.status();
  const moved = await startOf(page, free.task);
  expect(moved, "the move shifted the free task").not.toBe(free.start);

  // Clear: the readout opens the box, the text goes, Enter applies.
  await readout.click();
  await box.fill("");
  await box.press("Enter");
  await g.nextFrame(3);
  const cleared = await g.status();
  expect(cleared.dataDate, "no data date").toBe(unset.dataDate);
  expect(cleared.stamp, "one edit").toBe(set.stamp + 1);
  await expect(readout).toBeHidden();
  await expect(box, "the empty box is back on the toolbar").toBeVisible();
  await expect(box).toHaveValue("");
  await expect(page.locator("#edit-error")).toBeHidden();
  expect(await startOf(page, free.task), "the floor is gone").toBe(free.start);

  // Ctrl+Z (the chart has focus again) is one step: the date, the line and the bars come back.
  await page.keyboard.press("Control+z");
  await g.nextFrame(3);
  expect((await g.status()).dataDate).toBe(set.dataDate);
  await expect(readout).toHaveText(`Data date ${day} 00:00`);
  expect(await startOf(page, free.task)).toBe(moved);
  expect(await barX(page, free.task)).not.toBeNull();

  // Enter on an empty box with no data date changes nothing.
  await page.keyboard.press("Control+z");
  await g.nextFrame(3);
  const before = await g.status();
  await box.click();
  await box.press("Enter");
  await g.nextFrame(2);
  expect((await g.status()).stamp).toBe(before.stamp);
});

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

test("arrow keys step the date and Shift steps a week", async ({ page, g }) => {
  await g.open({ n: SIZE });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: LEDGER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const box = page.locator("#data-date-input");
  const readout = page.locator("#data-date");
  const text = (): Promise<string> => page.evaluate(() => window.gantt!.app.data_date_text());
  expect(await text(), "the fixture's data date").toBe("2024-01-10 08:00");

  await readout.click();
  await expect(box).toBeFocused();
  await expect(box).toHaveValue("2024-01-10");
  await box.press("ArrowUp");
  await expect(box, "Up adds a day").toHaveValue("2024-01-11");
  await box.press("Shift+ArrowDown");
  await expect(box, "Shift+Down takes a week off").toHaveValue("2024-01-04");
  expect(await text(), "the keys change the box's text only").toBe("2024-01-10 08:00");
  await box.press("Enter");
  await g.nextFrame(3);
  expect(await text(), "Enter applies the typed day").toBe("2024-01-04 00:00");

  await readout.click();
  await box.press("ArrowDown");
  await expect(box, "Down takes a day off").toHaveValue("2024-01-03");
  await box.press("Shift+ArrowUp");
  await expect(box, "Shift+Up adds a week").toHaveValue("2024-01-10");
  await box.press("Escape");
  await expect(readout, "Escape restores the shown date").toHaveText("Data date 2024-01-04 00:00");
  expect(await text()).toBe("2024-01-04 00:00");
  await readout.click();
  await expect(box).toHaveValue("2024-01-04");
});
