import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

// The Columns menu: a toolbar button whose panel hides and shows the Start, Finish and Float columns, with `data-hide-<column>` on the
// table (columns-menu.ts, table.css). The specs read the DOM boxes, not pixels. WebGL2 from the start: this file checks keys and boxes.
test.use({ webgl: true });

/** The left edge of a head cell and of the cell under it in row 3, or null for a column that has no box. */
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

async function hide(g: Gantt, ...columns: string[]): Promise<void> {
  await g.page.locator("#columns-button").click();
  for (const column of columns) await g.page.locator(`#columns-${column}`).click();
  await g.page.keyboard.press("Escape");
  await g.nextFrame(3);
}

test("the button opens the panel and Escape closes it; a hidden Start closes the columns up and brings them back", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  await expect(page.locator("#columns-panel")).toBeHidden();
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-panel")).toBeVisible();
  await expect(page.locator("#columns-panel"), "the panel is a menu, as the button's aria-haspopup says").toHaveAttribute("role", "menu");
  await expect(page.locator("#columns-float"), "each row is a checkbox").toHaveAttribute("role", "menuitemcheckbox");
  await expect(page.locator("#columns-button")).toHaveAttribute("aria-expanded", "true");
  for (const column of ["start", "finish", "float"]) await expect(page.locator(`#columns-${column}`), `${column} shows`).toHaveAttribute("aria-checked", "true");
  expect(await page.locator("#columns-start").evaluate((el) => getComputedStyle(el, "::before").content), "the checked tick is the heavy check mark, as text").toBe('"\u2714\uFE0E"');

  const before = await lefts(page, "dur");
  expect(before.head, "Dur has a box").not.toBeNull();
  await page.locator("#columns-start").click();
  await g.nextFrame(3);
  await expect(page.locator("#table")).toHaveAttribute("data-hide-start", "");
  await expect(page.locator("#columns-start")).toHaveAttribute("aria-checked", "false");
  expect(await lefts(page, "start"), "Start has no head and no cell").toEqual({ head: null, cell: null });
  // Task takes Start's track and gap (the columns end at the pane's edge), so Dur moves right by that much and Finish stays under its head.
  const after = await lefts(page, "dur");
  expect(after.head, "Dur moved right by Start's track and gap").toBeCloseTo(before.head! + 106, 0);
  expect(after.cell, "the Dur cells stay under their head").toBe(after.head);
  const finish = await lefts(page, "finish");
  expect(finish.cell, "the Finish cells stay under their head").toBe(finish.head);

  await page.keyboard.press("Escape");
  await expect(page.locator("#columns-panel")).toBeHidden();
  await expect(page.locator("#columns-button")).toBeFocused();
  await page.locator("#columns-button").click();
  await page.locator("#columns-start").click();
  await g.nextFrame(3);
  await expect(page.locator("#table")).not.toHaveAttribute("data-hide-start", /.*/);
  expect(await lefts(page, "dur"), "Dur is back where it was").toEqual(before);
});

test("with Progress on, each column still sits under its head and the table needs 106 px less for each hidden date column", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  await page.locator("#progress-toggle").click();
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await g.nextFrame(3);
  const width = (): Promise<number> => page.evaluate(() => document.getElementById("table-body")!.scrollWidth);
  const full = await width();

  await hide(g, "start", "finish");
  expect(await width(), "the two date columns' tracks and gaps leave the table's width").toBe(full - 212);
  for (const column of ["dur", "float", "percent", "actual-start"]) {
    const seen = await lefts(page, column);
    expect(seen.head, `${column} has a box`).not.toBeNull();
    expect(seen.cell, `${column} sits under its head`).toBe(seen.head);
  }
});

test("the cell cursor skips a hidden column", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  await hide(g, "finish");
  await page.locator('#table-rows .row[data-row="3"] > .dur').click();
  await g.nextFrame(2);
  await page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  expect(await marked(page), "ArrowRight from Dur reaches Start").toEqual(["start"]);
  await page.keyboard.press("ArrowRight");
  await g.nextFrame(2);
  expect(await marked(page), "the next ArrowRight passes the hidden Finish").toEqual(["float"]);
});

test("in a 420 px pane a hidden Start leaves Float whole in the pane, as the width fit counts the head", async ({ g }) => {
  await g.page.setViewportSize({ width: 800, height: 800 });
  await g.open({ n: 300 });
  await hide(g, "start");
  const room = await g.page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    return body.clientWidth - (document.querySelector("#table-head .cols > .float")!.getBoundingClientRect().right - body.getBoundingClientRect().left);
  });
  // Task gives up room so Dur, Finish and Float stay in the pane; a fit that counted the hidden Start head as a column left Float past the edge.
  expect(room, "pixels between Float's right edge and the pane's").toBeGreaterThanOrEqual(-1);
});

test("the rows read the table's flags when the panel opens, and a press writes nothing to localStorage", async ({ g }) => {
  await g.open({ n: 300 });
  const { page } = g;
  // A page can set the flags itself (the hosted page restores the last visit's): the panel shows them as they are when it opens.
  await page.evaluate(() => document.getElementById("table")!.setAttribute("data-hide-finish", ""));
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-finish"), "Finish is unticked").toHaveAttribute("aria-checked", "false");
  await expect(page.locator("#columns-start")).toHaveAttribute("aria-checked", "true");
  await page.locator("#columns-start").click();
  await expect(page.locator("#table")).toHaveAttribute("data-hide-start", "");
  expect(await page.evaluate(() => localStorage.length), "the package keeps nothing in the page's storage").toBe(0);
});
