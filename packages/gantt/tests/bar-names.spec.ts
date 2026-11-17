import { expect, test, type Gantt } from "./support/gantt";
import { dpr } from "./support/knobs";
import { drawnBars, geometryProblems } from "./support/label-geometry";
import { AXIS, ROW } from "./support/pixels";
import { labelsOk, placedLabels, zoomToward, type PlacedLabel } from "./support/view";

// Names beside bars (gs#316, story 4 of gs#305): from the weeks zoom inward each visible bar's name sits right of the bar when
// the gap, clear of the bar's link terminal, fits it (else inside), and past the weeks zoom none show. The checks read the labels the last frame placed through `window.gantt`,
// never pixels; the label colour is covered by the timeline crate's tests.

/** Room a name needs beyond its width, CSS px: `PAD_PX` in labels.rs. */
const PAD = 8;

/** Ctrl+wheel at row 6's bar until the tick level is `level`: in (negative `dy`) or out (positive). */
async function zoomTo(g: Gantt, level: string, dy: number): Promise<void> {
  const { page } = g;
  const box = await g.box();
  const t = await page.evaluate(() => window.gantt!.app.row_start(6) ?? 0);
  // Zooming in anchors on the bar; out, on the middle of the canvas, as the bar may have left it.
  const px = dy < 0 ? box.x + (await page.evaluate((tt) => window.gantt!.app.x_of(tt), t)) + 4 : box.x + box.width / 2;
  const py = box.y + AXIS + 6 * ROW + ROW / 2;
  for (let i = 0; i < 200 && (await g.status()).level !== level; i++) {
    await page.mouse.move(px, py);
    await page.keyboard.down("Control");
    await page.mouse.wheel(0, zoomToward((await g.status()).level, level));
    await page.keyboard.up("Control");
    await g.nextFrame();
  }
  expect((await g.status()).level).toBe(level);
  await g.nextFrame(3);
}

/** CSS px left between the panned bar's right edge and the canvas edge: less than any name plus `PAD`, so the name cannot go beside it. */
const EDGE = 4;

/** Pan sideways until the bar with the most room for its name (bar width less name width, task bars only) ends `EDGE` short of the canvas edge. */
async function panWidestBarToEdge(g: Gantt): Promise<void> {
  const labels = await placedLabels(g.page);
  const bars = await drawnBars(g.page);
  // A summary's rectangle is its thin top rule: its name never goes inside, so only full-height bars compete.
  const room = (l: PlacedLabel): number => {
    const bar = bars.find((b) => b.task === l.task);
    return bar && bar.height >= 8 * dpr ? bar.width / dpr - l.textW : -Infinity;
  };
  const best = labels.reduce((a, b) => (room(b) > room(a) ? b : a));
  const bar = bars.find((b) => b.task === best.task)!;
  expect(room(best), "a bar wide enough to hold its name").toBeGreaterThan(PAD);
  const canvasW = (await g.status()).surface[0] / dpr;
  const box = await g.box();
  await g.page.mouse.move(box.x + box.width / 2, box.y + AXIS + ROW / 2);
  await g.page.mouse.wheel((bar.x + bar.width) / dpr - (canvasW - EDGE), 0);
  await g.nextFrame(3);
}

// The checker itself, on drawn rectangles that break each rule: a name that passes the placement rule can still be drawn wrong.
test("names follow the placement rule from the weeks zoom inward and vanish past it", async ({ g }) => {
  await g.open();
  await zoomTo(g, "week", -120);
  const weeks = await placedLabels(g.page);
  const status = await g.status();
  expect(weeks.length, "the weeks zoom names bars").toBeGreaterThan(0);
  expect(labelsOk(weeks), "each label is its task's name, once per task").toBe(true);
  expect(weeks.length, "at most one label per visible row").toBeLessThanOrEqual(status.rows[1] - status.rows[0] + 1);
  for (const l of weeks) {
    // The rule (labels.rs `place`): right of the bar when the name plus padding fits `gap`, else inside. `gap` is the room to the
    // canvas edge less the link terminal's reach (`terminal-gap + terminal-size`) for a task bar, the way the page exports it (gs#440).
    const fits = l.textW + PAD <= l.gap;
    expect(l.placement, `"${l.text}": gap ${l.gap} for a name ${l.textW} wide`).toBe(fits ? "right" : "inside");
  }
  expect(weeks.some((l) => l.placement === "right"), "the demo still shows names right of bars").toBe(true);
  // The geometry as drawn, beside the rule above: bar rectangles and label rectangles only (support/label-geometry.ts).
  expect(geometryProblems(weeks, await drawnBars(g.page), status.surface[0], dpr), "weeks: names clear of their bars").toEqual([]);

  // The demo's bars end far from the canvas edge, so every name above is right of its bar. Pan the bar with the most room to the
  // edge: its name no longer fits beside it and must go inside, and the same geometry holds for it.
  await panWidestBarToEdge(g);
  const edge = await placedLabels(g.page);
  expect(edge.some((l) => l.placement === "inside"), "a bar at the canvas edge holds its name").toBe(true);
  expect(geometryProblems(edge, await drawnBars(g.page), status.surface[0], dpr), "edge: names clear of their bars").toEqual([]);

  await zoomTo(g, "month", 120);
  const months = await placedLabels(g.page);
  expect(months, "past the weeks zoom no names show").toEqual([]);
});
