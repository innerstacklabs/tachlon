// The steps of the README story that run on the 50,000-task schedule (gs#392, gs#393): open, drag, fling, close.
// steps.mjs puts them in order with the P6 steps (p6.mjs); the storyboard is gs#384. The video opens on a schedule a person can
// read (gs#508): the pre-roll (criticalView) leaves the critical path on the chart, and the first action is a drag that moves it.
// The fast scroll through all 50,000 tasks comes after it, short, with the slowest frame it measured in its caption.
import { criticalPoint, firstCritical, panFirstBarTo, revealCritical, shared, zoomTo } from "./helpers.mjs";

const centre = (box) => ({ x: box.x + box.width / 2, y: box.y + box.height / 2 });

/**
 * The story's pre-roll, after the opening view (opening.mjs): put the first critical task on the chart, its bar a fifth in
 * from the left at day zoom, so the first frame of the video is the schedule a scheduler reads: bars, names and the critical
 * path in red. The 50,000-task fixture's critical path is in its last rows, so this is the bottom of the schedule. The
 * task is the first critical row the engine reports (the drag step moves it).
 */
export async function criticalView(ctx) {
  const located = await firstCritical(ctx);
  if (located === null) throw new Error("the schedule has no critical task, so the video has no critical path to open on");
  shared.criticalRow = located.row;
  shared.task = located.task;
  await revealCritical(ctx);
  await ctx.until(() => window.gantt.status().criticalBars > 0);
  const box = await ctx.canvasBox();
  await ctx.page.mouse.move(box.x + box.width * 0.85, box.y + ctx.AXIS + ctx.ROW * 8);
  await ctx.hold(1);
  ctx.log(`opening on the critical path: row ${located.row}, task ${located.task}`);
}

export const open = {
  name: "open",
  journey: "J1",
  caption: "50,000 tasks open: bars, names and the critical path in red",
  run: async (ctx) => {
    await ctx.until(() => window.gantt.status().n === 50000);
    // The first scene must be a schedule that reads: the day level, bars on the chart and critical bars among them.
    const status = await ctx.status();
    if (status.level !== "day" || status.bars === 0 || status.criticalBars === 0) {
      throw new Error(`open step: the first view does not show a readable schedule with its critical path (level ${status.level}, ${status.bars} bars, ${status.criticalBars} critical)`);
    }
    await ctx.ring("#row-count");
    await ctx.hold(1.5);
  },
};

/** Start counting the gaps between animation frames; `stopFrameWatch` returns the slowest, in ms (the fling caption's number). */
const startFrameWatch = (ctx) =>
  ctx.page.evaluate(() => {
    const gaps = [];
    let last = performance.now();
    let on = true;
    const tick = (now) => {
      if (!on) return;
      gaps.push(now - last);
      last = now;
      requestAnimationFrame(tick);
    };
    requestAnimationFrame(tick);
    window.scFrameWatch = () => {
      on = false;
      return gaps.slice(1); // the first gap includes the set-up
    };
  });
const stopFrameWatch = async (ctx) => {
  const gaps = await ctx.page.evaluate(() => window.scFrameWatch());
  if (gaps.length === 0) throw new Error("fling step: no animation frame ran during the scroll");
  return Math.round(Math.max(...gaps) * 10) / 10;
};

export const fling = {
  name: "fling",
  journey: "J2",
  caption: "Blazingly fast: all 50,000 tasks, top to bottom",
  run: async (ctx) => {
    const box = await ctx.canvasBox();
    const middle = centre(box);
    // Zoom out until the whole project is on the chart with the first bar near the left edge: the year level alone can still be
    // narrower than the project. The widest zoom is five years across; a longer project is flung at month zoom instead.
    const long = shared.finish !== null && shared.finish.years > 4.5;
    await zoomTo(ctx, middle, long ? "month" : "year");
    await ctx.page.evaluate(() => window.gantt.app.set_scroll_y(0));
    await panFirstBarTo(ctx, box, 0.05);
    if (shared.finish !== null && !long) {
      const anchor = { x: box.x + box.width * 0.05, y: middle.y };
      const endX = (finish) => ctx.page.evaluate((minutes) => window.gantt.app.x_of(minutes), finish);
      for (let i = 0; i < 20 && (await endX(shared.finish.minutes)) > box.width * 0.95; i++) {
        await ctx.page.mouse.move(anchor.x, anchor.y);
        await ctx.page.keyboard.down("Control");
        try {
          await ctx.page.mouse.wheel(0, 120);
        } finally {
          await ctx.page.keyboard.up("Control");
        }
        await ctx.frames(2);
      }
      const x = await endX(shared.finish.minutes);
      if (x > box.width) throw new Error(`fling step: the project finish is off the chart at the widest zoom (x ${x.toFixed(0)} of ${box.width.toFixed(0)})`);
      await panFirstBarTo(ctx, box, 0.05);
    }
    await ctx.until(() => window.gantt.status().scrollY === 0);
    const { rowCount } = await ctx.status();
    const total = rowCount * ctx.ROW;
    const events = Math.max(10, Math.round(ctx.fps)); // one way, about 1 s, whatever the frame rate
    await startFrameWatch(ctx);
    for (let k = 0; k < events; k++) {
      await ctx.page.mouse.wheel(0, total / events);
      await ctx.frames(1);
    }
    const last = total - (box.height - ctx.AXIS);
    await ctx.until((end) => window.gantt.status().scrollY >= end - 1, last);
    const slowestMs = await stopFrameWatch(ctx);
    await ctx.ring("#row-count");
    return { slowestMs };
  },
  // The caption gains the measured slowest frame of the scroll.
  captionEnd: (_ctx, result) => `(slowest frame ${result.slowestMs.toFixed(1)} ms)`,
};

