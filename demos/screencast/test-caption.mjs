// Checks the words of the open-file caption with no browser: node demos/screencast/test-caption.mjs
// The scene's caption must name both file kinds the Open button reads (P6 and MS Project) and must not say the file stays in
// the browser (the page has no such promise to make, gs#633). test-story.mjs and test-ctx.mjs only require a non-empty caption.
import assert from "node:assert/strict";
import * as features from "./steps-features.mjs";

// The problems with a caption, as sentences; none means the wording is right.
const problems = (caption) => {
  const found = [];
  if (!/\bP6\b/.test(caption)) found.push("does not name P6");
  if (!/\bMS Project\b/.test(caption)) found.push("does not name MS Project");
  if (/stays? in (this|the) browser/i.test(caption)) found.push("says the file stays in the browser");
  return found;
};

// The checker itself: the old caption and each half-right one are caught, the current wording is not.
assert.deepEqual(problems("Open a P6 file: it stays in this browser"), ["does not name MS Project", "says the file stays in the browser"]);
assert.deepEqual(problems("Open a P6 file"), ["does not name MS Project"]);
assert.deepEqual(problems("Open an MS Project file"), ["does not name P6"]);
assert.deepEqual(problems("Open a P6 or MS Project file; the file stays in the browser"), ["says the file stays in the browser"]);
assert.deepEqual(problems("Open a P6 or MS Project file"), []);

// The real scene. A function caption is called the way the scene runner calls it (ctx.mjs, runScene), with no result yet.
const scene = features.steps.find((step) => step.name === "open-file");
assert.ok(scene, "steps-features.mjs holds the open-file scene");
const caption = typeof scene.caption === "function" ? await scene.caption({}, undefined) : scene.caption;
assert.equal(typeof caption, "string", "the open-file caption is text");
assert.deepEqual(problems(caption), [], `open-file caption "${caption}"`);
console.log("test-caption: ok");
