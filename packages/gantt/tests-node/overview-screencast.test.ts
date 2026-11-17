// The narrated overview (gantt#1048): each scene speaks one line of narration.txt, and join.ts lays a line over its own scene or fails.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { place } from "../../../demos/overview-screencast/join.ts";
import { LEAD, steps } from "../../../demos/overview-screencast/steps.mjs";
import { scriptLines } from "../../../scripts/lib/narrate.ts";

const clip = (text: string, seconds: number) => ({ text, file: `${text}.mp3`, seconds, start: 0, cached: true });

test("every_scene_speaks_one_line_of_the_narration", () => {
  const lines = scriptLines(readFileSync(new URL("../../../demos/overview-screencast/narration.txt", import.meta.url), "utf8"));
  assert.equal(lines.length, 12);
  assert.deepEqual(steps.map((s) => s.say), lines);
  assert.equal(new Set(steps.map((s) => s.name)).size, steps.length, "a scene name is used once");
});

test("a_line_starts_in_its_scene_and_one_that_runs_past_the_scene_fails", () => {
  const windows = [{ name: "a", start: 0, end: 8 }, { name: "b", start: 8, end: 12 }];
  assert.deepEqual(place(windows, [clip("one", 7), clip("two", 3)]).map((c) => c.start), [LEAD, 8 + LEAD]);
  assert.throws(() => place(windows, [clip("one", 7), clip("two", 4)]), /scene b: its line runs 4\.00 s/);
  assert.throws(() => place(windows, [clip("one", 7)]), /2 scenes but 1 narration lines/);
});
