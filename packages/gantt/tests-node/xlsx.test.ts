// The page's .xlsx file (gantt#1875): the zip's checksums, its size, and the stored fallback.
import assert from "node:assert/strict";
import { test } from "node:test";
import { crc32, inflateRawSync } from "node:zlib";
import { xlsxFile, type XlsxSheet } from "../src/xlsx.ts";

interface Part {
  name: string;
  method: number;
  stored: Buffer;
  crc: number;
  size: number;
}

/** The entries of a zip, read from its central directory (the local header's CRC and sizes must agree). */
function readZip(bytes: Uint8Array): Part[] {
  const zip = Buffer.from(bytes);
  const count = zip.readUInt16LE(zip.length - 22 + 10);
  const parts: Part[] = [];
  for (let at = zip.readUInt32LE(zip.length - 22 + 16), i = 0; i < count; i++) {
    const nameLength = zip.readUInt16LE(at + 28);
    const local = zip.readUInt32LE(at + 42);
    const part = { name: zip.toString("utf8", at + 46, at + 46 + nameLength), method: zip.readUInt16LE(at + 10), crc: zip.readUInt32LE(at + 16), size: zip.readUInt32LE(at + 24), stored: Buffer.alloc(0) };
    assert.equal(zip.readUInt32LE(local + 14), part.crc, `${part.name}: local CRC`);
    part.stored = zip.subarray(local + 30 + nameLength, local + 30 + nameLength + zip.readUInt32LE(at + 20));
    assert.equal(zip.readUInt32LE(local + 18), part.stored.length, `${part.name}: local size`);
    parts.push(part);
    at += 46 + nameLength;
  }
  return parts;
}

/** What each entry holds once unpacked. */
const unpack = (part: Part): Buffer => (part.method === 8 ? inflateRawSync(part.stored) : part.stored);

/** The worksheet part as the engine writes it (`App.list_xlsx_sheet`): inline strings, and a date cell in style 1. */
const sheetOf = (n: number): string =>
  `<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>${Array.from({ length: n }, (_, i) => `<row r="${i + 1}"><c r="A${i + 1}" t="inlineStr"><is><t xml:space="preserve">Task number ${i + 1} &amp; &lt;more&gt;</t></is></c><c r="B${i + 1}" s="1"><v>${46027 + i / 3}</v></c></row>`).join("")}</sheetData></worksheet>`;

const thousand: XlsxSheet = { name: "Schedule", sheetXml: sheetOf(1000) };

test("each entry's CRC equals the CRC-32 of its unpacked bytes, and its size is that length", async () => {
  const parts = readZip(await xlsxFile(thousand));
  assert.deepEqual(parts.map((p) => p.name), ["[Content_Types].xml", "_rels/.rels", "xl/_rels/workbook.xml.rels", "xl/styles.xml", "xl/workbook.xml", "xl/worksheets/sheet1.xml"]);
  for (const part of parts) {
    const data = unpack(part);
    assert.equal(part.crc, crc32(data), part.name);
    assert.equal(part.size, data.length, part.name);
  }
});

test("a 1,000-row sheet packs under 60,000 bytes, and the sheet part is the engine's XML, as given", async () => {
  const bytes = await xlsxFile(thousand);
  assert.ok(bytes.length < 60_000, `${bytes.length} bytes`);
  assert.equal(unpack(readZip(bytes).at(-1)!).toString("utf8"), thousand.sheetXml);
  assert.match(unpack(readZip(bytes)[4]!).toString("utf8"), /<sheet name="Schedule" sheetId="1" r:id="rId1"\/>/);
});

test("the workbook needs no repair: a styles part, related and typed, defines the date style the sheet's cells name", async () => {
  const parts = Object.fromEntries(readZip(await xlsxFile(thousand)).map((p) => [p.name, unpack(p).toString("utf8")]));
  assert.match(parts["xl/_rels/workbook.xml.rels"]!, /Type="[^"]*\/relationships\/styles" Target="styles\.xml"/);
  assert.match(parts["[Content_Types].xml"]!, /<Override PartName="\/xl\/styles\.xml" ContentType="[^"]*spreadsheetml\.styles\+xml"\/>/);
  const xfs = /<cellXfs count="(\d+)">(.*?)<\/cellXfs>/.exec(parts["xl/styles.xml"]!);
  assert.equal(xfs?.[1], "2");
  assert.equal(xfs?.[2]?.match(/<xf /g)?.length, 2, "entry 0 is the default, entry 1 the date style");
  assert.match(xfs![2]!, /<xf numFmtId="164"[^>]*applyNumberFormat="1"\/>$/);
  assert.match(parts["xl/styles.xml"]!, /<numFmt numFmtId="164" formatCode="yyyy\\-mm\\-dd\\ hh:mm"\/>/, "a date reads as a date, yyyy-mm-dd hh:mm");
  assert.match(parts["xl/worksheets/sheet1.xml"]!, / s="1"><v>/);
});

test("with no CompressionStream the entries are stored and equal the parts", async () => {
  const packed = readZip(await xlsxFile(thousand));
  const saved = Object.getOwnPropertyDescriptor(globalThis, "CompressionStream")!;
  delete (globalThis as { CompressionStream?: unknown }).CompressionStream;
  let stored: Part[];
  try {
    stored = readZip(await xlsxFile(thousand));
  } finally {
    Object.defineProperty(globalThis, "CompressionStream", saved);
  }
  assert.ok(packed.some((p) => p.method === 8), "the compressed file deflates");
  assert.ok(stored.every((p) => p.method === 0 && p.stored.length === p.size && p.crc === crc32(p.stored)));
  assert.deepEqual(stored.map((p) => p.stored), packed.map(unpack));
});

test("a sheet's name is made safe: forbidden characters go, at most 31 remain, an empty name is Sheet1", async () => {
  const name = async (name: string): Promise<string> => unpack(readZip(await xlsxFile({ name, sheetXml: "" }))[4]!).toString("utf8");
  assert.match(await name("[a/b]:*?\\"), /<sheet name="ab" /);
  assert.match(await name("x".repeat(40)), /<sheet name="x{31}" /);
  assert.match(await name("///"), /<sheet name="Sheet1" /);
});
