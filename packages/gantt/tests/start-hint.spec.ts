// The Start editor's hint (gs#703): a task with a real start constraint opens its Start editor with a title that names the date
// and how to clear it; a task that only holds the floor its import gave it has none.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** Double-click the Start cell of the row of `task` and return the editor's input. */
async function openStartEditor(page: Page, task: number): Promise<Locator> {
  const start = page.locator(`#table-rows .row[data-task="${task}"] .start`);
  await start.dblclick();
  const input = start.locator("input.cell-edit");
  await expect(input).toBeVisible();
  return input;
}

test("the_start_editor_names_a_real_constraint_and_a_floor_has_no_hint", async ({ page, g }) => {
  test.setTimeout(240_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const { constrained, floored } = await page.evaluate(() => {
    const rows = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")];
    const task = (el: HTMLElement | undefined): number => {
      if (el === undefined) throw new Error("no such row");
      return Number(el.dataset.task);
    };
    return {
      constrained: task(rows.find((r) => [...r.children].some((cell) => cell.textContent === "Activity 13"))),
      // Any leaf but Activity 13: the file's constraint is on that one alone.
      floored: task(rows.find((r) => ![...r.children].some((cell) => cell.textContent === "Activity 13"))),
    };
  });

  // The file's cstr_date for Activity 13 (CS_MSOA).
  const hint = await openStartEditor(page, constrained);
  await expect(hint).toHaveAttribute("title", /2024-01-15 08:00/);
  await expect(hint).toHaveAttribute("title", /Clear/);
  await hint.press("Escape");
  await expect(page.locator("#table-rows input.cell-edit")).toHaveCount(0);

  // Every task of the import holds a floor; one at it has no constraint to name.
  const plain = await openStartEditor(page, floored);
  await expect(plain).not.toHaveAttribute("title", /.*/);
});
