import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";
import { AXIS, ROW, countColour, decodePng } from "./support/pixels";

// The link journey (gs#347, story 5 of gs#49; J7 step 1): an Alt-drag or a drag from the terminal adds a finish-to-start link,
// the engine's refusals reach the alert line, a click on an arrow selects it, Delete removes it, and undo and redo carry it.
// State is read through `window.gantt` (no golden pixels); the one pixel check is that the preview line is drawn at all.
// What the engine does with a gesture (dates, history, refusals, selection, stepping) is checked in Rust, in
// `crates/app/tests/link_gesture.rs` and `links.rs` (gs#1163); this file keeps the drawing, the cursor, the alert line and
// one check that each pointer path and the Delete keys reach the engine (`]` and `[` are checked in key-map.spec.ts).
// The fixture is six tasks in `fixtures/links.xml`: the summary Phase over two unlinked chains, A -> C -> E and B -> D.

// A click that something covers fails in seconds, not after the four-minute test timeout.
test.use({ actionTimeout: 15_000 });
const fixture = readFileSync(new URL("./fixtures/links.xml", import.meta.url));

type Name = "Phase" | "A" | "B" | "C" | "D" | "E";
type Row = [number, string, string, number | null, boolean, number];
type Point = { x: number; y: number };

interface World {
  g: Gantt;
  page: Page;
  id: Record<Name, number>;
  row: Record<Name, number>;
}

async function openLinks(g: Gantt): Promise<World> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: fixture });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const rows0 = await rowsOf(g.page);
  const names = await g.page.evaluate(
    (ids) => ids.map((task) => window.gantt!.app.task_name(task).split(" / ").at(-1)!),
    rows0.map((r) => r[0]),
  );
  const id = {} as Record<Name, number>;
  const row = {} as Record<Name, number>;
  names.forEach((name, at) => {
    id[name as Name] = rows0[at][0];
    row[name as Name] = at;
  });
  expect(names, "the fixture's six tasks in outline order").toEqual(["Phase", "A", "B", "C", "D", "E"]);
  return { g, page: g.page, id, row };
}

const rowsOf = (page: Page): Promise<Row[]> =>
  page.evaluate(() => {
    const w = window.gantt!;
    return JSON.parse(w.app.table_rows(0, w.status().rowCount)) as Row[];
  });

const details = (page: Page, task: number) => page.evaluate((t) => window.gantt!.app.task_details(t)!, task);

/** A point inside the bar of `name`, `inset` CSS px right of its start, in page coordinates. */
async function barPoint(w: World, name: Name, inset = 20): Promise<Point> {
  const box = await w.g.box();
  const status = await w.g.status();
  const x = await w.page.evaluate((r) => window.gantt!.app.x_of(window.gantt!.app.row_start(r)!), w.row[name]);
  return { x: box.x + x + inset, y: box.y + AXIS + w.row[name] * ROW - status.scrollY + ROW / 2 };
}

/** Press on the bar of `from` with Alt down and move to `to`; leave the button down. */
async function altDragTo(w: World, from: Name, to: Point): Promise<void> {
  const a = await barPoint(w, from);
  await w.page.mouse.move(a.x, a.y);
  await w.page.keyboard.down("Alt");
  await w.page.mouse.down();
  await w.page.mouse.move((a.x + to.x) / 2, (a.y + to.y) / 2, { steps: 4 });
  await w.page.mouse.move(to.x, to.y, { steps: 4 });
  await w.g.nextFrame(2);
}

async function release(w: World): Promise<void> {
  await w.page.mouse.up();
  await w.page.keyboard.up("Alt");
  await w.g.nextFrame(2);
}

/** A whole Alt-drag from `from` onto the bar of `to`. */
async function altDrag(w: World, from: Name, to: Name): Promise<void> {
  await altDragTo(w, from, await barPoint(w, to));
  await release(w);
}

