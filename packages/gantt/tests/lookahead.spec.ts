import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt, openFilters } from "./support/gantt";

// The lookahead filter (gs#96, journey J2 step 4): a start date and a length in weeks that keep the activities whose
// computed dates intersect the window, with their summaries as context. Each spec drives the toolbar and reads the
// visible rows back through the GanttHook (window.gantt.app.table_rows). Which rows a window keeps, how it combines with
// the filters and collapse, what it refuses and its defaults are checked in crates/app/tests/lookahead.rs and
// date_origin.rs; these specs check the toolbar's wiring. The oracle below is TypeScript, written from the rule and not
// from the Rust code: the window is [start 00:00, start + weeks * 7 days 00:00), and a task occupies [start, finish), so
// one that finishes at the window's start is out, one that starts on its last day is in, and a milestone
// (start == finish) is in when its instant is in the window.

const SIZE = 2000;
const number = new Intl.NumberFormat("en-US");

type Row = [task: number, start: string, finish: string, float: number | null, critical: boolean, kind: number];

/** Every visible row, in order, as the table shows them. */
function allRows(page: Page): Promise<Row[]> {
  return page.evaluate(() => {
    const app = window.gantt!.app;
    return JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as Row[];
  });
}

/** The row names of the visible rows, by kind: leaf names and summary paths. */
function names(page: Page, tasks: number[]): Promise<string[]> {
  return page.evaluate((ids) => ids.map((id) => window.gantt!.app.task_name(id)), tasks);
}

/** `YYYY-MM-DD HH:MM` of the first minute after a window of `weeks` weeks that starts on `day`. */
function windowEnd(day: string, weeks: number): string {
  const end = new Date(Date.parse(`${day}T00:00:00Z`) + weeks * 7 * 86_400_000);
  return `${end.toISOString().slice(0, 10)} 00:00`;
}

function intersects(row: Row, day: string, weeks: number): boolean {
  const from = `${day} 00:00`;
  const to = windowEnd(day, weeks);
  const [, start, finish] = row;
  return start === finish ? from <= start && start < to : start < to && finish > from;
}

/**
 * What the window should leave visible among `rows` (the unfiltered rows, which are in outline order): the leaves that
 * intersect it, and each summary whose subtree holds one. A row's depth is the number of segments in its path, and a
 * summary's subtree is the rows after it until the next row at its depth or above. (Names are not unique: the demo has
 * two summaries called "Level 23 / Structural steel".)
 */
async function expectedTasks(
  page: Page,
  rows: Row[],
  day: string,
  weeks: number,
  also: (row: Row) => boolean = () => true,
): Promise<{ tasks: number[]; leaves: number; context: number }> {
  const depth = (await names(page, rows.map((row) => row[0]))).map((name) => name.split(" / ").length);
  const keep = rows.map((row) => row[5] === 0 && also(row) && intersects(row, day, weeks));
  let context = 0;
  for (let i = 0; i < rows.length; i++) {
    if (rows[i]![5] === 0) continue;
    let end = i + 1;
    while (end < rows.length && depth[end]! > depth[i]!) end++;
    if (keep.slice(i + 1, end).some((k, at) => k && rows[i + 1 + at]![5] === 0)) {
      keep[i] = true;
      context++;
    }
  }
  return {
    tasks: rows.filter((_, i) => keep[i]).map((row) => row[0]),
    leaves: rows.filter((row, i) => keep[i] && row[5] === 0).length,
    context,
  };
}

/** A start date that puts about a third of the leaves outside the window, taken from the leaves' own dates. */
function aStart(rows: Row[]): string {
  const leaves = rows.filter((row) => row[5] === 0).map((row) => row[1]).sort();
  return leaves[Math.floor(leaves.length / 2)]!.slice(0, 10);
}

