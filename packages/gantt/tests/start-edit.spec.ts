// Typing into the Start cell (gs#703): a date sets the task's start-no-earlier-than through the table's cell editor, one undo
// step. Real mouse and keyboard events; the app is only the oracle for what it holds.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";

// An MS Project file whose tasks carry no actuals: two summaries, each with leaves of eight working hours.
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));

const row = (page: Page, name: string): Locator => page.locator("#table-rows .row", { has: page.locator(".name", { hasText: name }) });
const stamp = (page: Page) => page.evaluate(() => window.gantt!.status().stamp);

async function openFile(g: Gantt): Promise<void> {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row .start") !== null);
}

/** Double-click the Start cell of `r` with the mouse, replace the editor's text and press Enter. */
async function type(page: Page, r: Locator, text: string): Promise<void> {
  // The rows are drawn again on the frame after an edit; a press before that would land on rows about to be replaced.
  await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
  await r.locator(".start").dblclick();
  const input = r.locator(".start input.cell-edit");
  await expect(input).toBeVisible();
  await input.fill(text);
  await input.press("Enter");
}

test("typing 2027-02-01 moves the bar, Ctrl+Z puts it back", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const child = row(page, "First child");
  const before = (await child.locator(".start").textContent()) ?? "";
  const settled = await stamp(page);

  await type(page, child, "2027-02-01");
  await expect(child.locator(".start")).toHaveText(/^2027-02-01/);
  expect(await stamp(page), "the edit bumps the stamp").toBeGreaterThan(settled);
  expect(await page.locator("#table-rows input.cell-edit").count(), "the editor closed").toBe(0);

  await page.keyboard.press("Control+z");
  await expect(child.locator(".start")).toHaveText(before);
});

test("a summary and text that is not a date are refused with the reason, changing nothing", async ({ g }) => {
  const { page } = g;
  await openFile(g);
  const settled = await stamp(page);

  const child = row(page, "First child");
  await type(page, child, "soon");
  await expect(child.locator(".cell-error")).toHaveText("Write the start as YYYY-MM-DD or YYYY-MM-DD HH:MM.");
  await child.locator(".start input").press("Escape");

  const summary = row(page, "First summary");
  await type(page, summary, "2027-02-01");
  await expect(summary.locator(".cell-error")).toContainText("is a summary");
  await summary.locator(".start input").press("Escape");
  expect(await stamp(page)).toBe(settled);
});
