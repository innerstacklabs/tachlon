// The scene context and the scene runner, shared by the recorder (record.mjs) and the e2e spec (apps/hosted/tests/journey.spec.ts).
//
// A scene (steps.mjs, steps-features.mjs) is { name, caption, run, journey?, clip?, pre?, captionEnd? }:
//   journey  the book chapter that owns it ("J2")
//   clip     true when the scene is cut into a per-scene clip (gs#152)
//   pre      an expression run in the page before the scene: the state it needs from the scene before it
//
// Two modes, one scene list:
//   record  hold(s) is s seconds in frames, the caption overlay is drawn (the recorder installs it)
//   test    hold(s) is one frame, the caption calls are no-ops; a caption is still resolved and must be a non-empty string

export const MODES = ["record", "test"];

function makeContext(page, fps, mode, initialBox, initialStatus) {
  const recording = mode === "record";
  const ctx = {
    page,
    mode,
    fps,
    box: initialBox,
    ROW: initialStatus.rowHeight,
    AXIS: initialStatus.axisHeight,
    status: () => page.evaluate(() => window.gantt.status()),
    frames: (k = 2) => page.evaluate((count) => new Promise((done) => {
      const target = window.gantt.status().frames + count;
      const wait = () => (window.gantt.status().frames >= target ? done() : requestAnimationFrame(wait));
      requestAnimationFrame(wait);
    }), k),
    until: async (source, arg) => {
      try {
        return await page.waitForFunction(source, arg, { timeout: 15_000 });
      } catch (err) {
        throw new Error(`wait failed: ${String(source).replace(/\s+/g, " ").slice(0, 200)} with ${JSON.stringify(arg)}; page state ${await pageState(page)}`, { cause: err });
      }
    },
    log: (message) => console.log(`  ${message}`),
    setCaption: recording ? (text) => page.evaluate((value) => window.scOverlay.show(value), text ?? "") : async () => {},
    // The caption gains an ending after the step ("re-solved in 1.8 ms"), in place and without a fade.
    extendCaption: recording ? (ending) => page.evaluate((value) => window.scOverlay.extend(value), ending) : async () => {},
    // Circle a page element (a selector) for a moment; resolves when the last pulse is done.
    ring: recording ? (selector, options = {}) => page.evaluate(([target, opts]) => window.scOverlay.ring(target, opts), [selector, options]) : async () => {},
  };
  ctx.canvasBox = async () => {
    ctx.box = await page.locator("#timeline").boundingBox();
    if (!ctx.box) throw new Error("#timeline has no bounding box");
    return ctx.box;
  };
  ctx.hold = recording ? (seconds) => ctx.frames(Math.max(1, Math.round(seconds * ctx.fps))) : () => ctx.frames(1);
  return ctx;
}

async function pageState(page) {
  return page.evaluate(() => {
    const { hover, drag, selected, rowCount, level, criticalOnly, revision, scrollY } = window.gantt.status();
    return JSON.stringify({ hover, drag, selected, rowCount, level, criticalOnly, revision, scrollY });
  }).catch(() => "status unavailable");
}

/** The context for a page that is loaded and ready. `fps` is the measured frame rate, which only record mode reads. */
export async function createContext(page, { mode = "record", fps = 60 } = {}) {
  if (!MODES.includes(mode)) throw new Error(`unknown context mode "${mode}"; expected ${MODES.join(" or ")}`);
  const initialStatus = await page.evaluate(() => window.gantt.status());
  const box = await page.locator("#timeline").boundingBox();
  if (!box) throw new Error("#timeline has no bounding box");
  return makeContext(page, fps, mode, box, initialStatus);
}

function captionText(scene, text, what = "caption") {
  if (typeof text !== "string" || text.trim() === "") throw new Error(`scene ${scene.name}: the ${what} is not a non-empty string (${JSON.stringify(text)})`);
  return text;
}

/** A scene whose `pre` is false in the page fails here, naming the state it wanted, not 15 s later in a wait. */
async function checkPre(ctx, scene) {
  if (scene.pre === undefined) return;
  let held;
  try {
    held = await ctx.page.evaluate(scene.pre);
  } catch (err) {
    throw new Error(`scene ${scene.name}: pre could not be evaluated (${scene.pre}): ${err?.message ?? err}`, { cause: err });
  }
  if (!held) throw new Error(`scene ${scene.name}: pre is false (${scene.pre}); the scenes before it did not leave the state it needs; page state ${await pageState(ctx.page)}`);
}

/** Run one scene: its `pre` check, its caption and its actions. Resolves to the scene's own result. */
export async function runScene(ctx, scene) {
  await checkPre(ctx, scene);
  if (typeof scene.caption === "function") {
    const result = await scene.run(ctx);
    await ctx.setCaption(captionText(scene, await scene.caption(ctx, result)));
    await ctx.hold(1.5);
    return result;
  }
  await ctx.setCaption(captionText(scene, scene.caption));
  const result = await scene.run(ctx);
  // A scene may add a measured ending to its caption once it has the number; it writes its own punctuation ("; re-solved in 1.8 ms").
  const ending = scene.captionEnd ? await scene.captionEnd(ctx, result) : null;
  if (ending !== null && ending !== undefined) {
    captionText(scene, ending, "caption ending");
    await ctx.extendCaption(ending);
    await ctx.hold(1.5);
  }
  return result;
}

/**
 * A scene that declares `say` (its spoken line) lasts at least as long as that line's audio: `audioSeconds(text)` (injected,
 * so no voice is called here) gives the length, and the runner holds from the scene's start until it has passed.
 * With no `audioSeconds`, or no `say`, the scene is not held.
 */
async function holdForNarration(ctx, scene, startMs, { now, audioSeconds }) {
  if (scene.say === undefined || !audioSeconds) return;
  const seconds = await audioSeconds(captionText(scene, scene.say, "say line"));
  if (!Number.isFinite(seconds) || seconds < 0) throw new Error(`scene ${scene.name}: audioSeconds gave ${seconds} for its say line`);
  const endMs = startMs + seconds * 1000;
  while (now() < endMs) await ctx.hold((endMs - now()) / 1000);
}

/**
 * Run the list in order. `now` is the clock the scene windows are read from; the recorder reads the same clock for the
 * moment the page was created, so a window is a position in the recorded video (gs#152, clip cutting).
 * `audioSeconds(text)`, when given, is the length of a scene's `say` line; the scene's window is at least that long.
 * Resolves to the names that ran and, for each scene, its window in ms.
 */
export async function runScenes(ctx, list, { now = Date.now, audioSeconds } = {}) {
  const run = [];
  const windows = [];
  for (let i = 0; i < list.length; i++) {
    const scene = list[i];
    console.log(`scene ${i + 1}/${list.length} ${scene.name}`);
    const startMs = now();
    try {
      await runScene(ctx, scene);
      await holdForNarration(ctx, scene, startMs, { now, audioSeconds });
    } catch (err) {
      console.error(`scene ${scene.name}: ${err?.message ?? err}`);
      throw Object.assign(new Error(`scene ${scene.name} failed`), { cause: err });
    }
    run.push(scene.name);
    windows.push({ name: scene.name, clip: scene.clip === true, startMs, endMs: now() });
  }
  await ctx.frames(2);
  return { run, windows };
}
