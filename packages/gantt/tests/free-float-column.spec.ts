import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The Free float column (to#151): a head `Free float` beside `Float` and a cell per row, hidden when the chart opens, shown, hidden and moved from the Columns menu. The
// five tasks of `fixtures/free-float-days.xml` under one summary (rows 1 to 5): total float 2d 2d 1d 1d 0d, free float 0d 1d 1d 0d 0d. The specs read
// the DOM boxes and the cell cursor, not pixels. WebGL2 from the start: this file checks boxes and keys.
test.use({ webgl: true });

async function openDays(g: Gantt): Promise<void> {
  await g.open({ n: 50 });
  await g.page.setInputFiles("#open-input", { name: "free-float-days.xml", mimeType: "application/xml", buffer: readFileSync(new URL("./fixtures/free-float-days.xml", import.meta.url)) });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await g.page.waitForFunction(() => document.querySelectorAll("#table-rows .row").length === 6);
}

const cells = (page: Page, column: string): Promise<string[]> =>
  page.evaluate((name) => [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => row.querySelector<HTMLElement>(`:scope > .${name}`)?.textContent ?? "<none>"), column);

/** The width of the Task head: it is the one 1fr track, so it gives up what another column takes. */
const width = (page: Page): Promise<number> => page.evaluate(() => document.querySelector("#table-head .cols > .name")!.getBoundingClientRect().width);

/** The left edge of the head cell and of row 3's cell of `column`, or null where it has no box. */
const lefts = (page: Page, column: string): Promise<{ head: number | null; cell: number | null }> =>
  page.evaluate((name) => {
    const left = (el: Element | null): number | null => {
      const box = el?.getBoundingClientRect();
      return box === undefined || box.width === 0 ? null : box.left;
    };
    return { head: left(document.querySelector(`#table-head .cols > .${name}`)), cell: left(document.querySelector(`#table-rows .row[data-row="3"] > .${name}`)) };
  }, column);

const marked = (page: Page): Promise<string[]> =>
  page.evaluate(() => [...document.querySelectorAll<HTMLElement>('#table-rows [data-cursor="true"]')].map((cell) => cell.className));

async function toggle(g: Gantt): Promise<void> {
  await g.page.locator("#columns-button").click();
  await g.page.locator("#columns-free-float").click();
  await g.page.keyboard.press("Escape");
  await g.nextFrame(3);
}

test("Free float opens hidden, the menu shows it with the values beside Float, and hiding it brings the old width back", async ({ g }) => {
  await openDays(g);
  const { page } = g;
  const before = await width(page);
  await expect(page.locator("#table")).toHaveAttribute("data-hide-free-float", "");
  expect(await lefts(page, "free-float"), "no head and no cell at open").toEqual({ head: null, cell: null });
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-free-float"), "the row is unticked").toHaveAttribute("aria-checked", "false");
  await expect(page.locator("#columns-free-float"), "the menu row says Free float").toHaveText("Free float");
  await page.keyboard.press("Escape");

  await toggle(g);
  await expect(page.locator("#table")).not.toHaveAttribute("data-hide-free-float", /.*/);
  const head = page.locator("#table-head .cols > .free-float");
  await expect(head).toHaveText("Free float");
  const [need, have] = await head.evaluate((el) => [el.scrollWidth, el.clientWidth]);
  expect(need, "the whole heading fits its track").toBeLessThanOrEqual(have);
  await expect(head).toHaveAttribute("title", "Free float, in working days of each task's own calendar");
  expect(await cells(page, "free-float"), "the summary's cell is empty; the leaves read their free float").toEqual(["", "0d", "1d", "1d", "0d", "0d"]);
  expect(await cells(page, "float"), "Float beside it reads the total float").toEqual(["", "2d", "2d", "1d", "1d", "0d"]);
  const seen = await lefts(page, "free-float");
  expect(seen.head, "Free float has a box").not.toBeNull();
  expect(seen.cell, "its cells sit under its head").toBe(seen.head);
  expect(await width(page), "Task gives up the track and a gap").toBe(before - 82);

  await toggle(g);
  expect(await lefts(page, "free-float"), "hidden again: no box").toEqual({ head: null, cell: null });
  expect(await width(page), "Task has its width back").toBe(before);
});

test("a finished task has an empty Free float cell", async ({ g }) => {
  await openDays(g);
  const { page } = g;
  await toggle(g);
  await page.evaluate(() => {
    const app = window.gantt!.app;
    const row = JSON.parse(app.table_rows(1, 2))[0] as [number, string, string];
    app.set_progress_field(row[0], "actual_start", row[1]);
    app.set_progress_field(row[0], "actual_finish", row[2]);
  });
  await g.nextFrame(3);
  expect(await cells(page, "free-float"), "First is finished; the summary's and its cell stay empty").toEqual(["", "", "1d", "1d", "0d", "0d"]);
});

test("Up and Down move the column, and the cell cursor walks it in its place", async ({ g }) => {
  await openDays(g);
  const { page } = g;
  await toggle(g);
  const walk = async (steps: number): Promise<string[]> => {
    await page.locator('#table-rows .row[data-row="3"] > .name').click();
    const seen: string[] = [];
    for (let i = 0; i < steps; i++) {
      await page.keyboard.press("ArrowRight");
      await g.nextFrame(2);
      seen.push(...(await marked(page)));
    }
    return seen;
  };
  expect(await walk(5), "it is the last column to begin with").toEqual(["dur", "start", "finish", "float", "free-float"]);
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-down-free-float"), "the last row cannot go down").toHaveAttribute("aria-disabled", "true");
  await page.locator("#columns-up-free-float").click();
  await page.keyboard.press("Escape");
  await g.nextFrame(3);
  const [float, free] = [await lefts(page, "float"), await lefts(page, "free-float")];
  expect(free.head!, "Free float moved left of Float").toBeLessThan(float.head!);
  expect(free.cell, "its cells follow its head").toBe(free.head);
  expect(await walk(5), "the cursor walks it before Float").toEqual(["dur", "start", "finish", "free-float", "float"]);
});
