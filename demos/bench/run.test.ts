// The pair orchestrator: its options, and whole runs with a stand-in driver (a script that writes a
// driver result without a browser), so the test checks the orchestration, not any side's speed.
// The runs that check the table's verdicts go through `runCells` with a fixed reader (load, swap and
// the busy test always the same), so they do not depend on the box's real load. The one run through
// the command line reads the real box, so it checks only what that cannot change.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import type { Reader } from "../dhtmlx-compare/pair.ts";
import { pagePath, parseArgs, parseCells, parseSides, runCells, type Options } from "./run.ts";

const here = dirname(fileURLToPath(import.meta.url));
const tmp = () => mkdtempSync(join(tmpdir(), "bench-run-"));

const SIDES = [
  { name: "one", bundle: "b1", probe: "ours", path: "?sample=bench-{shape}-{size}&seed={seed}" },
  { name: "two", bundle: "b2", probe: "sides/two.ts", path: "/two/{size}.html" },
];

/**
 * A driver that writes a result with `warnings` (from $FAKE_WARNING), the --url and --probe it got,
 * and the dataset hash it "loaded": the cell's, read beside the result's directory, or $FAKE_SHA.
 */
const FAKE = `
const fs = require("node:fs"), path = require("node:path");
const a = process.argv.slice(2);
const get = (n) => a[a.indexOf("--" + n) + 1];
const frames = { dnf: false, verified: true, frames: 10, p50: 16.7, p95: 17, longGaps: 0, droppedFrames: 0, max: 18 };
const mem = { kind: "pss", chromeBytes: 3e8, rendererBytes: 1e8, gpuProcessBytes: 1e8, jsHeapUsedBytes: 1e7 };
const w = process.env.FAKE_WARNING ? [process.env.FAKE_WARNING] : [];
const cellSha = fs.readFileSync(path.join(path.dirname(get("out")), "..", "..", "dataset.json.sha256"), "utf8").split(" ")[0];
fs.writeFileSync(get("out"), JSON.stringify({ url: get("url"), probe: get("probe"), rows: Number(get("rows")), extra: a.includes("--headless"),
  environment: { chrome: "150", os: "linux" }, options: { dnfS: 120 }, requestedDatasetSha256: get("dataset-sha256"), datasetSha256: process.env.FAKE_SHA || cellSha, warnings: w,
  phases: { load: { dnf: false, ms: 500 }, "scroll-y": frames, "scroll-x": frames, drag: { ...frames, task: "1" }, memory: { dnf: false, afterLoad: mem, afterScripts: mem } } }));
`;

/** A results dir, its sides.json, two bundles and the stand-in driver. */
function setup() {
  const root = tmp();
  for (const b of ["b1", "b2"]) mkdirSync(join(root, b));
  const sides = SIDES.map((s) => ({ ...s, bundle: join(root, s.bundle) }));
  writeFileSync(join(root, "sides.json"), JSON.stringify(sides));
  writeFileSync(join(root, "fake-drive.cjs"), FAKE);
  return root;
}

function run(root: string, extra: string[], env: Record<string, string> = {}) {
  const args = [join(here, "run.ts"), "--sides", join(root, "sides.json"), "--out", join(root, "out"), "--driver", join(root, "fake-drive.cjs"), "--wait-s", "60", "--max-load", "1000", ...extra];
  const r = spawnSync(process.execPath, args, { encoding: "utf8", env: { ...process.env, ...env } });
  return { code: r.status, out: r.stdout + r.stderr };
}

test("the options: sides, cells and out required; two named sides; cells checked as the dataset checks them", () => {
  const read = () => JSON.stringify(SIDES);
  const ok = parseArgs(["--sides", "s.json", "--cells", "tree4:10000,chain:1000", "--out", "o", "--", "--headless"], read);
  assert.ok(typeof ok === "object", String(ok));
  assert.deepEqual([ok.cells, ok.pairs, ok.seed, ok.driveArgs], [[{ shape: "tree4", size: 10000 }, { shape: "chain", size: 1000 }], 8, 20261020n, ["--headless"]]);
  for (const bad of [
    ["--cells", "tree4:10000", "--out", "o"],
    ["--sides", "s", "--cells", "tree4:10", "--out", "o"],
    ["--sides", "s", "--cells", "cube:10000", "--out", "o"],
    ["--sides", "s", "--cells", "tree4:10000,tree4:10000", "--out", "o"],
    ["--sides", "s", "--cells", "tree4:10000", "--out", "o", "--pairs", "9"],
    ["--sides", "s", "--cells", "tree4:10000", "--out", "o", "--seed", "-1"],
    ["--sides", "s", "--cells", "tree4:10000", "--out", "o", "--colour", "red"],
  ]) {
    assert.equal(typeof parseArgs(bad, read), "string", bad.join(" "));
  }
  assert.equal(typeof parseSides(JSON.stringify([SIDES[0]])), "string", "one side is not a pair");
  assert.equal(typeof parseSides(JSON.stringify([SIDES[0], SIDES[0]])), "string", "two of the same name");
  assert.equal(typeof parseSides(JSON.stringify([SIDES[0], { ...SIDES[1], name: "Two Sides" }])), "string");
  assert.equal(typeof parseCells("stress:5000", 1n), "object");
  assert.equal(pagePath(SIDES[0].path, { shape: "tree4", size: 10000 }, 7n), "{url}?sample=bench-tree4-10000&seed=7");
  assert.equal(pagePath(SIDES[1].path, { shape: "chain", size: 1000 }, 7n), "{url}two/1000.html");
});

