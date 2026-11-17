// Checks the scene context and runner with no browser: node demos/screencast/test-ctx.mjs
// In test mode a hold is one frame and the caption calls draw nothing, but a function caption is still called and a
// caption that is not text fails; a scene whose `pre` is false in the page fails before it runs; record mode keeps its holds.
import assert from "node:assert/strict";
import { MODES, createContext, runScene, runScenes } from "./ctx.mjs";

// A page that answers what createContext asks of it. `pre` expressions are answered from `answers`; every other
// evaluate (the overlay calls) is recorded in `overlay`.
function fakePage(answers = {}) {
  const overlay = [];
  const evaluated = [];
  return {
    overlay,
    evaluated,
    evaluate: async (fn, arg) => {
      const source = String(fn);
      if (typeof fn === "string") {
        evaluated.push(fn);
        if (!(fn in answers)) throw new Error(`unexpected pre ${fn}`);
        return answers[fn];
      }
      if (source.includes("scOverlay")) {
        overlay.push(source.match(/scOverlay\.(\w+)/)[1]);
        return undefined;
      }
      if (source.includes("criticalOnly")) return JSON.stringify({ level: "year" });
      return source.includes("status()") ? { rowHeight: 24, axisHeight: 48 } : arg;
    },
    locator: () => ({ boundingBox: async () => ({ x: 672, y: 36, width: 608, height: 684 }) }),
  };
}

// Count the frames a hold asks for: the context reads ctx.frames at call time.
async function holds(mode, fps, seconds) {
  const ctx = await createContext(fakePage(), { mode, fps });
  const asked = [];
  ctx.frames = async (k) => { asked.push(k); };
  await ctx.hold(seconds);
  return asked;
}

assert.deepEqual(MODES, ["record", "test"]);
await assert.rejects(() => createContext(fakePage(), { mode: "demo" }), /unknown context mode "demo"/);

// Test mode: a hold is one frame, however long and at whatever frame rate. Record mode: the seconds in frames.
assert.deepEqual(await holds("test", 60, 1.5), [1], "test-mode hold is one frame");
assert.deepEqual(await holds("test", 6, 30), [1], "test-mode hold ignores the seconds");
assert.deepEqual(await holds("record", 60, 1.5), [90], "record-mode hold is seconds in frames");
assert.deepEqual(await holds("record", 6, 0.01), [1], "record-mode hold is at least one frame");

// Test mode draws nothing: the caption, its ending and the ring never reach the page's overlay.
{
  const page = fakePage();
  const ctx = await createContext(page, { mode: "test" });
  ctx.frames = async () => {};
  await runScene(ctx, { name: "s", caption: "Shown", captionEnd: () => "; ending", run: async () => {} });
  assert.deepEqual(page.overlay, [], "test mode draws no overlay");
  await ctx.ring("#row-count");
  assert.deepEqual(page.overlay, []);
}
// Record mode draws them.
{
  const page = fakePage();
  const ctx = await createContext(page, { mode: "record", fps: 60 });
  ctx.frames = async () => {};
  await runScene(ctx, { name: "s", caption: "Shown", captionEnd: () => "; ending", run: async () => {} });
  assert.deepEqual(page.overlay, ["show", "extend"]);
}

// A function caption is called with the context and the scene's result, in test mode too, and must return text.
{
  const ctx = await createContext(fakePage(), { mode: "test" });
  ctx.frames = async () => {};
  const calls = [];
  await runScene(ctx, {
    name: "measured",
    caption: (given, result) => {
      calls.push([given === ctx, result]);
      return `re-solved in ${result.ms} ms`;
    },
    run: async () => ({ ms: 1.8 }),
  });
  assert.deepEqual(calls, [[true, { ms: 1.8 }]], "the function caption was called once, with the result");
  for (const bad of [() => "", () => "  ", () => undefined, () => 12, ""]) {
    await assert.rejects(() => runScene(ctx, { name: "bad", caption: bad, run: async () => ({}) }), /scene bad: the caption is not a non-empty string/);
  }
}

// A caption ending is called in test mode too, with the context and the result; it is text, or null for none.
{
  const ctx = await createContext(fakePage(), { mode: "test" });
  ctx.frames = async () => {};
  const calls = [];
  await runScene(ctx, {
    name: "ending",
    caption: "C",
    captionEnd: (given, result) => {
      calls.push([given === ctx, result]);
      return "(re-solved in 1.8 ms)";
    },
    run: async () => ({ ms: 1.8 }),
  });
  assert.deepEqual(calls, [[true, { ms: 1.8 }]], "the caption ending was called once, with the result");
  for (const none of [() => null, () => undefined]) await runScene(ctx, { name: "none", caption: "C", captionEnd: none, run: async () => ({}) });
  for (const bad of [() => "", () => "  ", () => 1.8, () => ({ text: "x" })]) {
    await assert.rejects(() => runScene(ctx, { name: "bad", caption: "C", captionEnd: bad, run: async () => ({}) }), /scene bad: the caption ending is not a non-empty string/);
  }
}

// A scene whose `pre` is false fails before it runs, and says which scene and what it wanted.
{
  const pre = 'window.gantt.status().level === "day"';
  const answers = { [pre]: false };
  const page = fakePage(answers);
  const ctx = await createContext(page, { mode: "test" });
  ctx.frames = async () => {};
  let ran = false;
  await assert.rejects(
    () => runScene(ctx, { name: "needs-day", pre, caption: "C", run: async () => { ran = true; } }),
    /scene needs-day: pre is false \(window\.gantt\.status\(\)\.level === "day"\).*"level":"year"/s,
  );
  assert.equal(ran, false, "the scene does not run when its pre is false");
  assert.deepEqual(page.evaluated, [pre], "the pre was evaluated in the page");

  answers[pre] = true;
  await runScene(ctx, { name: "needs-day", pre, caption: "C", run: async () => { ran = true; } });
  assert.equal(ran, true, "the scene runs when its pre holds");
}

// runScenes names the scene that failed and stops there; a list that passes returns every name.
{
  const ctx = await createContext(fakePage(), { mode: "test" });
  ctx.frames = async () => {};
  const order = [];
  const scene = (name, fail) => ({ name, caption: name, run: async () => { order.push(name); if (fail) throw new Error("boom"); } });
  const { run } = await runScenes(ctx, [scene("a"), scene("b")]);
  assert.deepEqual(run, ["a", "b"]);
  await assert.rejects(() => runScenes(ctx, [scene("c"), scene("d", true), scene("e")]), /scene d failed/);
  assert.deepEqual(order, ["a", "b", "c", "d"], "e never ran");
}

// runScenes reads each scene's window off the clock it is given: the recorder cuts clips from these (gs#152).
{
  const ctx = await createContext(fakePage(), { mode: "test" });
  ctx.frames = async () => {};
  let ticks = 0;
  const now = () => (ticks += 1000);
  const scene = (name, clip) => ({ name, clip, caption: name, run: async () => {} });
  const { windows } = await runScenes(ctx, [scene("a", true), scene("b"), scene("c", true)], { now });
  assert.deepEqual(windows, [
    { name: "a", clip: true, startMs: 1000, endMs: 2000 },
    { name: "b", clip: false, startMs: 3000, endMs: 4000 },
    { name: "c", clip: true, startMs: 5000, endMs: 6000 },
  ]);
}

console.log("test-ctx: ok");
