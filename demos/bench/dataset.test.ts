import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { toMspdi } from "../dhtmlx-compare/to-mspdi.ts";
import { buildDataset, datasetJson, SHAPES, type Shape } from "./dataset.ts";

const here = dirname(fileURLToPath(import.meta.url));
const sha = (s: string) => createHash("sha256").update(s).digest("hex");

test("equal inputs give equal bytes, for every shape", () => {
  for (const shape of SHAPES) {
    const a = datasetJson(buildDataset(shape, 5000, 7n));
    const b = datasetJson(buildDataset(shape, 5000, 7n));
    assert.equal(a, b, shape);
  }
});

test("the node count equals the size, for every shape, from 1,000 to 50,000", () => {
  for (const shape of SHAPES) {
    for (const size of [1000, 5000, 10000, 25000, 50000]) {
      const d = buildDataset(shape, size, 20261020n);
      assert.equal(d.data.length, size, `${shape} ${size}`);
      assert.equal(new Set(d.data.map((t) => t.id)).size, size, `${shape} ${size}: ids repeat`);
    }
  }
});

test("chain and tree4 hold the summary, leaf and link counts of the comparison harness's table", () => {
  // CONFIGURATION.md of the harness, "The two dataset shapes": size, summaries, leaves, links.
  const rows: [Shape, number, number, number, number][] = [
    ["chain", 1000, 1, 999, 998],
    ["tree4", 1000, 100, 900, 891],
    ["chain", 5000, 1, 4999, 4998],
    ["tree4", 5000, 498, 4502, 4456],
    ["chain", 10000, 1, 9999, 9998],
    ["tree4", 10000, 993, 9007, 8916],
    ["chain", 50000, 1, 49999, 49998],
    ["tree4", 50000, 4957, 45043, 44592],
    ["chain", 100000, 1, 99999, 99998],
    ["tree4", 100000, 9911, 90089, 89188],
  ];
  for (const [shape, size, summaries, leaves, links] of rows) {
    const d = buildDataset(shape, size, 1n);
    const parents = new Set(d.data.map((t) => t.parent).filter((p) => p !== undefined));
    assert.equal(parents.size, summaries, `${shape} ${size} summaries`);
    assert.equal(d.data.length - parents.size, leaves, `${shape} ${size} leaves`);
    assert.equal(d.links.length, links, `${shape} ${size} links`);
  }
});

test("the seed changes the stress records and is recorded in every shape", () => {
  // The records only, not the header: the header names the seed, so it differs whatever the generator does.
  const records = (seed: bigint) => {
    const { data, links } = buildDataset("stress", 1000, seed);
    return JSON.stringify({ data, links });
  };
  assert.notEqual(records(1n), records(2n));
  for (const shape of SHAPES) assert.match(datasetJson(buildDataset(shape, 1000, 3n)), /"seed":"3"/);
});

test("the stress shape is the engine's stress(seed, n): head task, link count, hash", () => {
  // From `engine::generate::stress(20261020, 1000)`: the head is TaskId 103 (id 104 here) and the
  // schedule has 3,225 dependencies. The hash pins every record: a generator change that alters a
  // dataset must change this line, since two runs compare only when their hashes are equal.
  const d = buildDataset("stress", 1000, 20261020n);
  assert.equal(d.links.length, 3225);
  const head = d.data.find((t) => t.text === "Head");
  assert.equal(head?.id, 104);
  assert.equal(sha(datasetJson(d)), STRESS_1000_SHA);
});

const STRESS_1000_SHA = "3584ff275ae14510d72cb9eada42e22ca66bd31bd95d90c33a83673a08305e4c";

