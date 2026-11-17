import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// The Ledger timeline (gs#110 PR 4): the non-working-day columns and the data-date line, read back through the
// GanttHook (window.gantt.status). The pixels are covered by the Rust scene tests.

// `xer::synth::Spec::small(3)` (40 activities, one project) with the project's `last_recalc_date` set to
// 2024-01-10 08:00. The origin is the Monday of the project's plan start, 2024-01-01, so the data date is day 9.
const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const MINUTES_PER_DAY = 1440;
const DATA_DATE = 9 * MINUTES_PER_DAY + 8 * 60;

/**
 * Runs of non-working days that overlap `[from, to]` (minutes from the origin Monday), computed from the calendar
 * the fixture declares: Saturday and Sunday are off, and Monday 2024-01-01 (day 0) is a holiday.
 */
function expectedRuns(from: number, to: number): number {
  const off = (day: number): boolean => day === 0 || day % 7 >= 5;
  const first = Math.max(0, Math.floor(from / MINUTES_PER_DAY));
  const last = Math.floor(to / MINUTES_PER_DAY);
  let runs = 0;
  for (let day = first; day <= last; day++) if (off(day) && !(day > first && off(day - 1))) runs++;
  return runs;
}

test("an XER file's data date is drawn as a line at its instant, and its non-working days as columns", async ({
  page,
  g,
}) => {
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);

  const status = await g.status();
  expect(status.dataDate, "the data date is the file's last_recalc_date, in minutes from the origin").toBe(DATA_DATE);
  const wantX = await page.evaluate((t) => window.gantt!.app.x_of(t), DATA_DATE);
  expect(status.dataDateX, "the line is where the app puts that instant").not.toBeNull();
  expect(status.dataDateX!).toBeCloseTo(wantX, 6);
  expect(status.dataDateX!, "the line is on the canvas").toBeGreaterThan(0);
  expect(status.dataDateX!).toBeLessThan((await g.box()).width);

  expect(status.pxPerMinute * MINUTES_PER_DAY, "a whole project fits with days wider than 3px").toBeGreaterThan(3);
  const want = expectedRuns(status.t[0], status.t[1]);
  expect(want, "the fixture's window holds several weekends").toBeGreaterThanOrEqual(4);
  expect(status.shaded, "one column per run of non-working days in view").toBe(want);
});

test("a schedule without a data date draws no line, and zoomed far out nothing is shaded", async ({ g }) => {
  await g.open({ n: 2000 });
  // The page opens fitted to its first screen (gs#821); zoom out to the demo's whole span, years across.
  const box = await g.box();
  await g.page.mouse.move(box.x + box.width / 2, box.y + 60);
  for (let i = 0; i < 60 && (await g.status()).pxPerMinute * MINUTES_PER_DAY > 3; i++) {
    await g.page.keyboard.down("Control");
    await g.page.mouse.wheel(0, 120);
    await g.page.keyboard.up("Control");
    await g.nextFrame();
  }
  const status = await g.status();
  expect(status.dataDate).toBeNull();
  expect(status.dataDateX).toBeNull();
  // The demo schedule's whole span is years, so a day is under 3px wide: nothing to shade.
  expect(status.pxPerMinute * MINUTES_PER_DAY).toBeLessThanOrEqual(3);
  expect(status.shaded).toBe(0);
});

test("an opened P6 file lists a work package's activities in ascending order of the Start column (gs#865)", async ({
  page,
  g,
}) => {
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  const rows = await page.locator("#table-rows .row").evaluateAll((els) =>
    els.map((el) => ({
      kind: (el as HTMLElement).dataset.kind,
      name: el.querySelector(".name")?.textContent ?? "",
      start: el.querySelector(".start")?.textContent ?? "",
    })),
  );
  const from = rows.findIndex((row) => row.kind !== "0" && row.name.includes("Work package 1"));
  expect(from, "Work package 1 is a row").toBeGreaterThanOrEqual(0);
  const starts: string[] = [];
  for (const row of rows.slice(from + 1)) {
    if (row.kind !== "0") break;
    starts.push(row.start);
  }
  expect(starts.length, "Work package 1 holds several activities").toBeGreaterThan(5);
  // The Start column writes `YYYY-MM-DD HH:MM`, so text order is date order.
  expect(starts, "the Start column never goes backwards").toEqual([...starts].sort());
});