/** Turn the window on (it starts at the default) and then set its date and weeks, each committed as the reader does by leaving the box. */
async function windowOn(g: Gantt, day: string, weeks: string): Promise<void> {
  await openFilters(g.page);
  await g.page.locator("#lookahead-toggle").click();
  await expect(g.page.locator("#lookahead-window")).toBeVisible();
  await openFilters(g.page);
  await g.page.locator("#lookahead-start").fill(day);
  await g.page.locator("#lookahead-start").blur();
  await openFilters(g.page);
  await g.page.locator("#lookahead-weeks").fill(weeks);
  await g.page.locator("#lookahead-weeks").blur();
  await g.nextFrame(3);
}

const shown = async (g: Gantt): Promise<number[]> => (await allRows(g.page)).map((row) => row[0]);

test("the inputs show the default window, off", { tag: "@smoke" }, async ({ page, g }) => {
  await g.open({ n: SIZE });
  await expect(page.locator("#lookahead-start")).toHaveValue("2026-01-05");
  await expect(page.locator("#lookahead-weeks")).toHaveValue("3");
  await expect(page.locator("#lookahead-toggle")).toHaveAttribute("aria-pressed", "false");
});

test("a length the box refuses leaves the last window and marks the input invalid", async ({ g }) => {
  await g.open({ n: SIZE });
  const day = aStart(await allRows(g.page));
  await windowOn(g, day, "2");

  for (const bad of ["0", "521", "", "2.5"]) {
    await openFilters(g.page);
    await g.page.locator("#lookahead-weeks").fill(bad);
    await g.page.locator("#lookahead-weeks").blur();
    await expect(g.page.locator("#lookahead-weeks"), `weeks ${JSON.stringify(bad)}`).toHaveAttribute(
      "aria-invalid",
      "true",
    );
    expect((await g.status()).lookahead).toEqual({ on: true, start: day, weeks: 2 });
  }
  await openFilters(g.page);
  await g.page.locator("#lookahead-weeks").fill("2");
  await g.page.locator("#lookahead-weeks").blur();
  await expect(g.page.locator("#lookahead-weeks")).toHaveAttribute("aria-invalid", "false");
});

test("the row count names Critical only and the window together", async ({ g }) => {
  await g.open({ n: SIZE });
  await windowOn(g, aStart(await allRows(g.page)), "1");
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await g.nextFrame(3);
  const status = await g.status();
  expect(status.filter).toBe("critical");
  expect(status.lookahead.on).toBe(true);
  await expect(g.page.locator("#row-count")).toHaveText(
    `Critical only + Lookahead · ${number.format(status.keptLeaves!)} of ${number.format(status.n)} activities`,
  );
});

test("typing in the window's inputs does not run the chart's keys", async ({ g }) => {
  await g.open({ n: SIZE });
  const all = await allRows(g.page);
  const day = aStart(all);
  await windowOn(g, day, "3");

  // "2" collapses to level 2 when the chart has the key; in a text box it is a digit.
  const weeks = g.page.locator("#lookahead-weeks");
  await weeks.focus();
  await weeks.press("Control+a");
  await weeks.press("2");
  await weeks.press("Tab");
  await g.nextFrame(3);
  expect((await g.status()).lookahead.weeks).toBe(2);
  expect(await shown(g), "the digit set the weeks and collapsed nothing").toEqual(
    (await expectedTasks(g.page, all, day, 2)).tasks,
  );
});

test("a file opened in place starts without a window and with the new file's default", async ({ page, g }) => {
  await g.open({ n: SIZE });
  const all = await allRows(g.page);
  await windowOn(g, aStart(all), "5");
  expect((await g.status()).lookahead.on).toBe(true);

  const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
  await page.setInputFiles("#open-input", {
    name: "ledger-timeline.xer",
    mimeType: "application/octet-stream",
    buffer: fixture,
  });
  await page.waitForFunction(() => window.gantt?.status().lookahead.start === "2024-01-10");
  await g.nextFrame(3);
  await expect(page.locator("#lookahead-start")).toHaveValue("2024-01-10");
  await expect(page.locator("#lookahead-weeks")).toHaveValue("3");
  await openFilters(g.page);
  await expect(page.locator("#lookahead-window")).toBeHidden();
  await expect(page.locator("#lookahead-toggle")).toHaveAttribute("aria-pressed", "false");
});
