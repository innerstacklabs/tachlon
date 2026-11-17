import { closeFilters, expect, openFilters, test, type Gantt } from "./support/gantt";
import { openLedger } from "./support/moved-note";

// The toolbar's two polish items (gs#829) on the ledger fixture at 1280 x 1000 with a baseline set: the row count reads from its
// first letter (it is cut with an ellipsis at its end, never at its front, and "59 rows" is not set right to left as "rows 59"),
// and the Clear button next to Baseline says what it clears.
test.use({ viewport: { width: 1280, height: 1000 } });

/** The ledger open, a baseline set and Critical only on. */
async function openBaselined(g: Gantt): Promise<void> {
  await openLedger(g);
  await g.page.locator("#baseline-set").click();
  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await closeFilters(g.page);
  await g.nextFrame(3);
}

test("the lone Clear next to Baseline says Clear baseline", async ({ g }) => {
  await openBaselined(g);
  const clear = g.page.locator("#baseline-clear");
  await expect(clear).toBeVisible();
  await expect(clear).toHaveText("Clear baseline");
  await expect(g.page.getByRole("button", { name: "Clear baseline", exact: true })).toBeVisible();
});

test("the row count is cut at its end, not its front", async ({ g }) => {
  await openBaselined(g);
  const count = g.page.locator("#row-count");
  await expect(count).toHaveText(/^Critical only · \d+ of \d+ activities$/);
  const seen = await count.evaluate((el) => {
    const bar = document.getElementById("toolbar")!.getBoundingClientRect();
    const box = el.getBoundingClientRect();
    const style = getComputedStyle(el);
    return {
      insideBar: box.left >= bar.left && box.right <= bar.right,
      whole: el.scrollWidth <= el.clientWidth,
      direction: style.direction,
      ellipsis: style.textOverflow,
    };
  });
  expect(seen.insideBar, "the row count lies inside the toolbar").toBe(true);
  expect(seen.direction, "the text starts at its left edge, so a cut falls at its end").toBe("ltr");
  expect(seen.whole || seen.ellipsis === "ellipsis", "it shows whole, or is shortened with an ellipsis").toBe(true);
  expect(await g.page.evaluate(() => document.getElementById("toolbar")!.scrollWidth <= document.getElementById("toolbar")!.clientWidth)).toBe(true);
});

test("with Slipped and Lookahead on at 1280 px the brand stays and the Lookahead row keeps its weeks label", async ({ g }) => {
  await openBaselined(g);
  await openFilters(g.page);
  await g.page.locator("#slipped-toggle").click();
  await g.page.locator("#lookahead-toggle").click();
  await expect(g.page.locator("#lookahead-window")).toBeVisible();
  await expect(g.page.locator("#toolbar .tb-brand")).toBeVisible();
  await expect(g.page.locator("#lookahead-window .tb-label")).toBeVisible();
});
