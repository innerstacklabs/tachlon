// The Progress button (gs#703): the four progress columns show, empty, on a file with no actuals, and go again.
import { readFileSync } from "node:fs";
import { expect, test } from "./support/gantt";
import { tableSettled } from "./support/table";

// An MS Project file whose tasks carry no actuals.
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
const PLAIN = ["Task", "Dur", "Start", "Finish", "Float"];
const PROGRESS_HEAD = ["Actual start", "Actual finish", "Remaining", "% complete"];
const PROGRESS_CLASSES = ["actual-start", "actual-finish", "remaining", "percent"];

test("the_toggle_shows_empty_progress_columns_on_a_file_with_no_actuals_and_hides_them_again", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  const read = () =>
    page.evaluate(() => ({
      head: [...document.querySelectorAll("#table-head .cols > span")].map((cell) => cell.textContent ?? ""),
      rows: [...document.querySelectorAll<HTMLElement>("#table-rows .row")].map((row) => ({
        classes: [...row.children].map((cell) => cell.className),
        text: [...row.children].map((cell) => cell.textContent ?? ""),
      })),
      flag: document.getElementById("table")!.dataset.progress,
      pressed: document.getElementById("progress-toggle")!.getAttribute("aria-pressed"),
      has: window.gantt!.app.has_progress(),
    }));

  const before = await read();
  expect(before.has).toBe(false);
  expect(before.head).toEqual(PLAIN);
  expect(before.flag).toBeUndefined();
  expect(before.pressed).toBe("false");

  await page.click("#progress-toggle");
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await page.waitForFunction(() => document.querySelector("#table-rows .row .percent") !== null);
  const on = await read();
  expect(on.pressed).toBe("true");
  expect(on.head).toEqual([...PLAIN, ...PROGRESS_HEAD]);
  expect(on.rows.length).toBeGreaterThan(0);
  for (const row of on.rows) {
    expect(row.classes).toEqual(["name", "dur", "start", "finish", "float", ...PROGRESS_CLASSES]);
    expect(row.text.slice(5), "the progress cells are empty").toEqual(["", "", "", ""]);
  }
  expect(await tableSettled(page), "writes to the table over 30 idle frames with the columns on").toBe(0);

  await page.click("#progress-toggle");
  await expect(page.locator("#table")).not.toHaveAttribute("data-progress", /.*/);
  await page.waitForFunction(() => document.querySelector("#table-rows .row .percent") === null);
  const off = await read();
  expect(off.pressed).toBe("false");
  expect(off.head).toEqual(PLAIN);
  for (const row of off.rows) expect(row.classes).toEqual(["name", "dur", "start", "finish", "float"]);
  expect(await tableSettled(page), "writes to the table over 30 idle frames with the columns off").toBe(0);
});

test("the_chart_works_with_the_toolbar_removed_and_keeps_its_five_columns", async ({ g }) => {
  await g.page.addInitScript(() => {
    new MutationObserver(() => document.getElementById("toolbar")?.remove()).observe(document, { childList: true, subtree: true });
  });
  // The g fixture fails the test on a page error, which is what a missing #progress-toggle used to raise.
  await g.open({ n: 1000 });
  expect(await g.page.locator("#progress-toggle").count()).toBe(0);
  await g.nextFrame(3);
  expect(await g.page.evaluate(() => document.getElementById("table")!.dataset.progress)).toBeUndefined();
});

test("a_summary_row_reads_the_duration_weighted_percent_of_its_leaves", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.click("#progress-toggle");
  await page.waitForFunction(() => document.querySelector("#table-rows .row .percent") !== null);

  const percent = (name: string) =>
    page.locator("#table-rows .row", { has: page.locator(".name", { hasText: name }) }).locator(".percent");
  await expect(percent("First summary"), "nothing under it has started").toHaveText("");

  // Finish one of the two 8-hour leaves under the first summary: half of its work is done.
  await page.evaluate(() => {
    const row = [...document.querySelectorAll<HTMLElement>("#table-rows .row")].find((r) => r.querySelector(".name")?.textContent?.includes("First child"))!;
    window.gantt!.app.set_progress(Number(row.dataset.task), "2026-10-05", "2026-10-05", "");
  });
  await expect(percent("First child")).toHaveText("100%");
  await expect(percent("First summary")).toHaveText("50%");
  await expect(percent("Second summary"), "another summary is untouched").toHaveText("");
});

