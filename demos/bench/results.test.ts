// The table's rules for a side's trial build, and the columns load is split into: a trial's
// debug-build warning refuses only library ready and the whole load, never the run; data to ready
// prints for every side; the row is labelled with the build the side names; rows on screen prints.

import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { readPairs, render } from "../dhtmlx-compare/table.ts";
import { buildKey, HEADER, isTrial, memoryTiming, pairRefusal, refusal, sideRow, TRIAL_CELL, type DriveResult } from "./results.ts";

const HASH = "cd".repeat(32);
const DEBUG = "debug build: the side says this page is not its production build";
const TRIAL = "Bryntum 7.3.8 trial build, dates given";
const ENV = { chrome: "150", os: "linux", node: "v22", gpu: { vendor: "V", renderer: "R" }, viewport: { width: 1280, height: 800 }, dpr: 1, refreshHz: 60 };
const BUILD = { name: "x", probe: "p", path: "?cell={shape}-{size}-{seed}", bundleSha256: "11".repeat(32), harness: "c0ffee" };

type Opts = { scrolledY?: number; longGaps?: number; max?: number; trial?: boolean; warnings?: string[]; library?: number; data?: number; settled?: number; ms?: number; onScreen?: number };

/** A driver result: a trial build (debugBuild, sideBuild, its warning) or ours, with load split as given. */
function result(o: Opts = {}): DriveResult {
  const frames = { dnf: false, p95: 17, frames: 1000, longGaps: o.longGaps ?? 12, max: o.max ?? 41.7 };
  const environment = o.trial
    ? { ...ENV, debugBuild: true, sideBuild: TRIAL, rowsOnScreen: o.onScreen ?? 16 }
    : { ...ENV, debugBuild: false, sideBuild: null, rowsOnScreen: o.onScreen ?? 31 };
  return {
    environment,
    options: { dnfS: 120 },
    requestedDatasetSha256: HASH,
    datasetSha256: HASH,
    inputBytes: 1234,
    build: BUILD,
    warnings: o.warnings ?? (o.trial ? [DEBUG] : []),
    phases: {
      load: { dnf: false, ms: o.ms ?? 3000, libraryMs: o.library ?? 800, dataToReadyMs: o.data ?? 2000, dataToSettledMs: o.settled ?? null },
      "scroll-y": { ...frames, scrolled: o.scrolledY ?? 40_000 },
      "scroll-x": { ...frames, scrolled: 18_707 },
      drag: { ...frames, task: "1", commitMs: 25 },
      memory: { dnf: false, afterLoad: { kind: "pss", chromeBytes: 4e8 } },
    },
  };
}
const col = (row: string[], name: string) => row[HEADER.indexOf(name)];

test("a trial build's debug-build warning does not refuse the run; any other warning, or a debug build that is no trial, does", () => {
  assert.equal(isTrial(result({ trial: true })), true);
  assert.deepEqual(refusal(result({ trial: true }), HASH), []);
  assert.deepEqual(refusal(result({ trial: true, warnings: [DEBUG, "headless: no window"] }), HASH), ["headless: no window"]);
  assert.deepEqual(refusal(result({ warnings: [DEBUG] }), HASH), [DEBUG], "a debug build that names no trial is refused");
  const named = result({ warnings: [DEBUG] });
  named.environment = { ...named.environment, sideBuild: "a development build" };
  assert.deepEqual(refusal(named, HASH), [DEBUG]);
});

test("a trial's row: its build as the label, data to ready and to settled and rows on screen; library, the whole load and memory refused", () => {
  const rs = [1800, 2000, 2200, 2400].map((data) => result({ trial: true, data, settled: data + 500 }));
  const row = sideRow("bryntum-dates", rs, 2, true);
  assert.equal(row.length, HEADER.length);
  assert.equal(col(row, "side"), `bryntum-dates: ${TRIAL}`);
  assert.equal(col(row, "data to ready median ms"), "2100.0");
  assert.equal(col(row, "data to settled median ms"), "2600.0", "the engine's first pass, after the frame the saved dates drew");
  assert.equal(col(row, "rows on screen"), "16.0");
  // The unminified bundle: more to fetch and parse, and more source held in the renderer.
  for (const c of ["library median ms", "load median ms", "load max ms", "memory median"]) assert.equal(col(row, c), TRIAL_CELL, c);
  assert.equal(col(row, "scroll-y p95 median ms"), "17.0", "the input phases are not touched by the bundle");
});

