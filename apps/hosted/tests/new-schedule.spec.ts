import { readFileSync } from "node:fs";
import { expect, test, type Page, type TestInfo } from "@playwright/test";
import { PRODUCT_NAME } from "../../../packages/gantt/src/product";
import { waitReady, waitSampleSettled } from "./support/page";

// The file menu's "New schedule" (gs#703) is its first item: an empty schedule that starts on this week's Monday and is named `untitled`.
// With unsaved changes it asks first, as Open does. Screenshots of the open menu in light and dark go to test-results/.

test.use({ viewport: { width: 1280, height: 800 } });

const LINKS = readFileSync(new URL("../../../packages/gantt/tests/fixtures/links.xml", import.meta.url));

async function start(page: Page): Promise<void> {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
}

async function shootBothSchemes(page: Page, info: TestInfo, name: string): Promise<void> {
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    // The chart takes the new colours on the next frame and draws them on the one after: a shot taken at once shows the old scheme's timeline.
    await page.waitForFunction((dark) => {
      const bg = window.gantt!.theme().bg!;
      const luma = (parseInt(bg.slice(1, 3), 16) * 0.299 + parseInt(bg.slice(3, 5), 16) * 0.587 + parseInt(bg.slice(5, 7), 16) * 0.114) / 255;
      return dark ? luma < 0.3 : luma > 0.7;
    }, scheme === "dark");
    const seen = await page.evaluate(() => window.gantt!.status().frames);
    await page.waitForFunction((from) => window.gantt!.status().frames >= from + 2, seen);
    await page.screenshot({ path: info.outputPath(`${name}-${scheme}-1280.png`) });
  }
}

test("choosing New schedule gives 0 rows named untitled, and Insert gives one row", { tag: "@smoke" }, async ({ page }, info) => {
  const asked: string[] = [];
  page.on("dialog", (dialog) => void (asked.push(dialog.message()), dialog.dismiss()));
  await start(page);

  await page.locator("#open-button").click();
  const item = page.locator("#sample-list").getByRole("menuitem").first();
  await expect(item, "the first item of the menu").toContainText("New schedule");
  await expect(item, "a button, so the arrow keys and Enter reach it").toHaveJSProperty("tagName", "BUTTON");
  for (const scheme of ["light", "dark"] as const) {
    await page.emulateMedia({ colorScheme: scheme });
    await page.screenshot({ path: info.outputPath(`file-menu-new-schedule-${scheme}-1280.png`), clip: { x: 0, y: 0, width: 1280, height: 420 } });
  }
  await item.click();
  await expect(page.locator("#sample-list"), "choosing the item closes the menu").toBeHidden();
  await expect(page.locator("#row-count"), "no tasks").toHaveText("0 rows");
  expect(asked, "a schedule with no edits asks nothing").toEqual([]);

  await page.locator("#open-button").click();
  const downloading = page.waitForEvent("download");
  await page.locator("#sample-list").getByRole("menuitem", { name: /^Save(?! to cloud| image| layout)/ }).click();
  expect((await downloading).suggestedFilename(), "named untitled").toBe("untitled.gantt");

  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#row-count"), "Insert adds the first row").toHaveText("1 rows");
  expect(await page.evaluate(() => window.gantt!.status().rowCount)).toBe(1);
});

test("a schedule with no tasks says to press Insert; the line goes with the first task and returns on undo", async ({ page }, info) => {
  page.on("dialog", (dialog) => void dialog.dismiss());
  await start(page);
  const note = page.locator("#table-empty");
  await expect(note, "a schedule with tasks has no line").toBeHidden();

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count")).toHaveText("0 rows");
  await expect(note, "the blank schedule says what to do").toBeVisible();
  await expect(note).toContainText("Insert");
  await shootBothSchemes(page, info, "blank-schedule");

  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#row-count")).toHaveText("1 rows");
  await expect(note, "the first task takes the line away").toBeHidden();
  await page.keyboard.press("Control+Z");
  await expect(page.locator("#row-count")).toHaveText("0 rows");
  await expect(note, "undoing it brings the line back").toBeVisible();
});

