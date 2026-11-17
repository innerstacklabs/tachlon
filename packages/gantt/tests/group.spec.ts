// Grouping (gantt#1875, decision 0015): `commands.setGroupBy` and the Group choice put one header per value of an activity code type
// into the table, a header closes and opens (its chevron, Alt+Left and Alt+Right), and null or the empty entry ungroups.
import { readFileSync } from "node:fs";
import { expect, type Gantt, openFilters, test } from "./support/gantt";

test.use({ webgl: true });

const LEDGER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url), "utf8");
const PHASES = ["Civil", "MEP", "Site", "Fit-out", "Test", "Handover"];
// The fixture gives every task a Phase-like code but defines no type or value: add a type "Phase" with six values.
const CODED = Buffer.from(
  LEDGER.replace(
    /%E\s*$/,
    "%T\tACTVTYPE\n%F\tactv_code_type_id\tactv_code_type\n%R\t7000\tPhase\n%T\tACTVCODE\n%F\tactv_code_id\tactv_code_type_id\tshort_name\n" +
      PHASES.map((phase, i) => `%R\t${7100 + i}\t7000\t${phase}\n`).join("") +
      "%E\n",
  ),
);

async function openCoded(g: Gantt): Promise<void> {
  await g.open({ n: 200 });
  await g.page.setInputFiles("#open-input", { name: "coded.xer", mimeType: "application/octet-stream", buffer: CODED });
  await g.page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
}

test("grouping by a code type draws headers, a header closes and opens, and null ungroups", async ({ g }) => {
  const { page } = g;
  await openCoded(g);
  const flat = (await g.status()).rowCount;
  const headers = page.locator('#table-rows .row[data-kind="3"]');
  // The table draws a window of rows, so the headers are counted in the app's rows.
  const count = (kind: number) => page.evaluate((k) => (JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as number[][]).filter((row) => row[5] === k).length, kind);
  const state = () => page.evaluate(() => window.gantt!.api!.state().groupBy);
  const set = (type: string | null) => page.evaluate((t) => window.gantt!.api!.commands.setGroupBy(t), type);

  expect(await set("Nope"), "a type no task carries is refused").toBe(false);
  expect(await set("Phase")).toBe(true);
  expect(await state()).toBe("Phase");
  expect(await count(3)).toBe(PHASES.length);
  await expect(headers.first()).toHaveText(/^Civil \(\d+\)$/);
  const grouped = (await g.status()).rowCount;

  // Alt+Left closes the group of the selected leaf (the first row under the first header), Alt+Right opens it.
  await page.evaluate(() => window.gantt!.api!.commands.select((JSON.parse(window.gantt!.app.table_rows(1, 2)) as number[][])[0]![0]!));
  await page.keyboard.press("Alt+ArrowLeft");
  await expect.poll(() => count(4)).toBe(1);
  expect((await g.status()).rowCount).toBeLessThan(grouped);
  await page.keyboard.press("Alt+ArrowRight");
  await expect.poll(() => count(3)).toBe(PHASES.length);
  expect((await g.status()).rowCount).toBe(grouped);

  // The chevron of a header closes it too.
  await headers.first().locator(".disclosure").click();
  await expect.poll(() => count(4)).toBe(1);

  // The Group choice, and the empty entry, give the same rows as the command.
  await openFilters(page);
  await page.selectOption("#group-by", "");
  await expect.poll(() => count(3)).toBe(0);
  expect(await state()).toBeNull();
  expect((await g.status()).rowCount).toBe(flat);
  await openFilters(page);
  await page.selectOption("#group-by", "Phase");
  await expect.poll(() => count(3)).toBe(PHASES.length);
  expect(await set(null)).toBe(true);
  expect((await g.status()).rowCount).toBe(flat);
});

test("Alt+Left and Alt+Right close and open the focused header's own group, with nothing selected", async ({ g }) => {
  const { page } = g;
  await openCoded(g);
  await page.evaluate(() => window.gantt!.api!.commands.setGroupBy("Phase"));
  const closed = page.locator(`#table-rows .row[data-kind="4"]`);
  const second = page.locator(`#table-rows .row[data-kind="3"]`).nth(1);
  const group = await second.getAttribute("data-group");
  await second.locator(".disclosure").focus();
  await page.keyboard.press("Alt+ArrowLeft");
  await expect(closed).toHaveAttribute("data-group", group!);
  await closed.locator(".disclosure").focus();
  await page.keyboard.press("Alt+ArrowRight");
  await expect(closed).toHaveCount(0);
});

test("a filter that leaves no bar in view, with the rows grouped, scrolls to the first task", async ({ g }) => {
  const { page } = g;
  await openCoded(g);
  const start = await page.evaluate(() => (JSON.parse(window.gantt!.app.table_rows(0, 1)) as string[][])[0]![1]!.slice(0, 10));
  await page.evaluate(() => window.gantt!.api!.commands.setGroupBy("Phase"));
  await page.evaluate(() => window.gantt!.app.go_to_date("2040-01-01"));
  await expect.poll(() => g.status().then((s) => s.bars)).toBe(0);
  await page.evaluate((day) => window.gantt!.api!.commands.setFilter({ lookahead: { start: day, weeks: 520 } }), start);
  await expect.poll(() => g.status().then((s) => s.bars)).toBeGreaterThan(0);
});
