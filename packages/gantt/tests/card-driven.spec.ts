import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Driven by row (gs#703): a leaf's card names the predecessors whose links set its start, and only those. In the fit-out
// sample "Design coordination review" follows both detailed design (120 h) and MEP design (96 h), so only the first sets its start.

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

/** Open the fit-out, then move the pointer along the row of the task named `name` until the card is for it. */
async function hoverNamed(g: Gantt, name: string): Promise<void> {
  const { page } = g;
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const { row, task } = await page.evaluate((wanted) => {
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")];
    const el = rows.find((r) => window.gantt!.app.task_name(Number(r.dataset.task)).split(" / ").at(-1) === wanted);
    if (el === undefined) throw new Error(`no row named ${wanted}`);
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  }, name);
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

test("the_card_names_the_predecessor_that_sets_the_start_and_not_the_other", async ({ g }) => {
  await hoverNamed(g, "Design coordination review");
  expect(await valueOf(g.page, "Driven by")).toBe("Design / Detailed design and specification");
  expect(await valueOf(g.page, "Predecessors (2)"), "both links are still listed").toContain("MEP engineering design");
});