function rgb(hex: string): [number, number, number] {
  const n = Number.parseInt(hex.slice(1, 7), 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

/** How many pixels of the theme colour `token` lie on the pointer's row, just left of the pointer (page coordinates). */
async function paintedAtPointer(w: World, token: string, at: Point): Promise<number> {
  const box = await w.g.box();
  const theme = await w.page.evaluate(() => window.gantt!.theme());
  const colour = theme[token];
  expect(colour, `the theme has a ${token} colour`).toMatch(/^#[0-9a-f]{6}/i);
  const png = decodePng(await w.page.locator("#timeline").screenshot());
  return countColour(png, rgb(colour), 3, [at.x - box.x - 24, at.y - box.y - 3, at.x - box.x + 1, at.y - box.y + 4]);
}

const cursor = (page: Page): Promise<string> =>
  page.evaluate(() => getComputedStyle(document.getElementById("timeline")!).cursor);

const errorLine = (page: Page) => ({ line: page.locator("#edit-error"), text: page.locator("#edit-error-text") });

test("an Alt-drag from A's bar to B's bar adds a link", { tag: "@smoke" }, async ({ g }) => {
  // The wiring: a mouse Alt-drag on the canvas reaches the engine's link gesture. What the add does is in Rust.
  const w = await openLinks(g);
  expect((await details(w.page, w.id.B)).predecessorCount, "B starts with no predecessor").toBe(0);
  await altDrag(w, "A", "B");
  expect((await details(w.page, w.id.B)).predecessorCount, "the drag added A to B").toBe(1);
  await expect(errorLine(w.page).line).toBeHidden();
});

test("mid-drag the crosshair shows and the preview line is drawn", async ({ g }) => {
  const w = await openLinks(g);
  const target = await barPoint(w, "B");
  await w.page.mouse.move(target.x, target.y);
  await g.nextFrame(2);
  const idle = await paintedAtPointer(w, "sel-edge", target);

  await altDragTo(w, "A", target);
  expect(await cursor(w.page), "a link drag shows the crosshair").toBe("crosshair");
  const drawn = await paintedAtPointer(w, "sel-edge", target);
  expect(idle, "no link-coloured pixel lies on the pointer's row before the drag").toBe(0);
  expect(drawn, "the preview line reaches the pointer").toBeGreaterThan(0);

  await release(w);
  expect(await cursor(w.page), "the crosshair ends with the drag").not.toBe("crosshair");
});

test("a press-drag from the link terminal adds the same link as an Alt-drag", async ({ g }) => {
  const w = await openLinks(g);
  const bar = await barPoint(w, "A");
  const box = await g.box();
  await w.page.mouse.move(bar.x, bar.y);
  await g.nextFrame(3);
  expect((await g.status()).hover?.task, "the pointer is over A").toBe(w.id.A);

  // The hovered bar shows its terminal just beyond the finish end: find it by its colour on the bar's centre line.
  const theme = await w.page.evaluate(() => window.gantt!.theme());
  const png = decodePng(await w.page.locator("#timeline").screenshot());
  const [r, gr, b] = rgb(theme["link-terminal"]);
  const y = Math.round(bar.y - box.y);
  let first = -1;
  // The band is the dot's middle five rows. Until gs#440 the bar's name, drawn after every rect and right of the bar, covered the
  // dot's centre rows (measured at 50% and 62% bar heights; the link line was never the cause), so a middle band found the
  // dot where one centre row did not. The name now starts past the dot, and the band stays as the wider search.
  for (let x = Math.round(bar.x - box.x) + 40; x < png.width && first < 0; x++) {
    for (let yy = y - 2; yy <= y + 2 && first < 0; yy++) {
      const i = (yy * png.width + x) * png.channels;
      if (Math.abs(png.px[i] - r) <= 3 && Math.abs(png.px[i + 1] - gr) <= 3 && Math.abs(png.px[i + 2] - b) <= 3) first = x;
    }
  }
  expect(first, "the hovered bar draws its link terminal").toBeGreaterThan(0);

  // The wiring: a press on the drawn dot, with no Alt key, reaches the engine as a link drag.
  await w.page.mouse.move(box.x + first + 4, bar.y);
  await w.page.mouse.down();
  const to = await barPoint(w, "B");
  await w.page.mouse.move((box.x + first + to.x) / 2, (bar.y + to.y) / 2, { steps: 4 });
  await w.page.mouse.move(to.x, to.y, { steps: 4 });
  await g.nextFrame(2);
  await w.page.mouse.up();
  await g.nextFrame(2);
  expect((await details(w.page, w.id.B)).predecessorCount, "B has A before it").toBe(1);
});

test("a loop is refused with its message, on the preview before release and in the alert line after", async ({ g }) => {
  const w = await openLinks(g);
  await altDrag(w, "A", "B");
  const loop = '"B" to "A" would make a loop: A → B → A.';

  const target = await barPoint(w, "A");
  await altDragTo(w, "B", target);
  expect(await cursor(w.page), "the drag over A shows the refusal cursor").toBe("not-allowed");
  expect(await paintedAtPointer(w, "link-refused", target), "the preview line turns the refused colour").toBeGreaterThan(0);
  await release(w);

  await expect(errorLine(w.page).line).toBeVisible();
  await expect(errorLine(w.page).text).toHaveText(loop);
});

test("a summary and an already-linked bar are refused with their messages", async ({ g }) => {
  const w = await openLinks(g);
  const { line, text } = errorLine(w.page);
  await altDrag(w, "A", "B");

  await altDrag(w, "A", "Phase");
  await expect(line).toBeVisible();
  await expect(text).toHaveText('"Phase" is a summary. Link its sub-tasks instead.');

  // The second refusal replaces the first in the same line.
  await altDrag(w, "A", "B");
  await expect(text).toHaveText('"A" is already linked to "B".');

  await w.page.locator("#edit-error-close").click();
  await expect(line, "the close button dismisses it").toBeHidden();
});

test("a click on the arrow selects the link and Delete removes it, not the selected row", async ({ g }) => {
  // The wiring: a mouse click reaches the link hit test and the Delete key reaches the link remove. Undo and redo are in Rust.
  const w = await openLinks(g);
  await altDrag(w, "A", "B");
  const id = (await g.status()).link.selected!;
  expect(id).not.toBeNull();
  const c = await barPoint(w, "C");
  await w.page.mouse.click(c.x, c.y);
  await g.nextFrame(2);
  expect((await g.status()).selected?.task).toBe(w.id.C);

  // The arrow into B: its last stub, just before B's start, on B's row.
  const b = await barPoint(w, "B", -3);
  await w.page.mouse.click(b.x, b.y);
  await g.nextFrame(2);
  expect((await g.status()).link.selected, "the click on the arrow selects that link").toBe(id);

  await w.page.keyboard.press("Delete");
  await g.nextFrame(2);
  expect((await g.status()).n, "Delete took out the link, not the selected row").toBe(6);
  expect((await details(w.page, w.id.B)).predecessorCount).toBe(0);
});

test("Mod+Backspace removes the selected link too, and a row is deleted only when no link is selected", async ({ g }) => {
  const w = await openLinks(g);
  await altDrag(w, "A", "B");
  expect((await g.status()).link.selected).not.toBeNull();
  await w.page.keyboard.press("ControlOrMeta+Backspace");
  await g.nextFrame(2);
  const s = await g.status();
  expect(s.n, "Mod+Backspace took out the link and no row").toBe(6);
  expect((await details(w.page, w.id.B)).predecessorCount).toBe(0);

  // With no link selected the same key deletes the selected row, so the check above could have failed.
  const e = await barPoint(w, "E");
  await w.page.mouse.click(e.x, e.y);
  await g.nextFrame(2);
  expect((await g.status()).selected?.task).toBe(w.id.E);
  await w.page.keyboard.press("ControlOrMeta+Backspace");
  await g.nextFrame(2);
  expect((await g.status()).n, "now the row goes").toBe(5);
});

test("Escape mid-drag ends the drag and the release adds nothing", async ({ g }) => {
  // The wiring: the Escape key reaches the engine's drag cancel. A release on empty canvas is in Rust.
  const w = await openLinks(g);
  const target = await barPoint(w, "B");
  await altDragTo(w, "A", target);
  await w.page.keyboard.press("Escape");
  await g.nextFrame(2);
  await w.page.mouse.move(target.x + 3, target.y);
  await release(w);
  expect((await details(w.page, w.id.B)).predecessorCount, "the release after Escape added nothing").toBe(0);
  await expect(errorLine(w.page).line).toBeHidden();
});

test("Delete in a cell editor or the find box does nothing to a selected link", async ({ g }) => {
  const w = await openLinks(g);
  await altDrag(w, "A", "B");
  const linked = await g.status();
  expect(linked.link.selected).not.toBeNull();

  // The find box: typing and deleting there is text editing.
  await w.page.keyboard.press("Control+f");
  await w.page.keyboard.type("zz");
  await w.page.keyboard.press("Backspace");
  await w.page.keyboard.press("ControlOrMeta+Backspace");
  await w.page.keyboard.press("Delete");
  await g.nextFrame(2);
  let s = await g.status();
  expect(s.link.selected, "the link is still selected").toBe(linked.link.selected);
  expect(s.revision).toBe(linked.revision);
  expect(s.n).toBe(6);

  // A cell editor: the duration cell of C's row.
  await w.page.locator("#find-input").fill("");
  await w.page.locator(`#table-rows .row[data-row="${w.row.C}"] .dur`).dblclick();
  await expect(w.page.locator(`#table-rows .row[data-row="${w.row.C}"] .dur input.cell-edit`)).toBeVisible();
  // The click that opened the editor selected C's row, which clears a selected link (gs#416). Select C's own link, A to C,
  // so a link is selected while the editor has focus.
  expect(await w.page.evaluate(() => window.gantt!.app.step_link(true)), "C has a link to select").toBe(true);
  const editing = await g.status();
  expect(editing.link.selected, "a link is selected while the cell is edited").not.toBeNull();
  await w.page.keyboard.press("Delete");
  await w.page.keyboard.press("Backspace");
  await w.page.keyboard.press("ControlOrMeta+Backspace");
  await g.nextFrame(2);
  s = await g.status();
  expect(s.link.selected, "the link is still selected").toBe(editing.link.selected);
  expect(s.revision, "no edit").toBe(linked.revision);
  expect(s.n).toBe(6);
  await w.page.keyboard.press("Escape");
  expect((await details(w.page, w.id.B)).predecessorCount, "the link is in the schedule").toBe(1);
});

test("the Alt key coming up is not left to the browser, and other keys are", async ({ g }) => {
  await openLinks(g);
  const prevented = await g.page.evaluate(() => {
    const root = document.getElementById("app")!;
    const up = (key: string): boolean => !root.dispatchEvent(new KeyboardEvent("keyup", { key, bubbles: true, cancelable: true }));
    return { alt: up("Alt"), other: up("x") };
  });
  expect(prevented.alt, "Alt keyup is default-prevented (the menu bar does not take focus)").toBe(true);
  expect(prevented.other, "another key is not").toBe(false);
});