test("a production build's row prints library ready, data to ready and the whole load", () => {
  const rs = [100, 200, 300, 400].map((d) => result({ library: 50 + d, data: d, ms: 1000 + d }));
  const row = sideRow("ours", rs, 2, true);
  assert.deepEqual(
    ["side", "rows on screen", "library median ms", "data to ready median ms", "data to settled median ms", "load median ms", "load max ms", "memory median"].map((c) => col(row, c)),
    ["ours", "31.0", "300.0", "250.0", "-", "1250.0", "1400.0", "400.0 MB PSS"],
  );
});

test("each frame phase prints the median share of frames over 1.5 periods and the longest frame of any run, beside the p95", () => {
  // Shares 0.5, 1, 2, 4 % of 1,000 frames: the median is 1.5 %; the longest of the four runs' worst frames is 90.1 ms.
  const rs = [[5, 20], [10, 30.5], [20, 90.1], [40, 50]].map(([longGaps, max]) => result({ longGaps, max }));
  const row = sideRow("ours", rs, 2, true);
  for (const phase of ["scroll-y", "scroll-x", "drag"]) {
    assert.deepEqual([`${phase} p95 median ms`, `${phase} long frames median %`, `${phase} longest frame max ms`].map((c) => col(row, c)), ["17.0", "1.5", "90.1"], phase);
  }
  // A run that did not finish a phase has no share and no longest frame: the columns say "-", never a number from the rest.
  const half = [result(), { ...result(), phases: { ...result().phases, "scroll-y": { dnf: false } } } as DriveResult];
  assert.deepEqual(["scroll-y long frames median %", "scroll-y longest frame max ms"].map((c) => col(sideRow("ours", half, 1, true), c)), ["-", "-"]);
  assert.equal(HEADER.filter((h) => /long frames median %$/.test(h)).length, 3);
});

test("each scroll phase prints the distance it scrolled, the median of the runs', beside its frame columns", () => {
  // The distance is the smaller of the fixed distance and the side's range, so a short schedule's row shows it: 10,000, 40,000 and 40,000 px.
  const rs = [10_000, 40_000, 40_000].map((scrolledY) => result({ scrolledY }));
  const row = sideRow("dhtmlx", rs, 1, true);
  assert.deepEqual(["scroll-y distance median px", "scroll-x distance median px"].map((c) => col(row, c)), ["40000.0", "18707.0"]);
  assert.equal(HEADER.indexOf("scroll-y distance median px"), HEADER.indexOf("scroll-y longest frame max ms") + 1, "beside the frame columns");
  assert.equal(HEADER.indexOf("scroll-x distance median px"), HEADER.indexOf("scroll-x longest frame max ms") + 1);
  assert.equal(HEADER.filter((h) => /distance median px$/.test(h)).length, 2, "drag covers no distance");
  const half = [result(), { ...result(), phases: { ...result().phases, "scroll-y": { dnf: false } } } as DriveResult];
  assert.equal(col(sideRow("ours", half, 1, true), "scroll-y distance median px"), "-");
});

test("two builds a side names are never one row", () => {
  const a = result({ trial: true });
  const b = result({ trial: true });
  b.environment = { ...b.environment, sideBuild: "Bryntum 7.3.8 trial build" };
  assert.notEqual(buildKey(a), buildKey(b));
});

