import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// `?theme=dark` or `?theme=light` sets the chart's `data-theme` and the page's own `color-scheme` (gs#703); anything
// else is ignored. Measured at 1280 x 1000 on a light system.

test.use({ viewport: { width: 1280, height: 1000 } });

const bodyLuma = (page: import("@playwright/test").Page): Promise<number> =>
  page.evaluate(() => {
    const canvas = document.createElement("canvas").getContext("2d")!;
    canvas.fillStyle = getComputedStyle(document.body).backgroundColor;
    canvas.fillRect(0, 0, 1, 1);
    const [r, g, b, a] = canvas.getImageData(0, 0, 1, 1).data;
    return a === 0 ? 1 : (r! * 0.299 + g! * 0.587 + b! * 0.114) / 255;
  });

test("theme_dark_makes_the_page_dark_on_a_light_system", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "light" });
  await page.goto("/?theme=dark");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  expect(await bodyLuma(page), "the page is dark").toBeLessThan(0.3);
  expect(await page.evaluate(() => document.getElementById("app")!.getAttribute("data-theme"))).toBe("dark");
  expect(await page.evaluate(() => window.gantt!.theme().bg), "the chart is dark").toBe("#0e1630");

  // An unknown value is ignored: the page stays light and the root gets no attribute.
  await page.goto("/?theme=nope");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  expect(await bodyLuma(page), "the page stays light").toBeGreaterThan(0.7);
  expect(await page.evaluate(() => document.getElementById("app")!.hasAttribute("data-theme"))).toBe(false);
});

test("theme_light_makes_the_page_light_on_a_dark_system", async ({ page }) => {
  await page.emulateMedia({ colorScheme: "dark" });
  await page.goto("/?theme=light");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  expect(await bodyLuma(page), "the page is light").toBeGreaterThan(0.7);
  expect(await page.evaluate(() => window.gantt!.theme().bg), "the chart is light").toBe("#f2f5f9");
});
