import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";
import { freezeClock, rowOf, taskOfCode } from "./support/moved-note";
import { AXIS, ROW } from "./support/pixels";

// The drag note keeps its 6 s from the release (gs#958): a drag on a started task is refused on its first frame, and the
// note says why from then on. A press held longer than 6 s must not lose it; after the release it stays 6 s. The ledger is
// opened with its actuals, so activity A1-16 has started. The frames come from `page.clock`.

const started = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "latin1");

test("the drag note stays while the button is down and for 6 s after the release", async ({ g }) => {
  await g.page.route("**/schedule.dat", (route) => route.fulfill({ status: 200, contentType: "application/octet-stream", body: Buffer.from(started, "latin1") }));
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);
  await freezeClock(g.page);

  const chip = g.page.locator("#drag-note");
  const task = await taskOfCode(g.page, "A1-16");
  const box = await g.box();
  const row = await rowOf(g.page, task);
  const y = box.y + AXIS + row * ROW + ROW / 2;
  const startX = await g.page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), row);
  let pressX: number | null = null;
  for (let dx = 4; dx <= 40 && pressX === null; dx += 4) {
    await g.page.mouse.move(box.x + startX + dx, y);
    await g.page.clock.runFor(64);
    if ((await g.status()).hover?.task === task) pressX = box.x + startX + dx;
  }
  expect(pressX, "a bar of the started task is under the pointer").not.toBeNull();
  await g.page.mouse.down();
  for (let k = 1; k <= 8; k++) await g.page.mouse.move(pressX! + 15 * k, y);
  await g.page.clock.runFor(64);
  await expect(chip, "the first refused frame shows the note").toBeVisible();
  await expect(chip).toContainText("has started");

  await g.page.clock.runFor(7000);
  await expect(chip, "still down after 7 s: the note stays").toBeVisible();

  await g.page.mouse.up();
  await g.page.clock.runFor(64);
  await g.page.clock.runFor(5800);
  await expect(chip, "5.9 s after the release").toBeVisible();
  await g.page.clock.runFor(200);
  await expect(chip, "6.1 s after the release").toBeHidden();
});
