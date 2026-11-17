// Typing into the Finish cell (gs#703): a day sets the task's duration, the start stays, one undo step. summary.xml starts Monday 2026-01-05.
import { readFileSync } from "node:fs";
import type { Locator, Page } from "@playwright/test";
import { type Gantt, expect, test } from "./support/gantt";
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
const row = (page: Page, name: string): Locator => page.locator("#table-rows .row", { has: page.locator(".name", { hasText: name }) });
async function openAndType(g: Gantt, name: string, text: string): Promise<Locator> {
  const { page } = g;
  test.setTimeout(240_000);
  page.setDefaultTimeout(30_000);
  await g.open({ n: 1000 });
  await page.setInputFiles("#open-input", { name: "summary.xml", mimeType: "application/octet-stream", buffer: MSPDI });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row .finish") !== null);
  await page.evaluate(() => new Promise<void>((done) => requestAnimationFrame(() => requestAnimationFrame(() => done()))));
  const r = row(page, name);
  await r.locator(".finish").dblclick();
  await expect(r.locator(".finish input.cell-edit")).toBeVisible();
  await r.locator(".finish input.cell-edit").fill(text);
  await r.locator(".finish input.cell-edit").press("Enter");
  return r;
}

test("typing a finish three working days after the start makes the duration 3d, Ctrl+Z puts it back", async ({ g }) => {
  const child = await openAndType(g, "First child", "2026-01-07");
  await expect(child.locator(".dur")).toHaveText("3d");
  await expect(child.locator(".start"), "the start stays").toHaveText(/^2026-01-05/);
  await g.page.keyboard.press("Control+z");
  await expect(child.locator(".dur")).toHaveText("1d");
});

test("a day before the start is refused with a sentence that says so", async ({ g }) => {
  const child = await openAndType(g, "First child", "2026-01-04");
  await expect(child.locator(".cell-error")).toHaveText("That day is before the task starts on 2026-01-05. Write a day on or after it.");
  await expect(child.locator(".dur")).toHaveText("1d");
});
