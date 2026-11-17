// results-file.ts writes the dated file from a run directory, and writes nothing when a harness is dirty, unknown, mixed or not the head, or a file is unreadable.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
const HEAD = "ab".repeat(20), DIRTY = `${HEAD}-dirty`, SHA = "cd".repeat(32), frames = { dnf: false, p95: 17, max: 20 }, NOTES = { story: "s", machine: "m", command: "c", driverOptions: "d", method: "x", tree: "t", box: { quietCap: "q", wallClock: "w" } };
const RESULT = { environment: {}, options: {}, requestedDatasetSha256: SHA, datasetSha256: SHA, inputBytes: 99, warnings: [], phases: { load: { dnf: false, ms: 100, libraryMs: 20, dataToReadyMs: 80 }, "scroll-y": frames, "scroll-x": frames, drag: { ...frames, task: "7" }, memory: { dnf: false, afterLoad: { chromeBytes: 3e8 } } } };
// One matched pair whose sides ran at the given harnesses (`edit` changes the cell): the writer's exit code and the file it wrote, null when none.
function run(ours = HEAD, dhtmlx = HEAD, o: { head?: string; notes?: string; result?: object; edit?: (cell: string) => void } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "results-file-")), cell = join(dir, "chain-10000");
  mkdirSync(cell);
  writeFileSync(join(cell, "dataset.json.sha256"), `${SHA}  dataset.json\n`);
  writeFileSync(join(dir, "notes.json"), o.notes ?? JSON.stringify(NOTES));
  for (const [side, harness] of [["ours", ours], ["dhtmlx", dhtmlx]]) {
    const d = join(cell, "1", side);
    mkdirSync(d, { recursive: true });
    for (const [name, text] of [["side.json", JSON.stringify({ bundleSha256: SHA, harness })], ["result.json", JSON.stringify(o.result ?? RESULT)], ["load.log", "1 2.00 1000 0\n4 2.00 1000 0\n"], ["wait.log", "1 load1=2.00 max=4 swapKb=0 busy=none quiet\n"]]) writeFileSync(join(d, name), text);
  }
  o.edit?.(cell);
  const out = join(dir, "out.json"), cli = fileURLToPath(new URL("./results-file.ts", import.meta.url));
  const r = spawnSync(process.execPath, [cli, "--runs", dir, "--head", o.head ?? HEAD, "--date", "2026-11-05", "--notes", join(dir, "notes.json"), "--out", out]);
  return { code: r.status, file: existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : null };
}
test("a run directory at the head gives its cells and medians", () => {
  const { code, file } = run();
  assert.equal(code, 0);
  assert.deepEqual(file.cells["chain-10000"], { datasetSha256: SHA, pairsRun: 1, matched: 1 });
  assert.deepEqual([file.medians["chain-10000"].ours.loadMedianMs, file.medians["chain-10000"].ours.memoryFootprintMB], [100, 300]);
});
test("a dirty, unknown or mixed harness, a side outside the counted pairs, or a figure that is not there, writes nothing", () => {
  const ninth = (c: string) => (mkdirSync(join(c, "9", "ours"), { recursive: true }), writeFileSync(join(c, "9", "ours", "side.json"), JSON.stringify({ harness: DIRTY })));
  for (const got of [run(DIRTY, DIRTY), run(HEAD, "ef".repeat(20)), run("unknown", "unknown", { head: "unknown" }), run(DIRTY, DIRTY, { head: DIRTY }), run(HEAD, HEAD, { edit: ninth }), run(HEAD, HEAD, { result: { ...RESULT, phases: { ...RESULT.phases, memory: { dnf: true, cause: "timeout", afterS: 120 } } } })]) assert.deepEqual(got, { code: 1, file: null });
});
test("unreadable or incomplete notes, or an unreadable result.json, exit 2 and write nothing", () => {
  for (const notes of ["{", "{}", "null", JSON.stringify({ ...NOTES, box: null }), JSON.stringify({ ...NOTES, story: undefined })]) assert.deepEqual(run(HEAD, HEAD, { notes }), { code: 2, file: null });
  assert.deepEqual(run(HEAD, HEAD, { edit: (c) => (rmSync(join(c, "1", "ours", "result.json")), mkdirSync(join(c, "1", "ours", "result.json"))) }), { code: 2, file: null });
});
