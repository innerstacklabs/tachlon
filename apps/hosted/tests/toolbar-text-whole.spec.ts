import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// With "Open a sample" in the toolbar, the toolbar's own text is still whole at 1280 px (gs#858, on gs#866): the file line, the data date, the
// row count and the find box's hint show with no ellipsis, with the office fit-out and with a P6 file, with the Columns button in the strip (tl703fx). Measured at 1280 x 800.

test.use({ viewport: { width: 1280, height: 800 } });

const LEDGER = readFileSync(new URL("../../../packages/gantt/tests/fixtures/ledger-timeline.xer", import.meta.url));

/** What each of the four texts needs, against the room it has: a text is whole when it needs no more than it has. */
async function cut(page: Page, giveWay = false): Promise<Record<string, string>> {
  return page.evaluate((giveWay) => {
    const cuts: Record<string, string> = {};
    const text = (id: string): void => {
      const el = document.getElementById(id)!;
      if (el.hidden || el.textContent === "") return;
      // scrollWidth and clientWidth are whole pixels, so a text that is 0.1 px too wide still reads as whole to them while its end is drawn as an ellipsis.
      const range = document.createRange();
      range.selectNodeContents(el);
      const needs = range.getBoundingClientRect().width;
      const has = el.getBoundingClientRect().width;
      // With a filter's count whole beside the `?` button and Find's 90 px hint (A devteam-1-368, 369), the file name may end in an ellipsis
      // when it gives way, if its tooltip (on #open-info) carries the full name; the data date may be up to 1 px short.
      const nameCutWithTitle = giveWay && id === "open-file" && (document.getElementById("open-info")!.title ?? "").includes(el.textContent ?? "");
      const slack = giveWay && id === "data-date" ? 1 : 0.01;
      if (needs > has + slack && !nameCutWithTitle) cuts[id] = `"${el.textContent}" needs ${needs.toFixed(1)}px, has ${has.toFixed(1)}px`;
    };
    text("open-file");
    text("data-date");
    text("row-count");
    const input = document.getElementById("find-input") as HTMLInputElement;
    const style = getComputedStyle(input);
    const canvas = document.createElement("canvas").getContext("2d")!;
    canvas.font = `${style.fontStyle} ${style.fontWeight} ${style.fontSize} ${style.fontFamily}`;
    const needed = canvas.measureText(input.placeholder).width;
    const room = input.clientWidth - parseFloat(style.paddingLeft) - parseFloat(style.paddingRight);
    if (needed > room) cuts["find-input"] = `"${input.placeholder}" needs ${Math.ceil(needed)}px, has ${Math.floor(room)}px`;
    const bar = document.getElementById("toolbar")!;
    if (bar.scrollWidth > bar.clientWidth) cuts.toolbar = `overflows by ${bar.scrollWidth - bar.clientWidth}px`;
    return cuts;
  }, giveWay);
}

/** Open the ledger file over the office fit-out the page starts on. */
async function openLedger(page: Page): Promise<void> {
  const from = await page.evaluate(() => window.gantt!.status().epoch);
  await page.setInputFiles("#open-input", { name: "ledger-timeline.xer", mimeType: "application/octet-stream", buffer: LEDGER });
  await page.waitForFunction((n) => window.gantt!.status().epoch === n, from + 1, { timeout: 60_000 });
  await expect(page.locator("#open-file")).toContainText("ledger-timeline.xer");
}

/** Lengthen the first task with no children, which moves the tasks after it and slips the finish against a baseline. */
async function slipALeaf(page: Page): Promise<void> {
  await page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    app.set_duration(rows.find((row) => row[5] === 0)![0], "300d");
  });
}

/**
 * The same edit made in the task list, which is what turns "Show moved" on once it moves another task: a task with no children gets a
 * duration of 300 days, taking the tasks with no children one after the other until one has a successor that moves. Started tasks
 * are tried last: with their actuals applied (gs#860) a longer duration may not move anything.
 */
