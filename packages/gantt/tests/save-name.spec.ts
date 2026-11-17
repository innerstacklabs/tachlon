// The file name a save is given, and the Ctrl+S guard during a cell edit (gs#703). Every save is a download named after the
// open document: `untitled.gantt` for the generated demo, the opened file's name without its last extension otherwise.
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";
import type { Download, Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const XER = readFileSync(new URL("./fixtures/one-project.xer", import.meta.url));
const MSPDI = readFileSync(new URL("../../../crates/import/tests/fixtures/summary.xml", import.meta.url));
const folder = mkdtempSync(join(tmpdir(), "save-name-"));

/** Press Ctrl+S (the chart has focus) and return the download. */
async function saveWithKey(page: Page): Promise<Download> {
  await page.locator("#app").focus();
  const download = page.waitForEvent("download");
  await page.keyboard.press("ControlOrMeta+S");
  return download;
}

/** The download's gunzipped text, and the file it was kept as (named `as`, so a later `setInputFiles` sends that name). */
async function keep(download: Download, as: string): Promise<{ path: string; text: string }> {
  const path = join(folder, as);
  await download.saveAs(path);
  return { path, text: gunzipSync(readFileSync(path)).toString("utf8") };
}

/** Upload `buffer` under `name` and wait until the open is the `epoch`th document. */
async function openAs(g: Gantt, name: string, buffer: Buffer, epoch: number): Promise<void> {
  await g.page.setInputFiles("#open-input", { name, mimeType: "application/octet-stream", buffer });
  await g.page.waitForFunction((e) => window.gantt!.status().epoch === e, epoch, { timeout: 60_000 });
  await g.nextFrame(3);
}

test("save_names_the_download_after_the_open_file", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  const demo = await saveWithKey(page);
  expect(demo.suggestedFilename(), "the generated demo has no file name").toBe("untitled.gantt");
  await keep(demo, "demo.gantt");

  await openAs(g, "plan v2.final.xer", XER, 1);
  const xer = await saveWithKey(page);
  expect(xer.suggestedFilename(), "only the last extension goes").toBe("plan v2.final.gantt");
  await keep(xer, "xer.gantt");

  await openAs(g, "second.xml", MSPDI, 2);
  const xml = await saveWithKey(page);
  expect(xml.suggestedFilename(), "a second open renames the document").toBe("second.gantt");
  await keep(xml, "xml.gantt");
});

test("ctrl_s_saves_nothing_during_a_cell_edit", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await expect(page.locator("input.cell-edit").first(), "the editor is open").toBeVisible();
  const during = page.waitForEvent("download", { timeout: 2000 });
  await page.keyboard.press("ControlOrMeta+S");
  // The editor's own input stops its keys at the input, so the chart's guard is reached by a key sent to the chart itself.
  await page.locator("#app").dispatchEvent("keydown", { key: "s", ctrlKey: true, metaKey: true, bubbles: true, cancelable: true });
  await expect(page.locator("input.cell-edit"), "the editor is still open").toHaveCount(1);
  await expect(during, "no download while a cell is being edited").rejects.toThrow(/timeout/i);
  await expect(page.locator("#edit-error"), "the alert line says why nothing was saved").toBeVisible();
  await expect(page.locator("#edit-error-text")).toHaveText("Nothing saved yet. Press Enter to finish the edit, then save.");

  await page.keyboard.press("Escape");
  await g.nextFrame(2);
  const after = await saveWithKey(page);
  expect(after.suggestedFilename()).toBe("untitled.gantt");
});
