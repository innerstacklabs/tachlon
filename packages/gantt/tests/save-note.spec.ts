// The save note (gs#703): after a save the page says `Saved <name>.gantt`, bottom right, until the next edit or the next open.
import { readFileSync } from "node:fs";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/one-project.xer", import.meta.url));

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

test("a_save_says_what_it_saved_and_the_next_edit_or_open_hides_it", async ({ g }) => {
  const { page } = g;
  const note = page.locator("#save-note");
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", { name: "one-project.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await g.nextFrame(3);
  await expect(note, "nothing has been saved yet").toBeHidden();

  await rename(page, "First edit");
  await g.nextFrame(2);
  await save(page);
  await g.nextFrame(3);
  await expect(note).toBeVisible();
  await expect(note).toHaveText("Saved one-project.gantt");
  await expect(page.locator("#toolbar #save-note"), "never in the toolbar").toHaveCount(0);

  await rename(page, "Second edit");
  await g.nextFrame(2);
  await expect(note, "the next edit hides it").toBeHidden();

  await save(page);
  await g.nextFrame(3);
  await expect(note).toBeVisible();
  await page.setInputFiles("#open-input", { name: "again.xer", mimeType: "application/octet-stream", buffer: XER });
  await page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await expect(note, "the next open hides it").toBeHidden();
});
