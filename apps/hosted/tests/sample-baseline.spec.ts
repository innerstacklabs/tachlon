import { expect, test } from "@playwright/test";
import { ROWS } from "../samples/office-fitout";
import { waitReady, waitSampleSettled } from "./support/page";

// The office fit-out opens with a saved baseline (gs#703), one for each row, phases included: the Baseline button is pressed, Clear baseline shows, and the chart
// draws the grey comparison bars. Measured at 1280 x 1000.

test.use({ viewport: { width: 1280, height: 1000 } });

test("the page opens with the baseline set, pressed, with Clear baseline shown and a task slipped", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#baseline-set")).toHaveAttribute("aria-pressed", "true");
  await expect(page.locator("#baseline-clear")).toBeVisible();
  const held = await page.evaluate(() => window.gantt!.app.baseline_rows());
  expect(held, "every row carries a baseline, so none reads as added").toBe(ROWS.length);
  const baseline = await page.evaluate(() => (JSON.parse(window.gantt!.app.status()) as { baseline: { slipped: number; added: number } | null }).baseline);
  expect(baseline?.slipped ?? 0, "at least one task has slipped").toBeGreaterThan(0);
  expect(baseline?.added, "no row reads as added").toBe(0);
  await expect(page.locator("#baseline-add"), "Add new is hidden").toBeHidden();
});
