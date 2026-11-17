import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Export the list as Excel" (gantt#1875), under Save, downloads the visible list as <name>.xlsx. A click and Enter both choose it,
// and the menu closes. The file's own contents are checked in packages/gantt/tests/export-xlsx.spec.ts.

test.use({ viewport: { width: 1280, height: 800 } });

/** Open the menu, choose the item and return the download's file name. */
async function exportXlsx(page: Page, how: "click" | "keyboard"): Promise<string> {
  await page.locator("#open-button").click();
  const item = page.getByRole("menuitem", { name: /Export the list as Excel/ });
  await expect(item).toBeVisible();
  await expect(item, "live, not Coming soon").not.toHaveAttribute("aria-disabled", "true");
  const choose = how === "click" ? item.click() : item.focus().then(() => page.keyboard.press("Enter"));
  const [download] = await Promise.all([page.waitForEvent("download"), choose]);
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  return download.suggestedFilename();
}

for (const how of ["click", "keyboard"] as const) {
  test(`the_menu_item_downloads_the_xlsx_by_${how}`, async ({ page }) => {
    await page.goto("/");
    await waitReady(page, { frames: 10, timeout: 180_000 });
    await waitSampleSettled(page, 120_000);
    // The default page opens the office-fitout file sample, so the name is that file's, without its extension.
    expect(await exportXlsx(page, how)).toBe("office-fitout.xlsx");
    const rows = await page.evaluate(() => window.gantt!.status().rowCount);
    await expect(page.locator("#save-note")).toHaveText(`Exported ${rows} rows to office-fitout.xlsx`);
  });
}

test("the_item_sits_under_the_csv_item_and_is_not_a_button", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await page.locator("#open-button").click();
  const labels = await page.locator('#sample-list [role="menuitem"] .sample-label').allTextContents();
  expect(labels.indexOf("Export the list as Excel"), "right after the CSV item").toBe(labels.indexOf("Export the list as CSV") + 1);
  await expect(page.locator("#sample-list button").filter({ hasText: /Export the list as Excel/ })).toHaveCount(0);
});
