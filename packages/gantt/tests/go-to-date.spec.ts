import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// Go to a date (gs#391): a find query that is a whole YYYY-MM-DD date jumps on Enter. The oracle is TypeScript, written
// from the rule: the date's 00:00 sits a fifth of the way in from the left, the zoom stays, and the first row of the
// table, in order, whose start is on or after that day is the top row. Anything that is not a whole real date still searches
// names, Shift+Enter wrap included. The date-hold specs rename tasks (App.rename_task) so that names hold a date: the demo's
// names hold none, and without one a hold that does nothing looks the same as no hold.
// The Rust edges (a bad date, a bar press, hidden rows, summaries) are in crates/app/tests/go_to_date.rs.

const SIZE = 2000;

type Row = [task: number, start: string, finish: string, float: number | null, critical: boolean, kind: number];

/** Every visible row, in table order. */
function allRows(page: Page): Promise<Row[]> {
  return page.evaluate(() => {
    const app = window.gantt!.app;
    return JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as Row[];
  });
}

/** The demo's origin is Monday 2026-01-05: minutes from it to 00:00 on `day`. */
const minutesFromOrigin = (day: string): number => (Date.parse(`${day}T00:00:00Z`) - Date.parse("2026-01-05T00:00:00Z")) / 60_000;

/** The view the specs compare: the left edge, the right edge, the zoom and the first row shown. */
async function view(g: Gantt): Promise<{ t: [number, number]; scale: number; top: number; scrollY: number }> {
  const s = await g.status();
  return { t: s.t, scale: s.pxPerMinute, top: s.scrollY / s.rowHeight, scrollY: s.scrollY };
}

/** The view once it has stopped moving: finding a name flies the view to it over 300 ms of the session clock (`crates/app/src/fly.rs`), so a few frames can land mid-flight on a fast runner. */
async function settledView(g: Gantt): Promise<Awaited<ReturnType<typeof view>>> {
  let last = await view(g);
  for (let k = 0; k < 200; k++) {
    await g.nextFrame(2);
    const now = await view(g);
    if (now.t[0] === last.t[0] && now.t[1] === last.t[1] && now.scale === last.scale && now.scrollY === last.scrollY) return now;
    last = now;
  }
  throw new Error("the view never settled");
}

/** The median leaf start's day: a date with rows before it and rows after it. */
function aDay(rows: Row[]): string {
  const leaves = rows.filter((row) => row[5] === 0).map((row) => row[1]).sort();
  return leaves[Math.floor(leaves.length / 2)]!.slice(0, 10);
}

test("a date in the find box goes to it on Enter", async ({ g }) => {
  await g.open({ n: SIZE });
  const rows = await allRows(g.page);
  const day = aDay(rows);
  const want = rows.findIndex((row) => row[1] >= `${day} 00:00`);
  expect(want, "rows before the date, so the scroll means something").toBeGreaterThan(0);
  const before = await view(g);

  const find = g.page.locator("#find-input");
  await find.fill(day);
  await g.nextFrame(3);

  await find.press("Enter");
  await g.nextFrame(3);
  const after = await view(g);
  expect(after.top, "the first row that starts on or after the day is on top").toBe(want);
  const [t0, t1] = after.t;
  expect(Math.abs(minutesFromOrigin(day) - (t0 + 0.2 * (t1 - t0)))).toBeLessThan(2);
  expect(after.scale, "the zoom stays").toBe(before.scale);
  expect((await g.status()).selected, "no task is selected").toBeNull();
  await expect(g.page.locator("#find-count")).toHaveText("went to date");
});

const selected = async (g: Gantt): Promise<number | undefined> => (await g.status()).selected?.task;

/** Leaf tasks, in table order, to rename. */
const leaves = (rows: Row[]): number[] => rows.filter((row) => row[5] === 0).map((row) => row[0]);

const rename = (g: Gantt, names: [number, string][]): Promise<void> =>
  g.page.evaluate((all) => all.forEach(([task, name]) => window.gantt!.app.rename_task(task, name)), names);

test("the keystroke that completes a date does not jump to a name that holds it, and Enter still goes to the date", async ({ g }) => {
  await g.open({ n: SIZE });
  const rows = await allRows(g.page);
  const day = aDay(rows);
  const [early, late] = [leaves(rows)[100]!, leaves(rows)[400]!];
  // The first nine characters match both names and select the first; the whole date matches only the second.
  await rename(g, [
    [early, `${day.slice(0, 9)}x`],
    [late, `Review ${day}`],
  ]);

  const find = g.page.locator("#find-input");
  await find.focus();
  await find.pressSequentially(day.slice(0, 9));
  await g.nextFrame(3);
  expect(await selected(g), "a partial date searches names as usual").toBe(early);
  const before = await settledView(g);

  await find.press(day.slice(9));
  await g.nextFrame(3);
  await expect(g.page.locator("#find-count"), "the search still counts the name that holds the date").toHaveText("1 of 1");
  expect(await selected(g), "the completing keystroke does not jump to it").toBe(early);
  expect(await view(g)).toEqual(before);

  await find.press("Enter");
  await g.nextFrame(3);
  const want = (await allRows(g.page)).findIndex((row) => row[1] >= `${day} 00:00`);
  expect((await view(g)).top, "Enter goes to the date").toBe(want);
  expect(await selected(g), "an existing selection stays").toBe(early);
});

test("a query that is not a real date searches names, and Shift+Enter wraps", async ({ g }) => {
  await g.open({ n: SIZE });
  const rows = await allRows(g.page);
  const [a, b, c] = [leaves(rows)[100]!, leaves(rows)[200]!, leaves(rows)[300]!];
  await rename(g, [
    [a, "Due 2026-02-30 a"],
    [b, "Due 2026-02-30 b"],
    [c, "Due 2026-02-30 c"],
  ]);

  const find = g.page.locator("#find-input");
  await find.focus();
  await find.pressSequentially("2026-02-30");
  await g.nextFrame(3);
  await expect(g.page.locator("#find-count")).toHaveText("1 of 3");
  expect(await selected(g), "typing selects the first match, as for any name").toBe(a);

  await find.press("Shift+Enter");
  await g.nextFrame(3);
  expect(await selected(g), "Shift+Enter wraps to the last match").toBe(c);
  await expect(g.page.locator("#find-count")).toHaveText("3 of 3");

  await find.press("Enter");
  await g.nextFrame(3);
  expect(await selected(g), "Enter wraps to the first").toBe(a);
});

test("go_to_date refuses a bad date and changes nothing", async ({ g }) => {
  await g.open({ n: SIZE });
  const before = await view(g);

  const refused = await g.page.evaluate(() =>
    ["", "2026-02-30", "2026-1-12", "soon"].map((text) => window.gantt!.app.go_to_date(text)),
  );
  expect(refused).toEqual([false, false, false, false]);
  expect(await view(g)).toEqual(before);

  expect(await g.page.evaluate(() => window.gantt!.app.go_to_date("2026-03-02"))).toBe(true);
  await g.nextFrame(2);
  expect((await view(g)).t).not.toEqual(before.t);
});
