import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// The hover card's Start no earlier than row (gs#703): a task with a start constraint shows it after Finish, a task with none shows no row.

const XER = readFileSync(new URL("./fixtures/ledger-timeline.xer", import.meta.url));

const valueOf = (page: Page, label: string): Promise<string | null> =>
  page.evaluate(
    (name) => {
      const cells = [...document.querySelectorAll<HTMLElement>("#hover-card .details > *")];
      const at = cells.findIndex((el) => el.textContent === name);
      return at < 0 ? null : (cells[at + 1]?.textContent ?? null);
    },
    label,
  );

/** Move the pointer along the row until the card is for `task`; a card for another task is moved off first. */
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

/** Move the pointer off the chart so the next hover rebuilds the card. */
async function leave(g: Gantt): Promise<void> {
  const canvas = await g.box();
  await g.page.mouse.move(canvas.x + canvas.width / 2, canvas.y - 5);
  await g.nextFrame(2);
}

/** Open the ledger file, whose every task holds the import floor and one, Activity 13, a constraint above it. */
async function openLedger(g: Gantt): Promise<void> {
  const { page } = g;
  test.setTimeout(120_000);
  await g.open({ n: 200 });
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row[data-kind='0']") !== null);
}

test("the_card_of_a_task_at_its_import_floor_shows_no_constraint_row", async ({ page, g }) => {
  await openLedger(g);
  const { row, task } = await page.evaluate(() => {
    // Any leaf but Activity 13: the file's constraint is on that one alone.
    const el = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")].find((r) => ![...r.children].some((cell) => cell.textContent === "Activity 13"));
    if (el === undefined) throw new Error("no other leaf");
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  });
  await hoverTask(g, row, task);
  expect(await valueOf(page, "Finish"), "the card is shown").not.toBeNull();
  expect(await valueOf(page, "Start no earlier than"), "the floor is no constraint").toBeNull();
});

test("the_card_of_a_p6_start_constraint_reads_the_files_date", async ({ page, g }) => {
  await openLedger(g);
  const { row, task } = await page.evaluate(() => {
    const el = [...document.querySelectorAll<HTMLElement>("#table-rows .row")].find((r) => [...r.children].some((cell) => cell.textContent === "Activity 13"));
    if (el === undefined) throw new Error("no Activity 13 row");
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  });
  await hoverTask(g, row, task);
  // The file's cstr_date for Activity 13 (CS_MSOA).
  expect(await valueOf(page, "Start no earlier than")).toBe("2024-01-15 08:00");
});

test("a_typed_start_makes_the_row_and_clearing_it_removes_it", async ({ page, g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 200 });
  const { row, task } = await page.evaluate(() => {
    const el = [...document.querySelectorAll<HTMLElement>("#table-rows .row[data-kind='0']")].find(
      (r) => window.gantt!.app.task_actual_start(Number(r.dataset.task)) === "",
    );
    if (el === undefined) throw new Error("no leaf without actuals");
    return { row: Number(el.dataset.row), task: Number(el.dataset.task) };
  });
  await hoverTask(g, row, task);
  expect(await valueOf(page, "Finish"), "the card is shown").not.toBeNull();
  expect(await valueOf(page, "Start no earlier than"), "no constraint, no row").toBeNull();

  // A date a few days after the task's start, so the links do not already pass it and the bar stays near the pointer's row.
  const start = (await valueOf(page, "Start"))!;
  const later = new Date(Date.parse(`${start.replace(" ", "T")}:00Z`) + 3 * 86_400_000).toISOString().slice(0, 16).replace("T", " ");
  await leave(g);
  await page.evaluate(([t, text]) => window.gantt!.app.set_start_text(t as number, text as string), [task, later]);
  await g.nextFrame(3);
  await hoverTask(g, row, task);
  expect(await valueOf(page, "Start no earlier than")).toBe(later);

  await leave(g);
  await page.evaluate((t) => window.gantt!.app.set_start_text(t, ""), task);
  await g.nextFrame(3);
  await hoverTask(g, row, task);
  expect(await valueOf(page, "Start no earlier than"), "cleared").toBeNull();
});
