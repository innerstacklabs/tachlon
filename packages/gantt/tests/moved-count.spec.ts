import { expect, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The row count names the Moved filter (gs#797): with Show moved on, the toolbar's count reads `Moved · N of M rows`,
// like the other filters, so a filtered table does not look like the whole table. The ledger and the edit are the ones
// of the moved note's specs (`support/moved-note.ts`).

const number = new Intl.NumberFormat("en-US");

test("with Show moved on, the row count reads Moved · N of M rows and the toolbar still fits", async ({ g }) => {
  await g.page.setViewportSize({ width: 1280, height: 800 });
  await openLedger(g);
  const all = (await g.status()).rowCount;
  await expect(g.page.locator("#row-count"), "no filter, no prefix").toHaveText(`${number.format(all)} rows`);

  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  await g.page.locator("#moved-show").click();
  await g.nextFrame(3);

  const s = await g.status();
  expect(s.filter).toBe("moved");
  expect(s.rowCount, "the filter hides rows").toBeLessThan(all);
  await expect(g.page.locator("#row-count")).toHaveText(`Moved · ${number.format(s.rowCount)} of ${number.format(s.n)} rows`);
  expect(s.n, "M is every row").toBe(all);

  const fit = await g.page.locator("#toolbar").evaluate((el) => ({ scroll: el.scrollWidth, client: el.clientWidth }));
  expect(fit.scroll, "the toolbar does not overflow at 1280px with Moved on").toBeLessThanOrEqual(fit.client);
});
