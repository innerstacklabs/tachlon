import { readFileSync } from "node:fs";
import { expect, test, type Gantt } from "./support/gantt";

// The list divider (gs#412, calm look S4): a strip on the table's right edge. Dragging it sets `--gantt-table-w` in px,
// within 420 px and the chart's width less 320 px; a double-click returns the default; the arrow keys move it 16 px.

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));
const LIST_MIN = 420;
const CANVAS_MIN = 320;
const GAP = 6;
/** The ID track the page set from the file's longest ID (`--gantt-id-col` on the root, in px). */
const idTrack = (g: Gantt): Promise<number> =>
  g.page.evaluate(() => parseFloat(getComputedStyle(document.getElementById("app")!).getPropertyValue("--gantt-id-col")));

interface Widths {
  table: number;
  canvas: number;
}

async function widths(g: Gantt): Promise<Widths> {
  return g.page.evaluate(() => ({
    table: document.getElementById("table")!.getBoundingClientRect().width,
    canvas: document.getElementById("timeline")!.getBoundingClientRect().width,
  }));
}

/** Press on the divider, move to `dx` pixels from where it was, and release. */
async function drag(g: Gantt, dx: number): Promise<void> {
  const box = (await g.page.locator("#split").boundingBox())!;
  const x = box.x + box.width / 2;
  const y = box.y + box.height / 2;
  await g.page.mouse.move(x, y);
  await g.page.mouse.down();
  await g.page.mouse.move(x + dx, y, { steps: 8 });
  await g.page.mouse.up();
  await g.nextFrame(2);
}

/** Move the chart root into a host box `width` wide, as another page would place it. */
async function embed(g: Gantt, width: number): Promise<void> {
  await g.page.evaluate((w) => {
    const host = document.createElement("div");
    host.style.cssText = `position: absolute; left: 0; top: 0; width: ${w}px; height: 700px`;
    const root = document.getElementById("app")!;
    document.body.append(host);
    host.append(root);
  }, width);
  await g.nextFrame(2);
}

test("a drag of +100 px widens the list by 100 px and narrows the canvas by the same", async ({ g }) => {
  await g.open({ n: 2000 });
  const before = await widths(g);
  await drag(g, 100);
  const after = await widths(g);
  expect(after.table - before.table, "the list").toBeCloseTo(100, 0);
  expect(before.canvas - after.canvas, "the canvas").toBeCloseTo(100, 0);
  expect(await g.page.evaluate(() => document.getElementById("app")!.style.getPropertyValue("--gantt-table-w"))).toBe(
    `${Math.round(after.table)}px`,
  );
  await drag(g, -60);
  expect((await widths(g)).table - after.table, "a drag back").toBeCloseTo(-60, 0);
});

test("the drag stops at 420 px and at 320 px of canvas", async ({ g }) => {
  await g.open({ n: 2000 });
  await embed(g, 1000);
  await drag(g, -900);
  expect((await widths(g)).table, "the narrowest list").toBe(LIST_MIN);
  await drag(g, 900);
  const wide = await widths(g);
  expect(wide.canvas, "the canvas keeps 320 px of the host's box").toBeCloseTo(CANVAS_MIN, 0);
  expect(wide.table + wide.canvas, "the host's whole box").toBeCloseTo(1000, 0);
});

test("a double-click puts the default width back", async ({ g }) => {
  await g.open({ n: 2000 });
  const start = (await widths(g)).table;
  await drag(g, 150);
  expect((await widths(g)).table).toBeGreaterThan(start + 100);
  await g.page.locator("#split").dblclick();
  await g.nextFrame(2);
  expect((await widths(g)).table, "back to the default").toBeCloseTo(start, 0);
  expect(await g.page.evaluate(() => document.getElementById("app")!.style.getPropertyValue("--gantt-table-w"))).toBe("");
});

test("the arrow keys move the divider 16 px with focus on it, and stop at the limits", async ({ g }) => {
  await g.open({ n: 2000 });
  const start = (await widths(g)).table;
  await g.page.locator("#split").focus();
  await g.page.keyboard.press("ArrowRight");
  expect((await widths(g)).table - start, "one step right").toBeCloseTo(16, 0);
  await g.page.keyboard.press("ArrowLeft");
  await g.page.keyboard.press("ArrowLeft");
  expect((await widths(g)).table - start, "two steps left").toBeCloseTo(-16, 0);
  for (let i = 0; i < 40; i += 1) await g.page.keyboard.press("ArrowLeft");
  expect((await widths(g)).table, "the narrowest list").toBe(LIST_MIN);
  await g.page.keyboard.press("ArrowLeft");
  expect((await widths(g)).table, "no step past the limit").toBe(LIST_MIN);
  expect(await g.page.locator("#split").getAttribute("aria-valuenow")).toBe(String(LIST_MIN));
});

