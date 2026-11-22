// results-file.ts writes the dated file from a run directory, and writes nothing when a harness is dirty, unknown, mixed or not the head, a cell has too few matched pairs, the ours side ran more than one build, or a file is unreadable.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { MIN_MATCHED } from "../dhtmlx-compare/table.ts";
const HEAD = "ab".repeat(20), BUILD = "12".repeat(20), DIRTY = `${HEAD}-dirty`, SHA = "cd".repeat(32), frames = { dnf: false, p95: 17, max: 20 }, NOTES = { story: "s", machine: "m", command: "c", driverOptions: "d", method: "x", tree: "t", box: { quietCap: "q", wallClock: "w" } };
const RESULT = { environment: {}, options: {}, requestedDatasetSha256: SHA, datasetSha256: SHA, inputBytes: 99, warnings: [], phases: { load: { dnf: false, ms: 100, libraryMs: 20, dataToReadyMs: 80 }, "scroll-y": frames, "scroll-x": frames, drag: { ...frames, task: "7" }, memory: { dnf: false, afterLoad: { chromeBytes: 3e8 } } } };
// MIN_MATCHED matched pairs (`pairs` of them) whose sides ran at the given harnesses (`edit` changes the cell): the writer's exit code, its message and the file it wrote, null when none.
function run(ours = HEAD, dhtmlx = HEAD, o: { head?: string; notes?: string; result?: object; pairs?: number; versions?: string; edit?: (cell: string) => void } = {}) {
  const dir = mkdtempSync(join(tmpdir(), "results-file-")), cell = join(dir, "chain-10000");
  mkdirSync(cell);
  writeFileSync(join(cell, "dataset.json.sha256"), `${SHA}  dataset.json\n`);
  writeFileSync(join(dir, "notes.json"), o.notes ?? JSON.stringify(NOTES));
  writeFileSync(join(dir, "versions.json"), o.versions ?? JSON.stringify({ ours: { commit: BUILD } }));
  for (let pair = 1; pair <= (o.pairs ?? MIN_MATCHED); pair++) for (const [side, harness] of [["ours", ours], ["dhtmlx", dhtmlx]]) {
    const d = join(cell, String(pair), side);
    mkdirSync(d, { recursive: true });
    for (const [name, text] of [["side.json", JSON.stringify({ bundleSha256: SHA, harness })], ["result.json", JSON.stringify(o.result ?? RESULT)], ["load.log", "1 2.00 1000 0\n4 2.00 1000 0\n"], ["wait.log", "1 load1=2.00 max=4 swapKb=0 busy=none quiet\n"]]) writeFileSync(join(d, name), text);
  }
  o.edit?.(cell);
  const out = join(dir, "out.json"), cli = fileURLToPath(new URL("./results-file.ts", import.meta.url));
  const r = spawnSync(process.execPath, [cli, "--runs", dir, "--head", o.head ?? HEAD, "--date", "2026-11-05", "--notes", join(dir, "notes.json"), "--versions", join(dir, "versions.json"), "--out", out]);
  return { code: r.status, err: String(r.stderr), file: existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : null };
}
const refused = (got: ReturnType<typeof run>) => ({ code: got.code, file: got.file });
test("a run directory at the head gives its cells and medians", () => {
  const { code, file } = run();
  assert.equal(code, 0);
  assert.deepEqual(file.cells["chain-10000"], { datasetSha256: SHA, pairsRun: MIN_MATCHED, matched: MIN_MATCHED });
  assert.deepEqual([file.medians["chain-10000"].ours.loadMedianMs, file.medians["chain-10000"].ours.memoryFootprintMB], [100, 300]);
});
test("the harness commit and the measured build are written under names that cannot be confused", () => {
  const { file } = run(HEAD, HEAD, { versions: JSON.stringify({ ours: { commit: BUILD } }) });
  assert.equal(file.harnessCommit, HEAD);
  assert.deepEqual(file.measuredBuild, { commit: BUILD, bundleSha256: SHA });
  assert.ok(!("harness" in file) && !("ours" in file), "the two old names, which both held the harness commit, are gone");
});
test("a cell with fewer than the minimum matched pairs writes nothing and names the cell", () => {
  const short = run(HEAD, HEAD, { pairs: MIN_MATCHED - 1 });
  assert.deepEqual(refused(short), { code: 1, file: null });
  assert.match(short.err, new RegExp(`chain-10000: ${MIN_MATCHED - 1} matched pairs, ${MIN_MATCHED} needed`));
  // Pairs run is not pairs matched: one pair whose two loads differ by more than the limit is listed, not counted.
  const unmatched = run(HEAD, HEAD, { edit: (c) => writeFileSync(join(c, String(MIN_MATCHED), "ours", "load.log"), "1 3.00 1000 0\n4 3.00 1000 0\n") });
  assert.deepEqual(refused(unmatched), { code: 1, file: null });
  assert.match(unmatched.err, new RegExp(`chain-10000: ${MIN_MATCHED - 1} matched pairs`));
});
test("matched runs of the ours side from more than one build write nothing", () => {
  const got = run(HEAD, HEAD, { edit: (c) => writeFileSync(join(c, "2", "ours", "side.json"), JSON.stringify({ bundleSha256: "ee".repeat(32), harness: HEAD })) });
  assert.deepEqual(refused(got), { code: 1, file: null });
  assert.match(got.err, /2 builds/);
});
test("a dirty, unknown or mixed harness, a side outside the counted pairs, or a figure that is not there, writes nothing", () => {
  const ninth = (c: string) => (mkdirSync(join(c, "9", "ours"), { recursive: true }), writeFileSync(join(c, "9", "ours", "side.json"), JSON.stringify({ harness: DIRTY })));
  for (const got of [run(DIRTY, DIRTY), run(HEAD, "ef".repeat(20)), run("unknown", "unknown", { head: "unknown" }), run(DIRTY, DIRTY, { head: DIRTY }), run(HEAD, HEAD, { edit: ninth }), run(HEAD, HEAD, { result: { ...RESULT, phases: { ...RESULT.phases, memory: { dnf: true, cause: "timeout", afterS: 120 } } } })]) assert.deepEqual(refused(got), { code: 1, file: null });
});
test("unreadable or incomplete notes, a versions file without the build commit, or an unreadable result.json, exit 2 and write nothing", () => {
  for (const notes of ["{", "{}", "null", JSON.stringify({ ...NOTES, box: null }), JSON.stringify({ ...NOTES, story: undefined })]) assert.deepEqual(refused(run(HEAD, HEAD, { notes })), { code: 2, file: null });
  for (const versions of ["{", "{}", JSON.stringify({ ours: { commit: "45f38c4" } })]) assert.deepEqual(refused(run(HEAD, HEAD, { versions })), { code: 2, file: null });
  assert.deepEqual(refused(run(HEAD, HEAD, { edit: (c) => (rmSync(join(c, "1", "ours", "result.json")), mkdirSync(join(c, "1", "ours", "result.json"))) })), { code: 2, file: null });
});
