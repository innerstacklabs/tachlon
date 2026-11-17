import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// The hosted page follows the system's dark setting, as the chart under it does (gs#829). Measured at 1280 x 1000.

test.use({ viewport: { width: 1280, height: 1000 } });

for (const scheme of ["light", "dark"] as const) {
  test(`the page is ${scheme} when the system is ${scheme}`, async ({ page }) => {
    await page.emulateMedia({ colorScheme: scheme });
    await page.goto("/");
    await waitReady(page, { frames: 5, timeout: 180_000 });
    const luma = await page.evaluate(() => {
      const canvas = document.createElement("canvas").getContext("2d")!;
      canvas.fillStyle = getComputedStyle(document.body).backgroundColor;
      canvas.fillRect(0, 0, 1, 1);
      const [r, g, b, a] = canvas.getImageData(0, 0, 1, 1).data;
      // A transparent body shows the browser's canvas, which is light whatever the system says: that reads as 1.
      return a === 0 ? 1 : (r! * 0.299 + g! * 0.587 + b! * 0.114) / 255;
    });
    if (scheme === "dark") expect(luma, "the page is dark").toBeLessThan(0.3);
    else expect(luma, "the page is light").toBeGreaterThan(0.7);
  });
}
