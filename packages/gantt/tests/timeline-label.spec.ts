import { closeFilters, expect, openFilters, test } from "./support/gantt";

// gs#703: the timeline canvas has a name ("Timeline") and the row count line as its description, so a screen reader reads
// "Timeline, image" and then "38 rows" or "12 of 38 rows". The full detail is in the task list.
test("the_timeline_has_a_name_and_the_row_count_as_its_description", async ({ g }) => {
  await g.open({ n: 200 });
  const timeline = g.page.getByRole("img", { name: "Timeline" });
  await expect(timeline).toHaveCount(1);
  const count = g.page.locator("#row-count");
  await expect(count).not.toHaveText("");
  const before = (await count.textContent()) ?? "";
  await expect(timeline).toHaveAccessibleDescription(before);

  await openFilters(g.page);
  await g.page.locator("#critical-toggle").click();
  await closeFilters(g.page);
  await g.nextFrame(3);
  await expect(count).toContainText("Critical only");
  const after = (await count.textContent()) ?? "";
  expect(after).not.toBe(before);
  await expect(timeline).toHaveAccessibleDescription(after);
});
