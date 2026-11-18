// Checks the story script with no browser: node demos/screencast/test-story.mjs
// The README story is the four steps in order, the other feature steps live in steps-features.mjs, no scene is optional
// (no `requires`, no skip), the scene fields are well formed, the captions gain their endings, and the P6 file the story
// opens is the one its steps name.
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import * as story from "./steps.mjs";
import * as features from "./steps-features.mjs";
import { PLAN } from "./steps/p6.mjs";
import { SCENES, STILLS, dryList } from "./stills.mjs";

const names = (list) => list.steps.map((step) => step.name);
assert.deepEqual(names(story), ["open", "drag", "fling", "close"]);
assert.equal(typeof story.setup, "function", "the story has a pre-roll");
assert.equal(typeof features.setup, "function", "the feature clips have a pre-roll");

for (const step of [...story.steps, ...features.steps]) {
  assert.ok(typeof step.caption === "string" || typeof step.caption === "function", `${step.name} has a caption`);
  assert.equal(typeof step.run, "function", `${step.name} runs`);
}

// Every scene is on main (a PR that adds a feature adds its scene), so none is optional: a missing method fails the
// scene. The fields: a unique name, a journey "J1".."J9" (the close card has none), `clip` a boolean, `pre` an expression.
for (const [label, list] of [["story", story], ["features", features]]) {
  const seen = new Set();
  for (const scene of list.steps) {
    assert.ok(!seen.has(scene.name), `${label}: ${scene.name} is unique`);
    seen.add(scene.name);
    for (const gone of ["requires", "skipReason", "skip"]) assert.ok(!(gone in scene), `${label}/${scene.name} has no ${gone}`);
    if (scene.journey !== undefined) assert.match(scene.journey, /^J[1-9]$/, `${label}/${scene.name} journey`);
    else assert.equal(scene.name, "close", `${label}/${scene.name} names its journey`);
    if (scene.clip !== undefined) assert.equal(typeof scene.clip, "boolean", `${label}/${scene.name} clip`);
    if (scene.pre !== undefined) assert.equal(typeof scene.pre, "string", `${label}/${scene.name} pre`);
  }
}
// The scenes that read the day zoom say so, so a reorder fails with a message and not a 15 s wait.
for (const name of ["bar-names", "header"]) assert.match(features.steps.find((step) => step.name === name).pre, /level === "day"/);
// The scenes that need what the one before them left say so too.
for (const name of ["show-moved", "code-filter"]) assert.equal(typeof features.steps.find((step) => step.name === name).pre, "string", `${name} checks its state`);

// The captions gain what the step measured or the page reported; the P6 steps add theirs while they run (extendCaption), because an
// ending that runs after the step is on screen after the state it describes has gone (the code filter after Clear).
const ending = (name, result) => story.steps.find((step) => step.name === name).captionEnd(null, result);
// The P6 steps (gs#384) left the story for the features list, where the journey spec still runs them, in this order.
const p6Names = ["open-p6", "code-filter", "edit-link", "show-moved"];
for (const name of p6Names) assert.ok(!names(story).includes(name), `the story no longer holds ${name}`);
const featureNames = names(features);
assert.deepEqual(featureNames.filter((name) => p6Names.includes(name)), p6Names, "the features list holds the four P6 steps in order");
assert.ok(featureNames.indexOf("open-p6") < featureNames.indexOf("open-file"), "open-file stays the last scene");
assert.equal(ending("drag", { recomputeMs: 1.84, moved: null }), "(re-solved in 1.8 ms)");
assert.equal(ending("drag", { recomputeMs: 1.84, moved: "8 tasks moved" }), "(8 tasks moved; re-solved in 1.8 ms)");
assert.equal(ending("fling", { slowestMs: 9.14 }), "(slowest frame 9.1 ms)");
// The video opens on a readable schedule and an edit, not on the fast scroll (gs#508): the scroll comes after the drag, and the
// P6 file replaces the 50,000-task schedule only after it. Show moved comes after the link edit it shows the report of.
assert.deepEqual(names(story).slice(0, 2), ["open", "drag"]);
assert.equal(featureNames.indexOf("show-moved"), featureNames.indexOf("edit-link") + 1, "Show moved follows the edit whose report it keeps");
assert.equal(names(story).at(-1), "close", "the story ends on the close card");
assert.ok(!story.steps.find((step) => step.name === "close").caption.includes("github.com/"), "the close caption names no repository");

