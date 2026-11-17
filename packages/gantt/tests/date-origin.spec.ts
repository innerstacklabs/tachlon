// Dates read the calendar date in the file (gs#150). The app used to write every date from a fixed origin, Monday
// 2026-01-05, so the 2024 fixture's data date showed as 2026-01-14. The expected dates below are read from the fixture's
// own PROJECT table, not from the app.
import { readFileSync } from "node:fs";
import { expect, test, type Gantt, openFilters } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** One field of the XER's first PROJECT row. */
function projectField(text: string, field: string): string {
  let inProject = false;
  let at = -1;
  for (const line of text.split(/\r?\n/)) {
    const cells = line.split("\t");
    if (cells[0] === "%T") inProject = cells[1] === "PROJECT";
    else if (cells[0] === "%F" && inProject) at = cells.indexOf(field);
    else if (cells[0] === "%R" && inProject) return cells[at]!;
  }
  throw new Error(`no PROJECT ${field}`);
}

const DATA_DATE = projectField(XER.toString("latin1"), "last_recalc_date");
const PLAN_START = projectField(XER.toString("latin1"), "plan_start_date");

type Row = [task: number, start: string, finish: string, float: number | null, critical: boolean, kind: number];

/** The toolbar, the hook's text, the table and the lookahead box all read the file's dates. */
async function expectFileDates(g: Gantt): Promise<void> {
  const page = g.page;
  expect(DATA_DATE, "the fixture is a 2024 file").toBe("2024-01-10 08:00");
  await expect(page.locator("#data-date")).toBeVisible();
  await expect(page.locator("#data-date")).toHaveText(`Data date ${DATA_DATE}`);
  expect(await page.evaluate(() => window.gantt!.app.data_date_text()), "the hook says what the toolbar says").toBe(
    DATA_DATE,
  );

  const rows = await page.evaluate(
    () => JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as Row[],
  );
  expect(rows.length).toBeGreaterThan(0);
  for (const [task, start, finish] of rows) {
    expect(start, `task ${task} start`).toMatch(/^2024-/);
    expect(finish, `task ${task} finish`).toMatch(/^2024-/);
  }
  const first = rows.map((row) => row[1]).sort()[0]!;
  expect(first.slice(0, 10) >= PLAN_START.slice(0, 10), "no task starts before the plan start").toBe(true);

  // The status line's from is written by the canvas axis's own label function (the hook exposes no axis label
  // list), so it reads the file's year too (gs#150, the axis).
  const view = await g.status();
  // The view opens a little before the origin, so the edge may still be in 2023.
  expect(view.from, "the view's left edge").toMatch(/ 202[34] /);

  const day = DATA_DATE.slice(0, 10);
  expect((await g.status()).lookahead).toEqual({ on: false, start: day, weeks: 3 });
  await expect(page.locator("#lookahead-start")).toHaveValue(day);
  // What the box shows is what it takes back: turn the window on and read the same date.
  await openFilters(page);
  await page.locator("#lookahead-toggle").click();
  await g.nextFrame(3);
  expect((await g.status()).lookahead).toEqual({ on: true, start: day, weeks: 3 });
  await expect(page.locator("#lookahead-start")).toHaveAttribute("aria-invalid", "false");
  await expect(page.locator("#lookahead-start")).toHaveValue(day);
}

test("a 2024 file opened at load shows 2024 dates", async ({ page, g }) => {
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: XER }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await expectFileDates(g);
});

test("a 2024 file opened in place over the demo shows 2024 dates, and the demo keeps its own", async ({ page, g }) => {
  await g.open({ n: 200 });
  await expect(page.locator("#data-date")).toBeHidden();
  expect((await g.status()).lookahead.start, "the demo has a stated origin").toBe("2026-01-05");

  await page.setInputFiles("#open-input", {
    name: "ledger-timeline.xer",
    mimeType: "application/octet-stream",
    buffer: XER,
  });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await expectFileDates(g);
});
