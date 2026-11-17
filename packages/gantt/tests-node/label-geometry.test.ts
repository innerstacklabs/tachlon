// The bar-name geometry check on made-up bars and labels, no page (moved from bar-names.spec.ts).
import { test } from "node:test";
import { expect } from "@playwright/test";
import { type DrawnBar, geometryProblems } from "../tests/support/label-geometry.ts";
import type { PlacedLabel } from "../tests/support/view.ts";

test("the geometry check fails a name drawn on its bar, past the next bar, off the canvas, or out of its bar", () => {
  const label = (over: Partial<PlacedLabel>): PlacedLabel => ({
    task: 1, text: "A", placement: "right", x: 108, y: 0, right: 600, textW: 50, gap: 400, expect: "A", minutes: 480, ...over,
  });
  const bar = (task: number, x: number, width: number, y = 0): DrawnBar => ({ task, x, y, width, height: 16 });
  const bars = [bar(1, 20, 80), bar(2, 300, 40), bar(3, 20, 80, 24)];
  const check = (l: PlacedLabel): string[] => geometryProblems([l], bars, 600, 1);
  // Right of the bar (ends at 100): 4 px clear, 50 wide, ends at 158 before the bar at 300.
  expect(check(label({}))).toEqual([]);
  expect(check(label({ x: 104 })), "exactly the margin").toEqual([]);
  expect(check(label({ x: 40 })), "offset onto its bar").toHaveLength(1);
  expect(check(label({ x: 99 })), "touching the bar edge").toHaveLength(1);
  expect(check(label({ x: 260 })), "runs into the next bar on the row").toHaveLength(1);
  expect(check(label({ x: 560 })), "runs off the canvas").toHaveLength(1);
  expect(check(label({ task: 3, x: 260 })), "a bar on another row is not in the way").toEqual([]);
  // Inside the bar 20..100.
  expect(check(label({ placement: "inside", x: 24, textW: 60 }))).toEqual([]);
  expect(check(label({ placement: "inside", x: 24, textW: 80 })), "too wide for the bar").toHaveLength(1);
  expect(check(label({ placement: "inside", x: 10, textW: 20 })), "starts left of the bar").toHaveLength(1);
  expect(check(label({ task: 9 })), "no bar drawn for the task").toHaveLength(1);
});
