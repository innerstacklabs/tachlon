import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "Open another schedule in a new tab" (gs#703) opens the hosted page in a second tab; the first tab keeps its schedule
// and its edits. The item sits in the menu, right after "Compare with a file...", and the toolbar gains nothing for it.

test.use({ viewport: { width: 1280, height: 800 } });

test("the_menu_item_opens_the_page_in_a_second_tab_and_this_tab_keeps_its_edit", async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await expect(page.locator("#toolbar a[target=_blank]:not(#sample-list *)"), "no toolbar link outside the menu").toHaveCount(0);

  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await expect(page.locator("#unsaved-mark"), "an edit shows the mark").toBeVisible();
  const name = await page.locator('#table-rows .row[data-row="0"] .name').innerText();
  const rows = await page.evaluate(() => window.gantt!.status().rowCount);

  await page.locator("#open-button").click();
  const items = page.locator("#sample-list").getByRole("menuitem");
  await expect(items.nth(3), "after New schedule, Open a file and Compare with a file").toContainText("Open another schedule in a new tab");
  await expect(items.nth(2)).toContainText("Compare with a file...");

  const [second] = await Promise.all([page.context().waitForEvent("page"), items.nth(3).click()]);
  await second.waitForLoadState("domcontentloaded");
  // This tab's address names the sample it opened; the link is to the page itself, so the second tab does not inherit that choice.
  expect(new URL(second.url()).pathname, "the hosted page").toBe(new URL(page.url()).pathname);
  await expect(second.locator("#open-button"), "the second tab is the hosted page").toBeVisible();
  await expect(second.locator("#unsaved-mark"), "the second tab has no edit").toBeHidden();
  await second.close();

  await expect(page.locator("#sample-list"), "the menu closes").toBeHidden();
  await expect(page.locator("#unsaved-mark"), "this tab keeps its mark").toBeVisible();
  expect(await page.locator('#table-rows .row[data-row="0"] .name').innerText(), "and the edit").toBe(name);
  expect(await page.evaluate(() => window.gantt!.status().rowCount), "and its rows").toBe(rows);
});