export const drag = {
  name: "drag",
  journey: "J7",
  caption: "Drag a critical task: the schedule re-solves as you move",
  run: async (ctx) => {
    // The pre-roll left the first critical task on the chart; this brings it back if a step before moved the view.
    await revealCritical(ctx);
    const box = await ctx.canvasBox();
    const task = shared.task;
    const row = shared.criticalRow;
    const point = await criticalPoint(ctx, 4);
    if (point.x < box.x || point.x > box.x + box.width) throw new Error(`drag step: the bar on row ${row} starts off the chart (x ${point.x.toFixed(0)})`);
    await ctx.page.mouse.move(point.x, point.y);
    await ctx.until((id) => window.gantt.status().hover?.task === id, task);
    const revision = (await ctx.status()).revision;
    const before = await ctx.page.evaluate((r) => window.gantt.app.row_start(r), row);
    let recomputeMs = 0;
    let measured = false;
    const note = (drag) => {
      if (typeof drag?.recomputeMs !== "number") return;
      measured = true;
      recomputeMs = Math.max(recomputeMs, drag.recomputeMs);
    };
    await ctx.page.mouse.down();
    try {
      await ctx.page.mouse.move(point.x + 4, point.y);
      await ctx.frames(1);
      await ctx.until(() => window.gantt.status().drag?.active === true);
      note((await ctx.status()).drag);
      for (let k = 1; k <= 8; k++) {
        await ctx.page.mouse.move(point.x + 15 * k, point.y);
        await ctx.frames(2);
        const status = await ctx.status();
        if (status.drag?.active) note(status.drag);
      }
    } finally {
      await ctx.page.mouse.up();
    }
    await ctx.until((was) => window.gantt.status().revision > was, revision);
    const after = await ctx.page.evaluate((r) => window.gantt.app.row_start(r), row);
    if (!(after > before)) throw new Error(`drag step: task ${task} did not move later (${before} -> ${after})`);
    if (!measured) throw new Error("drag step: the drag status never reported recomputeMs");
    await ctx.hold(1);
    // What the release moved, as the page reports it ("8 tasks moved"), null when nothing else moved.
    const moved = (await ctx.status()).moved?.text ?? null;
    // The release moves the critical path to other rows, and the first critical task is no longer in view: the red the viewer
    // saw goes out of frame (gs#575). Glide the view to the new first critical task and stay there for a beat, so the scene
    // shows what the drag changed.
    const now = await firstCritical(ctx);
    if (now === null) throw new Error(`drag step: the schedule has no critical task after the drag of task ${task}`);
    await revealCritical(ctx, now, Math.max(8, Math.round(ctx.fps / 2)));
    await ctx.frames(2);
    const shown = await ctx.status();
    if (shown.criticalBars === 0) throw new Error(`drag step: no critical bar is on screen after the drag (the first critical task is row ${now.row}, scroll ${shown.scrollY} px, rows ${shown.rows[0]} to ${shown.rows[1]})`);
    ctx.log(`the critical path after the drag: row ${now.row}, task ${now.task}, ${shown.criticalBars} critical bars on screen`);
    await ctx.hold(1.5);
    return { recomputeMs: Math.round(recomputeMs * 10) / 10, moved };
  },
  // The caption gains what moved and the measured time once the drag is released.
  captionEnd: (_ctx, result) => `(${result.moved === null ? "" : `${result.moved}; `}re-solved in ${result.recomputeMs.toFixed(1)} ms)`,
};

export const close = {
  name: "close",
  caption: "Rust, WebAssembly and the GPU, built as a component. github.com/innerstacklabs/gantt",
  run: async (ctx) => {
    await ctx.hold(2);
  },
};
