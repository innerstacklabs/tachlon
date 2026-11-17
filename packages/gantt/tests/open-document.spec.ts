// Open reads a file by its content, never its name (gs#703). A saved document uploaded as `renamed.xer` still opens as
// a document, and a gzip file that cannot be read is named in the error line while the schedule stays as it was.
// The fixture is tests/fixtures/one-project.xer; the document is saved as save-open.spec.ts saves it (Ctrl+S).
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Page } from "@playwright/test";
import { expect, test, type Gantt } from "./support/gantt";

const FIXTURE = { name: "one-project.xer", buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)) };
const folder = mkdtempSync(join(tmpdir(), "open-document-"));

/** Open the fixture (epoch 1) and save it with Ctrl+S; returns the saved bytes. */
async function savedDocument(g: Gantt): Promise<Buffer> {
  const { page } = g;
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", { name: FIXTURE.name, mimeType: "application/octet-stream", buffer: FIXTURE.buffer });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);
  await page.locator("#app").focus();
  const download = page.waitForEvent("download");
  await page.keyboard.press("ControlOrMeta+S");
  const path = join(folder, "saved.gantt");
  await (await download).saveAs(path);
  return readFileSync(path);
}

const rowTexts = (page: Page): Promise<(string | null)[]> =>
  page.evaluate(() => [...document.querySelectorAll("#table-rows .row")].map((row) => row.textContent));

test("open_reads_a_document_by_content_not_by_name", async ({ g }) => {
  const { page } = g;
  const saved = await savedDocument(g);
  // That the document reads back as the schedule it saved is checked in crates/app/tests/document_round_trip.rs.

  await page.setInputFiles("#open-input", { name: "renamed.xer", mimeType: "application/octet-stream", buffer: saved });
  // Stop at the epoch or at an error line, so a refused open fails on its message and not on a timeout.
  await page.waitForFunction(
    () => window.gantt!.status().epoch === 2 || (document.getElementById("open-error-text")?.textContent ?? "") !== "",
    undefined,
    { timeout: 60_000 },
  );
  await expect(page.locator("#open-error-text")).toHaveText("");
  expect((await g.status()).epoch, "the epoch moved").toBe(2);
  expect((await g.status()).n, "the document's tasks").toBe(14);
});

test("open_names_a_gzip_file_it_cannot_read", async ({ g }) => {
  const { page } = g;
  await savedDocument(g);
  const before = await g.status();
  const rowsBefore = await rowTexts(page);

  const garbage = Buffer.concat([Buffer.from([0x1f, 0x8b, 0x08]), Buffer.from("this is not a deflate stream, just garbage bytes")]);
  await page.setInputFiles("#open-input", { name: "corrupt.gantt", mimeType: "application/octet-stream", buffer: garbage });
  await expect(page.locator("#open-error-text")).toContainText("could not read corrupt.gantt");
  const after = await g.status();
  expect(after.epoch, "the epoch did not move").toBe(before.epoch);
  expect(after.rowCount, "the row count did not change").toBe(before.rowCount);
  expect(await rowTexts(page), "the rows did not change").toEqual(rowsBefore);
});
