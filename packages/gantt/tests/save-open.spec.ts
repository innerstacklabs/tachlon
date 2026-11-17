// Ctrl+S saves the open schedule as a .gantt file and Open reads it back (gs#740). The fixture is
// tests/fixtures/one-project.xer (14 tasks). Every save is a download: nothing is written over a chosen file.
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync, gzipSync } from "node:zlib";
import type { Download, Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const FIXTURE = { name: "one-project.xer", buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)) };
const RENAMED = "Renamed before the save";
const folder = mkdtempSync(join(tmpdir(), "save-open-"));

/** Open the fixture, rename the task on row 0 (a summary row, still visible when collapsed) through the cell editor, collapse to level 1 and set a baseline. */
async function editedFixture(g: Gantt): Promise<{ expanded: number }> {
  const { page } = g;
  // WebGL2 from the start: a page that starts on WebGPU can reload onto WebGL2 at any moment after its first frame (`fallBackToWebgl` in
  // src/boot.ts, and on a runner with no GPU it always does), and a file chosen or a save made during that reload is lost with the page,
  // which timed this spec out on a loaded runner (gs#1119). Saving and opening do not depend on the backend.
  await g.open({ n: 20, params: { backend: "webgl" } });
  await page.setInputFiles("#open-input", { name: FIXTURE.name, mimeType: "application/octet-stream", buffer: FIXTURE.buffer });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  const expanded = (await g.status()).rowCount;
  await page.dblclick('#table-rows .row[data-row="0"] .name');
  await page.keyboard.press("Control+A");
  await page.keyboard.type(RENAMED);
  await page.keyboard.press("Enter");
  await g.nextFrame(2);
  await page.locator("#baseline-set").click();
  await g.nextFrame(2);
  await page.evaluate(() => window.gantt!.app.collapse_to_level(1));
  await g.nextFrame(2);
  return { expanded };
}

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

test("ctrl_s_downloads_the_document", async ({ g }) => {
  const requests: string[] = [];
  g.page.on("request", (request) => requests.push(request.url()));
  await editedFixture(g);
  const before = requests.length;
  const download = await saveWithKey(g.page);
  expect(download.suggestedFilename()).toBe("one-project.gantt");
  const { text } = await keep(download, "ctrl-s.gantt");
  const doc = JSON.parse(text) as { format: string; version: number; tasks: { name: string }[]; baseline: unknown; source: { format: string } };
  expect(doc.format).toBe("gantt");
  expect(doc.version).toBe(1);
  expect(doc.tasks.map((task) => task.name).some((name) => name.includes(RENAMED))).toBe(true);
  expect(doc.baseline).not.toBeNull();
  expect(doc.source.format).toBe("xer");
  expect(requests.slice(before), "the save made no network request").toEqual([]);
});

test("open_brings_back_what_was_saved", { tag: "@smoke" }, async ({ g }) => {
  const { page } = g;
  const { expanded } = await editedFixture(g);
  const collapsedRows = (await g.status()).rowCount;
  expect(collapsedRows, "a summary is collapsed before the save").toBeLessThan(expanded);
  const rowsBefore = await page.evaluate(() => [...document.querySelectorAll("#table-rows .row")].map((row) => row.textContent));
  // A save is not seen by the Open command until the next frame clears the unsaved mark: until then Open asks "changes not saved"
  // and Playwright dismisses that ask unseen, so an Open sent at once is dropped and `epoch` never moves (gs#1119). This waits for the
  // mark to show (the edits are unsaved) and then to clear (the save was counted) before it opens the file again.
  const mark = page.locator("#unsaved-mark");
  await expect(mark).toBeVisible();
  const saved = await keep(await saveWithKey(page), "one-project.gantt");
  await expect(mark).toBeHidden();

  await page.setInputFiles("#open-input", saved.path);
  await page.waitForFunction(() => window.gantt!.status().epoch === 2, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  expect((await g.status()).rowCount, "the collapsed summary is collapsed after the open").toBe(collapsedRows);
  expect(await page.evaluate(() => window.gantt!.app.baseline_rows()), "the baseline is back").toBeGreaterThan(0);
  const rowsAfter = await page.evaluate(() => [...document.querySelectorAll("#table-rows .row")].map((row) => row.textContent));
  expect(rowsAfter, "names and dates as saved").toEqual(rowsBefore);
  expect(rowsAfter.some((row) => row?.includes(RENAMED))).toBe(true);

  const again = await keep(await saveWithKey(page), "again.gantt");
  expect(again.text, "saving the reopened document gives the same text").toBe(saved.text);

  // A document from a newer version of the app is refused with the reader's text, and the schedule stays as it was.
  const newer = join(folder, "newer.gantt");
  writeFileSync(newer, gzipSync(saved.text.replace('"version":1', '"version":2')));
  await page.setInputFiles("#open-input", newer);
  await expect(page.locator("#open-error-text")).toContainText("this document is version 2; this app reads version 1");
  expect((await g.status()).epoch).toBe(2);
  expect((await g.status()).rowCount).toBe(collapsedRows);
});

test("a file whose format value is the retired project name is refused, and the open schedule stays (gs#1120)", async ({ g }) => {
  const { page } = g;
  await editedFixture(g);
  const rowsBefore = await page.evaluate(() => [...document.querySelectorAll("#table-rows .row")].map((row) => row.textContent));
  const mark = page.locator("#unsaved-mark");
  await expect(mark).toBeVisible();
  const saved = await keep(await saveWithKey(page), "fresh.gantt");
  await expect(mark).toBeHidden();
  const retired = ["man", "tis"].join(""); // built at run time, so this file does not name it
  const old = join(folder, `refused.${retired}`);
  writeFileSync(old, gzipSync(saved.text.replace('"format":"gantt"', `"format":"${retired}"`)));
  await page.setInputFiles("#open-input", old);
  await expect(page.locator("#open-error-text")).toContainText(`could not open refused.${retired}`);
  expect((await g.status()).epoch, "the refused file changed nothing").toBe(1);
  const rowsAfter = await page.evaluate(() => [...document.querySelectorAll("#table-rows .row")].map((row) => row.textContent));
  expect(rowsAfter).toEqual(rowsBefore);
});
