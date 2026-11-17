import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The first screen's line says "This is a sample schedule" (gantt#1280). After the file menu's "New schedule" the schedule on screen is the viewer's own, empty one, so the line is gone (gantt#1727).
// Shots of the blank first screen in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

test("the line shows on the sample; after New schedule it is gone", async ({ page }, info) => {
  page.on("dialog", (dialog) => void dialog.dismiss());
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const line = page.locator("#first-screen-line");
  await expect(line, "the sample's first screen has the line").toContainText("This is a sample schedule");

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count")).toHaveText("0 rows");
  await expect(line, "the new schedule is not a sample").toHaveCount(0);
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    // The chart takes the new colours on the next frame and draws them on the one after: a shot taken at once shows the old scheme's timeline.
    await page.waitForFunction((dark) => {
      const bg = window.gantt!.theme().bg!;
      const luma = (parseInt(bg.slice(1, 3), 16) * 0.299 + parseInt(bg.slice(3, 5), 16) * 0.587 + parseInt(bg.slice(5, 7), 16) * 0.114) / 255;
      return dark ? luma < 0.3 : luma > 0.7;
    }, scheme === "dark");
    const seen = await page.evaluate(() => window.gantt!.status().frames);
    await page.waitForFunction((from) => window.gantt!.status().frames >= from + 2, seen);
    await page.screenshot({ path: info.outputPath(`first-screen-new-schedule-${scheme}-1280.png`) });
  }
});
