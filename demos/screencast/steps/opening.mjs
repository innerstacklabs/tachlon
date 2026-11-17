// The pre-roll: put the page on the opening view before the video starts. The page opens fitted to the dates of its
// first screen of rows (gs#821), which for the 50,000 task demo is already day zoom; the video opens on the top rows at
// day zoom with the first row's bar starting a fifth of the way in from the left. Runs after the frame-rate check, so none of it is filmed.

import { panFirstBarTo, shared, zoomTo } from "./helpers.mjs";

const MINUTES_PER_YEAR = 365 * 24 * 60;

export async function openingView(ctx) {
  const box = await ctx.canvasBox();
  const first = await ctx.page.evaluate(() => {
    const app = window.gantt.app;
    app.set_scroll_y(0);
    const start = app.row_start(0);
    if (start === undefined) throw new Error("the schedule has no first row");
    return {
      x: app.x_of(start),
      finish: typeof app.test_project_finish === "function" ? app.test_project_finish() : null,
    };
  });
  // The fling needs to know whether the whole project fits the widest zoom (five years across the chart).
  shared.finish = first.finish === null ? null : { minutes: first.finish, years: first.finish / MINUTES_PER_YEAR };

  // Zoom with the pointer on the first bar's start: the zoom keeps the instant under the pointer where it is.
  const point = { x: box.x + Math.min(Math.max(first.x, 20), box.width - 20), y: box.y + ctx.AXIS + ctx.ROW / 2 };
  await ctx.page.mouse.move(point.x, point.y); // the wheel pans the chart only with the pointer over it, and a page already at day zoom does not move it
  await zoomTo(ctx, point, "day");

  // Pan the bar's start to a fifth in, the same place a find jump puts it.
  await panFirstBarTo(ctx, box, 1 / 5);

  // Park the pointer on empty chart, away from the first bar, and let the frame settle before the cut.
  await ctx.page.mouse.move(box.x + box.width * 0.85, box.y + ctx.AXIS + ctx.ROW * 8);
  await ctx.hold(1);
  const view = await ctx.page.evaluate((width) => {
    const app = window.gantt.app;
    const status = window.gantt.status();
    return { level: status.level, scrollY: status.scrollY, offset: app.x_of(app.row_start(0)) - width / 5 };
  }, box.width);
  if (view.level !== "day" || view.scrollY !== 0 || Math.abs(view.offset) >= 1) {
    throw new Error(`the opening view is not set: ${JSON.stringify(view)}`);
  }
  ctx.log(`opening view: day zoom, row 0 at the top${shared.finish ? `, project ${shared.finish.years.toFixed(1)} years long` : ""}`);
}