test("the table prints a trial build's row over its matched pairs, with the dataset hash", () => {
  const root = mkdtempSync(join(tmpdir(), "bench-results-"));
  mkdirSync(join(root, "tree4-1000"));
  writeFileSync(join(root, "tree4-1000", "dataset.json.sha256"), `${HASH}  dataset.json\n`);
  for (let k = 1; k <= 4; k++) {
    for (const [side, trial, t] of [["ours", false, k % 2 ? 1000 : 1010], ["bryntum", true, k % 2 ? 1010 : 1000]] as const) {
      const dir = join(root, "tree4-1000", String(k), side);
      mkdirSync(dir, { recursive: true });
      writeFileSync(join(dir, "load.log"), `${t} 1.00 100 50\n${t + 3} 1.00 100 50\n`);
      writeFileSync(join(dir, "result.json"), JSON.stringify({ ...result({ trial, data: 2000 + k }), build: undefined }));
      writeFileSync(join(dir, "side.json"), JSON.stringify({ ...BUILD, name: side }));
    }
  }
  const { text, code } = render(readPairs(root), root);
  assert.equal(code, 0, text);
  assert.match(text, new RegExp(`tree4-1000: dataset sha256 ${HASH}`));
  assert.match(text, /tree4-1000: 4 matched of 4 attempts$/m);
  assert.match(text, /^ {2}bryntum: Bryntum 7\.3\.8 trial build, dates given +4 +2 +16\.0 +trial build +2002\.5 +- +trial build +trial build +17\.0 +1\.2 +41\.7 +40000\.0 +17\.0 +1\.2 +41\.7 +18707\.0 +17\.0 +1\.2 +41\.7 +25\.0 +trial build$/m);
  assert.match(text, /^ {2}ours +4 +2 +31\.0 +800\.0 +2002\.5 +- +3000\.0 +3000\.0 +17\.0 +1\.2 +41\.7 +40000\.0 +17\.0 +1\.2 +41\.7 +18707\.0 +17\.0 +1\.2 +41\.7 +25\.0 +400\.0 MB PSS$/m);
});

test("a pair compares only when both drags moved the same task of the dataset, or a drag did not run", () => {
  const withDrag = (drag: object) => ({ ...result(), phases: { ...result().phases, drag } }) as DriveResult;
  const on = (task: string | null) => withDrag({ dnf: false, p95: 17, task });
  assert.equal(pairRefusal([{ side: "a", result: on("7") }, { side: "b", result: on("7") }]), null);
  assert.equal(pairRefusal([{ side: "a", result: on("7") }, { side: "b", result: on("8") }]), "drag: the sides dragged different tasks of the dataset (a task 7, b task 8)");
  assert.equal(pairRefusal([{ side: "a", result: on(null) }, { side: "b", result: on("8") }]), "drag: a did not record which task of the dataset it dragged");
  // A drag that did not finish is the run's own refusal or its timeout, not a mismatch.
  const timedOut = withDrag({ dnf: true, cause: "timeout", afterS: 120, why: "the phase did not finish" });
  assert.equal(pairRefusal([{ side: "a", result: timedOut }, { side: "b", result: on("8") }]), null);
});

test("a memory read that began over 2 s past the asked second is refused, and the caption's timing is derived from the runs' options", () => {
  const at = (asked: number | undefined, readAtS: number) => {
    const r = result();
    r.options = asked === undefined ? { dnfS: 120 } : { dnfS: 120, memoryAtS: asked };
    r.phases.memory = { dnf: false, afterLoad: { kind: "pss", chromeBytes: 4e8, readAtS } };
    return r;
  };
  assert.deepEqual(refusal(at(20, 21.9), HASH), [], "within the tolerance");
  assert.match(refusal(at(20, 30), HASH)[0], /memory: read began 30 s after the page opened, over 2 s past the 20 s asked/);
  assert.equal(memoryTiming([at(20, 20), at(20, 20.1)]), "read 20 s after the page opened, settled");
  assert.match(memoryTiming([at(undefined, 1), at(undefined, 1)]), /^read right after load \(legacy runs/);
  assert.equal(memoryTiming([at(20, 20), at(undefined, 1)]), "runs read memory at different times");
});
