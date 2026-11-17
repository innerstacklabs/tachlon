import { expect, test, type Gantt } from "./support/gantt";

// The zoom keys (gs#703): with the chart focused and the cell cursor not in the list, `+` (and `=`) zooms in one step about the
// middle of the canvas, `-` zooms out one step, `f` fits the project, as the three buttons do. With the cell cursor in the list
// the keys type into the cell and the view does not change.

/** The visible span in minutes, read from the right edge and the left edge as fit-button.spec.ts reads the right edge. */
const span = (g: Gantt): Promise<number> =>
  g.page.evaluate(() => {
    const app = window.gantt!.app;
    return app.time_at(document.getElementById("timeline")!.clientWidth) - app.time_at(0);
  });

const rightEdge = (g: Gantt): Promise<number> =>
  g.page.evaluate(() => window.gantt!.app.time_at(document.getElementById("timeline")!.clientWidth));

/** A press on the timeline puts the cell cursor away and gives the chart focus. */
async function focusChart(g: Gantt): Promise<void> {
  await g.page.locator("#timeline").click({ position: { x: 20, y: 400 } });
  await g.nextFrame(2);
}

test("plus_minus_and_f_zoom_and_fit_like_the_buttons", async ({ g }) => {
  // No fly: Fit (and `f`) jump to the fit, so the reads two frames after a key are not mid-flight (gs#309).
  await g.page.emulateMedia({ reducedMotion: "reduce" });
  await g.open({ n: 2_000 });
  const finish = await g.page.evaluate(() => window.gantt!.app.test_project_finish());
  await focusChart(g);
  const start = await span(g);

  // The oracle: one click of Zoom in, then back with Zoom out.
  await g.page.locator("#toolbar #zoom-in").click();
  await g.nextFrame(2);
  const zoomedIn = await span(g);
  expect(zoomedIn, "one click of Zoom in narrows the span").toBeLessThan(start);
  await g.page.locator("#toolbar #zoom-out").click();
  await g.nextFrame(2);
  expect(await span(g), "Zoom out returns it").toBeCloseTo(start, 3);

  await g.page.keyboard.press("Shift+=");
  await g.nextFrame(2);
  expect(await span(g), "+ is one step of Zoom in").toBeCloseTo(zoomedIn, 3);
  await g.page.keyboard.press("-");
  await g.nextFrame(2);
  expect(await span(g), "- returns it").toBeCloseTo(start, 3);
  await g.page.keyboard.press("=");
  await g.nextFrame(2);
  expect(await span(g), "= is the same key without Shift").toBeCloseTo(zoomedIn, 3);

  await g.page.keyboard.press("+");
  await g.nextFrame(2);
  expect(await rightEdge(g), "two zoom-ins put the project finish out of view").toBeLessThan(finish);
  await g.page.keyboard.press("f");
  await g.nextFrame(2);
  expect(await rightEdge(g), "f brings the project finish into view, as Fit does").toBeGreaterThanOrEqual(finish);

  // With the cell cursor in the list the keys type into the cell and the span does not change.
  await g.page.keyboard.press("Escape");
  await g.page.locator('#table-rows .row[data-row="3"] > .dur').click();
  await g.nextFrame(2);
  const inList = await span(g);
  for (const key of ["+", "-", "f"]) {
    await g.page.keyboard.press(key);
    await g.nextFrame(2);
    expect(await span(g), `${key} in the cell cursor is typing`).toBeCloseTo(inList, 3);
    await g.page.keyboard.press("Escape");
  }
});

test("the zoom buttons' tooltips end in their keys", async ({ g }) => {
  await g.open({ n: 200 });
  const titles = await g.page.evaluate(() =>
    ["zoom-in", "zoom-out", "fit-button"].map((id) => document.getElementById(id)!.title),
  );
  expect(titles[0]).toMatch(/\(\+\)$/);
  expect(titles[1]).toMatch(/\(-\)$/);
  expect(titles[2]).toMatch(/\(f\)$/);
});
