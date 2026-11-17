import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu has a live Save item (gs#703): choosing it makes the call Ctrl+S makes, so the page downloads <file>.gantt and the
// unsaved mark beside the file name clears. Screenshots of the open menu in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

test("choosing Save downloads <file>.gantt and hides #unsaved-mark", { tag: "@smoke" }, async ({ page }, info) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await expect(page.locator("#unsaved-mark"), "an edit shows the mark").toBeVisible();

  await page.locator("#open-button").click();
  const save = page.locator("#sample-list").getByRole("menuitem", { name: /^Save(?! to cloud| image| layout)/ });
  await expect(save).toHaveCount(1);
  await expect(save).toContainText("Downloads a .gantt file");
  await expect(save, "a live item, not a Coming soon one").not.toHaveAttribute("aria-disabled", "true");

  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.screenshot({ path: info.outputPath(`file-menu-save-${scheme}-1280.png`), clip: { x: 0, y: 0, width: 1280, height: 420 } });
  }

  const downloading = page.waitForEvent("download");
  await save.click();
  const download = await downloading;
  expect(download.suggestedFilename()).toBe("office-fitout.gantt");
  await expect(page.locator("#unsaved-mark"), "the save clears the mark").toBeHidden();
  await expect(page.locator("#sample-list"), "choosing Save closes the menu").toBeHidden();
});