test("turning_progress_on_at_1280_puts_percent_complete_inside_the_table_pane_and_the_chart_keeps_half", async ({ g }) => {
  const { page } = g;
  test.setTimeout(240_000);
  await page.setViewportSize({ width: 1280, height: 800 });
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  const seen = () =>
    page.evaluate(() => {
      const table = document.getElementById("table")!.getBoundingClientRect();
      const head = (cls: string) => document.querySelector(`#table-head .${cls}`)?.getBoundingClientRect() ?? new DOMRect();
      const name = document.querySelector<HTMLElement>("#table-rows .row .name")!;
      return {
        table: table.width,
        chart: window.innerWidth - table.right,
        tableRight: table.right,
        percentRight: head("percent").right,
        percentLeft: head("percent").left,
        taskRight: head("name").right,
        scrollLeft: document.getElementById("table-body")!.scrollLeft,
        dragged: document.querySelector<HTMLElement>(".gantt")!.style.getPropertyValue("--gantt-table-w"),
        ellipsis: getComputedStyle(name).textOverflow,
        title: name.title,
        text: name.textContent,
      };
    });

  const before = await seen();
  expect(before.chart, "the chart keeps half the width with Progress off").toBeGreaterThanOrEqual(640);
  await page.click("#progress-toggle");
  await expect(page.locator("#table")).toHaveAttribute("data-progress", "true");
  await g.nextFrame(3);
  const on = await seen();
  expect(on.scrollLeft, "no scroll").toBe(0);
  expect(on.dragged, "no divider drag").toBe(before.dragged);
  expect(on.percentRight, "the % complete head is inside the table pane").toBeLessThanOrEqual(on.tableRight);
  expect(on.percentLeft, "% complete follows Task").toBeCloseTo(on.taskRight + 6, 0);
  expect(on.chart, "the chart keeps half the width with Progress on").toBeGreaterThanOrEqual(640);
  expect(on.table, "the table is no wider").toBe(before.table);
  expect(on.ellipsis, "a Task name is cut with an ellipsis").toBe("ellipsis");
  expect(on.title.length, "the full name is the title").toBeGreaterThanOrEqual((on.text ?? "").length);

  const heads = () =>
    page.evaluate(() => {
      const left = (cls: string) => document.querySelector(`#table-head .${cls}`)!.getBoundingClientRect().left;
      const body = document.getElementById("table-body")!;
      return {
        percent: left("percent"),
        actualStart: left("actual-start"),
        actualFinish: left("actual-finish"),
        remaining: left("remaining"),
        remainingRight: document.querySelector("#table-head .remaining")!.getBoundingClientRect().right,
        task: document.querySelector("#table-head .name")!.getBoundingClientRect().width,
        taskLeft: left("name"),
        rowTaskLeft: document.querySelector("#table-rows .row .name")!.getBoundingClientRect().left,
        wide: body.scrollWidth > body.clientWidth,
        bodyRight: body.getBoundingClientRect().right,
      };
    });
  const rest = await heads();
  expect(rest.task, "Task keeps at least 60px").toBeGreaterThanOrEqual(60);
  expect(rest.percent, "Actual start follows % complete").toBeLessThan(rest.actualStart);
  expect(rest.actualStart, "Actual finish follows Actual start").toBeLessThan(rest.actualFinish);
  expect(rest.actualFinish, "Remaining follows Actual finish").toBeLessThan(rest.remaining);
  expect(rest.wide, "the table is wider than its pane").toBe(true);
  await page.evaluate(() => {
    const body = document.getElementById("table-body")!;
    body.scrollLeft = body.scrollWidth;
  });
  await g.nextFrame(3);
  const end = await heads();
  expect(end.remainingRight, "the table scrolls to Remaining").toBeLessThanOrEqual(end.bodyRight);
  expect(end.taskLeft, "the Task head stays frozen").toBeCloseTo(rest.taskLeft, 0);
  expect(end.rowTaskLeft, "the Task cells stay frozen").toBeCloseTo(rest.rowTaskLeft, 0);
  await page.evaluate(() => {
    document.getElementById("table-body")!.scrollLeft = 0;
  });
  await g.nextFrame(3);

  await page.click("#progress-toggle");
  await expect(page.locator("#table")).not.toHaveAttribute("data-progress", /.*/);
  await g.nextFrame(3);
  expect((await seen()).table, "the table width before Progress came back").toBe(before.table);
});
