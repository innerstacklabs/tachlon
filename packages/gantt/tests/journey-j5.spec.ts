import { closeFilters, expect, openFilters, test } from "./support/gantt";

// Journey J5 ("J5. Export a PDF of a controlled layout"), walked on one page, one step per journey step.
// It covers what is built today: the layout's filter (Longest path), then Ctrl+P's printed pages, on Letter and on A4. The spec fires
// `beforeprint` and `afterprint` itself and emulates the print media, so no print dialog opens. The page count, the page
// SVG and the print plan are checked in crates/app/tests/print_pages.rs; this checks what the reader sees.
// Not built yet, so not walked: a column choice that reaches the pages (they print the file's columns, whatever the
// Progress and Variance buttons show), grouping, a date range and timescale for the printed axis (the pages fit the rows'
// span), a layout name in the header, and saving the layout (step 3).

const SIZE = 2000;
/** The printed legend's entries, one swatch each (crates/app/src/print/legend.rs). */
const LEGEND = 11;

interface Printed {
  /** The page's own lines: title, sub line (data date, filter) and "Page i of n". */
  header: string[];
  /** The list cells, row by row in page order; the first is the column header. */
  cells: string[];
  swatches: number;
}

test("J5: the reader prints the longest path, and every row it shows prints in order under a header and a legend", async ({ g }) => {
  const { page } = g;
  const fire = (name: "beforeprint" | "afterprint") => page.evaluate((type) => window.dispatchEvent(new Event(type)), name);
  await g.open({ n: SIZE });
  const all = await g.status();

  await test.step("1. chooses a layout: the Longest path filter", async () => {
    await openFilters(page);
    await page.locator("#longest-toggle").click();
    await expect(page.locator("#longest-toggle")).toHaveAttribute("aria-pressed", "true");
    await closeFilters(page);
    await g.nextFrame(2);
    const s = await g.status();
    expect(s.filter).toBe("longest");
    expect(s.rowCount).toBeGreaterThan(0);
    expect(s.rowCount).toBeLessThan(all.rowCount);
  });

  await test.step("2. prints: each page has the header and legend, and the pages hold every shown row in order", async () => {
    const { rowCount } = await g.status();
    const title = await page.title();
    await fire("beforeprint");
    const pages = page.locator(".gantt-print-pages > svg");
    await expect(pages.first()).toBeAttached();
    const printed = await pages.evaluateAll((svgs) =>
      svgs.map((svg) => ({
        header: [...svg.querySelectorAll(":scope > text")].map((t) => t.textContent ?? ""),
        cells: [...svg.querySelectorAll("g[clip-path] > text")].map((t) => t.textContent ?? ""),
        swatches: svg.querySelectorAll("rect.legend-swatch").length,
      })),
    ) as Printed[];

    // The sheet holds the rows the layout shows, as text: a header line, then one line per row, in table order.
    const sheet = (await page.locator("#print-sheet").textContent())!.split("\n").map((line) => line.split("\t"));
    expect(sheet).toHaveLength(rowCount + 1);
    const head = sheet[0]!;
    expect(printed.length).toBe(Math.max(1, Math.ceil(rowCount / (printed[0]!.cells.length / head.length - 1))));

    const rows: string[][] = [];
    printed.forEach((p, i) => {
      expect(p.header[0], `page ${i + 1} title`).toBe(title);
      // The demo has no data date; a file with one prints "Data date: ..." ahead of the filter.
      expect(p.header[1], `page ${i + 1} header`).toBe("Filter: longest");
      expect(p.header).toContain(`Page ${i + 1} of ${printed.length}`);
      expect(p.swatches, `page ${i + 1} legend`).toBe(LEGEND);
      expect(p.cells.length % head.length, `page ${i + 1} cells fill whole rows`).toBe(0);
      expect(p.cells.slice(0, head.length), `page ${i + 1} column header`).toEqual(head);
      for (let at = head.length; at < p.cells.length; at += head.length) rows.push(p.cells.slice(at, at + head.length));
    });
    // A text cell (Task, WBS, ID) may print clipped: a shorter prefix of the sheet cell, at least one
    // character long, then "…". A value (Dur, Float) must print exactly, and a date prints as its day, without the time the sheet holds (gantt#1755).
    const clippable = new Set(["Task", "WBS", "ID"]);
    const dates = new Set(["Start", "Finish", "Actual start", "Actual finish"]);
    const fits = (shown: string, full: string, col: string) => {
      if (shown === full || (dates.has(col) && shown === full.split(" ")[0])) return true;
      const kept = shown.slice(0, -1);
      return clippable.has(col) && shown.endsWith("…") && kept.length > 0 && kept.length < full.length && full.startsWith(kept);
    };
    expect(rows.length, "every row the layout shows, and no other").toBe(sheet.length - 1);
    rows.forEach((row, r) =>
      row.forEach((shown, c) => expect(fits(shown, sheet[r + 1]![c]!, head[c]!), `row ${r + 1} ${head[c]}: "${shown}" vs "${sheet[r + 1]![c]}"`).toBe(true)),
    );

    // On paper only the pages show: not the toolbar, the timeline, the legend strip or the text sheet.
    await page.emulateMedia({ media: "print" });
    await expect(page.locator(".gantt-print-pages")).toBeVisible();
    for (const hidden of ["#toolbar", "#timeline", "ul#legend", "#print-sheet"]) await expect(page.locator(hidden), hidden).toBeHidden();

    await page.emulateMedia({ media: "screen" });
    await fire("afterprint");
    await expect(page.locator(".gantt-print-pages > svg")).toHaveCount(0);
    for (const shown of ["#toolbar", "#timeline", "ul#legend"]) await expect(page.locator(shown), shown).toBeVisible();
  });

  await test.step("2b. chooses A4 as the page size: its pages are A4, and the next print is Letter again", async () => {
    // `printOn` is what the hosted menu's "Print on A4..." calls; the stub stands in for the print dialog, so it fires `beforeprint` and the spec fires `afterprint`.
    await page.evaluate(() => {
      window.print = () => void window.dispatchEvent(new Event("beforeprint"));
      window.gantt!.api!.printOn("a4");
    });
    const first = page.locator(".gantt-print-pages > svg").first();
    await expect(first).toHaveAttribute("viewBox", "0 0 1123 794");
    await fire("afterprint");
    await fire("beforeprint");
    await expect(first).toHaveAttribute("viewBox", "0 0 1056 816");
    await fire("afterprint");
  });
});