// The P6 steps name what the file holds: its activities, the link, and the code type and value.
const fixture = readFileSync(new URL("./fixtures/fit-out.xer", import.meta.url), "utf8");
for (const name of [PLAN.pred, PLAN.succ]) assert.ok(fixture.includes(`\t${name}\t`), `fit-out.xer has the task ${name}`);
assert.ok(fixture.includes(`\t${PLAN.trade}\t`) && fixture.includes(`\t${PLAN.value}\t`), "fit-out.xer has the code type and value the filter step picks");
assert.equal(fixture.split("\n").filter((line) => line.startsWith("%R\t") && /\tTK_(NotStart|Active|Complete)\t/.test(line)).length + 7, PLAN.rows, "the rows the open step expects: the activities and seven summaries");

// The feature steps are what the story dropped: the old walk without its open and close, and the full links clip.
const moved = names(features);
for (const name of ["scroll-to-critical", "bar-look", "header", "pan", "drag-bar", "select", "collapse", "critical-only", "lookahead", "toolbar", "insert-delete", "indent-outdent", "cell-cursor", "links"]) {
  assert.ok(moved.includes(name), `steps-features.mjs holds ${name}`);
}
assert.ok(!moved.includes("open") && !moved.includes("close"), "open and close belong to the story");

// The chapter stills script (gantt#1558), read with no browser: its dry list is every chapter still the book carries, each scene still
// is written by the scene it names (that scene calls `ctx.shot` with the still's name), the scenes start and end where the stills need
// them to, and the manifest re-records each still with this script.
const here = new URL("./", import.meta.url);
const stillNames = STILLS.map((still) => still.name);
const bookStills = readdirSync(new URL("../../book/src/media/", here)).filter((file) => /^(j\d-.*|keys)\.webp$/.test(file)).map((file) => file.replace(".webp", ""));
assert.deepEqual([...stillNames].sort(), bookStills.sort(), "the stills list is the book's chapter stills, no more and no fewer");
assert.equal(dryList().length, STILLS.length, "the dry list has one line per still");
const sceneSource = ["steps-features.mjs", "steps-rows.mjs", "steps-cursor.mjs", ...readdirSync(new URL("steps/", here)).map((file) => `steps/${file}`)].map((file) => readFileSync(new URL(file, here), "utf8")).join("\n");
const played = SCENES.filter((name) => name !== "park");
assert.equal(SCENES[0], "scroll-to-critical", "drag-bar needs the critical task scroll-to-critical shows");
assert.equal(SCENES.at(-1), "drag-bar", "a drag leaves the Show moved button on the page, so it comes after find");
assert.ok(played.every((name) => featureNames.includes(name)), "every scene the stills script plays is in steps-features.mjs");
for (const still of STILLS.filter((item) => item.scene)) {
  assert.ok(SCENES.includes(still.scene), `${still.name}: ${still.scene} is played`);
  assert.ok(sceneSource.includes(`ctx.shot?.("${still.name}"`), `${still.name}: no scene takes a still with that name`);
}
assert.deepEqual(STILLS.filter((item) => item.scene).map((item) => item.scene), played.filter((name) => STILLS.some((item) => item.scene === name)), "scene stills come in play order");
const rows = JSON.parse(readFileSync(new URL("../../media/manifest.json", here), "utf8"));
for (const name of stillNames) assert.match(rows.find((row) => row.file === `book/src/media/${name}.webp`)?.record ?? "", /demos\/screencast\/stills\.mjs/, `${name}: the manifest re-records it with stills.mjs`);

console.log("test-story: ok");
