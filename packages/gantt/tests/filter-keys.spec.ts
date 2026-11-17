import { expect, openFilters, test } from "./support/gantt";
import { editDuration, openLedger } from "./support/moved-note";

// The filter keys (gs#703): with the chart focused, Shift+S turns Slipped on or off while a baseline is set, and
// Shift+M turns Show moved on or off while the last edit moved something, as Shift+C and Shift+L do for their filters.
// With the cell cursor in the list the keys type into the cell until Escape.
test("shift_s_and_shift_m_toggle_slipped_and_show_moved", async ({ page, g }) => {
  test.setTimeout(120_000);
  await openLedger(g);
  const filter = async (): Promise<string> => (await g.status()).filter;
  // An edit leaves the cell cursor in the grid, which takes the plain keys; a press on the timeline puts it away.
  const focusChart = async (): Promise<void> => {
    await page.locator("#timeline").click({ position: { x: 20, y: 400 } });
    await g.nextFrame(2);
  };

  await editDuration(page, "10d");
  await g.nextFrame(3);
  await focusChart();
  await page.keyboard.press("Shift+S");
  await g.nextFrame(2);
  expect(await filter(), "no baseline, Shift+S changes nothing").toBe("off");

  // A baseline, then an edit that slips tasks behind it.
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await editDuration(page, "20d");
  await g.nextFrame(3);
  expect((await g.status()).baseline!.slipped, "the edit slipped some tasks").toBeGreaterThan(0);

  // With the cell cursor in the list the key types into the cell, and the filter stays off.
  await page.keyboard.press("Shift+S");
  await g.nextFrame(2);
  expect(await filter(), "Shift+S in the cell cursor is typing").toBe("off");
  await page.keyboard.press("Escape");

  await focusChart();
  await page.keyboard.press("Shift+S");
  await page.waitForFunction(() => window.gantt!.status().filter === "slipped");
  await page.keyboard.press("Shift+S");
  await page.waitForFunction(() => window.gantt!.status().filter === "off");

  await page.keyboard.press("Shift+M");
  await page.waitForFunction(() => window.gantt!.status().filter === "moved");
  await page.keyboard.press("Shift+M");
  await page.waitForFunction(() => window.gantt!.status().filter === "off");
});

test("the Slipped button names its key", async ({ g }) => {
  await g.open({ n: 2000 });
  await expect(g.page.locator("#slipped-toggle")).toHaveAttribute("title", /\(Shift\+S\)$/);
});

// The key does nothing while Show moved is disabled, so the disabled button says why and the enabled one names the key.
test("the disabled Show moved button says why and the enabled one names its key", async ({ g }) => {
  test.setTimeout(120_000);
  await g.open({ n: 2000 });
  await openFilters(g.page);
  const moved = g.page.locator("#moved-filter");
  await expect(moved).toBeDisabled();
  await expect(moved).toHaveAttribute("title", "Nothing has moved yet: drag a bar or change a date, then this shows what moved.");

  await openLedger(g);
  await editDuration(g.page, "10d");
  await g.nextFrame(3);
  await openFilters(g.page);
  await expect(moved).toBeEnabled();
  await expect(moved).toHaveAttribute("title", /\(Shift\+M\)$/);
});
