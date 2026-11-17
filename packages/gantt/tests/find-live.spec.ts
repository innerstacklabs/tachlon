import { expect, test } from "./support/gantt";

// The Find count is a status region (gs#703): a change in it is read aloud, so a screen-reader user hears how many names match.

test("the Find count is a status region", async ({ g }) => {
  await g.open({ n: 2000 });
  const name = await g.page.evaluate(() => {
    const app = window.gantt!.app;
    const row = JSON.parse(app.table_rows(0, 1)) as number[][];
    return app.task_name(row[0][0]);
  });
  await g.page.locator("#find-input").fill(name);
  await expect(g.page.getByRole("status").filter({ hasText: /^1 of \d+$/ })).toHaveCount(1);
});
