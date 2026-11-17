import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

// The task list marks a Start cell held by a real start constraint (gs#703 K7): data-constrained and a title with the date; the floor an import gives is none.

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

/** The Start cells of the leaf rows drawn now, as [task, data-constrained, title, text]. */
const startCells = (page: Page): Promise<[number, string | undefined, string, string][]> =>
  page.evaluate(() =>
    [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")].map((r) => {
      const cell = r.querySelector<HTMLElement>(".start")!;
      return [Number(r.dataset.task), cell.dataset["constrained"], cell.title, cell.textContent ?? ""];
    }),
  );

test("a_p6_start_constraint_marks_its_start_cell_and_the_import_floor_does_not", async ({ page, g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  const cells = await startCells(page);
  // The file's cstr_date for Activities 13, 26 and 39 (CS_MSOA). Activity 26 has an actual start, which pins its early start (decision 0008), so it holds no constraint and no mark; every other task holds only the import floor.
  const marked = cells.filter(([, mark]) => mark === "true").map(([, , title]) => title);
  expect(marked.sort()).toEqual(["2024-01-15 08:00", "2024-02-13 08:00"].map((date) => `Start no earlier than ${date}`));
  expect(cells.length - marked.length, "floor rows are drawn beside them, unmarked").toBeGreaterThan(1);
});

test("a_typed_start_marks_the_cell_and_clearing_it_removes_the_mark", async ({ page, g }) => {
  await g.open({ n: 200 });
  const [task, mark, , start] = (await startCells(page))[0]!;
  expect(mark, "no constraint, no mark").toBeUndefined();
  const later = new Date(Date.parse(`${start.replace(" ", "T")}:00Z`) + 3 * 86_400_000).toISOString().slice(0, 16).replace("T", " ");
  const cellOf = async (i: number) => (await startCells(page)).find(([t]) => t === i)!;
  await page.evaluate(([t, text]) => window.gantt!.app.set_start_text(t as number, text as string), [task, later]);
  await expect.poll(async () => (await cellOf(task))[1]).toBe("true");
  expect((await cellOf(task))[2]).toBe(`Start no earlier than ${later}`);
  await page.evaluate((t) => window.gantt!.app.set_start_text(t, ""), task);
  await expect.poll(async () => (await cellOf(task))[1]).toBeUndefined();
});
