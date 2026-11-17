import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// A wheel or a pinch over the "what moved" note reaches the chart (gs#549). The note takes the pointer while it shows
// (`moved.css`), and the chart's wheel listener is on the canvas, so before the fix a ctrl-wheel over the note zoomed the
// browser page and a plain wheel scrolled it. The note holds while the pointer is over it, so the check does not race the
// note's own fade.

// The 43-row ledger fixture: editing task 16 to ten working days makes a report ("8 tasks moved ...").
const fixture = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const EDITED = 16;

test("ctrl-wheel and plain wheel over the note zoom and scroll the chart, and the page does not take them", async ({ g }) => {
  const { page } = g;
  await page.route("**/schedule.dat", (route) =>
    route.fulfill({ status: 200, contentType: "application/octet-stream", body: fixture }),
  );
  await g.open({ params: { schedule: "1" }, frames: 5, timeout: 60_000 });
  await g.nextFrame(3);

  const row = await page.evaluate((t) => window.gantt!.app.test_task_row(t), EDITED);
  expect(row, "the edited task has a row").not.toBeUndefined();
  await page.dblclick(`#table-rows .row[data-row="${row}"] .dur`);
  await page.keyboard.press("Control+A");
  await page.keyboard.type("10d");
  await page.keyboard.press("Enter");
  await expect(page.locator("#moved-note")).toContainText("tasks moved");

  // Point at the note, which holds the report while the pointer stays over it.
  const noteBox = await page.locator("#moved-note").boundingBox();
  expect(noteBox, "the note has a box").not.toBeNull();
  const x = noteBox!.x + noteBox!.width / 2;
  const y = noteBox!.y + noteBox!.height / 2;
  await page.mouse.move(x, y);
  expect(
    await page.evaluate(([px, py]) => document.elementFromPoint(px!, py!)?.id, [x, y]),
    "the note is what the pointer is over",
  ).toBe("moved-note");

  // Record, after every listener ran, whether the wheel's default was cancelled: an uncancelled one scrolls or zooms the page.
  await page.evaluate(() => {
    const w = window as unknown as { wheelPrevented: boolean[] };
    w.wheelPrevented = [];
    window.addEventListener("wheel", (e) => w.wheelPrevented.push(e.defaultPrevented), { passive: true });
  });
  const prevented = (): Promise<boolean[]> =>
    page.evaluate(() => (window as unknown as { wheelPrevented: boolean[] }).wheelPrevented);

  const before = await g.status();
  await page.keyboard.down("Control");
  await page.mouse.wheel(0, -240);
  await page.keyboard.up("Control");
  await g.nextFrame(2);
  const zoomed = await g.status();
  expect(zoomed.pxPerMinute, "ctrl-wheel over the note zoomed in").toBeGreaterThan(before.pxPerMinute);
  expect(await prevented(), "the page did not take the ctrl-wheel").toEqual([true]);

  await page.mouse.wheel(300, 0);
  await g.nextFrame(2);
  const panned = await g.status();
  expect(panned.t[0], "plain wheel over the note panned the chart").not.toBe(zoomed.t[0]);
  expect(panned.pxPerMinute, "a plain wheel does not zoom").toBe(zoomed.pxPerMinute);
  expect(await prevented(), "the page did not take the plain wheel either").toEqual([true, true]);
});