test("in a right-to-left chart, a drag of -100 px widens the list by 100 px and Left widens it by 16 px", async ({ g }) => {
  await g.open({ n: 2000 });
  await g.page.evaluate(() => document.getElementById("app")!.setAttribute("dir", "rtl"));
  await g.nextFrame(2);
  const start = (await widths(g)).table;
  await drag(g, -100);
  const dragged = (await widths(g)).table;
  expect(dragged - start, "a drag left").toBeCloseTo(100, 0);
  await g.page.locator("#split").focus();
  await g.page.keyboard.press("ArrowLeft");
  expect((await widths(g)).table - dragged, "Left").toBeCloseTo(16, 0);
});

test("an arrow key moves the divider only while it has focus", async ({ g }) => {
  await g.open({ n: 2000 });
  const start = (await widths(g)).table;
  await g.page.locator("#app").focus();
  await g.page.keyboard.press("ArrowRight");
  expect((await widths(g)).table, "focus on the chart's root").toBeCloseTo(start, 0);
});

test("with the ID column, the divider sets the width without it and the limits keep their meaning", async ({ g }) => {
  await g.open({ n: 2000 });
  await g.page.setInputFiles("#open-input", { name: "ledger.xer", mimeType: "application/octet-stream", buffer: XER });
  await g.page.waitForFunction(() => document.getElementById("table")!.dataset["ids"] === "true", undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const before = await widths(g);
  await drag(g, 40);
  expect((await widths(g)).table - before.table, "+40 px").toBeCloseTo(40, 0);
  await drag(g, -900);
  expect((await widths(g)).table, "420 px of list plus the ID track and its gap").toBeCloseTo(LIST_MIN + (await idTrack(g)) + GAP, 0);
});

/** The limits the divider says (`aria-valuemin`, `aria-valuemax`), read before any touch, then the ones a drag to each end sets on the root. */
async function limits(g: Gantt): Promise<{ said: number[]; held: number[] }> {
  const said = await g.page.evaluate(() => ["aria-valuemin", "aria-valuemax"].map((a) => Number(document.getElementById("split")!.getAttribute(a))));
  const held: number[] = [];
  for (const dx of [-3000, 3000]) {
    await drag(g, dx);
    held.push(parseFloat(await g.page.evaluate(() => document.getElementById("app")!.style.getPropertyValue("--gantt-table-w"))));
  }
  return { said, held };
}

/** The divider has been read once (focus), so a later change that it does not see leaves its limits stale. */
async function primed(g: Gantt): Promise<void> {
  await g.open({ n: 2000 });
  await g.page.locator("#split").focus();
}

async function expectSaid(g: Gantt): Promise<void> {
  const { said, held } = await limits(g);
  expect(said[0], "the narrowest list it says").toBeCloseTo(held[0]!, 0);
  expect(said[1], "the widest list it says").toBeCloseTo(held[1]!, 0);
}

test("the limits it says follow a new file with an ID column", async ({ g }) => {
  await primed(g);
  await g.page.setInputFiles("#open-input", { name: "ledger.xer", mimeType: "application/octet-stream", buffer: XER });
  await g.page.waitForFunction(() => document.getElementById("table")!.dataset["ids"] === "true", undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await expectSaid(g);
});

test("the limits it says follow the variance columns and the Variance button", async ({ g }) => {
  await primed(g);
  await g.page.locator("#baseline-set").click();
  await expect(g.page.locator("#table")).toHaveAttribute("data-variance", "true");
  await g.nextFrame(3);
  await expectSaid(g);
  await g.page.locator("#variance-toggle").click();
  await g.nextFrame(3);
  await expectSaid(g);
});

test("the limits it says follow the window's width", async ({ g }) => {
  await primed(g);
  await g.page.setViewportSize({ width: 900, height: 800 });
  await g.nextFrame(3);
  await expectSaid(g);
});

test("a double-click puts back the value the page has now, not the one it had at mount", async ({ g }) => {
  await g.open({ n: 2000 });
  const token = (): Promise<string> => g.page.evaluate(() => document.getElementById("app")!.style.getPropertyValue("--gantt-table-w"));
  for (const host of ["500px", "700px"]) {
    await g.page.evaluate((v) => document.getElementById("app")!.style.setProperty("--gantt-table-w", v), host);
    await drag(g, -40);
    await g.page.locator("#split").dblclick();
    await g.nextFrame(2);
    expect(await token(), `the page's ${host}`).toBe(host);
  }
});
