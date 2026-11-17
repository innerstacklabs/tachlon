import { readFileSync } from "node:fs";
import { expect, test, type Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Export the list as CSV" (gs#703), under Save, downloads the visible list as <name>.csv: a header line, then a line per
// visible row. The toolbar gains nothing for it, and the sample list's buttons stay the samples and Save.

test.use({ viewport: { width: 1280, height: 800 } });

/** Open the menu, choose the item and return the download's file name, bytes and lines. */
async function exportCsv(page: Page, how: "click" | "keyboard"): Promise<{ name: string; bytes: Buffer; lines: string[] }> {
  await page.locator("#open-button").click();
  const item = page.getByRole("menuitem", { name: /Export the list as CSV/ });
  await expect(item).toBeVisible();
  await expect(item, "live, not Coming soon").not.toHaveAttribute("aria-disabled", "true");
  const choose = how === "click" ? item.click() : item.focus().then(() => page.keyboard.press("Enter"));
  const [download] = await Promise.all([page.waitForEvent("download"), choose]);
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  const bytes = readFileSync(await download.path());
  return { name: download.suggestedFilename(), bytes, lines: bytes.toString("utf8").split("\n") };
}

test("the_menu_item_downloads_the_visible_list", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await expect(page.locator("#toolbar #export-csv-button"), "no toolbar button").toHaveCount(0);

  const { name, lines } = await exportCsv(page, "click");
  // The default page opens the office-fitout file sample, so the name is that file's, without its extension.
  expect(name).toBe("office-fitout.csv");
  expect(lines[0]).toContain("Task,Dur,Start,Finish,Float");
  const rows = await page.evaluate(() => window.gantt!.status().rowCount);
  expect(lines.length, "a header line and a line per visible row").toBe(rows + 1);

  await page.locator("#open-button").click();
  await expect(page.locator("#sample-list button").filter({ hasText: /Export the list as CSV/ }), "the item is not a button in the sample list").toHaveCount(0);
});

test("the_item_works_from_the_keyboard", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const { name, lines } = await exportCsv(page, "keyboard");
  expect(name).toBe("office-fitout.csv");
  expect(lines[0]).toContain("Task,Dur,Start,Finish,Float");
});

test("a_generated_sample_downloads_untitled_csv", async ({ page }) => {
  await page.goto("/?sample=demo-50k");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  const { name, lines } = await exportCsv(page, "click");
  expect(name).toBe("untitled.csv");
  expect(lines[0]).toContain("Task,Dur,Start,Finish,Float");
  expect(lines.length).toBeGreaterThan(1);
});

test("the_file_starts_with_a_byte_order_mark", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const { bytes } = await exportCsv(page, "click");
  expect([...bytes.subarray(0, 3)], "EF BB BF, so Excel reads UTF-8").toEqual([0xef, 0xbb, 0xbf]);
});

test("the_export_says_how_many_rows_it_wrote", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const note = page.locator("#save-note");
  await expect(note, "no note before an export").toBeHidden();
  await exportCsv(page, "click");
  const all = await page.evaluate(() => window.gantt!.status());
  await expect(note).toHaveText(`Exported ${all.rowCount} rows to office-fitout.csv`);

  await page.locator("#filters-button").click();
  await page.locator("#critical-toggle").click();
  await page.keyboard.press("Escape");
  await exportCsv(page, "click");
  const some = await page.evaluate(() => window.gantt!.status());
  expect(some.rowCount, "the filter hides rows").toBeLessThan(some.n);
  await expect(note).toHaveText(`Exported ${some.rowCount} of ${some.n} rows to office-fitout.csv`);
});
