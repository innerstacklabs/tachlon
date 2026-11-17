import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW } from "./support/pixels";

// `Status.gesture` (gantt#1152): while a bar is pressed or dragged, the keys that scroll the host page are taken, a cell editor does
// not open, and a press by a second pointer is ignored.

const SIZE = 2000;

/** The canvas point on the start of the first leaf bar. */
async function barPoint(g: Gantt): Promise<{ x: number; y: number }> {
  const { page } = g;
  const box = await g.box();
  const status = await g.status();
  const rows = await page.evaluate(
    (count) => (JSON.parse(window.gantt!.app.table_rows(0, count)) as number[][]).map((r) => r[5]),
    status.rowCount,
  );
  const row = rows.indexOf(0);
  const start = await page.evaluate((r) => window.gantt!.app.row_start(r), row);
  if (start === undefined) throw new Error(`row ${row} has no early start`);
  const x = await page.evaluate((t) => window.gantt!.app.x_of(t), start);
  return { x: box.x + x + 2, y: box.y + AXIS + row * ROW - status.scrollY + ROW / 2 };
}

/** Press the bar and hold it. */
async function press(g: Gantt): Promise<{ x: number; y: number }> {
  const at = await barPoint(g);
  await g.page.mouse.move(at.x, at.y);
  await g.page.mouse.down();
  await g.nextFrame(2);
  return at;
}

/** Send each key to the chart; say whether the page's default for it was prevented (the document's listener runs after the chart's). */
const prevented = (g: Gantt, keys: string[]): Promise<boolean[]> =>
  g.page.evaluate((names) => {
    const seen: boolean[] = [];
    const listener = (e: KeyboardEvent): void => void seen.push(e.defaultPrevented);
    document.addEventListener("keydown", listener);
    for (const key of names) {
      document.getElementById("timeline")!.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
    }
    document.removeEventListener("keydown", listener);
    return seen;
  }, keys);

test("while a bar is pressed or dragged the keys that scroll the page are taken", async ({ g }) => {
  await g.open({ n: SIZE });
  const at = await press(g);
  expect((await g.status()).gesture, "a press that has not moved").toBe("bar-pressed");
  expect(await prevented(g, ["ArrowDown", "PageDown"]), "pressed").toEqual([true, true]);
  await g.page.mouse.move(at.x + 60, at.y, { steps: 6 });
  await g.nextFrame(2);
  expect((await g.status()).gesture, "past the threshold").toBe("bar-dragging");
  expect(await prevented(g, ["ArrowDown", "PageDown"]), "dragging").toEqual([true, true]);
  await g.page.mouse.up();
  await g.nextFrame(2);
  expect((await g.status()).gesture).toBe("idle");
});

test("a press on a bar's right edge reads resize-pressed, then resize-dragging", async ({ g }) => {
  await g.open({ n: SIZE });
  const at = await barPoint(g);
  // The first leaf's bar in CSS pixels: `barPoint` is 2 px in from its start, and the edge is the last 6 px.
  const width = await g.page.evaluate(() => {
    const id = (JSON.parse(window.gantt!.app.table_rows(0, 40)) as number[][]).find((r) => r[5] === 0)![0];
    return (window.gantt!.app.test_bar_rects() as { task: number; width: number }[]).find((r) => r.task === id)!.width / window.devicePixelRatio;
  });
  const x = at.x + width - 4;
  await g.page.mouse.move(x, at.y);
  await g.page.mouse.down();
  await g.nextFrame(2);
  expect((await g.status()).gesture, "a press on the edge").toBe("resize-pressed");
  await g.page.mouse.move(x + 60, at.y, { steps: 6 });
  await g.nextFrame(2);
  expect((await g.status()).gesture, "past the threshold").toBe("resize-dragging");
  await g.page.mouse.up();
});

test("a press by a second pointer is ignored, and one by the holding pointer ends its old press and starts a new one", async ({ g }) => {
  await g.open({ n: SIZE });
  const at = await press(g);
  const r0 = (await g.status()).revision;
  await g.page.mouse.move(at.x + 60, at.y, { steps: 6 });
  await g.nextFrame(2);
  expect((await g.status()).revision, "the drag edited the schedule").toBeGreaterThan(r0);
  // The mouse is pointer 1. A press at the canvas corner lands on no bar.
  const down = (pointerId: number, x: number, y: number): Promise<string> =>
    g.page.evaluate(
      ([id, clientX, clientY]) => {
        const init = { pointerId: id, button: 0, bubbles: true, clientX, clientY };
        document.getElementById("timeline")!.dispatchEvent(new PointerEvent("pointerdown", init));
        return window.gantt!.status().gesture;
      },
      [pointerId, x, y],
    );
  expect(await down(99, 5, 5), "a second pointer's press changes nothing").toBe("bar-dragging");
  expect(await down(1, at.x, at.y), "the holder's press starts a new press on the bar").toBe("bar-pressed");
  expect((await g.status()).revision, "the old drag was cancelled: the task is back at its start").toBe(r0);
  await g.page.mouse.up();
  await g.nextFrame(2);
  expect((await g.status()).gesture, "the new press ends on its release").toBe("idle");
  expect((await g.status()).revision, "and moved nothing").toBe(r0);
});

test("a cell editor does not open while a bar is pressed, and does after the release", async ({ g }) => {
  await g.open({ n: SIZE });
  const open = (): Promise<boolean> =>
    g.page.evaluate(() => {
      const cell = document.querySelector<HTMLElement>("#table-rows .row .name")!;
      const r = cell.getBoundingClientRect();
      cell.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, cancelable: true, clientX: r.left + 4, clientY: r.top + 4 }));
      return document.querySelector("input.cell-edit") !== null;
    });
  await press(g);
  expect(await open(), "a double-tap on the table under a press opens no editor").toBe(false);
  await g.page.mouse.up();
  await g.nextFrame(2);
  expect(await open(), "the same double-tap after the release does").toBe(true);
});
