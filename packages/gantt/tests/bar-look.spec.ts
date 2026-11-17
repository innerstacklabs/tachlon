import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW, decodePng, type Png } from "./support/pixels";
import { zoomToward } from "./support/view";

/** `day` (YYYY-MM-DD) and `days` later. */
const plusDays = (day: string, days: number): string => new Date(Date.parse(`${day}T00:00:00Z`) + days * 86_400_000).toISOString().slice(0, 10);

// The bar look (gs#314, story 1 of gs#305): rounded corners from a signed distance field in the shader, air above and
// below, a pastel body and a saturated progress quad. Sizes and colours are `--gantt-*` tokens, so the checks read the
// colours from the page's own theme and change the sizes through the tokens.


type Rgb = readonly [number, number, number];

function rgb(hex: string): Rgb {
  const n = Number.parseInt(hex.slice(1, 7), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function near(png: Png, x: number, y: number, [r, g, b]: Rgb, tol = 3): boolean {
  if (x < 0 || y < 0 || x >= png.width || y >= png.height) return false;
  const i = (y * png.width + x) * png.channels;
  return Math.abs(png.px[i] - r) <= tol && Math.abs(png.px[i + 1] - g) <= tol && Math.abs(png.px[i + 2] - b) <= tol;
}

interface Bar {
  left: number;
  top: number;
  height: number;
}

/** Bars of a normal task, found from the pixels: a pastel or saturated column with its left end and top and bottom rows. */
function normalBars(png: Png, soft: Rgb, done: Rgb): Bar[] {
  const barish = (x: number, y: number): boolean => near(png, x, y, soft) || near(png, x, y, done);
  const bars: Bar[] = [];
  for (let row = 0; row < Math.floor((png.height - AXIS) / ROW); row++) {
    const mid = AXIS + row * ROW + ROW / 2;
    for (let x = 0; x < png.width; x++) {
      // A bar that runs off the left edge has no left end to look at.
      // Only bars 14px wide or more: a narrower one is all corner.
      if (x === 0 || !barish(x, mid) || barish(x - 1, mid) || !barish(x + 13, mid)) continue;
      // `x` is the left end at mid row. Measure the columns 6 to 9px in, past the corner curve, and keep the tallest:
      // a dependency arrow can cross one column.
      let best: Bar | null = null;
      for (let c = x + 6; c <= x + 9; c++) {
        if (!barish(c, mid)) continue;
        const reach = (step: number): number => {
          // The name inside a bar crosses the column: step over a gap of up to 2px of text.
          let y = mid;
          while (barish(c, y + step) || barish(c, y + 2 * step) || barish(c, y + 3 * step)) y += step;
          return y;
        };
        const [top, bottom] = [reach(-1), reach(1)];
        // A row cut by the bottom of the canvas draws a cut bar.
        if (bottom >= png.height - 1) continue;
        if (best === null || bottom - top + 1 > best.height) best = { left: x, top, height: bottom - top + 1 };
      }
      if (best !== null) {
        bars.push(best);
        break;
      }
    }
  }
  return bars;
}

function cornerIsBar(png: Png, bar: Bar, soft: Rgb, done: Rgb): boolean {
  return near(png, bar.left, bar.top, soft) || near(png, bar.left, bar.top, done);
}

async function openAtWeeks(g: Gantt, finished = false): Promise<Buffer> {
  const { page } = g;
  await g.open();
  // The demo carries no actuals, so no bar has a fill: finish every other leaf of the first screens at its own dates.
  if (finished) {
    const some = (await leaves(page, 200)).filter((_, i) => i % 2 === 0);
    await page.evaluate((list) => {
      for (const [task, start, finish] of list) {
        window.gantt!.app.set_progress_field(task as number, "actual_start", start as string);
        window.gantt!.app.set_progress_field(task as number, "actual_finish", finish as string);
      }
    }, some);
  }
  const box = await g.box();
  const t = await page.evaluate(() => window.gantt!.app.row_start(6) ?? 0);
  const px = box.x + (await page.evaluate((tt) => window.gantt!.app.x_of(tt), t)) + 4;
  const py = box.y + AXIS + 6 * ROW + ROW / 2;
  for (let i = 0; i < 200 && (await g.status()).level !== "week"; i++) {
    await page.mouse.move(px, py);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, zoomToward((await g.status()).level, "week"));
    await page.keyboard.up("Control");
    await g.nextFrame();
  }
  expect((await g.status()).level).toBe("week");
  // The zoom keeps the anchor where it was, at the left edge: pan until row 6's bar starts 150px in, so the bars show their left ends.
  for (let i = 0; i < 6; i++) {
    const x = await page.evaluate((tt) => window.gantt!.app.x_of(tt), t);
    if (Math.abs(x - 150) < 3) break;
    await page.evaluate((dx) => window.gantt!.app.wheel(0, 0, dx, 0, false), x - 150);
    await g.nextFrame();
  }
  // The calm bar-soft (#d9dde3) is within 3 per channel of the weekend shading, so the pixel search would take a weekend column for a bar. Clear the shading (a root token, applied on the next frames) and look at bars only (gs#409).
  await page.evaluate(() => document.getElementById("app")!.style.setProperty("--gantt-nonwork", "#00000000"));
  await page.mouse.move(box.x + box.width - 5, box.y + box.height - 5);
  await g.nextFrame(4);
  return page.locator("#timeline").screenshot();
}

test("bars are rounded, sit in the row with air, and show progress over a pastel body", async ({ g }, testInfo) => {
  const shot = await openAtWeeks(g, true);
  await testInfo.attach("weeks-50k", { body: shot, contentType: "image/png" });
  const theme = await g.page.evaluate(() => window.gantt!.theme());
  const [soft, done] = [rgb(theme["bar-soft"]), rgb(theme.bar)];
  const png = decodePng(shot);
  const bars = normalBars(png, soft, done);
  expect(bars.length, "normal bars found in the view").toBeGreaterThanOrEqual(5);

  // 50% of a 24px row is 12px: 45 to 55% of the row, with 5px of air above and below.
  for (const bar of bars) {
    expect(bar.height / ROW, `bar at y ${bar.top}`).toBeGreaterThanOrEqual(0.45);
    expect(bar.height / ROW, `bar at y ${bar.top}`).toBeLessThanOrEqual(0.55);
    const rowTop = AXIS + Math.floor((bar.top - AXIS) / ROW) * ROW;
    expect(bar.top - rowTop, "air above").toBeGreaterThanOrEqual(5);
    expect(rowTop + ROW - (bar.top + bar.height), "air below").toBeGreaterThanOrEqual(5);
  }
  // Rounded: the corner pixel is not the bar and the pixel four in is. A dependency line can sit on a left end, so most bars, not all.
  const rounded = bars.filter(
    (bar) => !cornerIsBar(png, bar, soft, done) && (near(png, bar.left + 4, bar.top, soft) || near(png, bar.left + 4, bar.top, done)),
  );
  expect(rounded.length, `${rounded.length} of ${bars.length} bars have a rounded corner`).toBeGreaterThanOrEqual(bars.length * 0.7);
  // Both colours are on the page: a pastel body and a saturated progress quad.
  let pastel = 0;
  let saturated = 0;
  for (let y = AXIS; y < png.height; y++) {
    for (let x = 0; x < png.width; x++) {
      pastel += near(png, x, y, soft) ? 1 : 0;
      saturated += near(png, x, y, done) ? 1 : 0;
    }
  }
  expect(pastel, "pastel body pixels").toBeGreaterThan(300);
  expect(saturated, "saturated progress pixels").toBeGreaterThan(300);
});

test("the corner radius and the bar height are tokens", async ({ g, page }) => {
  await openAtWeeks(g);
  const theme = await page.evaluate(() => window.gantt!.theme());
  const [soft, done] = [rgb(theme["bar-soft"]), rgb(theme.bar)];
  await page.evaluate(() => {
    const root = document.getElementById("app")!;
    root.style.setProperty("--gantt-bar-r", "0px");
    root.style.setProperty("--gantt-bar-h", "40%");
  });
  // The shell watches the root's style attribute and applies the tokens on the next animation frame.
  await g.nextFrame(4);
  const png = decodePng(await page.locator("#timeline").screenshot());
  const bars = normalBars(png, soft, done);
  expect(bars.length).toBeGreaterThanOrEqual(5);
  for (const bar of bars) {
    expect(bar.height, `40% of ${ROW}px, rounded down`).toBeGreaterThanOrEqual(Math.floor(0.4 * ROW));
    expect(bar.height, `40% of ${ROW}px, rounded up`).toBeLessThanOrEqual(Math.ceil(0.4 * ROW));
  }
  const square = bars.filter((bar) => cornerIsBar(png, bar, soft, done));
  expect(square.length, `${square.length} of ${bars.length} bars have a square corner`).toBeGreaterThanOrEqual(bars.length * 0.7);
});

/** The first `count` leaves in row order, as `[task, start, finish]` texts (the table's own). */
async function leaves(page: Page, count: number): Promise<[number, string, string][]> {
  const all = await page.evaluate(() => JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][]);
  return all.filter((row) => row[5] === 0).slice(0, count).map((row) => [row[0], row[1], row[2]]);
}

