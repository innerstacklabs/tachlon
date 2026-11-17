// The unsaved mark (gs#703): a dot beside the open file's name after any edit, cleared by a save or by opening another file.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const fixture = (name: string): { name: string; mimeType: string; buffer: Buffer } => ({
  name,
  mimeType: "application/octet-stream",
  buffer: readFileSync(new URL(`./fixtures/${name}`, import.meta.url)),
});

async function rename(page: Page, text: string): Promise<void> {
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type(text);
  await page.keyboard.press("Enter");
}

async function save(page: Page): Promise<void> {
  await page.locator("#app").focus();
  const download = page.waitForEvent("download");
  await page.keyboard.press("ControlOrMeta+S");
  await download;
}

test("an_edit_shows_the_mark_and_saving_hides_it", async ({ g }) => {
  const { page } = g;
  // The last open is over unsaved changes, so it asks (gs#703); the test answers yes.
  page.on("dialog", (dialog) => void dialog.accept());
  const mark = page.locator("#unsaved-mark");
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", fixture("one-project.xer"));
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);
  await expect(mark).toBeHidden();

  await rename(page, "First edit");
  await g.nextFrame(2);
  await expect(mark).toBeVisible();
  await expect(mark).toHaveAttribute("title", "Unsaved changes");

  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  // A baseline set, with no other edit, is a change.
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await expect(mark).toBeVisible();
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  // Undo is a change.
  await page.locator("#app").focus();
  await page.keyboard.press("ControlOrMeta+Z");
  await g.nextFrame(2);
  await expect(mark).toBeVisible();

  await rename(page, "Second edit");
  await g.nextFrame(2);
  await expect(mark).toBeVisible();

  await page.setInputFiles("#open-input", fixture("two-projects.xer"));
  await page.locator('#picker button[data-project="all"]').click();
  await page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await expect(mark).toBeHidden();
});

test("the_mark_follows_a_baseline_reset_and_add_new", async ({ g }) => {
  const { page } = g;
  const mark = page.locator("#unsaved-mark");
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", fixture("one-project.xer"));
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);

  await rename(page, "First edit");
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  await page.locator("#baseline-set").click();
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  await page.evaluate(() => {
    const app = window.gantt!.app;
    const rows = JSON.parse(app.table_rows(0, window.gantt!.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    app.set_duration(rows.find((r) => r[5] === 0 && r[4])![0], "300d");
  });
  await g.nextFrame(2);
  await expect(mark).toBeVisible();
  const slipped = (await g.status()).baseline!.slipped;
  expect(slipped, "tasks slipped").toBeGreaterThan(0);
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  // (a) Baseline again over the changed schedule: the summary changes, and the revision does not.
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  expect((await g.status()).baseline!.slipped, "the slips are re-set").not.toBe(slipped);
  await expect(mark).toBeVisible();
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();

  // (b) Add new after an inserted row: the added count goes from 1 to 0.
  await page.locator("#insert-row").click();
  await g.nextFrame(2);
  await expect(mark).toBeVisible();
  expect((await g.status()).baseline!.added).toBe(1);
  await save(page);
  await g.nextFrame(2);
  await expect(mark).toBeHidden();
  const revision = (await g.status()).revision;
  await page.locator("#baseline-add").click();
  await g.nextFrame(2);
  expect((await g.status()).baseline!.added).toBe(0);
  expect((await g.status()).revision, "Add new does not move the revision").toBe(revision);
  await expect(mark).toBeVisible();
});
