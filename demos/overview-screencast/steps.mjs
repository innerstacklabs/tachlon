// The scenes of the narrated overview (gantt#1048): twelve scenes of the README story and the feature list, each speaking
// one line of narration.txt. The scenes are the existing ones, reused as they are; their order is the story's (open, zoom,
// select, drag, fling) and then the P6 steps in the order steps-features.mjs holds them. `say` makes the recorder hold a
// scene until its line has played (ctx.mjs, runScenes).
import { readFileSync } from "node:fs";
import * as features from "../screencast/steps-features.mjs";
import { openingView } from "../screencast/steps/opening.mjs";
import { close, criticalView, drag, fling, open } from "../screencast/steps/story.mjs";

const lines = readFileSync(new URL("./narration.txt", import.meta.url), "utf8").split("\n").map((line) => line.trim()).filter((line) => line !== "");
const feature = (name) => {
  const scene = features.steps.find((item) => item.name === name);
  if (!scene) throw new Error(`overview: steps-features.mjs has no scene "${name}"`);
  return scene;
};
const scenes = [open, feature("zoom"), feature("driving-path"), drag, fling, feature("critical-only"), feature("lookahead"), feature("open-p6"), feature("code-filter"), feature("edit-link"), feature("show-moved"), close];

/** Seconds of silence before a line starts in its scene, and after it ends: a scene lasts its line's audio plus both (record.mjs). */
export const LEAD = 0.2;
export const TAIL = 0.4;
export const steps = scenes.map((scene, i) => ({ ...scene, say: lines[i] }));
/** The story's pre-roll: the opening view, then the critical path on the chart. */
export async function setup(ctx) {
  await openingView(ctx);
  await criticalView(ctx);
}