test("the MS Project XML keeps the dataset's dates: the project start and each leaf's start", () => {
  for (const shape of SHAPES) {
    const d = buildDataset(shape, 1000, 20261020n);
    const xml = toMspdi(d);
    assert.match(xml, new RegExp(`<Project [^>]*><StartDate>${d.start}T08:00:00</StartDate>`), shape);
    const constraint = new Map<string, string>();
    for (const m of xml.matchAll(/<Task><UID>\d+<\/UID><Name>([^<]*)<\/Name>.*?<\/Task>/g)) {
      const c = m[0].match(/<ConstraintType>4<\/ConstraintType><ConstraintDate>([^<]*)<\/ConstraintDate>/);
      if (c) constraint.set(m[1], c[1]);
    }
    const starts = d.data.filter((t) => t.start !== undefined);
    assert.equal(constraint.size, starts.length, `${shape}: one start-no-earlier-than per dated task`);
    for (const t of starts) assert.equal(constraint.get(t.text), `${t.start}T08:00:00`, `${shape} ${t.text}`);
  }
  // The dates follow the chain: a schedule that lost them would start every leaf on one day.
  assert.ok(new Set(buildDataset("chain", 1000, 1n).data.map((t) => t.start)).size > 700);
});

test("every shape opens as MS Project XML with one task per node", () => {
  for (const shape of SHAPES) {
    const xml = toMspdi(buildDataset(shape, 1000, 20261020n));
    assert.equal((xml.match(/<Task>/g) ?? []).length, 1000, shape);
  }
});

test("the CLI writes dataset.json and its SHA-256 in sha256sum format", () => {
  const out = mkdtempSync(join(tmpdir(), "bench-dataset-"));
  const args = [join(here, "dataset.ts"), "--shape", "tree4", "--size", "1000", "--seed", "5", "--out", out];
  const r = spawnSync(process.execPath, args, { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const json = readFileSync(join(out, "dataset.json"), "utf8");
  const hash = sha(json);
  assert.equal(readFileSync(join(out, "dataset.json.sha256"), "utf8"), `${hash}  dataset.json\n`);
  assert.match(r.stdout, new RegExp(hash));
  assert.equal(json, datasetJson(buildDataset("tree4", 1000, 5n)));
});

test("the CLI refuses an unknown shape and a size outside 1,000 to 100,000", () => {
  const out = mkdtempSync(join(tmpdir(), "bench-dataset-"));
  for (const bad of [["--shape", "ring", "--size", "1000"], ["--shape", "chain", "--size", "999"], ["--shape", "chain", "--size", "100001"]]) {
    const r = spawnSync(process.execPath, [join(here, "dataset.ts"), ...bad, "--seed", "1", "--out", out], { encoding: "utf8" });
    assert.equal(r.status, 64, bad.join(" "));
  }
});

test("the CLI refuses an option with no value and writes nothing", () => {
  const cases = [
    ["--shape", "chain", "--size", "1000", "--out", "--seed", "1"],
    ["--shape", "chain", "--size", "1000", "--seed", "1", "--out"],
    ["--shape", "--size", "1000", "--seed", "1", "--out", "x"],
    ["--shape", "chain", "--size", "1000", "--seed", "1", "--out", "x", "--out", "y"],
    ["--shape", "chain", "--size", "1000", "--seed", "1", "--out", "x", "--colour", "red"],
  ];
  for (const bad of cases) {
    const cwd = mkdtempSync(join(tmpdir(), "bench-dataset-"));
    const r = spawnSync(process.execPath, [join(here, "dataset.ts"), ...bad], { encoding: "utf8", cwd });
    assert.equal(r.status, 64, bad.join(" "));
    assert.deepEqual(readdirSync(cwd), [], `${bad.join(" ")}: wrote ${readdirSync(cwd).join(", ")}`);
  }
});

test("tree4seq and tree4long are tree4 with one unique host key on every task: t<id>, and 54 characters that differ only in their last six", () => {
  const plain = buildDataset("tree4", 1000, 1n).data;
  for (const shape of ["tree4seq", "tree4long"] as const) {
    const d = buildDataset(shape, 1000, 1n);
    assert.deepEqual(d.data.map(({ key, ...t }) => t), plain, shape);
    assert.equal(new Set(d.data.map((t) => t.key)).size, 1000, shape);
  }
  assert.ok(buildDataset("tree4seq", 1000, 1n).data.every((t) => t.key === `t${t.id}`));
  const long = buildDataset("tree4long", 1000, 1n).data.map((t) => t.key!);
  assert.deepEqual([...new Set(long.map((k) => k.length))], [54]);
  assert.equal(new Set(long.map((k) => k.slice(0, 48))).size, 1);
});
