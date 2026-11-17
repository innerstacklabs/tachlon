import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Calendar row (gs#703): a leaf's card names the days a week and hours a day of its calendar. The fit-out sample works
// Monday to Friday, 08:00 to 17:00 with lunch.

const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate(
    (name) => {
      const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
      const at = cells.findIndex((el) => el.textContent === name);
      return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
    },
    label,
  );

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

/** Move the pointer along the row until the card is for `task`. */
async function hoverTask(g: Gantt, row: number, task: number): Promise<void> {
  const { page } = g;
  const s = await g.status();
  const canvas = await g.box();
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  await page.mouse.move(canvas.x + 1, y);
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.task === task) {
      await expect(page.locator("#hover-card")).toBeVisible();
      return;
    }
  }
  throw new Error("no bar of the task under the pointer");
}

/** Open the fit-out and hover the first leaf or summary row. */
async function hoverFirst(g: Gantt, leaf: boolean): Promise<void> {
  const { page } = g;
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const { row, task } = await page.evaluate((wantLeaf) => {
    const el = document.querySelector<HTMLElement>(wantLeaf ? "#table-rows .row[data-kind='0']" : "#table-rows .row:not([data-kind='0'])");
    if (el === null) throw new Error("no such row");
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  }, leaf);
  await hoverTask(g, row, task);
}

test("the_card_of_a_leaf_names_its_calendar", async ({ g }) => {
  await hoverFirst(g, true);
  expect(await valueOf(g.page, "Calendar")).toBe("5 days a week, 8 h a day");
});

test("the_card_of_a_summary_has_no_calendar_row", async ({ g }) => {
  await hoverFirst(g, false);
  expect(await valueOf(g.page, "Finish"), "the summary's card is shown").not.toBeNull();
  expect(await valueOf(g.page, "Float"), "a summary has no float").toBeNull();
  expect(await valueOf(g.page, "Calendar")).toBeNull();
});
