import { expect, test } from "./support/gantt";

// The chart root (gs#703): a group named "Gantt chart" for a screen reader, and a focus ring that shows for keyboard
// focus only, so a mouse click on the chart shows none.

const outlineStyle = (page: import("@playwright/test").Page): Promise<string> =>
  page.evaluate(() => getComputedStyle(document.querySelector(".gantt")!).outlineStyle);

test("the chart root is a group named Gantt chart", async ({ g }) => {
  await g.open();
  await expect(g.page.getByRole("group", { name: "Gantt chart" })).toHaveCount(1);
});

test("the root shows a ring on keyboard focus and none after a mouse click", async ({ g }) => {
  await g.open();
  // The root holds focus at load, so Tab leaves it and Shift+Tab brings keyboard focus back to it.
  await g.page.keyboard.press("Tab");
  await g.page.keyboard.press("Shift+Tab");
  expect(await g.page.evaluate(() => document.activeElement?.classList.contains("gantt")), "Shift+Tab returns to the root").toBe(true);
  expect(await outlineStyle(g.page), "ring on keyboard focus").not.toBe("none");

  // Tab to the toolbar so the click, not the keyboard, is what brings focus back to the root.
  await g.page.keyboard.press("Tab");
  await g.page.mouse.click(...(await clickPoint(g)));
  expect(await g.page.evaluate(() => document.activeElement?.classList.contains("gantt")), "the click leaves focus on the root").toBe(true);
  expect(await outlineStyle(g.page), "no ring after a mouse click").toBe("none");
});

async function clickPoint(g: { box(): Promise<{ x: number; y: number; width: number; height: number }> }): Promise<[number, number]> {
  const b = await g.box();
  return [b.x + b.width / 2, b.y + b.height / 2];
}
