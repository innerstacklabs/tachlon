import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// The resize cursor (gantt#1875): over the last pixels of a leaf bar `Status.hover.edge` is set and the canvas shows a two-way arrow; a drag
// from there changes the task's duration cell. The press, the duration and the undo step are checked on `Session` in
// crates/app/tests/drag_resize.rs; this checks what the page shows.

const SIZE = 2000;

/** The `nth` leaf bar: its task and table row, and its left edge, width and middle in page CSS pixels. */
async function leafBar(g: Gantt, nth = 0): Promise<{ task: number; row: number; x: number; width: number; y: number }> {
  const box = await g.box();
  const status = await g.status();
  const bar = await g.page.evaluate((n) => {
    const rows = JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as number[][];
    const row = rows.map((r, i) => (r[5] === 0 ? i : -1)).filter((i) => i >= 0)[n];
    const task = rows[row][0];
    const rect = (window.gantt!.app.test_bar_rects() as { task: number; x: number; width: number }[]).find((r) => r.task === task)!;
    return { task, row, x: rect.x / window.devicePixelRatio, width: rect.width / window.devicePixelRatio };
  }, nth);
  return { task: bar.task, row: bar.row, x: box.x + bar.x, width: bar.width, y: box.y + AXIS + bar.row * ROW - status.scrollY + ROW / 2 };
}

/** The text of the duration cell of table row `row`. */
const duration = (g: Gantt, row: number): Promise<string | null> =>
  g.page.evaluate((r) => document.querySelector(`#table-rows .row[data-row="${r}"] > .dur`)?.textContent ?? null, row);

const cursor = (g: Gantt): Promise<string> => g.page.evaluate(() => getComputedStyle(document.getElementById("timeline")!).cursor);

test("over a leaf bar's finish edge the status reports the edge and the cursor is a two-way arrow", async ({ g }) => {
  await g.open({ n: SIZE });
  const bar = await leafBar(g);
  expect(bar.width, "the bar is wide enough to have an edge").toBeGreaterThan(24);

  await g.page.mouse.move(bar.x + bar.width / 2, bar.y);
  await g.nextFrame(2);
  const body = await g.status();
  expect(body.hover, "the pointer is on the bar").not.toBeNull();
  expect(body.hover?.edge, "the body of the bar is not its edge").toBeUndefined();
  expect(await cursor(g), "the body keeps the grab cursor").toBe("grab");

  await g.page.mouse.move(bar.x + bar.width - 3, bar.y);
  await g.nextFrame(2);
  expect((await g.status()).hover?.edge, "the last pixels are the edge").toBe(true);
  expect(await cursor(g), "a two-way arrow").toBe("ew-resize");
});

test("a drag from the edge keeps the two-way arrow and changes the duration cell", async ({ g }) => {
  await g.open({ n: SIZE });
  const bar = await leafBar(g);
  const before = await duration(g, bar.row);
  const x = bar.x + bar.width - 3;

  await g.page.mouse.move(x, bar.y);
  await g.page.mouse.down();
  await g.page.mouse.move(x + 90, bar.y, { steps: 9 });
  await g.nextFrame(2);
  expect((await g.status()).gesture).toBe("resize-dragging");
  expect(await cursor(g), "the pointer left the edge, the arrow stays").toBe("ew-resize");
  await g.page.mouse.up();
  await g.nextFrame(2);
  expect(await duration(g, bar.row), "the duration cell changed").not.toBe(before);
});

test("a bar move keeps its own cursor over another bar's finish edge", async ({ g }) => {
  // The bar moved is a later one and the target the first leaf, so the move cannot push the target away from the pointer.
  await g.open({ n: SIZE });
  const bar = await leafBar(g, 5);
  await g.page.mouse.move(bar.x + bar.width / 2, bar.y);
  await g.page.mouse.down();
  await g.page.mouse.move(bar.x + bar.width / 2 + 40, bar.y, { steps: 6 });
  await g.nextFrame(2);
  expect((await g.status()).gesture, "a move of the body").toBe("bar-dragging");
  const other = await leafBar(g, 0);
  await g.page.mouse.move(other.x + other.width - 3, other.y, { steps: 4 });
  await g.nextFrame(2);
  const status = await g.status();
  expect(status.hover?.task, "the pointer is on the other bar").toBe(other.task);
  expect(status.hover?.edge, "and on its edge").toBe(true);
  expect(status.gesture, "the move goes on").toBe("bar-dragging");
  // `grabbing`, or `not-allowed` while predecessors hold the moved bar: either way not the resize arrow.
  expect(["grabbing", "not-allowed"], "the move keeps its own cursor").toContain(await cursor(g));
  expect(await g.page.evaluate(() => document.getElementById("timeline")!.classList.contains("resize")), "no resize class mid-move").toBe(false);
  await g.page.mouse.up();
});

test("a finished task's edge is not offered: no edge in the status and no two-way arrow", async ({ g }) => {
  await g.open({ n: SIZE });
  const first = await leafBar(g);
  await g.page.evaluate((task) => {
    const row = (JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string][]).find((r) => r[0] === task)!;
    window.gantt!.app.set_progress_field(task, "actual_start", row[1]);
    window.gantt!.app.set_progress_field(task, "actual_finish", row[2]);
  }, first.task);
  await g.nextFrame(2);
  const bar = await leafBar(g);
  await g.page.mouse.move(bar.x + bar.width - 3, bar.y);
  await g.nextFrame(2);
  const status = await g.status();
  expect(status.hover?.task, "the pointer is on the finished task").toBe(first.task);
  expect(status.hover?.edge, "its edge is not offered").toBeUndefined();
  expect(await cursor(g), "the cursor stays the grab").toBe("grab");
});