async function editALeaf(page: Page): Promise<void> {
  const leaves = await page.evaluate(() => {
    const rows = JSON.parse(window.gantt!.app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    const leaves = rows.flatMap((row, i) => (row[5] === 0 ? [i] : []));
    const started = (i: number) => window.gantt!.app.task_details(rows[i][0]).status !== "Not started";
    return [...leaves.filter((i) => !started(i)), ...leaves.filter(started)].slice(0, 8);
  });
  for (const row of leaves) {
    await page.dblclick(`#table-rows .row[data-row="${row}"] .dur`);
    await page.keyboard.press("Control+A");
    await page.keyboard.type("300d");
    await page.keyboard.press("Enter");
    await page.waitForTimeout(300);
    await page.locator("#filters-button").click();
    const moved = await page.locator("#moved-filter").isEnabled();
    await page.keyboard.press("Escape");
    if (moved) return;
  }
  throw new Error("no edit of the first tasks moved another task");
}

/** Press a control in the Filters menu, and close the menu again. */
async function inFilters(page: Page, id: string): Promise<void> {
  await page.locator("#filters-button").click();
  await page.locator(`#${id}`).click();
  await page.keyboard.press("Escape");
}

/** The filter states the toolbar's text must survive: each one adds to the row count or to what the strip holds. */
const STATES: { name: string; giveWay?: boolean; set(page: Page): Promise<void> }[] = [
  { name: "no filter", set: async () => {} },
  {
    name: "Show moved on",
    giveWay: true,
    set: async (page) => {
      await editALeaf(page);
      await page.locator("#filters-button").click();
      await expect(page.locator("#moved-filter")).toBeEnabled();
      await page.locator("#moved-filter").click();
      await page.keyboard.press("Escape");
    },
  },
  { name: "Critical only on", giveWay: true, set: (page) => inFilters(page, "critical-toggle") },
  {
    name: "a baseline set and Slipped on",
    set: async (page) => {
      await page.locator("#baseline-set").click();
      await slipALeaf(page);
      await expect(page.locator("#baseline-clear")).toBeVisible();
      await page.locator("#filters-button").click();
      await expect(page.locator("#slipped-toggle")).toBeVisible();
      await page.locator("#slipped-toggle").click();
      await page.keyboard.press("Escape");
    },
  },
  {
    name: "a baseline set and Variance on",
    set: async (page) => {
      await page.locator("#baseline-set").click();
      await expect(page.locator("#variance-toggle")).toBeVisible();
      await expect(page.locator("#variance-toggle")).toHaveAttribute("aria-pressed", "true");
    },
  },
  {
    name: "a baseline set, a slipped task and Variance on",
    set: async (page) => {
      await page.locator("#baseline-set").click();
      await slipALeaf(page);
      await expect(page.locator("#variance-toggle")).toHaveAttribute("aria-pressed", "true");
    },
  },
  {
    name: "a baseline set and Variance off",
    set: async (page) => {
      await page.locator("#baseline-set").click();
      await page.locator("#variance-toggle").click();
      await expect(page.locator("#variance-toggle")).toHaveAttribute("aria-pressed", "false");
    },
  },
];

for (const file of ["the office fit-out", "ledger-timeline.xer"]) {
  for (const state of STATES) {
    test(`${file}, ${state.name}: the file line, data date, row count and find hint are whole at 1280 px`, async ({ page }) => {
      await page.goto("/");
      await waitReady(page, { frames: 10, timeout: 180_000 });
      await waitSampleSettled(page, 120_000);
      if (file !== "the office fit-out") await openLedger(page);
      await state.set(page);
      const frames = await page.evaluate(() => window.gantt!.status().frames);
      await page.waitForFunction((k) => window.gantt!.status().frames >= k + 3, frames);
      await expect(page.locator("#row-count")).toHaveText(/(rows?|activities|slipped)$/);
      if (file === "the office fit-out") await expect(page.locator("#data-date")).toBeVisible();
      expect(await cut(page, state.giveWay === true && file === "the office fit-out")).toEqual({});
    });
  }
}
