import assert from "node:assert/strict";
import type { Page } from "@playwright/test";

// gantt#1092: the plain page shows the chart, hears a row picked and stops cleanly.
export default async function (page: Page, url: string): Promise<void> {
  const problems: string[] = [];
  page.on("pageerror", (e) => problems.push(e.message));
  page.on("console", (m) => m.type() === "error" && problems.push(m.text()));
  await page.goto(url);
  const row = page.locator('#table-rows .row[data-row="2"]');
  await row.waitFor({ timeout: 60_000 });
  await row.click({ position: { x: 4, y: 4 } });
  await page.waitForFunction(() => document.getElementById("picked")?.textContent === "Picked: Ship");
  await page.click("#stop");
  await page.waitForFunction(() => document.getElementById("chart")?.childElementCount === 0);
  assert.deepEqual(problems, []);
}
