// A press on a cell with the table scrolled right (gs#703, walk 3 W1): the cell cursor goes to the cell under the pointer, never back
// to Name, so typing "60" in the % complete cell of "Order glazing and joinery" sets its percent and leaves its name alone.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));
const TASK = "Order glazing and joinery";

const frames = (page: Page) => page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => requestAnimationFrame(() => done())))));
const row = (page: Page): Locator => page.locator("#table-rows .row", { has: page.locator(".name", { hasText: TASK }) }).first();

/** Open the fit-out with the Progress columns on, scroll the table right and return the row of TASK. */
async function scrolled(page: Page, open: () => Promise<void>, to: "end" | "start-column" | "a-little" = "end"): Promise<Locator> {
  await open();
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  if ((await page.locator("#table").getAttribute("data-progress")) !== "true") await page.click("#progress-toggle");
  // The head takes the progress columns at the click, the rows at the table's next sync: measure Start only once the row has them (tl#1276).
  await expect(row(page).locator(":scope > .percent")).toBeAttached();
  await page.evaluate((where) => {
    const body = document.getElementById("table-body")!;
    const first = body.querySelector(".row")!;
    // "start-column": scrolled just far enough that Start, Finish and Float sit right of the frozen block, whole.
    // "a-little": % complete follows Task now (gs#974), so at the far end it is scrolled under the frozen Task; a short scroll keeps it in view.
    if (where === "a-little") body.scrollLeft = 24;
    else body.scrollLeft = where === "end" ? body.scrollWidth : body.scrollLeft + first.querySelector(":scope > .start")!.getBoundingClientRect().left - first.querySelector(":scope > .name")!.getBoundingClientRect().right + 40;
  }, to);
  await page.waitForFunction(() => document.getElementById("table")!.dataset.scrolled === "true");
  await frames(page);
  const r = row(page);
  await expect(r).toBeVisible();
  return r;
}

/** The row's task and the first class of the cell that carries the cell cursor. */
const cursorCell = (page: Page) =>
  page.evaluate(() => {
    const cell = document.querySelector<HTMLElement>("#table-rows .row > [data-cursor]");
    return cell === null ? null : { task: Number(cell.parentElement!.dataset.task), cls: cell.className.split(" ")[0] };
  });

/** The point on the row's middle at the horizontal centre of its `cls` cell: an empty cell has no height, so the press goes to the row. */
const centre = (r: Locator, cls: string) =>
  r.evaluate((rowEl, name) => {
    const cell = rowEl.querySelector(`:scope > .${name}`)!.getBoundingClientRect();
    const box = rowEl.getBoundingClientRect();
    return { x: cell.left + cell.width / 2, y: box.top + box.height / 2 };
  }, cls);

test("typing 60 in the % complete cell of a scrolled table sets the percent and keeps the name", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  const r = await scrolled(page, () => g.open({ n: 200 }), "a-little");
  const task = Number(await r.getAttribute("data-task"));
  const name = await r.locator(".name").innerText();
  expect(name).toBe(TASK);
  // The cursor starts on Name: the column a press that misses its cell falls back to.
  await r.locator(".name").click();
  await frames(page);
  const at = await centre(r, "percent");
  // A press puts the cursor on the % complete cell itself, before anything is typed.
  await page.mouse.click(at.x, at.y);
  await frames(page);
  expect(await cursorCell(page), "the cursor after a press on % complete").toEqual({ task, cls: "percent" });
  await page.mouse.dblclick(at.x, at.y);
  await frames(page);
  await page.keyboard.type("60");
  await page.keyboard.press("Enter");
  await frames(page);
  await expect(page.locator("#table-rows input.cell-edit")).toHaveCount(0);
  expect(await page.evaluate((t) => window.gantt!.app.task_percent(t), task)).toBe("60%");
  await expect(page.locator(`#table-rows .row[data-task="${task}"] .name`)).toHaveText(name);
});

test("a click on a column that takes no edit never moves the cell cursor to Name", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  const r = await scrolled(page, () => g.open({ n: 200 }), "start-column");
  const task = Number(await r.getAttribute("data-task"));
  await r.locator(".name").click();
  await frames(page);
  for (const cls of ["finish", "float", "start"]) {
    const at = await centre(r, cls);
    await page.mouse.click(at.x, at.y);
    await frames(page);
    expect(await cursorCell(page), `the cursor after a click on ${cls}`).toEqual({ task, cls });
  }
});
