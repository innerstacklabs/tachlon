import { expect, test } from "./support/gantt";

// The Float column heading (gs#703): hovering it says what the number is, so a reader who knows P6 or MS Project need not guess whether
// it counts calendar days or working days, or whose calendar.

test("the_float_heading_says_what_it_counts", async ({ g }) => {
  await g.open({ n: 200 });
  const head = g.page.locator("[data-part=table-head] .cols > .float");
  await expect(head).toHaveText("Float");
  await expect(head).toHaveAttribute("title", "Total float, in working days of each task's own calendar");
});
