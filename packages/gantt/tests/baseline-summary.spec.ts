import { expect, test } from "./support/gantt";
import { AXIS } from "./support/pixels";

// The Show moved bubble sits under the time axis (gs#864). How the baseline summary chip reads and the Slipped row count are
// walked in `journey-j3.spec.ts`.

const SIZE = 200;

test("the Show moved bubble sits under the time axis", async ({ page, g }) => {
  await g.open({ n: SIZE });
  await page.setViewportSize({ width: 1280, height: 800 });
  await g.nextFrame(2);
  await page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    app.set_duration(rows.find((r) => r[5] === 0)![0], "300d");
  });
  await g.nextFrame(3);
  const show = page.locator("#moved-show");
  await expect(show).toBeVisible();
  const box = (await show.boundingBox())!;
  const canvas = (await page.locator("#timeline").boundingBox())!;
  expect(box.y, "below the axis band").toBeGreaterThanOrEqual(canvas.y + AXIS);
});
