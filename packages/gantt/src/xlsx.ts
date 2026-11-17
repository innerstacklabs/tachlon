// The page's .xlsx file (gantt#1875): one sheet, the worksheet XML the engine wrote (`App.list_xlsx_sheet`) with the other Office Open XML
// parts around it, zipped in the browser, so an export needs no server. Parts are compressed with `CompressionStream("deflate-raw")` when
// the browser has it, else stored. No network request.

/** A sheet to write: its tab name (Excel allows 31 characters and none of `[]:*?/\`) and its worksheet part, as `App.list_xlsx_sheet` returns it. */
export interface XlsxSheet {
  name: string;
  sheetXml: string;
}

const XML = '<?xml version="1.0" encoding="UTF-8" standalone="yes"?>\n';
const MAIN = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const REL = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PACKAGE_REL = "http://schemas.openxmlformats.org/package/2006/relationships";
const SHEET = "application/vnd.openxmlformats-officedocument.spreadsheetml";

/** The styles part: `cellXfs` entry 0 is the default, entry 1 the date style (`s="1"`) the engine's date cells name, `yyyy-mm-dd hh:mm`. */
const STYLES = `${XML}<styleSheet xmlns="${MAIN}"><numFmts count="1"><numFmt numFmtId="164" formatCode="yyyy\\-mm\\-dd\\ hh:mm"/></numFmts><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border><left/><right/><top/><bottom/><diagonal/></border></borders><cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs><cellXfs count="2"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/><xf numFmtId="164" fontId="0" fillId="0" borderId="0" xfId="0" applyNumberFormat="1"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>`;

/** The parts that do not depend on the sheet, as zip entry name and text. */
const STATIC_PARTS: ReadonlyArray<readonly [string, string]> = [
  [
    "[Content_Types].xml",
    `${XML}<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/workbook.xml" ContentType="${SHEET}.sheet.main+xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="${SHEET}.worksheet+xml"/><Override PartName="/xl/styles.xml" ContentType="${SHEET}.styles+xml"/></Types>`,
  ],
  ["_rels/.rels", `${XML}<Relationships xmlns="${PACKAGE_REL}"><Relationship Id="rId1" Type="${REL}/officeDocument" Target="xl/workbook.xml"/></Relationships>`],
  ["xl/_rels/workbook.xml.rels", `${XML}<Relationships xmlns="${PACKAGE_REL}"><Relationship Id="rId1" Type="${REL}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="${REL}/styles" Target="styles.xml"/></Relationships>`],
  ["xl/styles.xml", STYLES],
];

/** `text` safe in XML: the control characters XML 1.0 forbids dropped, the four markup characters escaped. */
const escapeXml = (text: string): string =>
  text.replace(/[\u0000-\u0008\u000b\u000c\u000e-\u001f￾￿]/g, "").replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");

/** The workbook part: one sheet, its name without the characters Excel forbids, at most 31 characters, `Sheet1` when nothing is left. */
const workbookXml = (name: string): string =>
  `${XML}<workbook xmlns="${MAIN}" xmlns:r="${REL}"><sheets><sheet name="${escapeXml(name.replace(/[[\]:*?/\\]/g, "").slice(0, 31) || "Sheet1")}" sheetId="1" r:id="rId1"/></sheets></workbook>`;

let crcTable: Uint32Array | undefined;

/** The CRC-32 (the zip checksum, polynomial 0xEDB88320) of `bytes`. */
function crc32(bytes: Uint8Array): number {
  if (!crcTable) {
    crcTable = Uint32Array.from({ length: 256 }, (_, n) => {
      for (let k = 0; k < 8; k++) n = n & 1 ? 0xedb88320 ^ (n >>> 1) : n >>> 1;
      return n >>> 0;
    });
  }
  let crc = 0xffffffff;
  for (const byte of bytes) crc = crcTable[(crc ^ byte) & 0xff]! ^ (crc >>> 8);
  return (crc ^ 0xffffffff) >>> 0;
}

/** `data` as a raw deflate stream, or `null` when the browser has no `CompressionStream("deflate-raw")`. */
async function deflateRaw(data: Uint8Array<ArrayBuffer>): Promise<Uint8Array<ArrayBuffer> | null> {
  if (typeof CompressionStream === "undefined") return null;
  try {
    return new Uint8Array(await new Response(new Blob([data]).stream().pipeThrough(new CompressionStream("deflate-raw"))).arrayBuffer());
  } catch {
    return null;
  }
}

/** One zip entry: its name, the method (8 deflate, 0 stored), the bytes as stored, and the CRC and length of the original. */
interface Entry {
  name: Uint8Array<ArrayBuffer>;
  method: 0 | 8;
  data: Uint8Array<ArrayBuffer>;
  crc: number;
  size: number;
}

/** `value` as `size` little-endian bytes. */
const le = (value: number, size: 2 | 4): Uint8Array<ArrayBuffer> =>
  new Uint8Array(size === 2 ? [value & 255, (value >>> 8) & 255] : [value & 255, (value >>> 8) & 255, (value >>> 16) & 255, value >>> 24]);

/** The ZIP of `entries`: a local header and the data of each, the central directory, the end record. The date is fixed (1980-01-01), so one sheet gives one file. */
async function zip(entries: Entry[]): Promise<Uint8Array<ArrayBuffer>> {
  const files: BlobPart[] = [];
  const directory: BlobPart[] = [];
  let offset = 0;
  for (const { name, method, data, crc, size } of entries) {
    // The fields both headers share, from the method to the extra length: time 0, date 0x21, CRC, stored length, length, name length, no extra.
    const shared = [le(method, 2), le(0, 2), le(0x21, 2), le(crc, 4), le(data.length, 4), le(size, 4), le(name.length, 2), le(0, 2)];
    files.push(le(0x04034b50, 4), le(20, 2), le(0, 2), ...shared, name, data);
    directory.push(le(0x02014b50, 4), le(20, 2), le(20, 2), le(0, 2), ...shared, le(0, 2), le(0, 2), le(0, 2), le(0, 4), le(offset, 4), name);
    offset += 30 + name.length + data.length;
  }
  const end = [le(0x06054b50, 4), le(0, 4), le(entries.length, 2), le(entries.length, 2), le(entries.reduce((n, e) => n + 46 + e.name.length, 0), 4), le(offset, 4), le(0, 2)];
  return new Uint8Array(await new Blob([...files, ...directory, ...end]).arrayBuffer());
}

/** The bytes of an `.xlsx` file holding `sheet`. A part is deflated when the browser can and that is smaller, else stored, so a browser with no `CompressionStream` still gives a file Excel opens. */
export async function xlsxFile(sheet: XlsxSheet): Promise<Uint8Array<ArrayBuffer>> {
  const encoder = new TextEncoder();
  const parts: Array<readonly [string, string]> = [...STATIC_PARTS, ["xl/workbook.xml", workbookXml(sheet.name)], ["xl/worksheets/sheet1.xml", sheet.sheetXml]];
  const entries: Entry[] = [];
  for (const [name, text] of parts) {
    const raw = encoder.encode(text) as Uint8Array<ArrayBuffer>;
    const packed = await deflateRaw(raw);
    const deflated = packed !== null && packed.length < raw.length;
    entries.push({ name: encoder.encode(name) as Uint8Array<ArrayBuffer>, method: deflated ? 8 : 0, data: deflated ? packed : raw, crc: crc32(raw), size: raw.length });
  }
  return zip(entries);
}
