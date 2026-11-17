// Checks the recording guard of clip.mjs (gs#654) with no browser and no ffmpeg: node demos/screencast/test-clip-guard.mjs
// A recording is the metadata clip.mjs reads from the page: { backend, fps }. Into book/src/media it must be WebGPU at
// 50 fps or more (the guard of copy-clips.mjs, reused); into any other folder nothing is checked. A fake result stands in
// for the browser, so the test says what the guard decides; the last block reads clip.mjs to check the guard sits between
// the scene and the first write, because no test here can run the page.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { clipLengthProblem, guardClip, parseArgs, writesToBookMedia } from "./clip.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const book = join(here, "..", "..", "book", "src", "media");
const scratch = join(tmpdir(), "gantt-clip-guard-test");
const software = { backend: "WebGL2", fps: 6 };
const slowGpu = { backend: "WebGPU", fps: 30 };

// Where a clip goes: the book's media folder, in this checkout or any other, and what is inside it.
assert.equal(writesToBookMedia(book), true);
assert.equal(writesToBookMedia(join(book, "features")), true, "a folder inside book/src/media");
assert.equal(writesToBookMedia("/work/other-checkout/book/src/media"), true, "another checkout's book");
assert.equal(writesToBookMedia("book/src/media"), true, "a relative --out");
assert.equal(writesToBookMedia(scratch), false);
assert.equal(writesToBookMedia(join(here, "draft")), false);
assert.equal(writesToBookMedia(join(book, "..", "media-draft")), false, "a sibling folder is not the book's media");
assert.equal(writesToBookMedia(join(tmpdir(), "mybook", "src", "media")), false, "book is a whole path segment");

// Software GL into the book is refused, and the message says why and where to record.
assert.throws(() => guardClip(book, software, "open-file"), /clip open-file: backend is "WebGL2", not "WebGPU".*MacBook.*outside book\/src\/media.*nothing was written/s);
// Into a scratch folder it is allowed, so a hub draft can still be cut.
assert.doesNotThrow(() => guardClip(scratch, software, "open-file"));
assert.doesNotThrow(() => guardClip(scratch, undefined, "open-file"), "outside the book no recording is read at all");
// WebGPU at 50 fps is allowed into the book; 49.9 is not; nor is a missing fps or backend.
assert.doesNotThrow(() => guardClip(book, { backend: "WebGPU", fps: 50 }, "open-file"));
assert.doesNotThrow(() => guardClip(book, { backend: "WebGPU", fps: 120 }, "open-file"));
assert.throws(() => guardClip(book, { backend: "WebGPU", fps: 49.9 }, "open-file"), /clip open-file: fps is 49\.9, under 50/);
assert.throws(() => guardClip(book, slowGpu, "open-file"), /fps is 30, under 50/);
assert.throws(() => guardClip(book, { backend: "WebGPU" }, "open-file"), /fps is undefined/);
assert.throws(() => guardClip(book, undefined, "open-file"), /backend is undefined/);

// A stills-only run (gantt#1557) takes the flag with no value, and the clip-length check does not apply: the stills are the product.
assert.equal(parseArgs(["--step", "open-file", "--out", scratch, "--stills-only"]).stillsOnly, true);
assert.equal(parseArgs(["--stills-only", "--step", "open-file", "--out", scratch]).stillsOnly, true, "the flag may come first");
assert.equal(parseArgs(["--step", "open-file", "--out", scratch]).stillsOnly, undefined);
assert.match(clipLengthProblem(4.2, false), /the clip is 4\.2 s; the budget is 10 to 20 s/);
assert.equal(clipLengthProblem(4.2, true), undefined);
assert.equal(clipLengthProblem(12, false), undefined);

// clip.mjs calls the guard after the scene has run and before it creates the output folder or runs ffmpeg, and it reads
// the backend itself (a recording that says WebGPU only because the caller claimed it would pass).
const source = readFileSync(join(here, "clip.mjs"), "utf8");
const at = (text) => {
  const index = source.indexOf(text);
  assert.ok(index >= 0, `clip.mjs has ${text}`);
  return index;
};
const guard = at("guardClip(out, writesToBookMedia(out)");
assert.ok(at("await runScenes(ctx, [step])") < guard, "the guard comes after the scene");
assert.ok(guard < at("await mkdir(out"), "the guard comes before the output folder is made");
assert.ok(guard < at("await execFile(ffmpeg"), "the guard comes before the first encode");
assert.match(source, /window\.gantt\.status\(\)\)\)\.backend, fps \} : undefined, options\.step/, "the backend comes from the page and the fps from measureFps");
assert.equal(source.split("await mkdir(out").length, 2, "the output folder is made once, after the guard");
console.log("ok: clip.mjs refuses a non-WebGPU or under-50 fps recording into book/src/media");
