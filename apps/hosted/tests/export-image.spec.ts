import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Save image (PNG)" (gs#703), under "Export the list as CSV": it downloads a picture of the first printed page as <name>.png.

test.use({ viewport: { width: 1280, height: 800 } });

test("Save image downloads office-fitout.png, a PNG of 2112 by 1632", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);

  await page.locator("#open-button").click();
  const item = page.getByRole("menuitem", { name: /Save image/ });
  await expect(item).toBeVisible();
  await expect(item, "live, not Coming soon").not.toHaveAttribute("aria-disabled", "true");
  const [download] = await Promise.all([page.waitForEvent("download"), item.click()]);
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();

  expect(download.suggestedFilename()).toBe("office-fitout.png");
  const png = readFileSync(await download.path());
  expect(png.subarray(0, 8).toString("hex"), "the PNG signature").toBe("89504e470d0a1a0a");
  expect(png.subarray(12, 16).toString("latin1"), "the first chunk is IHDR").toBe("IHDR");
  expect([png.readUInt32BE(16), png.readUInt32BE(20)], "width and height in pixels").toEqual([2112, 1632]);
});