/** The columns of the saturated fill on the middle row of the bar of `task`: its left and one past its right, in physical pixels, or null. */
async function fillOf(page: Page, task: number): Promise<{ left: number; right: number; bar: { x: number; width: number } } | null> {
  const theme = await page.evaluate(() => window.gantt!.theme());
  const fills = [rgb(theme.bar), rgb(theme["crit-fill"])];
  const bar = await page.evaluate((id) => (window.gantt!.app.test_bar_rects() as { task: number; x: number; y: number; width: number; height: number }[]).find((r) => r.task === id), task);
  if (bar === undefined) return null;
  const png = decodePng(await page.locator("#timeline").screenshot());
  const y = Math.round(bar.y + bar.height / 2);
  let [left, right] = [-1, -1];
  for (let x = Math.max(0, Math.floor(bar.x)); x < Math.min(png.width, Math.ceil(bar.x + bar.width)); x++) {
    if (!fills.some((fill) => near(png, x, y, fill))) continue;
    if (left < 0) left = x;
    right = x + 1;
  }
  return left < 0 ? null : { left, right, bar };
}

test("a started bar is filled from its actual start to the data date, and a finished bar whole", async ({ g, page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await g.open({ n: 200 });
  const [started, done] = (await leaves(page, 40)).filter(([, start, finish]) => finish.slice(0, 10) > plusDays(start.slice(0, 10), 6)).slice(0, 2);
  expect(done, "two leaves at least six days long").toBeDefined();
  expect(await fillOf(page, started[0]), "no actuals, no fill").toBeNull();

  // Started on its first day; the data date three days in. No other bar moves, so the fill is read from the frame as it stands.
  await page.evaluate(([task, start]) => window.gantt!.app.set_progress_field(task as number, "actual_start", start as string), started);
  await page.evaluate((day) => window.gantt!.app.set_data_date(day), plusDays(started[1].slice(0, 10), 3));
  await g.nextFrame(4);
  const s = await g.status();
  const fill = await fillOf(page, started[0]);
  expect(fill, "a started bar has a fill").not.toBeNull();
  expect(Math.abs(fill!.left - fill!.bar.x), "from the bar's start").toBeLessThanOrEqual(2);
  expect(Math.abs(fill!.right - s.dataDateX!), "to the data date").toBeLessThanOrEqual(2);
  expect(fill!.right, "the rest is the plain bar").toBeLessThan(fill!.bar.x + fill!.bar.width - 2);

  // An actual date after the data date is refused, so the data date moves past the finish first.
  const past = [plusDays(done[2].slice(0, 10), 1), plusDays(started[1].slice(0, 10), 3)].sort().pop()!;
  await page.evaluate((day) => window.gantt!.app.set_data_date(day), past);
  await page.evaluate(([task, start, finish]) => {
    window.gantt!.app.set_progress_field(task as number, "actual_start", start as string);
    window.gantt!.app.set_progress_field(task as number, "actual_finish", finish as string);
  }, done);
  await g.nextFrame(4);
  const whole = await fillOf(page, done[0]);
  expect(whole, "a finished bar has a fill").not.toBeNull();
  expect(Math.abs(whole!.right - (whole!.bar.x + whole!.bar.width)), "to its end").toBeLessThanOrEqual(3);
});
