// Our side reports the dataset it loaded, so the table prints our numbers, and the file its bench page
// hands to `mount` is the dataset's schedule.
//
// The adapter puts a real cell's `ours.json` in a bundle; our side's `datasetSha256` hook then runs as it runs in the
// page, against that bundle, with the globals `ours-page.ts` sets. Its answer goes into driver results for every run
// of four pairs, and `table.ts` prints the cell.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { fromTasks } from "../../../packages/gantt/src/adapters/plain.ts";
import { writeDataset } from "../run.ts";
import { adapt, isoDate } from "./dhtmlx-adapt.ts";
import { adaptInto } from "./ours-adapt.ts";
import ours from "./ours.ts";

const here = dirname(fileURLToPath(import.meta.url));
const ORIGIN = "http://bench.test/";

/** The page after `ours-page.ts` ran for `cell` in `bundle`: the globals the hook reads, and a fetch that serves the bundle. */
function page(bundle: string, cell: string, sha: string | null): void {
  const g = globalThis as any;
  g.window = { ganttDataUrl: `${ORIGIN}cells/${cell}/ours.json`, ...(sha === null ? {} : { ganttDatasetSha256: sha }) };
  g.fetch = async (url: string | URL) => {
    try {
      return new Response(readFileSync(join(bundle, String(url).slice(ORIGIN.length))));
    } catch {
      return new Response("", { status: 404 });
    }
  };
}

/** What `drive.ts` asks the side: the hook's answer, null when the side has none. A throw fails the test: every "no" must be the hook's own. */
async function asked(): Promise<string | null> {
  const v = await ours.datasetSha256?.();
  return typeof v === "string" ? v : null;
}

/** A driver result of our side that reports `sha`, with numbers in every phase. */
function result(sha: string | null, cellSha: string, loadMs: number): object {
  const frames = { dnf: false, verified: true, frames: 240, p50: 16.7, p95: 17, longGaps: 0, droppedFrames: 0, max: 20 };
  const mem = { kind: "pss", chromeBytes: 400e6 };
  return {
    tool: "drive", side: "ours", rows: 1000, warnings: [],
    environment: { chrome: "150.0.1", os: "linux 6.1", node: "v22", gpu: { vendor: "V", renderer: "R" }, viewport: { width: 1280, height: 800 }, dpr: 1, refreshHz: 60 },
    options: { dnfS: 120 },
    requestedDatasetSha256: cellSha,
    datasetSha256: sha,
    inputBytes: 1234,
    phases: { load: { dnf: false, ms: loadMs }, "scroll-y": frames, "scroll-x": frames, drag: { ...frames, task: "1" }, memory: { dnf: false, afterLoad: mem, afterScripts: mem } },
  };
}


test("a run of our side reports the dataset it loaded, and table.ts prints its numbers", async (t) => {
  const root = mkdtempSync(join(tmpdir(), "ours-dataset-"));
  const out = join(root, "results");
  const cell = "tree4-1000";
  const cellSha = writeDataset(join(out, cell), { shape: "tree4", size: 1000 }, 20261020n)!;
  const bundle = join(root, "bundle");
  const { data } = adaptInto(join(out, cell, "dataset.json"), bundle);
  page(bundle, "tree4-1000-20261020", data.datasetSha256);
  const sha = await asked();

  for (let k = 1; k <= 4; k++) {
    for (const [side, start] of [["ours-a", k % 2 === 1 ? 1000 : 1010], ["ours-b", k % 2 === 1 ? 1010 : 1000]] as const) {
      const sideDir = join(out, cell, String(k), side);
      mkdirSync(sideDir, { recursive: true });
      writeFileSync(join(sideDir, "load.log"), [0, 3, 6].map((t) => `${start + t} 2.00 500000 100`).join("\n") + "\n");
      writeFileSync(join(sideDir, "result.json"), JSON.stringify(result(sha, cellSha, 700 + k)));
      writeFileSync(join(sideDir, "side.json"), JSON.stringify({ name: side, probe: "ours", path: `ours-page.html?cell=tree4-1000-20261020`, bundleSha256: "11".repeat(32), harness: "c0ffee" }));
    }
  }
  const r = spawnSync(process.execPath, [join(here, "../../dhtmlx-compare/table.ts"), out], { encoding: "utf8" });
  const text = r.stdout + r.stderr;
  for (const line of text.trimEnd().split("\n")) t.diagnostic(line);
  assert.doesNotMatch(text, /refused: /, text);
  assert.equal(r.status, 0, text);
  assert.match(text, new RegExp(`${cell}: dataset sha256 ${cellSha}`));
  assert.match(text, /^ {2}ours-a +4 +2 .* 702\.5 +704\.0 +17\.0 /m);
  assert.match(text, /^ {2}ours-b +4 +2 .* 702\.5 +704\.0 +17\.0 /m);
  assert.match(text, /^ {2}input files: ours-a 1,234 bytes, ours-b 1,234 bytes$/m);
});

