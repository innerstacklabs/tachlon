// The wait helper against a stub `status.slide`, no page (moved from measure.spec.ts).
import { test } from "node:test";
import { expect } from "@playwright/test";
import { SLIDE_WAIT_MS, waitSlideIdle } from "../src/measure/slide.ts";

test("waitSlideIdle yields while the slide is active and is bounded", () => {
  const active = [true, true, true, false];
  let reads = 0;
  const read = () => ({ active: active[Math.min(reads++, active.length - 1)]!, copyMs: null });
  const wait = waitSlideIdle(read, () => 0);
  let frames = 0;
  while (!wait.next().done) frames++;
  expect(frames, "one frame per read that found the slide active").toBe(3);
  expect(reads, "the wait stopped at the first inactive read").toBe(4);

  expect(waitSlideIdle(() => null, () => 0).next().done, "no slide: no wait").toBe(true);

  let clock = 0;
  const stuck = waitSlideIdle(
    () => ({ active: true, copyMs: null }),
    () => (clock += 400),
  );
  expect(() => {
    // Bounded, so a regression of the 1 s bound fails here instead of hanging the worker.
    for (let i = 0; i < 100; i++) stuck.next();
  }, "a slide that never ends fails the phase").toThrow(`${SLIDE_WAIT_MS} ms after the undo`);
});
