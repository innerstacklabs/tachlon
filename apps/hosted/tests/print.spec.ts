import { expect, test } from "@playwright/test";
import { waitReady } from "./support/page";

// Ctrl+P on the hosted demo (gs#703): `beforeprint` fills `.gantt-print-pages` with one svg per page, print media shows
// only those, and `afterprint` empties them. The spec fires the events itself, so no print dialog opens.

test.use({ viewport: { width: 1280, height: 1000 } });

test("print_shows_one_svg_per_page_and_nothing_else", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  await page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  const pages = page.locator(".gantt-print-pages > svg");
  const want = await page.evaluate(() => (window.gantt!.app.print_pages(document.title, "") as string[]).length);
  expect(want).toBeGreaterThan(0);
  await expect(pages).toHaveCount(want);

  await page.emulateMedia({ media: "print" });
  await expect(pages.first()).toBeVisible();
  await expect(page.locator("#toolbar")).toBeHidden();
  await expect(page.locator("#print-sheet")).toBeHidden();
  await page.emulateMedia({ media: "screen" });

  await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  await expect(pages).toHaveCount(0);
});

// The first-screen line is fixed to the window's bottom, so on paper it would cross page 1 (gantt#1604).
test("print_hides_the_first_screen_line", async ({ page }) => {
  await page.goto("/?sample=office-fitout");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  await expect(page.locator("#first-screen-line")).toBeVisible();
  await page.emulateMedia({ media: "print" });
  await expect(page.locator("#first-screen-line")).toBeHidden();
});

// The sheet is US Letter landscape (gs#703, Q5): the page's own `@page` rule sets it, and `preferCSSPageSize` makes the PDF obey the rule instead of the call's paper size.
test("print_saves_landscape_letter_pages", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  await page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  await expect(page.locator(".gantt-print-pages > svg").first()).toBeAttached();
  const pdf = (await page.pdf({ preferCSSPageSize: true })).toString("latin1");
  const box = /\/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)\s*\]/.exec(pdf);
  expect(box, "the first page has a MediaBox").not.toBeNull();
  expect([Math.round(Number(box![1])), Math.round(Number(box![2]))]).toEqual([792, 612]);
});

// "Print on A4..." (gantt#703): the menu item prints once on A4 landscape paper, and the next print is Letter again. The stub for `window.print` fires
// `beforeprint` as the browser does, and the spec fires `afterprint` itself.
test("print_on_a4_item_prints_a4_pages_once", async ({ page }) => {
  await page.addInitScript(() => {
    window.print = () => void window.dispatchEvent(new Event("beforeprint"));
  });
  await page.goto("/");
  await waitReady(page, { frames: 5, timeout: 180_000 });
  const paper = async () => {
    const pdf = (await page.pdf({ preferCSSPageSize: true })).toString("latin1");
    const box = /\/MediaBox\s*\[\s*0\s+0\s+([\d.]+)\s+([\d.]+)\s*\]/.exec(pdf);
    return box ? [Math.round(Number(box[1])), Math.round(Number(box[2]))] : null;
  };
  const viewBox = () => page.locator(".gantt-print-pages > svg").first().getAttribute("viewBox");

  await page.locator("#open-button").click();
  await page.getByRole("menuitem", { name: /Print on A4\.\.\./ }).click();
  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  await expect(page.locator(".gantt-print-pages > svg").first()).toBeAttached();
  expect(await viewBox(), "A4 pages").toBe("0 0 1123 794");
  expect(await paper(), "A4 landscape, in points").toEqual([842, 595]);

  await page.evaluate(() => window.dispatchEvent(new Event("afterprint")));
  await page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  expect(await viewBox(), "the next print is Letter").toBe("0 0 1056 816");
  expect(await paper(), "Letter landscape, in points").toEqual([792, 612]);
});
