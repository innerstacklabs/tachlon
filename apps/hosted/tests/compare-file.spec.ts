import { readFileSync } from "node:fs";
import { expect, test } from "@playwright/test";
import type { Page } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// "Compare with a file..." in the File menu (gs#703, journey J3): choosing a second file, such as last month's, sets it as the
// baseline in one undo step; a file that is not a schedule is refused and the baseline stays. What `set_baseline_from` matches is checked on
// `Session` in `crates/app/tests/compare_file.rs`; this checks the item, the summary the reader sees and Ctrl+Z. Screenshots of the open menu
// in light and dark go to test-results/, and the menu must fit the 800 px page; so do the screenshots of the panel that lists what differs.

test.use({ viewport: { width: 1280, height: 800 } });

const FIXTURES = "../../../packages/gantt/tests/fixtures/";
const FITOUT = readFileSync(new URL(`${FIXTURES}office-fitout.xml`, import.meta.url));
const LAST_MONTH = readFileSync(new URL(`${FIXTURES}office-fitout-update.xml`, import.meta.url));

/** Open the fit-out, then lengthen its first leaf so that one task and the finish are later than last month's file has them. */
async function openFitoutLater(page: Page): Promise<void> {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  const epoch = await page.evaluate(() => window.gantt!.status().epoch);
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction((wanted) => window.gantt!.status().epoch === wanted, epoch + 1, { timeout: 60_000 });
  await page.evaluate(() => {
    const { app } = window.gantt!;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    app.set_duration(rows.find((r) => r[5] === 0)![0], "400d");
  });
}

/** Choose "Compare with a file..." in the open File menu and give the chooser `buffer`. */
async function compareWith(page: Page, name: string, buffer: Buffer): Promise<void> {
  await page.locator("#open-button").click();
  await page.getByRole("menuitem", { name: /^Compare with a file\.\.\./ }).click();
  await expect(page.locator("#sample-list"), "choosing the item closes the menu").toBeHidden();
  await page.setInputFiles("#compare-input", { name, mimeType: "application/octet-stream", buffer });
}

test("the File menu lists Compare with a file... next to Open, in two lines, and fits the page", async ({ page }, info) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.locator("#open-button").click();
  const list = page.locator("#sample-list");
  const items = list.getByRole("menuitem");
  await expect(items.nth(0)).toContainText("New schedule");
  await expect(items.nth(1)).toContainText("Open a file…");
  const compare = items.nth(2);
  await expect(compare).toContainText("Compare with a file...");
  await expect(compare).toContainText("Last month's P6 or MS Project file, as a baseline");
  await expect(compare.locator("span")).toHaveCount(2);
  const box = await list.boundingBox();
  expect(box!.y + box!.height, "the open menu ends above the page's bottom edge").toBeLessThanOrEqual(800);
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.screenshot({ path: info.outputPath(`file-menu-compare-${scheme}-1280.png`), clip: { x: 0, y: 0, width: 1280, height: 800 } });
  }
});

test("choosing a second file sets it as the baseline, says what slipped, and Ctrl+Z takes it back", async ({ page }) => {
  await openFitoutLater(page);
  const chip = page.locator("#baseline-summary");
  await page.locator("#baseline-set").click(); // the comparison so far: the schedule as it is
  await expect(chip).toHaveText("vs baseline: 0 slipped");

  await compareWith(page, "last-month.xml", LAST_MONTH);
  await expect(chip).toContainText(/^vs baseline: [1-9]\d* slipped · finish \+\d+ days?/);
  await expect(page.locator("#baseline-set")).toHaveAttribute("aria-pressed", "true");

  // The chart has focus again: Ctrl+Z puts the old comparison back, Ctrl+Shift+Z the file's.
  await page.keyboard.press("Control+z");
  await expect(chip).toHaveText("vs baseline: 0 slipped");
  await page.keyboard.press("Control+Shift+z");
  await expect(chip).toContainText(/^vs baseline: [1-9]\d* slipped/);
});

test("a file of another format is refused and the baseline is unchanged", async ({ page }) => {
  await openFitoutLater(page);
  const chip = page.locator("#baseline-summary");
  await page.locator("#baseline-set").click();
  await expect(chip).toHaveText("vs baseline: 0 slipped");

  await compareWith(page, "notes.txt", Buffer.from("last month's notes, not a schedule"));
  await expect(page.locator("#edit-error-text")).toContainText("could not compare with notes.txt");
  await expect(chip, "the baseline is as it was").toHaveText("vs baseline: 0 slipped");
  await page.keyboard.press("Control+z");
  await expect(chip, "one undo step takes the baseline set before back; the refused file made none").toBeHidden();
});

test("a closable panel lists the tasks and links only in last month's file and those only in this month's, and files with none show no panel", async ({ page }, info) => {
  await openFitoutLater(page);
  const panel = page.locator("#file-only");
  const gone = `<Task><UID>990</UID><ID>60</ID><Name>Pour slab</Name><OutlineLevel>2</OutlineLevel><Summary>0</Summary><Duration>PT8H0M0S</Duration></Task>`;
  const withGone = Buffer.from(LAST_MONTH.toString().replace("</Tasks>", `${gone}</Tasks>`));

  await compareWith(page, "last-month.xml", withGone);
  await expect(panel).toBeVisible();
  await expect(panel).toContainText("Only in last month's file");
  await expect(page.locator("#file-only-list")).toHaveText("990 Pour slab");
  await expect(page.locator("#here-only-list"), "every task here is in last month's file").toHaveText("None");
  await expect(page.locator("#file-only-links-list"), "the extra task has no link, so no link is only in last month's file").toHaveText("None");
  await expect(page.locator("#here-only-links-list")).toHaveText("None");
  await page.locator("#file-only-close").click();
  await expect(panel).toBeHidden();

  // Task 904 is 990 in last month's file: each file has one task the other lacks, and each is listed under its own heading.
  await compareWith(page, "last-month.xml", Buffer.from(LAST_MONTH.toString().replace("<UID>904</UID>", "<UID>990</UID>")));
  await expect(panel).toContainText("Only in this month's file");
  await expect(page.locator("#file-only-list")).toHaveText("990 Practical completion");
  await expect(page.locator("#here-only-list")).toHaveText("904 Practical completion");
  // The two links into that task, 902 and 903 to it, are the links that differ; the type and the other ends are the same.
  await expect(page.locator("#file-only-links-list")).toHaveText("902 → 990 (FS)\n903 → 990 (FS)");
  await expect(page.locator("#here-only-links-list")).toHaveText("902 → 904 (FS)\n903 → 904 (FS)");
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.screenshot({ path: info.outputPath(`compare-panel-${scheme}-1280.png`), clip: { x: 0, y: 0, width: 1280, height: 800 } });
  }
  // The panel starts below the Show moved button, and its four headings are heavier than the lists under them (gantt#1598).
  await expect(page.locator("#moved-show")).toBeVisible();
  const show = (await page.locator("#moved-show").boundingBox())!;
  expect((await panel.boundingBox())!.y, "the panel is clear of the Show moved button").toBeGreaterThanOrEqual(show.y + show.height);
  for (const id of ["file-only-title", "here-only-title", "file-only-links-title", "here-only-links-title"]) {
    await expect(page.locator(`#${id}`), `${id} is heavier than the list under it`).toHaveCSS("font-weight", "600");
  }

  await compareWith(page, "last-month.xml", LAST_MONTH);
  await expect(page.locator("#baseline-summary")).toContainText("vs baseline:");
  await expect(panel, "every task is in both files: nothing to list").toBeHidden();
});
