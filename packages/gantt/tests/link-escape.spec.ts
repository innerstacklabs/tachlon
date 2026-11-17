import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// Escape during a link gesture (gs#346 fix round 2): the key reaches `App.pointer_cancel` through packages/gantt/src/keys.ts, so the
// release that follows adds no link and no history step. The native test of the same name calls `Session::pointer_cancel`
// directly and would pass with the key's wiring removed; this one goes through the keyboard.

const SIZE = 2000;
/** How many leaf rows from the top are tried as the link's two ends. */
const TRIED = 8;

interface Ends {
  from: { x: number; y: number };
  to: { x: number; y: number };
  pred: number;
  succ: number;
}

const counts = (page: Page, pred: number, succ: number) =>
  page.evaluate(
    ([p, s]) => {
      const app = window.gantt!.app;
      return { successors: app.task_details(p).successorCount, predecessors: app.task_details(s).predecessorCount };
    },
    [pred, succ],
  );

/** Alt-drag from a bar to another until a drag shows a target it would accept; leave that drag in progress. */
async function startLinkDrag(g: Gantt): Promise<Ends> {
  const { page } = g;
  const box = await g.box();
  const status = await g.status();
  const rows = await page.evaluate(
    (count) => (JSON.parse(window.gantt!.app.table_rows(0, count)) as number[][]).map((r) => [r[0], r[5]]),
    status.rowCount,
  );
  const leaves = rows
    .map(([task, kind], row) => ({ task, kind, row }))
    .filter((r) => r.kind === 0)
    .slice(0, TRIED);
  const point = async (row: number) => {
    const start = await page.evaluate((r) => window.gantt!.app.row_start(r), row);
    if (start === undefined) throw new Error(`row ${row} has no early start`);
    const x = await page.evaluate((t) => window.gantt!.app.x_of(t), start);
    return { x: box.x + x + 1, y: box.y + AXIS + row * ROW - status.scrollY + ROW / 2 };
  };
  for (const a of leaves) {
    for (const b of leaves) {
      if (a.task === b.task) continue;
      const from = await point(a.row);
      const to = await point(b.row);
      await page.mouse.move(from.x, from.y);
      await page.keyboard.down("Alt");
      await page.mouse.down();
      await page.mouse.move((from.x + to.x) / 2, (from.y + to.y) / 2, { steps: 4 });
      await page.mouse.move(to.x, to.y, { steps: 4 });
      await g.nextFrame(2);
      const drag = (await g.status()).link.drag;
      if (drag !== null && drag.target === b.task && drag.refused === null) {
        await page.keyboard.up("Alt");
        return { from, to, pred: a.task, succ: b.task };
      }
      // Not a link this schedule takes: release (a refused or empty release adds nothing) and try the next pair.
      await page.mouse.up();
      await page.keyboard.up("Alt");
      await g.nextFrame(2);
    }
  }
  throw new Error(`no pair of the first ${TRIED} leaf rows takes a new link`);
}

test("Escape during a link drag cancels it: the release adds no link and no history step", async ({ g }) => {
  await g.open({ n: SIZE });
  const { page } = g;
  const ends = await startLinkDrag(g);
  const before = { status: await g.status(), counts: await counts(page, ends.pred, ends.succ) };
  expect(before.status.link.drag?.target, "the preview is on the target before Escape").toBe(ends.succ);
  expect(await page.evaluate(() => window.gantt!.app.linking()), "a link drag is in progress").toBe(true);

  await page.keyboard.press("Escape");
  await g.nextFrame(2);
  const cancelled = await g.status();
  expect(cancelled.link.drag, "Escape ended the link drag").toBeNull();
  expect(await page.evaluate(() => window.gantt!.app.linking()), "no link gesture is in progress after Escape").toBe(false);

  // The button is still down: the pointer keeps moving and then lets go over the target.
  await page.mouse.move(ends.to.x + 3, ends.to.y);
  await page.mouse.up();
  await g.nextFrame(2);
  const after = await g.status();
  expect(after.revision, "the release recorded no history step").toBe(before.status.revision);
  expect(after.stamp, "the release showed no new solution").toBe(before.status.stamp);
  expect(after.link.refusal, "the release was not a refused link either").toBeNull();
  expect(await counts(page, ends.pred, ends.succ), "no link was added").toEqual(before.counts);

  // The same drag without Escape adds the link, so the checks above could have failed.
  await page.mouse.move(ends.from.x, ends.from.y);
  await page.keyboard.down("Alt");
  await page.mouse.down();
  await page.mouse.move(ends.to.x, ends.to.y, { steps: 8 });
  await page.keyboard.up("Alt");
  await page.mouse.up();
  await g.nextFrame(2);
  const linked = await counts(page, ends.pred, ends.succ);
  expect(linked.successors, "without Escape the release adds the link").toBe(before.counts.successors + 1);
  expect(linked.predecessors).toBe(before.counts.predecessors + 1);
  expect((await g.status()).revision, "and records one history step").toBeGreaterThan(before.status.revision);
});
