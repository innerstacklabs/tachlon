import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The hosted page remembers which of Start, Finish and Float the viewer hid (gantt#703): a reload brings the table back with them hidden, the
// Columns menu shows them unticked, and showing a column again is remembered too.
test.use({ viewport: { width: 1280, height: 800 } });

test("a reload keeps the columns hidden in the Columns menu, and shows again the ones shown again", async ({ page }) => {
  const load = async (): Promise<void> => {
    await page.goto("/");
    await waitReady(page, { frames: 10, timeout: 180_000 });
    await waitSampleSettled(page, 120_000);
  };
  const toggle = async (...columns: string[]): Promise<void> => {
    await page.locator("#columns-button").click();
    for (const column of columns) await page.locator(`#columns-${column}`).click();
    await page.keyboard.press("Escape");
  };
  await load();
  await toggle("start", "float");
  await expect(page.locator("#table")).toHaveAttribute("data-hide-start", "");

  await page.reload();
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const table = page.locator("#table");
  await expect(table, "Start stays hidden").toHaveAttribute("data-hide-start", "");
  await expect(table, "Float stays hidden").toHaveAttribute("data-hide-float", "");
  await expect(table, "Finish was never hidden").not.toHaveAttribute("data-hide-finish", /.*/);
  await page.locator("#columns-button").click();
  await expect(page.locator("#columns-start"), "the menu shows Start unticked").toHaveAttribute("aria-checked", "false");
  await expect(page.locator("#columns-finish")).toHaveAttribute("aria-checked", "true");
  await expect(page.locator("#columns-float"), "the menu shows Float unticked").toHaveAttribute("aria-checked", "false");
  await page.locator("#columns-start").click();
  await page.keyboard.press("Escape");

  await page.reload();
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(table, "Start, shown again, stays shown").not.toHaveAttribute("data-hide-start", /.*/);
  await expect(table, "Float is still hidden").toHaveAttribute("data-hide-float", "");
});