/** A box that is always quiet: load1 1, swap steady, nothing busy. */
const QUIET: Reader = { load1: () => 1, swapKb: () => 0, chromeRssKb: () => 0, busy: () => null, loadText: () => "" };

/** The options for `root` (from setup()), as the command line would parse them. */
function options(root: string, extra: string[]): Options {
  const o = parseArgs(["--sides", join(root, "sides.json"), "--out", join(root, "out"), "--driver", join(root, "fake-drive.cjs"), "--wait-s", "60", ...extra]);
  assert.ok(typeof o === "object", String(o));
  return o;
}

/** runCells on the quiet box with $FAKE_* set for the stand-in driver while it runs. */
async function runQuiet(root: string, extra: string[], env: Record<string, string> = {}) {
  Object.assign(process.env, env);
  try {
    return await runCells(options(root, extra), QUIET, "c0ffee", 20);
  } finally {
    for (const k of Object.keys(env)) delete process.env[k];
  }
}

test("one pair through the command line: both sides driven with the cell's page, rows and dataset", () => {
  const root = setup();
  const { code, out } = run(root, ["--cells", "chain:1000", "--pairs", "1", "--", "--headless"]);
  // The real box's swap may grow while the pair runs (exit 2); whether its pair matched depends on its load.
  assert.ok(code === 0 || code === 2, out);
  const hash = readFileSync(join(root, "out", "chain-1000", "dataset.json.sha256"), "utf8").split(" ")[0];
  assert.match(out, new RegExp(`chain-1000: dataset sha256 ${hash}`));
  assert.match(out, /chain-1000: [01] matched of 1 attempts \(unmatched: needs 4\)/);
  const one = JSON.parse(readFileSync(join(root, "out", "chain-1000", "1", "one", "result.json"), "utf8"));
  assert.match(one.url, /^http:\/\/127\.0\.0\.1:\d+\/\?sample=bench-chain-1000&seed=20261020$/);
  assert.deepEqual([one.probe, one.rows, one.extra, one.requestedDatasetSha256], ["ours", 1000, true, hash]);
  const two = JSON.parse(readFileSync(join(root, "out", "chain-1000", "1", "two", "result.json"), "utf8"));
  assert.match(two.url, /\/two\/1000\.html$/);
  const build = JSON.parse(readFileSync(join(root, "out", "chain-1000", "1", "two", "side.json"), "utf8"));
  assert.deepEqual([build.name, build.probe, build.path], ["two", "sides/two.ts", "/two/{size}.html"]);
  assert.match(build.bundleSha256, /^[0-9a-f]{64}$/);
});

test("on a quiet box a cell runs to 4 matched pairs, prints each side's row, and a second run adds none", async () => {
  const root = setup();
  const first = await runQuiet(root, ["--cells", "chain:1000"]);
  assert.equal(first.code, 0, first.text);
  assert.match(first.text, /chain-1000: 4 matched of 4 attempts$/m);
  assert.match(first.text, /^ {2}one +4 +\d +- +- +- +- +500\.0 +500\.0 +17\.0 +0\.0 +18\.0 +- +17\.0 +0\.0 +18\.0 +- +17\.0 +0\.0 +18\.0 +- +300\.0 MB PSS$/m);
  assert.match(first.text, /^ {2}two +4 +\d +- +- +- +- +500\.0 /m);
  const again = await runQuiet(root, ["--cells", "chain:1000"]);
  assert.match(again.text, /chain-1000: 4 matched of 4 attempts$/m, "the cell already has its 4");
});

test("a side rebuilt between two runs into one --out is not combined into one row", async () => {
  const root = setup();
  assert.equal((await runQuiet(root, ["--cells", "chain:1000", "--pairs", "2"])).code, 0);
  writeFileSync(join(root, "b1", "index.html"), "<p>rebuilt</p>");
  const r = await runQuiet(root, ["--cells", "chain:1000"]);
  assert.equal(r.code, 3, r.text);
  assert.match(r.text, /chain-1000: side one ran more than one build, not combined into one row:/);
  assert.doesNotMatch(r.text, /^ {2}side +pairs/m);
});

test("results with warnings or another dataset are refused, and a cell never mixes two datasets", async () => {
  const root = setup();
  const warned = await runQuiet(root, ["--cells", "chain:1000", "--pairs", "1"], { FAKE_WARNING: "headless: no window, software rendering; a smoke run, not a result" });
  assert.equal(warned.code, 3, warned.text);
  assert.match(warned.text, /refused: chain-1000\/1\/one: headless/);
  const other = await runQuiet(root, ["--cells", "chain:1000", "--pairs", "2"], { FAKE_SHA: "cd".repeat(32) });
  assert.equal(other.code, 3, other.text);
  assert.match(other.text, /refused: chain-1000\/2\/one: the side loaded dataset cdcd/);
  // The file records the seed, so another seed is another dataset, in the same cell.
  const mixed = await runQuiet(root, ["--cells", "chain:1000", "--pairs", "1", "--seed", "1"]);
  assert.equal(mixed.code, 65, mixed.text);
  assert.match(mixed.text, /holds another dataset/);
});
