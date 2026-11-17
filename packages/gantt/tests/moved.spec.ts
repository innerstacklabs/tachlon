import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";
import { AXIS, ROW, decodePng, rgb, type Png } from "./support/pixels";
import { dpr } from "./support/knobs";

// What moved (gs#428, story S4 of #309): after a duration edit the chart draws a dashed outline at the old place of
// every moved task, for about 4 seconds, and then it is gone. The pixels are read from screenshots of the canvas, so
// this checks the shader's ring, not only the draw list. There is no clock control in the page, so the spec waits in
// real time (about 6 s).

// The 43-row ledger fixture of the Rust tests (`crates/app/tests/moved.rs`): editing task 16 to ten working days moves
// eight other tasks and the finish by seven days.
const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const EDITED = 16;
/** CSS px of the old top edge, left of the moved bar, that a long enough bar leaves for the dash check. */
const STRETCH = 40;

type Row = [task: number, start: number, finish: number, float: number | null, critical: number, kind: number];

function pixel(png: Png, x: number, y: number): [number, number, number] {
  const i = (y * png.width + x) * png.channels;
  return [png.px[i], png.px[i + 1], png.px[i + 2]];
}

function differs(a: [number, number, number], b: [number, number, number]): boolean {
  return a.some((v, i) => Math.abs(v - b[i]) > 6);
}

test("a duration edit outlines the old places of the moved tasks for four seconds", { tag: "@smoke" }, async ({ page, g }, testInfo) => {
  test.setTimeout(120_000);
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  // The list opens at half the chart (gs#821), so a wider window keeps the canvas as wide as the 40% list left it: the moved bars still leave 40px.
  await page.setViewportSize({ width: 1800, height: 800 });
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
  const box = await g.box();
  // Keep the pointer off the canvas, so no hover tint changes between the two screenshots.
  await page.mouse.move(box.x + box.width + 5, box.y + box.height + 5);

  const edit = await page.evaluate((task) => {
    const app = window.gantt!.app;
    const count = window.gantt!.status().rowCount;
    const rows = JSON.parse(app.table_rows(0, count)) as Row[];
    const leaves = rows.map((r, row) => ({ row, r })).filter(({ r }) => r[5] === 0 && r[0] !== task);
    const was = leaves.map(({ row }) => app.row_start(row)!);
    app.set_duration(task, "10d");
    // A leaf that moved: where its bar began, and where its bar begins now (the old top edge left of that is uncovered).
    const moved = leaves
      .map(({ row }, i) => ({ row, x0: app.x_of(was[i]), nowX0: app.x_of(app.row_start(row)!), moves: app.row_start(row)! !== was[i] }))
      .filter((m) => m.moves);
    return { at: Date.now(), moved, status: window.gantt!.status() };
  }, EDITED);
  expect(edit.status.moved?.others, "the edit moved other tasks").toBeGreaterThan(0);

  const shot = async (after: number): Promise<Png> => {
    await page.waitForTimeout(Math.max(0, edit.at + after - Date.now()));
    const buf = await page.locator("#timeline").screenshot();
    await testInfo.attach(`at-${after}ms`, { body: buf, contentType: "image/png" });
    return decodePng(buf);
  };
  const during = await shot(1_000);
  const gone = await shot(5_500);

  // The top edge of a normal bar: half the row, centred. Where the two shots differ along the old top edge, left of the
  // moved bar, is the outline: dashes with gaps between. Take the moved task with the most of it.
  const barH = Math.round(ROW * 0.5);
  const tried = edit.moved.map((m) => {
    const y = Math.round((AXIS + m.row * ROW + Math.round((ROW - barH) / 2)) * dpr);
    const from = Math.max(Math.ceil(m.x0 * dpr) + 2, 0);
    const to = Math.min(Math.floor(m.nowX0 * dpr) - 2, during.width);
    const xs = Array.from({ length: Math.max(0, to - from) }, (_, i) => from + i);
    const ring = xs.map((x) => differs(pixel(during, x, y), pixel(gone, x, y)));
    return { m, y, xs, ring, drawn: ring.filter(Boolean).length };
  });
  const best = tried.sort((a, b) => b.drawn - a.drawn)[0];
  expect(best.drawn, `a moved bar leaves ${STRETCH}px of its old top edge uncovered, drawn at 60% (${JSON.stringify(tried.map((t) => t.drawn))})`).toBeGreaterThanOrEqual(
    Math.round(0.6 * STRETCH * dpr) - 4,
  );
  const start = best.ring.indexOf(true);
  const span = best.ring.slice(start, start + STRETCH * dpr);
  const flips = span.filter((on, i) => i > 0 && on !== span[i - 1]).length;
  expect(flips, `${STRETCH}px of the old top edge alternate dash and gap (${span.map(Number).join("")})`).toBeGreaterThanOrEqual(4);
  const { xs, ring, y } = best;

  // A dash is neither the bar colour nor the background it covers.
  const theme = await page.evaluate(() => window.gantt!.theme());
  const bars = [rgb(theme["bar-soft"]), rgb(theme.bar)];
  const dash = xs.filter((_, i) => ring[i]).map((x) => pixel(during, x, y));
  expect(dash.length).toBeGreaterThan(0);
  for (const colour of dash) {
    expect(
      bars.some((bar) => !differs(colour, [bar[0], bar[1], bar[2]])),
      `dash ${colour} is not a bar colour`,
    ).toBe(false);
  }

  // The report has faded out, which is why the second shot is the background the first one is compared with.
  const alpha = (await g.status()).moved?.alpha;
  expect(alpha, "the report has faded out").toBe(0);
});
