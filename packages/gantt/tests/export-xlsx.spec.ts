// `Gantt.exportXlsx` (gantt#1875): the visible list downloads as `<name>.xlsx`, a zip whose sheet holds the rows the note counts, with a styles
// part that gives the date cells their format, and the note at the bottom right says how many rows it wrote.
import { readFileSync } from "node:fs";
import { inflateRawSync } from "node:zlib";
import type { Page } from "@playwright/test";
import { expect, test } from "./support/gantt";

const FITOUT = readFileSync(new URL("./fixtures/office-fitout.xml", import.meta.url));

/** The parts of a zip as text, by name, read from its central directory. */
function unzip(zip: Buffer): Record<string, string> {
  const parts: Record<string, string> = {};
  const count = zip.readUInt16LE(zip.length - 22 + 10);
  for (let at = zip.readUInt32LE(zip.length - 22 + 16), i = 0; i < count; i++) {
    const nameLength = zip.readUInt16LE(at + 28);
    const local = zip.readUInt32LE(at + 42);
    const start = local + 30 + zip.readUInt16LE(local + 26) + zip.readUInt16LE(local + 28);
    const stored = zip.subarray(start, start + zip.readUInt32LE(at + 20));
    parts[zip.toString("utf8", at + 46, at + 46 + nameLength)] = (zip.readUInt16LE(at + 10) === 8 ? inflateRawSync(stored) : stored).toString("utf8");
    at += 46 + nameLength + zip.readUInt16LE(at + 30) + zip.readUInt16LE(at + 32);
  }
  return parts;
}

/** Call `exportXlsx` and return the download's file name and the parts of the file it holds. */
async function exportXlsx(page: Page): Promise<{ name: string; parts: Record<string, string> }> {
  const [download] = await Promise.all([page.waitForEvent("download"), page.evaluate(() => window.gantt!.api!.exportXlsx())]);
  return { name: download.suggestedFilename(), parts: unzip(readFileSync(await download.path())) };
}

test("export_xlsx_downloads_the_list_as_an_excel_file_named_for_the_open_file", async ({ g }) => {
  const { page } = g;
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", { name: "office-fitout.xml", mimeType: "application/xml", buffer: FITOUT });
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  const { rowCount } = await g.status();

  const { name, parts } = await exportXlsx(page);
  expect(name).toBe("office-fitout.xlsx");
  expect(Object.keys(parts).sort(), "the parts of a workbook with one sheet and its styles").toEqual(
    ["[Content_Types].xml", "_rels/.rels", "xl/_rels/workbook.xml.rels", "xl/styles.xml", "xl/workbook.xml", "xl/worksheets/sheet1.xml"].sort(),
  );
  const sheet = parts["xl/worksheets/sheet1.xml"]!;
  expect(sheet.match(/<row /g)?.length, "a header row, then a row per visible row").toBe(rowCount + 1);
  await expect(page.locator("#save-note")).toHaveText(`Exported ${rowCount} rows to office-fitout.xlsx`);

  // The file needs no repair: the sheet's date cells name style 1, and the styles part defines it as a date format.
  expect(sheet, "a date is a number in style 1").toMatch(/<c r="C2" s="1"><v>\d{5}(\.\d+)?<\/v><\/c>/);
  expect(parts["xl/styles.xml"], "style 1 is yyyy-mm-dd hh:mm").toMatch(/<cellXfs count="2">.*<xf numFmtId="164"[^>]*\/><\/cellXfs>/);
  expect(parts["xl/_rels/workbook.xml.rels"]).toContain('Target="styles.xml"');
  expect(parts["xl/workbook.xml"]).toContain('<sheet name="office-fitout"');
});

test("export_xlsx_names_a_generated_sample_untitled", async ({ g }) => {
  await g.open({ n: 20 });
  const { name, parts } = await exportXlsx(g.page);
  expect(name).toBe("untitled.xlsx");
  expect(parts["xl/worksheets/sheet1.xml"]!.match(/<row /g)?.length, "a header row and a row per task").toBe(21);
  await expect(g.page.locator("#save-note")).toHaveText("Exported 20 rows to untitled.xlsx");
});
