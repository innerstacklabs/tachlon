// Helpers shared by the screencast steps: the located critical task and the moves that reach it.

export const shared = {
  criticalRow: null,
  task: null,
  finish: null, // the project finish, read once by the pre-roll: { minutes, years }, null where the build cannot say
};

/**
 * Answer "open anyway?" with OK, as a person would: opening a file over unsaved changes asks first (gs#703), and the
 * scenes that open a file do so after earlier scenes edited the schedule. Call it before the file is chosen.
 */
export function acceptOpenAsk(page) {
  page.once("dialog", (dialog) => void dialog.accept());
}

export async function criticalPoint(ctx, inset = 1) {
  if (shared.criticalRow === null || shared.task === null) throw new Error("critical task has not been located");
  const box = await ctx.canvasBox();
  const point = await ctx.page.evaluate(({ row, task }) => {
    const app = window.gantt.app;
    const status = window.gantt.status();
    const current = JSON.parse(app.table_rows(row, row + 1))[0];
    if (current?.[0] !== task) throw new Error(`critical row ${row} no longer holds task ${task}`);
    return {
      x: app.x_of(app.row_start(row)),
      y: status.axisHeight + row * status.rowHeight - status.scrollY + status.rowHeight / 2,
    };
  }, { row: shared.criticalRow, task: shared.task });
  return { x: box.x + point.x + inset, y: box.y + point.y };
}

/**
 * The first critical task of the schedule as it stands now, `{ row, task }`, or null with no critical task. The pre-roll reads it
 * once (`shared`); after an edit the critical path is somewhere else, and this reads where.
 */
export const firstCritical = (ctx) =>
  ctx.page.evaluate(() => {
    const app = window.gantt.app;
    const row = app.test_first_critical_row() ?? null;
    if (row === null) return null;
    const task = JSON.parse(app.table_rows(row, row + 1))[0]?.[0] ?? null;
    return task === null ? null : { row, task };
  });

/**
 * Wheel the chart until the bar of `target` (default: the pre-roll's critical task) starts a fifth in from the left, with its row
 * near the top. `glide` is the number of wheel events to spread the first leg of the way over, one a frame (a camera move a viewer
 * follows, gs#575); 0 goes in the fewest, large events (the pre-roll, which is not on the video).
 */
export async function revealCritical(ctx, target = { row: shared.criticalRow, task: shared.task }, glide = 0) {
  const { row, task } = target;
  if (row === null || task === null) throw new Error("critical task has not been located");
  const box = await ctx.canvasBox();
  await ctx.page.mouse.move(box.x + box.width / 2, box.y + ctx.AXIS + 10 * ctx.ROW);
  const offsetOf = () =>
    ctx.page.evaluate(({ row, task, width, rowHeight }) => {
      const app = window.gantt.app;
      const status = window.gantt.status();
      const current = JSON.parse(app.table_rows(row, row + 1))[0];
      if (current?.[0] !== task) throw new Error(`critical row ${row} no longer holds task ${task}`);
      return {
        dx: app.x_of(app.row_start(row)) - width / 5,
        dy: Math.max(0, (row - 5) * rowHeight) - status.scrollY,
      };
    }, { row, task, width: box.width, rowHeight: ctx.ROW });
  if (glide > 0) {
    const { dx, dy } = await offsetOf();
    for (let k = 0; k < glide; k++) {
      await ctx.page.mouse.wheel(dx / glide, dy / glide);
      await ctx.frames(1);
    }
  }
  let before = Infinity;
  for (let i = 0; i < 1000; i++) {
    const offset = await offsetOf();
    const distance = Math.abs(offset.dx) + Math.abs(offset.dy);
    if (distance < 1 || distance >= before) break;
    before = distance;
    const step = (value) => Math.sign(value) * Math.min(Math.abs(value), 3000);
    await ctx.page.mouse.wheel(step(offset.dx), step(offset.dy));
    await ctx.frames(1);
  }
  await ctx.until(({ row, task, width }) => {
    const app = window.gantt.app;
    const status = window.gantt.status();
    const current = JSON.parse(app.table_rows(row, row + 1))[0];
    const x = app.x_of(app.row_start(row));
    const y = status.axisHeight + row * status.rowHeight - status.scrollY + status.rowHeight / 2;
    return current?.[0] === task && x >= 0 && x < width && y >= status.axisHeight;
  }, { row, task, width: box.width });
}

/** A page point `inset` px into the bar on `row`, where the scrolled canvas draws it. */
export const barAt = (ctx, box, row, inset, dy = 0) =>
  ctx.page.evaluate(({ box, row, inset, dy, axis, rowHeight }) => {
    const app = window.gantt.app;
    const x = box.x + app.x_of(app.row_start(row)) + inset;
    return { x, y: box.y + axis + row * rowHeight - window.gantt.status().scrollY + rowHeight / 2 + dy };
  }, { box, row, inset, dy, axis: ctx.AXIS, rowHeight: ctx.ROW });

// Finest to coarsest, the order of the axis levels.
const LEVELS = ["hour", "day", "week", "month", "year"];

// Ctrl+wheel at `point` until the axis reads `level`, zooming in from a coarser level and out from a finer one
// (the pre-roll leaves the view at day zoom, so a step that wants weeks has to zoom out).
export async function zoomTo(ctx, point, level) {
  if (!LEVELS.includes(level)) throw new Error(`unknown zoom level ${level}`);
  for (let i = 0; i < 200; i++) {
    const current = (await ctx.status()).level;
    if (current === level) return;
    const delta = LEVELS.indexOf(current) > LEVELS.indexOf(level) ? -240 : 240;
    await ctx.page.mouse.move(point.x, point.y);
    await ctx.page.keyboard.down("Control");
    try {
      await ctx.page.mouse.wheel(0, delta);
    } finally {
      await ctx.page.keyboard.up("Control");
    }
    await ctx.frames(2);
  }
  const status = await ctx.status();
  throw new Error(`did not reach ${level} after 200 zoom steps (at ${status.level})`);
}

// Pan with the wheel until the first row's bar starts `fraction` of the way across the chart (a find jump puts it at a
// fifth). Nothing here moves the rows; the pointer has to be over the chart.
export async function panFirstBarTo(ctx, box, fraction) {
  let before = Infinity;
  for (let i = 0; i < 100; i++) {
    const dx = await ctx.page.evaluate(({ width, fraction }) => {
      const app = window.gantt.app;
      return app.x_of(app.row_start(0)) - width * fraction;
    }, { width: box.width, fraction });
    if (Math.abs(dx) < 1) return;
    if (Math.abs(dx) >= before) throw new Error(`the pan toward the first bar stopped converging (${dx.toFixed(1)} px off)`);
    before = Math.abs(dx);
    await ctx.page.mouse.wheel(Math.sign(dx) * Math.min(Math.abs(dx), 3000), 0);
    await ctx.frames(1);
  }
  throw new Error("the pan toward the first bar did not finish in 100 wheel events");
}

/** The filters sit in the toolbar's Filters menu (gs#808): open it by key if it is shut. */
export async function openFilters(page) {
  if (await page.locator("#filters-panel").isHidden()) {
    await page.focus("#filters-button");
    await page.keyboard.press("Enter");
  }
}
