import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Copy the list" (gs#703), under "Export the list as CSV", puts the visible rows on the clipboard as tab-separated text:
// the header line, then a line per visible row, as the CSV export writes them. A tablet has no Ctrl+A and Ctrl+C.

test.use({ viewport: { width: 1280, height: 800 }, permissions: ["clipboard-read", "clipboard-write"] });

/** Open the menu, choose the item and return the clipboard's lines. */
async function copyList(page: Page, how: "click" | "keyboard"): Promise<string[]> {
  await page.locator("#open-button").click();
  const item = page.getByRole("menuitem", { name: /Copy the list/ });
  await expect(item).toBeVisible();
  await expect(item, "live, not Coming soon").not.toHaveAttribute("aria-disabled", "true");
  if (how === "click") await item.click();
  else {
    await item.focus();
    await page.keyboard.press("Enter");
  }
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  await expect(page.locator("#save-note")).toContainText("Copied");
  return (await page.evaluate(() => navigator.clipboard.readText())).split("\n");
}

test("the_menu_item_copies_the_visible_list", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const lines = await copyList(page, "click");
  expect(lines[0], "tab-separated columns").toContain("Task\tDur\tStart\tFinish\tFloat");
  const rows = await page.evaluate(() => window.gantt!.status().rowCount);
  expect(lines.length, "a header line and a line per visible row").toBe(rows + 1);
  await expect(page.locator("#save-note")).toHaveText(`Copied ${rows} rows`);
});

test("the_item_works_from_the_keyboard_and_follows_a_filter", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.locator("#filters-button").click();
  await page.locator("#critical-toggle").click();
  await page.keyboard.press("Escape");
  const lines = await copyList(page, "keyboard");
  const { rowCount, n } = await page.evaluate(() => window.gantt!.status());
  expect(rowCount, "the filter hides rows").toBeLessThan(n);
  expect(lines.length, "only the visible rows are copied").toBe(rowCount + 1);
  await expect(page.locator("#save-note")).toHaveText(`Copied ${rowCount} of ${n} rows`);
});