test("a filter that keeps no row says so and where Clear filters is; clearing it brings the row and takes the line away", async ({ page }, info) => {
  page.on("dialog", (dialog) => void dialog.dismiss());
  await start(page);
  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#row-count")).toHaveText("1 rows");
  const note = page.locator("#table-empty");
  await expect(note, "a row shows, so no line").toBeHidden();

  await page.locator("#filters-button").click();
  await page.locator("#status-filter").selectOption("complete");
  await expect(page.locator("#row-count"), "the one new task is not complete").toHaveText("Status = Complete · 0 of 1 rows");
  await page.keyboard.press("Escape");
  await expect(note, "a filter that keeps nothing says why").toBeVisible();
  await expect(note, "in the row's words").toHaveText("No rows match. A filter or the Lookahead window is on; press Clear filters.");
  await shootBothSchemes(page, info, "filtered-out");

  await page.locator("#filters-button").click();
  await page.locator("#filters-clear").click();
  await expect(page.locator("#row-count")).toHaveText("1 rows");
  await expect(note, "the row is back, so the line goes").toBeHidden();
});

test("an unsaved edit asks before New schedule, and Cancel keeps the schedule", async ({ page }) => {
  const asked: string[] = [];
  let accept = false;
  page.on("dialog", (dialog) => void (asked.push(dialog.message()), accept ? dialog.accept() : dialog.dismiss()));
  await start(page);

  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await expect(page.locator("#unsaved-mark"), "an edit shows the mark").toBeVisible();
  const rows = await page.evaluate(() => window.gantt!.status().rowCount);

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect.poll(() => asked.length, "the unsaved edit asks first").toBe(1);
  expect(asked[0]).toContain("Start a new schedule?");
  await expect(page.locator("#unsaved-mark"), "Cancel keeps the edit").toBeVisible();
  expect(await page.evaluate(() => window.gantt!.status().rowCount), "and the rows").toBe(rows);

  accept = true;
  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count"), "OK replaces the schedule").toHaveText("0 rows");
  await expect(page.locator("#unsaved-mark"), "the new schedule has no unsaved changes").toBeHidden();
  expect(asked, "one question for each try").toHaveLength(2);
});

test("New schedule over an edited file puts untitled in the tab, with no bullet", async ({ page }) => {
  page.on("dialog", (dialog) => void dialog.accept());
  await start(page);
  await page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: LINKS });
  await expect(page, "an open file names the tab").toHaveTitle(`links.xml - ${PRODUCT_NAME}`);
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Renamed");
  await page.keyboard.press("Enter");
  await expect(page, "an edit puts the bullet in front").toHaveTitle(`• links.xml - ${PRODUCT_NAME}`);

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count")).toHaveText("0 rows");
  await expect(page, "the tab says untitled and the bullet is gone").toHaveTitle(`untitled - ${PRODUCT_NAME}`);
});

test("an edited untitled schedule puts the bullet in front of untitled in the tab", async ({ page }) => {
  await start(page);
  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page, "a new schedule reads untitled").toHaveTitle(`untitled - ${PRODUCT_NAME}`);
  await expect(page.locator("#open-file"), "and so does the file line").toHaveText("untitled");
  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#unsaved-mark"), "the edit shows the mark").toBeVisible();
  await expect(page, "and the bullet goes in front of untitled").toHaveTitle(`• untitled - ${PRODUCT_NAME}`);
});

test("after New schedule the file line reads untitled, next to the unsaved mark once it is edited", async ({ page }, info) => {
  page.on("dialog", (dialog) => void dialog.accept());
  await start(page);
  await page.setInputFiles("#open-input", { name: "links.xml", mimeType: "application/xml", buffer: LINKS });
  await expect(page.locator("#open-file"), "an open file shows its name").toHaveText("links.xml");

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count")).toHaveText("0 rows");
  await expect(page.locator("#open-file"), "a new schedule reads untitled, not the last file's name").toHaveText("untitled");
  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#unsaved-mark"), "the edit shows the mark").toBeVisible();
  await expect(page.locator("#open-file"), "the name stays beside the mark").toHaveText("untitled");
  await shootBothSchemes(page, info, "untitled-file-line");
});
