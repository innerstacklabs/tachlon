// Export… writes the open schedule as MS Project XML or as a P6 XER file (gs#703): edit a row, export, and the downloaded file carries
// the edit. That the MS Project file reopens with the edit is checked in crates/app/tests/export_mspdi.rs.
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { expect, test, type Gantt } from "./support/gantt";

const FIXTURE = { name: "one-project.xer", buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)) };
const folder = mkdtempSync(join(tmpdir(), "export-mspdi-"));
const RENAMED = "Exported and renamed";

/** Open the XER fixture and rename its first leaf row, so a downloaded file can be checked for the edit. */
async function openAndRename(g: Gantt): Promise<void> {
  const { page } = g;
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", { name: FIXTURE.name, mimeType: "application/octet-stream", buffer: FIXTURE.buffer });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.waitForFunction(() => document.querySelector("#table-rows .row") !== null);

  // Rename the first leaf row.
  await page.evaluate(() => {
    const w = window.gantt!;
    const rows = JSON.parse(w.app.table_rows(0, w.status().rowCount)) as [number, string, string, number | null, boolean, number][];
    const leaf = rows.find((r) => r[5] === 0)!;
    w.app.rename_task(leaf[0], "Exported and renamed");
  });
  await g.nextFrame(3);
}

test("an_edited_schedule_exports_to_ms_project_xml_with_the_edit", async ({ g }) => {
  const { page } = g;
  await openAndRename(g);

  await page.locator("#export-button").click();
  await expect(page.locator("#export-dialog")).toBeVisible();
  const download = page.waitForEvent("download");
  await page.locator("#export-mspdi").click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("one-project.xml");
  const path = join(folder, "exported.xml");
  await file.saveAs(path);
  expect(readFileSync(path, "utf8")).toContain(RENAMED);
  await expect(page.locator("#export-result")).toContainText("one-project.xml");
  await page.locator("#export-close").click();
});

test("an_edited_schedule_exports_to_p6_xer_with_the_edit_and_names_the_p6_version", async ({ g }) => {
  const { page } = g;
  await openAndRename(g);

  await page.locator("#export-button").click();
  const download = page.waitForEvent("download");
  await page.locator("#export-xer").click();
  const file = await download;
  expect(file.suggestedFilename()).toBe("one-project.xer");
  const path = join(folder, "exported.xer");
  await file.saveAs(path);
  const text = readFileSync(path, "utf8");
  expect(text.startsWith("ERMHDR")).toBe(true);
  expect(text).toContain(RENAMED);
  await expect(page.locator("#export-result")).toContainText("one-project.xer as P6 19.12");
});

test("export_names_the_download_as_save_does", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  // A document loaded with load({ document }) has no file name, and its own name is the one Save writes.
  await page.evaluate(async () => {
    const api = window.gantt!.api!;
    const document = { ...(api.toDocument() as object), name: "stored name" };
    await api.load({ document });
  });
  await g.nextFrame(3);
  await page.locator("#export-button").click();
  const download = page.waitForEvent("download");
  await page.locator("#export-mspdi").click();
  expect((await download).suggestedFilename()).toBe("stored name.xml");
  await expect(page.locator("#export-result")).toContainText("stored name.xml");
});
