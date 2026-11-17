import { readFile } from "node:fs/promises";
import { expect, test } from "@playwright/test";
import { waitReady, waitSampleSettled } from "./support/page";

// The loop of a new schedule closes (gs#703, N5): choose New schedule, Insert a task, rename it, choose Save, then open the saved
// file with the Open input, and the task is there with its new name. New schedule and Save have their own specs (new-schedule,
// save-menu); this one checks that what Save writes for a new schedule is what Open reads back.

test.use({ viewport: { width: 1280, height: 800 } });

test("a new schedule with one renamed task, saved and opened again, keeps the task and its name", { tag: "@smoke" }, async ({ page }) => {
  await page.goto("/");
  await waitReady(page, { frames: 10, timeout: 180_000 });
  await waitSampleSettled(page, 120_000);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count"), "no tasks").toHaveText("0 rows");

  await page.locator("#app").focus();
  await page.keyboard.press("Insert");
  await expect(page.locator("#row-count"), "Insert adds the first row").toHaveText("1 rows");
  await page.keyboard.press("F2");
  await page.keyboard.press("Control+A");
  await page.keyboard.type("Pour the slab");
  await page.keyboard.press("Enter");
  await expect(page.locator("#table-rows .row").first(), "the row has its new name").toContainText("Pour the slab");
  await expect(page.locator("#unsaved-mark"), "the edits show the mark").toBeVisible();

  await page.locator("#open-button").click();
  const downloading = page.waitForEvent("download");
  await page.locator("#sample-list").getByRole("menuitem", { name: /^Save(?! to cloud| image| layout)/ }).click();
  const download = await downloading;
  expect(download.suggestedFilename(), "an untitled schedule saves as untitled.gantt").toBe("untitled.gantt");
  await expect(page.locator("#unsaved-mark"), "the save clears the mark").toBeHidden();
  const saved = await readFile(await download.path());
  expect(saved.length, "the file is not empty").toBeGreaterThan(0);

  // Open the file again: a different schedule first, so the check cannot pass on the chart that was never replaced.
  await page.locator("#open-button").click();
  await page.locator("#sample-list").getByRole("menuitem", { name: /^New schedule/ }).click();
  await expect(page.locator("#row-count"), "a fresh empty schedule").toHaveText("0 rows");
  await page.setInputFiles("#open-input", { name: download.suggestedFilename(), mimeType: "application/octet-stream", buffer: saved });
  await expect(page.locator("#row-count"), "the saved file opens with its one task").toHaveText("1 rows");
  await expect(page.locator("#table-rows .row").first(), "named as renamed").toContainText("Pour the slab");
  expect(await page.evaluate(() => window.gantt!.app.task_label(0))).toBe("Pour the slab");
  await expect(page.locator("#unsaved-mark"), "a file just opened has no unsaved changes").toBeHidden();
});
