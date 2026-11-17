import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";

// `Gantt.speed()` (gantt#1790) gives a host the last open's phases and the recent frames' numbers. It reads what the loop already keeps and
// calls no app method, because every app call wakes the frame loop (`loop.ts`) and a readout that woke it would keep the chart drawing.

test.use({ webgl: true });

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");

test("frames is null at the start and holds the frame numbers once 120 frames are drawn", async ({ g, page }) => {
  await g.open({ frames: 5 });
  const start = await page.evaluate(() => ({ speed: window.gantt!.api!.speed(), drawn: window.gantt!.status().frames }));
  expect(start.drawn, "frames drawn at the first read").toBeLessThan(120);
  expect(start.speed.frames).toBeNull();
  await g.nextFrame(130);
  const { frames } = await page.evaluate(() => window.gantt!.api!.speed());
  expect(frames).not.toBeNull();
  // The shape only: how long a frame took, and the refresh rate read from it, belong to the machine, not the readout (a loaded runner drew 10 frames a second).
  expect(frames!.count).toBeGreaterThan(100); // a count, not a timing: the window holds 118 or more intervals by the time `frames` is non-null
  expect(frames!.count).toBeLessThanOrEqual(120);
  expect(Number.isFinite(frames!.p50) && frames!.p50 >= 0).toBe(true);
  expect(frames!.p95).toBeGreaterThanOrEqual(frames!.p50);
  expect(Number.isInteger(frames!.missed) && frames!.missed >= 0 && frames!.missed <= frames!.count).toBe(true);
  expect(Number.isFinite(frames!.refreshHz) && frames!.refreshHz > 0).toBe(true);
});

test("open is null until a file is opened, then its eight phases add up to total", async ({ g, page }) => {
  await g.open({ frames: 5 });
  expect(await page.evaluate(() => window.gantt!.api!.speed().open)).toBeNull();
  await page.evaluate((text) => window.gantt!.api!.open(new File([new TextEncoder().encode(text)], "ledger.xer")), LEDGER);
  await page.waitForFunction(() => window.gantt!.api!.speed().open !== null);
  const open = (await page.evaluate(() => window.gantt!.api!.speed().open))!;
  const t = open.timings;
  expect(open.name).toBe("ledger.xer");
  expect(t.total).toBeGreaterThan(0);
  const sum = t.read + t.paint + t.copy + t.parse + t.build + t.compute + t.swap + t.firstFrame;
  expect(Math.abs(sum - t.total), `the phases ${sum} ms against total ${t.total} ms`).toBeLessThan(1);
});

test("50 reads on a sleeping loop ask for no animation frame", async ({ g, page }) => {
  await g.open({ frames: 5 });
  await page.waitForTimeout(1500); // the loop stops asking for frames 500 ms after the last change
  const asked = await page.evaluate(() => {
    const real = window.requestAnimationFrame.bind(window);
    let count = 0;
    window.requestAnimationFrame = (cb) => {
      count += 1;
      return real(cb);
    };
    const reads = (): number => {
      const before = count;
      for (let i = 0; i < 50; i++) window.gantt!.api!.speed();
      return count - before;
    };
    const speed = reads();
    window.gantt!.status(); // the control: a call into the app wakes the loop, which asks for a frame
    const control = count;
    window.requestAnimationFrame = real;
    return { speed, control };
  });
  expect(asked.control, "the control read did not wake a sleeping loop: the test measures nothing").toBeGreaterThan(0);
  expect(asked.speed, "animation frames asked for by 50 speed() reads").toBe(0);
});