/** A bundle holding an adapted chain cell, its directory name under `cells/`, and the cell's hash. */
function adapted(): { bundle: string; cell: string; cellSha: string } {
  const root = mkdtempSync(join(tmpdir(), "ours-dataset-"));
  const cellSha = writeDataset(join(root, "cell"), { shape: "chain", size: 1000 }, 20261020n)!;
  const bundle = join(root, "bundle");
  adaptInto(join(root, "cell", "dataset.json"), bundle);
  return { bundle, cell: "chain-1000-20261020", cellSha };
}

test("our side reports the cell's hash for the file its page fetched", async () => {
  const { bundle, cell, cellSha } = adapted();
  page(bundle, cell, cellSha);
  assert.equal(await asked(), cellSha);
});

test("our side reports nothing when the file is not the one its hash file names", async () => {
  const { bundle, cell, cellSha } = adapted();
  const file = join(bundle, "cells", cell, "ours.json");
  writeFileSync(file, readFileSync(file, "utf8").replace('"name":"', '"name":"X'));
  page(bundle, cell, cellSha);
  assert.equal(await asked(), null);
});

test("our side reports nothing when the page took out another hash than the file holds, or none (a page that is not the bench page)", async () => {
  const { bundle, cell } = adapted();
  page(bundle, cell, "00".repeat(32));
  assert.equal(await asked(), null);
  page(bundle, cell, null);
  assert.equal(await asked(), null);
  (globalThis as any).window = {};
  assert.equal(await asked(), null);
});

test("what the bench page hands to mount is the dataset's schedule: every task and link, each leaf's work, each task's id as its activity ID", () => {
  const root = mkdtempSync(join(tmpdir(), "ours-plain-"));
  writeDataset(join(root, "cell"), { shape: "tree4", size: 1000 }, 20261020n);
  const { data } = adaptInto(join(root, "cell", "dataset.json"), join(root, "bundle"));
  const dataset = JSON.parse(readFileSync(join(root, "cell", "dataset.json"), "utf8")) as { data: { id: number; duration: number }[]; links: unknown[] };
  const { document, leftOut } = fromTasks({ tasks: data.tasks, links: data.links });
  assert.equal(document.tasks.length, 1000);
  assert.equal(document.links.length, dataset.links.length);
  // Ours are the dataset's own ids (the drag's task) and a leaf's duration in minutes (8-hour days), whatever the calendar span of its dates.
  const work = new Map(dataset.data.map((t) => [String(t.id), t.duration * 480]));
  document.tasks.forEach((t, i) => {
    assert.equal(t.code, String(data.tasks[i].id));
    if (work.has(t.code) && t.duration > 0) assert.equal(t.duration, work.get(t.code), `task ${t.code}`);
  });
  assert.equal(document.tasks.find((t) => t.code === "1")!.duration, 960);
  // The only thing left out is what a summary's own dates would say: they come from its children.
  assert.equal(leftOut.length, 1);
  assert.match(leftOut[0], /summary tasks take their dates/);
});

test("our file and the DHTMLX file, made from one dataset, hold the same tasks, links and dates", () => {
  const root = mkdtempSync(join(tmpdir(), "ours-same-"));
  writeDataset(join(root, "cell"), { shape: "tree4", size: 1000 }, 20261020n);
  const { data } = adaptInto(join(root, "cell", "dataset.json"), join(root, "bundle"));
  const theirs = JSON.parse(adapt(JSON.parse(readFileSync(join(root, "cell", "dataset.json"), "utf8")), data.datasetSha256).dhtmlx);
  assert.equal(theirs.datasetSha256, data.datasetSha256, "one dataset hash");
  assert.deepEqual(data.tasks.map((t) => [t.id, t.name, t.start, t.end, t.parent ?? 0]), theirs.data.map((t: any) => [t.id, t.text, isoDate(t.start_date), isoDate(t.end_date), t.parent]));
  const kinds = ["FS", "SS", "FF", "SF"];
  assert.deepEqual(data.links.map((l) => [l.from, l.to, l.type]), theirs.links.map((l: any) => [l.source, l.target, kinds[Number(l.type)]]));
});
