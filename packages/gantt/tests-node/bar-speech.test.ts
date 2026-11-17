// The sentence a screen reader hears for the selected bar (gantt#1875): `barSentence` in `bar-speech.ts`.
import { test } from "node:test";
import { expect } from "@playwright/test";
import type { TaskDetails } from "../src/api.ts";
import { barSentence } from "../src/bar-speech.ts";

const LEAF = {
  task: 7, name: "Pour slab", start: "2026-03-02", finish: "2026-03-06", floatText: "2d", critical: false, percentComplete: "40%",
  predecessorCount: 2, successorCount: 3, baselineStart: "2026-03-01",
} as TaskDetails;

test("a leaf's sentence holds the percent, both link counts and the baseline start", () => {
  expect(barSentence({}, LEAF)).toBe(
    "Pour slab. Start 2026-03-02, finish 2026-03-06, float 2d, critical no, 40% complete, 2 predecessors, 3 successors, baseline start 2026-03-01",
  );
});

test("a count of one says predecessor and successor, not the plural", () => {
  expect(barSentence({}, { ...LEAF, predecessorCount: 1, successorCount: 1 })).toContain("1 predecessor, 1 successor, baseline");
});

test("the percent and the baseline start are left out when the task has none", () => {
  const sentence = barSentence({}, { ...LEAF, percentComplete: "", baselineStart: undefined });
  expect(sentence).toBe("Pour slab. Start 2026-03-02, finish 2026-03-06, float 2d, critical no, 2 predecessors, 3 successors");
});
