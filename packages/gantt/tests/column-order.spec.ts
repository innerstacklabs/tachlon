import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The Columns menu's move buttons put Dur, Start, Finish and Float in a new order for the visit (column-order.ts, column-order.css). The specs
// read the DOM boxes and the cell cursor, not pixels. WebGL2 from the start: this file checks boxes and keys.
test.use({ webgl: true });

/** The columns that have a box, left to right by their head cells, and whether the cells of row 3 sit under their heads. */
const shown = (page: Page): Promise<{ order: string[]; under: boolean }> =>
  page.evaluate(() => {
    const left = (selector: string): number => document.querySelector(selector)?.getBoundingClientRect().left ?? NaN;
    const names = ["dur", "start", "finish", "float"].filter((name) => document.querySelector(`#table-head .cols > .${name}`)!.getBoundingClientRect().width > 0).sort((a, b) => left(`#table-head .cols > .${a}`) - left(`#table-head .cols > .${b}`));
    return { order: names, under: names.every((name) => left(`#table-head .cols > .${name}`) === left(`#table-rows .row[data-row="3"] > .${name}`)) };
  });

async function move(g: Gantt, column: string, way: "up" | "down", times = 1): Promise<void> {
  if (await g.page.locator("#columns-panel").isHidden()) await g.page.locator("#columns-button").click();
  for (let i = 0; i < times; i++) await g.page.locator(`#columns-${way}-${column}`).click();
  await g.nextFrame(3);
}

test("Start moved up takes Dur's place in the head and every row, and the panel lists the new order", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  expect(await shown(page), "Dur, Start, Finish, Float to begin with").toEqual({ order: ["dur", "start", "finish", "float"], under: true });
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-up-dur"), "the first row cannot go up").toHaveAttribute("aria-disabled", "true");
  await move(g, "start", "up");
  expect(await shown(page), "Start comes before Dur, and the cells follow their heads").toEqual({ order: ["start", "dur", "finish", "float"], under: true });
  await expect(page.locator("#table")).toHaveAttribute("data-column-order", "start dur finish float free-float");
  expect(await page.locator("#columns-panel [data-row]").evaluateAll((rows) => rows.map((row) => (row as HTMLElement).dataset.row)), "the rows stand in the same order").toEqual(["start", "dur", "finish", "float", "free-float"]);
  await expect(page.locator("#columns-up-start"), "Start is first now").toHaveAttribute("aria-disabled", "true");
  await expect(page.locator("#columns-up-start"), "focus stays on the button just pressed").toBeFocused();
  await move(g, "dur", "down", 3);
  expect(await shown(page), "Dur moved down three places, the last past the hidden Free float").toEqual({ order: ["start", "finish", "float", "dur"], under: true });
  await page.locator("#columns-down-dur").click({ force: true }); // aria-disabled: Playwright waits for "enabled" otherwise
  expect((await shown(page)).order, "Dur is last: Down does nothing").toEqual(["start", "finish", "float", "dur"]);
});

test("the cell cursor walks the columns in the new order", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  await move(g, "float", "up", 3);
  await page.keyboard.press("Escape");
  await page.locator('#table-rows .row[data-row="3"] > .name').click();
  const walk: string[] = [];
  for (let i = 0; i < 4; i++) {
    await page.keyboard.press("ArrowRight");
    await g.nextFrame(2);
    walk.push(...(await page.evaluate(() => [...document.querySelectorAll<HTMLElement>('#table-rows [data-cursor="true"]')].map((cell) => cell.className))));
  }
  expect(walk, "Float is first after Task now").toEqual(["float", "dur", "start", "finish"]);
});

test("with Progress on the four follow the progress columns in the new order, and a hidden column leaves its track", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  await page.locator("#progress-toggle").click();
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await move(g, "finish", "up", 2);
  const after = await page.evaluate(() => {
    const left = (name: string): number => document.querySelector(`#table-head .cols > .${name}`)!.getBoundingClientRect().left;
    return { remainingBeforeFinish: left("remaining") < left("finish"), width: document.getElementById("table-body")!.scrollWidth };
  });
  expect(after.remainingBeforeFinish, "Remaining still comes before the four").toBe(true);
  expect(await shown(page), "Finish first of the four").toEqual({ order: ["finish", "dur", "start", "float"], under: true });
  await page.locator("#columns-start").click();
  await g.nextFrame(3);
  expect(await shown(page), "Start has no box, the rest keep their order").toEqual({ order: ["finish", "dur", "float"], under: true });
  expect(await page.evaluate(() => document.getElementById("table-body")!.scrollWidth), "the hidden Start's track and gap leave the width").toBe(after.width - 106);
});

test("dragging a head cell onto another takes its place with a real pointer; a drop on Task or itself and a click change nothing", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  const centre = async (name: string): Promise<{ x: number; y: number }> => {
    const box = (await page.locator(`#table-head .cols > .${name}`).boundingBox())!;
    return { x: box.x + box.width / 2, y: box.y + box.height / 2 };
  };
  const drag = async (from: string, to: string, hold?: () => Promise<void>): Promise<void> => {
    const [a, b] = [await centre(from), await centre(to)];
    await page.mouse.move(a.x, a.y);
    await page.mouse.down();
    await page.mouse.move(b.x, b.y, { steps: 6 });
    await hold?.();
    await page.mouse.up();
    await g.nextFrame(3);
  };
  await drag("float", "dur", async () => await expect(page.locator("#table-head .cols > .dur"), "the target shows the side Float lands on").toHaveAttribute("data-drop", "before"));
  expect(await shown(page), "Float took Dur's place, in the head and the cells").toEqual({ order: ["float", "dur", "start", "finish"], under: true });
  await expect(page.locator("#table-head [data-drop]"), "the bar is gone after the drop").toHaveCount(0);
  await drag("dur", "finish");
  expect(await shown(page), "Dur took Finish's place, moving right").toEqual({ order: ["float", "start", "finish", "dur"], under: true });
  await drag("start", "name");
  await drag("start", "start");
  expect((await shown(page)).order, "a drop on Task and a drop on itself change nothing").toEqual(["float", "start", "finish", "dur"]);
  await page.locator("#table-head .cols > .float").click();
  expect((await shown(page)).order, "a click changes nothing").toEqual(["float", "start", "finish", "dur"]);
});
