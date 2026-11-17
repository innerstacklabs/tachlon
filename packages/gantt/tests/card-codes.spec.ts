import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's WBS and activity code rows: the WBS code, then one row for each activity code type (`Discipline | D0`). A task with
// neither shows no such row, and the card reads them only when the hovered task changes.

/** The fixture with Activity 12 stripped of its code: Activity 1 to 11 carry Discipline D0 to D5 (then D0 on), and every task sits under WBS-1-0 / WBS-1-1. */
const XER = Buffer.from(readFileSync(new URL("./fixtures/one-project.xer", import.meta.url), "utf8").replace("%R\t1000011\t7000\t7105\t1000\n", ""));

/** The card's rows as [label, value], in order. */
const rowsOf = (page: Page): Promise<[string, string][]> =>
  page.evaluate(() => {
    const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")].map((el) => el.textContent ?? "");
    return Array.from({ length: cells.length / 2 }, (_, i): [string, string] => [cells[2 * i]!, cells[2 * i + 1]!]);
  });

/** Hover the bar of the leaf whose table row holds `name` (the first leaf when `name` is null), until the card is for it. */
async function hover(g: Gantt, name: string | null): Promise<void> {
  const { row, task } = await g.page.evaluate((label) => {
    const leaves = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")];
    const el = label === null ? leaves[0] : leaves.find((r) => [...r.children].some((cell) => cell.textContent === label));
    if (el === undefined) throw new Error(`no row named ${label}`);
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  }, name);
  const [s, canvas] = [await g.status(), await g.box()];
  const y = canvas.y + s.axisHeight + row * s.rowHeight + s.rowHeight / 2;
  for (let x = canvas.x + 4; x < canvas.x + canvas.width - 3; x += 4) {
    await g.page.mouse.move(x, y);
    await g.nextFrame(1);
    if ((await g.status()).hover?.task === task) return expect(g.page.locator("#hover-card")).toBeVisible();
  }
  throw new Error("no bar of the task under the pointer");
}

test("the_card_shows_the_wbs_code_then_a_row_for_each_activity_code_type", async ({ page, g }) => {
  await g.open({ n: 200 });
  await hover(g, null);
  const demo = (await rowsOf(page)).map(([label]) => label);
  expect(demo, "the demo has no WBS code and no activity code").not.toContain("WBS");
  await page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  await hover(g, "Activity 1");
  const coded = await rowsOf(page);
  const at = coded.findIndex(([label]) => label === "WBS");
  expect(coded.slice(at, at + 2), "the WBS row, then the code type's row").toEqual([
    ["WBS", "WBS-1-0.WBS-1-1"],
    ["Discipline", "D0"],
  ]);
  await hover(g, "Activity 3");
  expect(await rowsOf(page)).toContainEqual(["Discipline", "D2"]);
  await hover(g, "Activity 12");
  const plain = await rowsOf(page);
  expect(plain, "a task with a WBS code and no activity code has the WBS row only").toContainEqual(["WBS", "WBS-1-0.WBS-1-1"]);
  expect(plain.map(([label]) => label)).not.toContain("Discipline");
});

test("the_card_reads_the_codes_when_the_hovered_task_changes_not_every_frame", async ({ page, g }) => {
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
  await page.evaluate(() => {
    const app = window.gantt!.app as unknown as { task_activity_codes: (i: number) => string };
    const read = app.task_activity_codes.bind(app);
    const counter = { reads: 0 };
    app.task_activity_codes = (i) => {
      counter.reads += 1;
      return read(i);
    };
    (window as unknown as { codeReads: typeof counter }).codeReads = counter;
  });
  await hover(g, "Activity 2");
  const reads = (): Promise<number> => page.evaluate(() => (window as unknown as { codeReads: { reads: number } }).codeReads.reads);
  const first = await reads();
  expect(first, "the card read the codes for the hovered task").toBeGreaterThan(0);
  await g.nextFrame(30);
  expect(await reads(), "thirty frames over the same task read nothing more").toBe(first);
});
