// Opening a file over unsaved changes asks first (gs#703): a refusal keeps the schedule and its edit, an accept opens the file.
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

/** Record every dialog, answer each with `accept`, and return the messages seen. */
function answerDialogs(page: Page, accept: boolean): string[] {
  const seen: string[] = [];
  page.on("dialog", (dialog) => {
    seen.push(`${dialog.type()}: ${dialog.message()}`);
    void (accept ? dialog.accept() : dialog.dismiss());
  });
  return seen;
}

test("a_refused_open_keeps_the_schedule_and_its_edit", async ({ g }) => {
  const { page } = g;
  const dialogs = answerDialogs(page, false);
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", fixture("one-project.xer"));
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);
  // Nothing is unsaved yet, so the first open asked nothing.
  expect(dialogs).toEqual([]);

  await rename(page, "Kept edit");
  await g.nextFrame(3);
  await expect(page.locator("#unsaved-mark")).toBeVisible();

  await page.setInputFiles("#open-input", fixture("two-projects.xer"));
  await g.nextFrame(5);
  expect(dialogs).toHaveLength(1);
  expect(dialogs[0]).toBe("confirm: Open two-projects.xer? The changes to the current schedule are not saved and will be lost.");
  expect(await page.evaluate(() => window.gantt!.status().epoch)).toBe(1);
  await expect(page.locator('#table-rows .row[data-row="0"] .name')).toContainText("Kept edit");
  await expect(page.locator("#unsaved-mark")).toBeVisible();

  // The edit's undo history is kept too: an undo still reverts the edit.
  await page.locator("#app").focus();
  await page.keyboard.press("ControlOrMeta+Z");
  await g.nextFrame(3);
  await expect(page.locator('#table-rows .row[data-row="0"] .name')).not.toContainText("Kept edit");
});

test("an_accepted_open_replaces_the_schedule", async ({ g }) => {
  const { page } = g;
  const dialogs = answerDialogs(page, true);
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", fixture("one-project.xer"));
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);

  await rename(page, "Lost edit");
  await g.nextFrame(3);
  await page.setInputFiles("#open-input", fixture("two-projects.xer"));
  // two-projects.xer holds two projects: the project list is the second step of the same open and asks nothing more.
  await page.locator('#picker-list button[data-project="all"]').click();
  await page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  expect(dialogs).toHaveLength(1);
  await g.nextFrame(3);
  await expect(page.locator("#unsaved-mark")).toBeHidden();
});
