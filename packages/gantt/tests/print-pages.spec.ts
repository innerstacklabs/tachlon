import { expect, test } from "./support/gantt";
import type { PrintPages } from "../src/api";

// App.print_pages (gs#703): the printed pages as SVG, from the print plan. The page count and the SVG itself are checked in
// crates/app/tests/print_pages.rs.

const SIZE = 2000;

test("the live scroll and zoom are unchanged by the call", async ({ g }) => {
  await g.open({ n: SIZE });
  await g.page.evaluate(() => window.gantt!.app.wheel(100, 200, 0, 300, false));
  await g.nextFrame(2);
  const before = await g.status();
  await g.page.evaluate(() => window.gantt!.app.print_pages("Title", ""));
  await g.nextFrame(2);
  const after = await g.status();
  expect(before.scrollY).toBeGreaterThan(0);
  expect(after.scrollY).toBe(before.scrollY);
  expect(after.pxPerMinute).toBe(before.pxPerMinute);
  expect(after.rows).toEqual(before.rows);
  expect(after.t).toEqual(before.t);
});

test("beforeprint puts one svg per page in .gantt-print-pages, afterprint removes them", { tag: "@smoke" }, async ({ g }) => {
  await g.open({ n: SIZE });
  const fire = (name: "beforeprint" | "afterprint") => g.page.evaluate((type) => window.dispatchEvent(new Event(type)), name);
  const pages = g.page.locator(".gantt-print-pages");
  await expect(pages.locator("svg")).toHaveCount(0);

  await fire("beforeprint");
  const want = await g.page.evaluate(() => (window.gantt!.app.print_pages(document.title, "") as PrintPages).length);
  expect(want).toBeGreaterThan(0);
  await expect(pages.locator(":scope > svg")).toHaveCount(want);

  // Each page says the local day it was printed (sv-SE writes a date as YYYY-MM-DD).
  const today = await g.page.evaluate(() => new Date().toLocaleDateString("sv-SE"));
  expect(today).toMatch(/^\d{4}-\d{2}-\d{2}$/);
  const printed = await pages.locator(":scope > svg").evaluateAll((svgs, day) => svgs.map((svg) => [...svg.querySelectorAll("text")].some((t) => t.textContent === `Printed ${day}`)), today);
  expect(printed).toEqual(Array(want).fill(true));

  // In print the pages show and the text sheet does not.
  await g.page.emulateMedia({ media: "print" });
  await expect(pages).toBeVisible();
  await expect(g.page.locator("#print-sheet")).toBeHidden();
  await g.page.emulateMedia({ media: "screen" });

  await fire("afterprint");
  await expect(pages.locator("svg")).toHaveCount(0);
});

test("a column the Columns menu hid is not on the printed page", async ({ g }) => {
  await g.open({ n: 300 });
  const heads = () =>
    g.page.evaluate(() => {
      window.dispatchEvent(new Event("beforeprint"));
      const first = document.querySelector(".gantt-print-pages > svg");
      return [...first!.querySelectorAll("text")].map((t) => t.textContent);
    });
  expect(await heads(), "the page lists Start while the table does").toContain("Start");

  await g.page.locator("#columns-button").click();
  await g.page.locator("#columns-start").click();
  await g.page.keyboard.press("Escape");
  await g.nextFrame(3);
  const after = await heads();
  expect(after, "Start is hidden on screen, so it is not printed").not.toContain("Start");
  expect(after).toEqual(expect.arrayContaining(["Task", "Dur", "Finish", "Float"]));
});

test("the axis month row is dark ink on the white page in a dark browser (gantt#1748)", async ({ g }) => {
  await g.open({ n: 60 });
  await g.page.emulateMedia({ media: "print", colorScheme: "dark" });
  await g.page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  // The top axis row ("Jan 2026") is the one label with `fill="currentColor"`, so it takes the page's text colour; the print ink is black.
  const month = g.page.locator('.gantt-print-pages > svg text[fill="currentColor"]').first();
  await expect(month).toHaveText(/^[A-Z][a-z]{2} \d{4}$/);
  expect(await month.evaluate((el) => getComputedStyle(el).fill)).toBe("rgb(0, 0, 0)");
});
