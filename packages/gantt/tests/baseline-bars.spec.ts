import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";
import { AXIS, ROW, decodePng, rgb, type Png } from "./support/pixels";
import { dpr } from "./support/knobs";

// The comparison bars (gs#703): with a baseline set, a thin bar under each task bar sits at the planned dates. After a
// duration edit the bars slide and the comparison bars stay, so the gap opens. The pixels are read from screenshots of
// the canvas, so this checks the shader's quad, not only the draw list.

const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const EDITED = 16;
/** CSS px: the bar is half the row, centred (6 px above it), the strip starts 1 px under it. */
const BAR_H = Math.round(ROW * 0.5);
const STRIP_TOP = Math.round((ROW - BAR_H) / 2) + BAR_H + 1;
const STRIP_H = 3;

type Row = [task: number, start: number, finish: number, float: number | null, critical: number, kind: number];

function pixel(png: Png, x: number, y: number): [number, number, number] {
  const i = (y * png.width + x) * png.channels;
  return [png.px[i], png.px[i + 1], png.px[i + 2]];
}

function near(a: [number, number, number], b: readonly [number, number, number]): boolean {
  return a.every((v, i) => Math.abs(v - b[i]) <= 6);
}

test("a baseline puts a grey bar under each task bar, and an edit opens the gap", async ({ page, g }, testInfo) => {
  test.setTimeout(120_000);
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await page.setViewportSize({ width: 1800, height: 800 });
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
  const box = await g.box();
  await page.mouse.move(box.x + box.width + 5, box.y + box.height + 5);
  const shot = async (name: string): Promise<Png> => {
    const buf = await page.locator("#timeline").screenshot();
    await testInfo.attach(name, { body: buf, contentType: "image/png" });
    return decodePng(buf);
  };
  const theme = await page.evaluate(() => window.gantt!.theme());
  const grey = rgb(theme.baseline);

  const none = await shot("no-baseline");
  await page.locator("#baseline-set").click();
  await g.nextFrame(3);
  const before = await shot("baseline-set");

  const edit = await page.evaluate((task) => {
    const app = window.gantt!.app;
    const count = window.gantt!.status().rowCount;
    const rows = JSON.parse(app.table_rows(0, count)) as Row[];
    const leaves = rows.map((r, row) => ({ row, r })).filter(({ r }) => r[5] === 0 && r[0] !== task);
    const was = leaves.map(({ row }) => app.row_start(row)!);
    app.set_duration(task, "10d");
    return {
      moved: leaves
        .map(({ row }, i) => ({ row, x0: app.x_of(was[i]), nowX0: app.x_of(app.row_start(row)!), moves: app.row_start(row)! !== was[i] }))
        .filter((m) => m.moves),
      kept: leaves
        .map(({ row }, i) => ({ row, x0: app.x_of(was[i]), moves: app.row_start(row)! !== was[i] }))
        .filter((m) => !m.moves),
    };
  }, EDITED);
  expect(edit.moved.length, "the edit moved tasks").toBeGreaterThan(0);
  // Past the slide, before the report fades.
  await page.waitForTimeout(2_000);
  const after = await shot("after-edit");

  const y = (row: number): number => Math.round((AXIS + row * ROW + STRIP_TOP + 1) * dpr);
  // Left of its moved bar the strip is uncovered: grey in `after` and `before`'s bar sat on it, nothing in `none`.
  // `before` shows the whole strip, so its grey run is the baseline extent.
  const runEnd = (row: number, from: number): number => {
    let x = from;
    while (x < before.width && near(pixel(before, x, y(row)), grey)) x++;
    return x;
  };
  const gap = edit.moved
    .map((m) => {
      const from = Math.ceil(m.x0 * dpr) + 2;
      return { m, from, to: Math.min(Math.floor(m.nowX0 * dpr), runEnd(m.row, from)) - 2 };
    })
    .filter(({ from, to }) => to - from >= 4 && to < after.width)
    .sort((a, b) => b.to - b.from - (a.to - a.from))[0];
  expect(gap, "a moved bar leaves part of its baseline uncovered").toBeTruthy();
  // A dependency line may cross the gap, so most of it, not all of it, is the colour.
  const xs = Array.from({ length: gap.to - gap.from }, (_, i) => gap.from + i);
  const greyAfter = xs.filter((x) => near(pixel(after, x, y(gap.m.row)), grey)).length;
  const greyNone = xs.filter((x) => near(pixel(none, x, y(gap.m.row)), grey)).length;
  expect(greyAfter, `the gap under row ${gap.m.row} is the baseline colour`).toBeGreaterThanOrEqual(Math.floor(0.8 * xs.length));
  expect(greyNone, "without a baseline it is not").toBe(0);
  // The strip is 3 px tall and no more.
  const rowY = Math.round((AXIS + gap.m.row * ROW + STRIP_TOP + STRIP_H) * dpr);
  const mid = Math.floor((gap.from + gap.to) / 2);
  expect(near(pixel(after, mid, rowY + 1), grey), "nothing under the strip").toBe(false);

  // A task whose baseline equals its dates keeps a flat strip right under its bar.
  const flat = edit.kept.find((k) => Math.ceil(k.x0 * dpr) + 4 < after.width && k.x0 > 0);
  expect(flat, "the edit left a task where it was").toBeTruthy();
  const fx = Math.ceil(flat!.x0 * dpr) + 3;
  expect(near(pixel(before, fx, y(flat!.row)), grey), "flat strip before the edit").toBe(true);
  expect(near(pixel(after, fx, y(flat!.row)), grey), "flat strip after the edit").toBe(true);
});
