// Checks the recording frame with no browser: node demos/screencast/test-frame.mjs
// The pre-roll stamps the cut after the set-up; a caption shows before its step and gains its ending after; zoomTo
// zooms in from a coarser level and out from a finer one.
import assert from "node:assert/strict";
import { preRoll } from "./record/browser.mjs";
import { runScenes } from "./ctx.mjs";
import { zoomTo } from "./steps/helpers.mjs";

// A page that answers what createContext asks of it.
const fakePage = {
  evaluate: async (fn) => (String(fn).includes("status()") ? { rowHeight: 24, axisHeight: 48 } : true),
  locator: () => ({ boundingBox: async () => ({ x: 672, y: 36, width: 608, height: 684 }) }),
};

// The stamp is taken after the set-up, never before it. The test holds the set-up open on a gate and uses its own
// counting clock, so no timer or wall clock decides the outcome.
{
  const realNow = Date.now;
  let tick = 0;
  Date.now = () => ++tick;
  try {
    let open;
    const gate = new Promise((resolve) => (open = resolve));
    let setupDone = 0;
    let settled = false;
    const rolling = preRoll(fakePage, 60, async () => {
      await gate;
      setupDone = Date.now();
    }).then((result) => {
      settled = true;
      return result;
    });
    await new Promise((resolve) => setImmediate(resolve)); // every ready microtask has run
    assert.equal(settled, false, "the stamp waited for the set-up");
    open();
    const { tReady } = await rolling;
    assert.ok(setupDone > 0, "setup ran");
    assert.ok(tReady > setupDone, "the cut is stamped after the set-up");
  } finally {
    Date.now = realNow;
  }
}

// With no setup export there is nothing to wait for, and the pre-roll still returns a context.
{
  const { ctx, tReady } = await preRoll(fakePage, 60, null);
  assert.equal(ctx.ROW, 24);
  assert.ok(tReady > 0);
}

// A caption shows before its step runs; the ending, when the step gives one, follows the step.
{
  const { ctx } = await preRoll(fakePage, 60, null);
  const calls = [];
  ctx.setCaption = async (text) => calls.push(`caption:${text}`);
  ctx.extendCaption = async (text) => calls.push(`ending:${text}`);
  ctx.hold = async () => calls.push("hold");
  ctx.frames = async () => {};
  const list = [
    { name: "plain", caption: "A", run: async () => calls.push("run:plain") },
    {
      name: "measured",
      caption: "B",
      run: async () => {
        calls.push("run:measured");
        return { ms: 1.8 };
      },
      captionEnd: (_ctx, result) => `re-solved in ${result.ms} ms`,
    },
    { name: "none", caption: "C", run: async () => {}, captionEnd: () => null },
  ];
  const { run } = await runScenes(ctx, list);
  assert.deepEqual(run, ["plain", "measured", "none"]);
  assert.deepEqual(calls, [
    "caption:A", "run:plain",
    "caption:B", "run:measured", "ending:re-solved in 1.8 ms", "hold",
    "caption:C",
  ]);
}

// zoomTo: Ctrl+wheel up zooms in, down zooms out, by the order of the axis levels.
for (const [from, to, delta] of [["year", "day", -240], ["day", "week", 240], ["hour", "month", 240], ["week", "hour", -240]]) {
  let level = from;
  const wheels = [];
  const ctx = {
    status: async () => ({ level }),
    frames: async () => {
      level = to; // one wheel step reaches the target
    },
    page: {
      mouse: { move: async () => {}, wheel: async (_x, y) => wheels.push(y) },
      keyboard: { down: async () => {}, up: async () => {} },
    },
  };
  await zoomTo(ctx, { x: 1, y: 1 }, to);
  assert.deepEqual(wheels, [delta], `${from} to ${to}`);
}
await assert.rejects(() => zoomTo({}, { x: 0, y: 0 }, "decade"), /unknown zoom level/);

// screencast.json carries each step's start and end in seconds from the cut, and a recording over 45 s fails (gs#384).
// The windows and the duration are fake: absolute ms as runScenes writes them, a page opened at 1000 ms, a cut 2 s in.
{
  const { stepWindows, maxSeconds, checkDuration } = await import("./record.mjs");
  const windows = [
    { name: "open", clip: false, startMs: 4000, endMs: 9500 },
    { name: "drag", clip: false, startMs: 9500, endMs: 20000 },
  ];
  assert.deepEqual(stepWindows(windows, 1000, 2), [
    { name: "open", startSeconds: 1, endSeconds: 6.5 },
    { name: "drag", startSeconds: 6.5, endSeconds: 17 },
  ]);
  assert.equal(stepWindows([{ name: "early", startMs: 1500, endMs: 2500 }], 1000, 2)[0].startSeconds, 0, "never before the cut");
  assert.equal(maxSeconds, 45);
  assert.equal(checkDuration(45), null, "45 s is allowed");
  assert.match(checkDuration(46), /46\.0 s.*45 s/, "a 46 s run fails the recording");
}

console.log("test-frame: ok");
