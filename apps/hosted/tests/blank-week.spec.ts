import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// A new, empty schedule opens on the week it starts in (gantt#1728): Monday 00:00 to Sunday 24:00 in view, not one hour.
// Shots of the blank schedule in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

test("after New schedule the timeline shows the start week, not one hour", async ({ page }, info) => {
  page.on("dialog", (dialog) => void dialog.dismiss());
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count")).toHaveText("0 rows");

  // The origin is the start week's Monday, so minute 0 is its Monday 00:00 and 7 * 1440 is the next Monday.
  const week = 7 * 1440;
  const [from, to] = await page.evaluate(() => window.gantt!.status().t);
  expect(from, "the view starts at or before the Monday").toBeLessThanOrEqual(0);
  expect(to, "the view reaches the end of Sunday").toBeGreaterThanOrEqual(week);
  expect(to - from, "the view is about one week wide").toBeLessThan(week * 1.2);

  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.waitForFunction((dark) => {
      const bg = window.gantt!.theme().bg!;
      const luma = (parseInt(bg.slice(1, 3), 16) * 0.299 + parseInt(bg.slice(3, 5), 16) * 0.587 + parseInt(bg.slice(5, 7), 16) * 0.114) / 255;
      return dark ? luma < 0.3 : luma > 0.7;
    }, scheme === "dark");
    const seen = await page.evaluate(() => window.gantt!.status().frames);
    await page.waitForFunction((since) => window.gantt!.status().frames >= since + 2, seen);
    // Saturday and Sunday are non-working on the Monday-to-Friday calendar, so the week holds a shaded column (gantt#1734).
    expect(await page.evaluate(() => window.gantt!.status().shaded), `${scheme}: the weekend is shaded`).toBeGreaterThanOrEqual(1);
    await page.screenshot({ path: info.outputPath(`blank-week-${scheme}-1280.png`) });
  }
});
