// The links feature clip (gs#347, gs#392): drag a link, refuse a loop, select and delete it, as one scene. The README story no
// longer has these steps (gs#393); its link scene changes a link's type and lag (steps/p6.mjs). Kept apart from steps-features.mjs, which has a 400-line budget.
import { barAt } from "./steps/helpers.mjs";

/**
 * The link this step adds, by task id in the 50,000-task fixture (gs#376): the first two of the top leaf rows, A then B.
 * The step never searches for a pair. It fails if either task is not on a top row, if the drag does not end on B, or if the
 * release does not add exactly this link, so a recording can never show a link the caption does not describe.
 */
export const PLAN = { pred: 13476, succ: 501 };

/** The row of each planned task among the first 60 rows (the table is scrolled to the top), or an error naming the missing one. */
async function plannedRows(ctx, plan) {
  const rows = await ctx.page.evaluate(() => JSON.parse(window.gantt.app.table_rows(0, 60)).map((r, row) => ({ task: r[0], row, kind: r[5] })));
  const rowOf = (task, role) => {
    const found = rows.find((r) => r.task === task);
    if (found === undefined) throw new Error(`links step: planned ${role} task ${task} is not in the top 60 rows`);
    if (found.kind !== 0) throw new Error(`links step: planned ${role} task ${task} is a summary, not a leaf`);
    return found.row;
  };
  return { pred: rowOf(plan.pred, "source"), succ: rowOf(plan.succ, "target") };
}

/** How many distinct successors the source has and predecessors the target has. */
const linkCounts = (ctx, plan) =>
  ctx.page.evaluate(({ pred, succ }) => ({
    successors: window.gantt.app.task_details(pred)?.successorCount,
    predecessors: window.gantt.app.task_details(succ)?.predecessorCount,
  }), plan);

/** The table at the top, the chart box, the planned rows and the link counts before any link of the plan. */
async function startAt(ctx, plan) {
  await ctx.page.evaluate(() => { document.getElementById("table-body").scrollTop = 0; });
  await ctx.frames(2);
  const box = await ctx.canvasBox();
  const row = await plannedRows(ctx, plan);
  for (const [role, r] of [["source", row.pred], ["target", row.succ]]) {
    const point = await barAt(ctx, box, r, 3);
    if (point.x < box.x || point.x > box.x + box.width) throw new Error(`links step: the ${role} bar starts off the chart (x ${point.x.toFixed(0)}, chart ${box.x.toFixed(0)}-${(box.x + box.width).toFixed(0)})`);
  }
  return { box, row, before: await linkCounts(ctx, plan) };
}

/** Alt-drag from the source bar onto the target and release: one link, checked against the plan. Returns the link's id. */
async function addLink(ctx, plan, { box, row, before }) {
  const m = ctx.page.mouse;
  // The planned drag, left running so the preview line is on screen. A drag that is not the planned one is cancelled, never released.
  const [start, end] = [await barAt(ctx, box, row.pred, 3), await barAt(ctx, box, row.succ, 3)];
  await m.move(start.x, start.y);
  await ctx.page.keyboard.down("Alt");
  await m.down();
  await m.move(end.x, end.y, { steps: 12 });
  await ctx.frames(2);
  const drag = (await ctx.status()).link.drag;
  if (drag === null || drag.pred !== plan.pred || drag.target !== plan.succ || drag.refused !== null) {
    await ctx.page.keyboard.press("Escape");
    await m.up();
    await ctx.page.keyboard.up("Alt");
    throw new Error(`links step: the drag is ${JSON.stringify(drag)}, not task ${plan.pred} onto task ${plan.succ} with no refusal`);
  }
  await ctx.hold(1);
  await m.up();
  await ctx.page.keyboard.up("Alt");
  await ctx.until(() => window.gantt.status().link.selected !== null);
  const id = (await ctx.status()).link.selected;
  const after = await linkCounts(ctx, plan);
  if (after.successors !== before.successors + 1 || after.predecessors !== before.predecessors + 1) {
    throw new Error(`links step: link ${id} did not add task ${plan.pred} -> task ${plan.succ} (successors ${before.successors} -> ${after.successors}, predecessors ${before.predecessors} -> ${after.predecessors})`);
  }
  await ctx.hold(1);
  return id;
}

/** Alt-drag back from the new successor to its predecessor: a loop, refused on the preview and then in the line over the chart. */
async function refuseLoop(ctx, plan, { box, row }) {
  const m = ctx.page.mouse;
  const [from, to] = [await barAt(ctx, box, row.succ, 3), await barAt(ctx, box, row.pred, 3)];
  await m.move(from.x, from.y);
  await ctx.page.keyboard.down("Alt");
  await m.down();
  await m.move(to.x, to.y, { steps: 12 });
  await ctx.until(() => window.gantt.status().link.drag?.refused != null);
  const loop = (await ctx.status()).link.drag;
  if (loop.pred !== plan.succ || loop.target !== plan.pred) {
    throw new Error(`links step: the refused drag is task ${loop.pred} onto task ${loop.target}, not the planned loop back from ${plan.succ} onto ${plan.pred}`);
  }
  await ctx.hold(1);
  await m.up();
  await ctx.page.keyboard.up("Alt");
  await ctx.until(() => !document.getElementById("edit-error").hidden);
  await ctx.hold(1.5);
}

/** Click the new link's arrow and press Delete: the counts are back where they were. */
async function removeLink(ctx, plan, { box, row, before }, id) {
  const m = ctx.page.mouse;
  // The arrow's last vertical run, 6 px before the successor's bar and in the air above or below the bar (a bar wins a press
  // on its own height). The press clears the refusal.
  const arrow = await barAt(ctx, box, row.succ, -6, (row.pred < row.succ ? -1 : 1) * (ctx.ROW / 2 - 2));
  await m.click(arrow.x, arrow.y);
  await ctx.until((link) => window.gantt.status().link.selected === link, id);
  await ctx.hold(1);
  await ctx.page.keyboard.press("Delete");
  await ctx.until(() => window.gantt.status().link.selected === null);
  const removed = await linkCounts(ctx, plan);
  if (removed.successors !== before.successors || removed.predecessors !== before.predecessors) {
    throw new Error(`links step: Delete left task ${plan.pred} -> task ${plan.succ} in place (successors ${removed.successors}, predecessors ${removed.predecessors})`);
  }
  await ctx.hold(1);
}

/** The feature clip: link, loop, then select the arrow and delete it. */
export const links = {
  name: "links",
  journey: "J6",
  caption: "Alt-drag one bar onto another to link them; a loop is refused with its message; click the arrow and Delete removes it",
  run: async (ctx, plan = PLAN) => {
    const start = await startAt(ctx, plan);
    const id = await addLink(ctx, plan, start);
    await refuseLoop(ctx, plan, start);
    await removeLink(ctx, plan, start, id);
  },
};
