import { expect, test } from "./support/gantt";

// "Row N of M" (gs#703): the list says where you are for one second after a scroll, and is hidden at rest.
test.use({ webgl: true });

test("the list scrolls and 'Row N of M' shows, then hides", async ({ g }) => {
  await g.open({ n: 2000 });
  const label = g.page.locator("[data-part=row-position]");
  await expect(label).toBeHidden();
  const { rowHeight, rowCount } = await g.status();
  await g.page.evaluate((top) => {
    document.querySelector<HTMLElement>("[data-part=table-body]")!.scrollTop = top;
  }, 500 * rowHeight);
  await expect(label).toBeVisible();
  await expect(label).toHaveText(`Row 501 of ${rowCount.toLocaleString("en-US")}`);
  await expect(label).toBeHidden({ timeout: 5_000 });
});
