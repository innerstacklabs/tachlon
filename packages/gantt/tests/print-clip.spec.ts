import { expect, test } from "./support/gantt";

// The developer page clips `html` and `body` inline (`height: 100%; overflow: hidden`); its print rule lifts that, so every filled sheet
// prints. The spec fills the sheets and prints to a PDF, and counts the pages.

test("a print of the developer page holds one PDF page for each sheet", async ({ g }) => {
  await g.open({ n: 120 });
  await g.page.evaluate(() => window.dispatchEvent(new Event("beforeprint")));
  const sheets = await g.page.locator(".gantt-print-pages > svg").count();
  expect(sheets).toBeGreaterThan(1);

  await g.page.emulateMedia({ media: "print" });
  const pdf = (await g.page.pdf()).toString("latin1");
  expect((pdf.match(/\/Type\s*\/Page(?![A-Za-z])/g) ?? []).length).toBe(sheets);
});
