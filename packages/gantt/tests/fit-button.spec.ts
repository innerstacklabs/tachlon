import { expect, test } from "./support/gantt";

// The Fit button (gs#703): the Zoom group's third control shows the earliest start to the project finish, the way back after
// zooming in.

test("Fit after two zoom-ins shows the last task's bar", async ({ g }) => {
  await g.open({ n: 2_000 });
  const finish = await g.page.evaluate(() => window.gantt!.app.test_project_finish());
  const rightEdge = () =>
    g.page.evaluate(() => {
      const canvas = document.getElementById("timeline")!;
      return window.gantt!.app.time_at(canvas.clientWidth);
    });

  await g.page.locator("#toolbar #zoom-in").click();
  await g.page.locator("#toolbar #zoom-in").click();
  await g.nextFrame(2);
  expect(await rightEdge(), "two zoom-ins put the project finish out of view").toBeLessThan(finish);

  await g.page.locator("#toolbar #fit-button").click();
  // Fit flies to its target over 300 ms (#1114), so a fixed count of frames can read the view mid-flight on a fast runner.
  await expect.poll(rightEdge, { message: "Fit brings the project finish into view", timeout: 10_000 }).toBeGreaterThanOrEqual(finish);
  const left = await g.page.evaluate(() => window.gantt!.app.time_at(0));
  const first = await g.page.evaluate(() => window.gantt!.app.row_start(0) ?? 0);
  expect(left, "the view starts at or before the first row's start").toBeLessThanOrEqual(first);
});

test("the Fit button sits after the plus in the Zoom group", async ({ g }) => {
  await g.open({ n: 2_000 });
  const ids = await g.page.evaluate(() =>
    [...document.querySelectorAll('#toolbar [aria-label="Zoom"] button')].map((b) => b.id),
  );
  expect(ids).toEqual(["zoom-out", "zoom-in", "fit-button"]);
});
