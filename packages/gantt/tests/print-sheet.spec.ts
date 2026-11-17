import { expect, test } from "./support/gantt";

// Print (gs#703): Ctrl+P prints the visible rows as text in place of the toolbar, the canvas and the table. The spec fires
// `beforeprint` and `afterprint` itself and emulates the print media, so no print dialog opens. The sheet's text
// (`App.list_text`, under a filter too) is checked in crates/app/tests/range_text.rs, and what shows in print when the pages
// are filled in journey-j5.spec.ts.

const SIZE = 2000;

const fire = (page: import("@playwright/test").Page, name: "beforeprint" | "afterprint") =>
  page.evaluate((type) => window.dispatchEvent(new Event(type)), name);

test("before printing the sheet holds a header line and one line per visible row, and afterprint empties it", async ({ g }) => {
  await g.open({ n: SIZE });
  await fire(g.page, "beforeprint");
  const { rowCount } = await g.status();
  expect((await g.page.locator("#print-sheet").textContent())!.split("\n")).toHaveLength(rowCount + 1);

  await fire(g.page, "afterprint");
  expect(await g.page.locator("#print-sheet").textContent()).toBe("");
});

test("the sheet carries the page title as its title line, drawn in print and gone after", async ({ g }) => {
  await g.open({ n: SIZE });
  const title = await g.page.title();
  expect(title).not.toBe("");

  await fire(g.page, "beforeprint");
  await expect(g.page.locator("#print-sheet")).toHaveAttribute("data-title", title);
  await g.page.emulateMedia({ media: "print" });
  const before = () => g.page.evaluate(() => getComputedStyle(document.querySelector("#print-sheet")!, "::before").content);
  expect(await before()).toContain(title);

  await fire(g.page, "afterprint");
  await expect(g.page.locator("#print-sheet")).not.toHaveAttribute("data-title", /.*/);
});

test("with no pages filled the text sheet is what prints, and the toolbar and the timeline are not displayed", async ({ g }) => {
  await g.open({ n: SIZE });
  await fire(g.page, "beforeprint");
  // The state when no page was filled: the container is empty, the sheet still holds the rows.
  await g.page.evaluate(() => document.querySelector(".gantt-print-pages")!.replaceChildren());
  await g.page.emulateMedia({ media: "print" });
  await expect(g.page.locator("#toolbar")).toBeHidden();
  await expect(g.page.locator("#timeline")).toBeHidden();
  await expect(g.page.locator(".gantt-print-pages")).toBeHidden();
  await expect(g.page.locator("#print-sheet")).toBeVisible();
});
