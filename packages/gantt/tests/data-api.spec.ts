// The data API (gs#1084): a host feeds a schedule to the chart as JSON and reads it back, and takes an
// MS Project XML export. The chart is driven through `window.gantt.api`, the object `mount` returned.
// The fixture is a document saved by the app itself (Ctrl+S) from tests/fixtures/one-project.xer, as
// open-document.spec.ts saves one.
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { gunzipSync } from "node:zlib";
import { expect, test, type Gantt } from "./support/gantt";

const XER = { name: "one-project.xer", mimeType: "application/octet-stream", buffer: readFileSync(new URL("./fixtures/one-project.xer", import.meta.url)) };
const folder = mkdtempSync(join(tmpdir(), "data-api-"));

/** Open the XER fixture, save it with Ctrl+S, and return the saved document as JSON. */
async function savedFixture(g: Gantt): Promise<unknown> {
  const { page } = g;
  await g.open({ n: 20 });
  await page.setInputFiles("#open-input", XER);
  await page.waitForFunction(() => window.gantt!.status().epoch === 1, undefined, { timeout: 60_000 });
  await g.nextFrame(3);
  await page.locator("#app").focus();
  const download = page.waitForEvent("download");
  await page.keyboard.press("ControlOrMeta+S");
  const path = join(folder, "saved.gantt");
  await (await download).saveAs(path);
  return JSON.parse(gunzipSync(readFileSync(path)).toString("utf8"));
}

test("load_a_document_then_to_document_gives_it_back", async ({ g }) => {
  const fixture = await savedFixture(g);
  await g.open({ n: 20 });
  const result = await g.page.evaluate((document) => window.gantt!.api!.load({ document }), fixture);
  expect(result).toEqual({ ok: true });
  const back = await g.page.evaluate(() => window.gantt!.api!.toDocument());
  expect(back).toEqual(fixture);
});

test("load_a_bad_file_is_a_result_not_a_throw", async ({ g }) => {
  await g.open({ n: 20 });
  const before = await g.status();
  const result = await g.page.evaluate(async () => {
    const file = new File(["this is not a schedule"], "bad.xml");
    return window.gantt!.api!.load({ file });
  });
  expect(result.ok).toBe(false);
  expect(result).toMatchObject({ error: expect.any(String) });
  expect((await g.status()).epoch, "the schedule stays").toBe(before.epoch);
  const bad = await g.page.evaluate(() => window.gantt!.api!.load({ document: { format: "gantt", version: 2 } }));
  expect(bad.ok).toBe(false);
});

test("load_a_file_replaces_the_schedule", async ({ g }) => {
  await g.open({ n: 20 });
  const result = await g.page.evaluate(async (xer) => {
    const file = new File([new Uint8Array(xer.bytes)], xer.name);
    return window.gantt!.api!.load({ file });
  }, { name: XER.name, bytes: [...XER.buffer] });
  expect(result).toEqual({ ok: true });
  expect((await g.status()).epoch).toBe(1);
  const doc = await g.page.evaluate(() => window.gantt!.api!.toDocument());
  expect(doc).toMatchObject({ format: "gantt", name: "one-project" });
});

test("export_mspdi_returns_bytes_and_left_out", async ({ g }) => {
  await g.open({ n: 20 });
  const out = await g.page.evaluate(() => {
    const { bytes, leftOut } = window.gantt!.api!.exportMspdi();
    return { head: new TextDecoder().decode(bytes.slice(0, 5)), isBytes: bytes instanceof Uint8Array, leftOut };
  });
  expect(out.isBytes).toBe(true);
  expect(out.head).toBe("<?xml");
  expect(Array.isArray(out.leftOut)).toBe(true);
});

test("export_xer_returns_bytes_and_left_out", async ({ g }) => {
  await g.open({ n: 20 });
  const out = await g.page.evaluate(() => {
    const { bytes, leftOut } = window.gantt!.api!.exportXer();
    return { head: new TextDecoder().decode(bytes.slice(0, 6)), isBytes: bytes instanceof Uint8Array, leftOut };
  });
  expect(out.isBytes).toBe(true);
  expect(out.head).toBe("ERMHDR");
  expect(Array.isArray(out.leftOut)).toBe(true);
});

test("export_xer_reads_back_and_a_second_round_trip_changes_nothing", async ({ g }) => {
  await g.open({ n: 20 });
  const rounds = await g.page.evaluate(async () => {
    const api = window.gantt!.api!;
    const counts: number[] = [];
    const results: unknown[] = [];
    for (let round = 0; round < 2; round += 1) {
      const { bytes } = api.exportXer();
      results.push(await api.load({ file: new File([bytes], "round.xer") }));
      counts.push(window.gantt!.status().n);
    }
    return { results, counts };
  });
  expect(rounds.results).toEqual([{ ok: true }, { ok: true }]);
  expect(rounds.counts[0], "the file reads back with the tasks").toBeGreaterThanOrEqual(20);
  expect(rounds.counts[1], "the second round trip keeps the count").toBe(rounds.counts[0]);
});
