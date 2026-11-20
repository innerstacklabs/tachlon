// One benchmark schedule from (shape, size, seed), written as dataset.json with its SHA-256.
//
//   node demos/bench/dataset.ts --shape <chain|tree4|stress|tree4seq|tree4long> --size <n> --seed <u64> --out <dir>
//
// Every side of the benchmark loads this one file (through its own adapter), and two runs compare
// only when the hash is equal. The file is a pure function of (shape, size, seed): equal inputs
// give equal bytes, and every record counts, summaries included, so the node count is the size.
//
// Shapes:
// - `chain` and `tree4`: the two shapes of the comparison harness (its CONFIGURATION.md, "The two
//   dataset shapes"), rebuilt here from that description. `chain`: one summary over size - 1 leaves
//   linked finish-to-start in one chain. `tree4`: a portfolio root, projects, ten work packages a
//   project and ten tasks a package, filled in document order until the size is reached; leaves
//   are chained inside a project. Every leaf takes 2 days. Neither uses the seed, which is still
//   written into the file.
// - `stress`: ours, not the harness's. The engine's stress fixture (`stress.ts`): no outline,
//   dense cross-phase links of all four types, one head task every stream depends on.
//
// Records use the shape `demos/dhtmlx-compare/to-mspdi.ts` reads, which keeps every date: the
// file's `start` (the project start, a Monday) and a task `{ id, text, duration, parent?, start? }`
// (duration in 8-hour days) and a link `{ id, source, target, type }` (type "0" finish-to-start,
// "1" start-to-start, "2" finish-to-finish, "3" start-to-finish). Leaves take ids 1 up, summaries
// size + 1 up. Every leaf of `chain` and `tree4` carries its start as YYYY-MM-DD (08:00), worked out
// by the forward pass of `plan.ts` over the shape's own links on the Monday-to-Friday, 08:00-16:00
// calendar every side uses: the dates our engine computes from the links, so the start each leaf is
// given (a start-no-earlier-than in our file and Bryntum's) never moves it. `stress` tasks carry
// none: some start mid-day, and the links alone place them. Summaries carry none either.
// `tree4seq` and `tree4long` (#100) are `tree4` with a host `key` on every task, to open a keyed schedule against
// the unkeyed `tree4`: `tree4seq` keys `t<id>`, `tree4long` keys 54 characters long that differ only in their last six.
// The hash goes to <out>/dataset.json.sha256 in `sha256sum` format. Datasets are generated at run
// time and never committed.
// Exit code: 0 done; 64 usage (unknown shape, size outside 1,000 to 100,000, bad seed).

import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { DatasetLink, DatasetTask } from "../dhtmlx-compare/to-mspdi.ts";
import { placeLeafDays } from "./plan.ts";
import { stress, type Kind } from "./stress.ts";

export const SHAPES = ["chain", "tree4", "stress"] as const;

/** `tree4` with a host key on every task (#100): the key each shape gives the task of id `n`. */
export const KEYED = {
  tree4seq: (n: number) => `t${n}`,
  tree4long: (n: number) => `${"k".repeat(48)}${String(n).padStart(6, "0")}`,
} as const;
export type Shape = (typeof SHAPES)[number] | keyof typeof KEYED;

/** Who the shape comes from: the comparison harness, or our own fixtures. */
export const SOURCE: Record<Shape, "harness" | "ours"> = { chain: "harness", tree4: "harness", stress: "ours", tree4seq: "ours", tree4long: "ours" };

export const MIN_SIZE = 1000;
export const MAX_SIZE = 100000;
export const EXIT_USAGE = 64;

/** The first day of the schedule: a Monday, so a working day. */
export const START = "2026-03-02";
const LEAF_DAYS = 2;
const FANOUT = 10;

export interface BenchDataset {
  shape: Shape;
  size: number;
  /** A u64 in decimal. */
  seed: string;
  source: "harness" | "ours";
  start: string;
  data: BenchTask[];
  links: DatasetLink[];
}

/** A task record: the shape `to-mspdi.ts` reads. */
export type BenchTask = DatasetTask;

const LINK_TYPE: Record<Kind, string> = { FS: "0", SS: "1", FF: "2", SF: "3" };

interface Node {
  text: string;
  parent?: number;
  /** A work item: a leaf that takes part in the chain. */
  work: boolean;
  /** The node index that starts its link chain (the project, or the root for `chain`). */
  group: number;
}

/** Turn an outline in document order into records and the finish-to-start chains inside each group. */
function fromOutline(nodes: Node[], size: number): { data: BenchTask[]; links: DatasetLink[] } {
  const hasChild = new Set(nodes.flatMap((n) => (n.parent === undefined ? [] : [n.parent])));
  const ids: number[] = [];
  let leaf = 0;
  let summary = size;
  for (let i = 0; i < nodes.length; i++) ids.push(hasChild.has(i) ? ++summary : ++leaf);
  const data = nodes.map((n, i): BenchTask => {
    const t: BenchTask = { id: ids[i], text: n.text, duration: hasChild.has(i) ? 0 : LEAF_DAYS };
    if (n.parent !== undefined) t.parent = ids[n.parent];
    return t;
  });
  const links: DatasetLink[] = [];
  const last = new Map<number, number>();
  nodes.forEach((n, i) => {
    if (!n.work || hasChild.has(i)) return;
    const prev = last.get(n.group);
    if (prev !== undefined) links.push({ id: links.length + 1, source: ids[prev], target: ids[i], type: "0" });
    last.set(n.group, i);
  });
  return { data, links };
}

function chain(size: number) {
  const nodes: Node[] = [{ text: "Project", work: false, group: 0 }];
  for (let k = 1; k < size; k++) nodes.push({ text: `Task ${k}`, parent: 0, work: true, group: 0 });
  return fromOutline(nodes, size);
}

function tree4(size: number) {
  const nodes: Node[] = [{ text: "Portfolio", work: false, group: 0 }];
  const add = (text: string, parent: number, work: boolean, group: number): number | undefined => {
    if (nodes.length >= size) return undefined;
    nodes.push({ text, parent, work, group });
    return nodes.length - 1;
  };
  for (let p = 1; nodes.length < size; p++) {
    const project = add(`Project ${p}`, 0, false, -1)!;
    for (let k = 1; k <= FANOUT; k++) {
      const pkg = add(`Package ${p}.${k}`, project, false, project);
      if (pkg === undefined) break;
      for (let t = 1; t <= FANOUT; t++) if (add(`Task ${p}.${k}.${t}`, pkg, true, project) === undefined) break;
    }
  }
  return fromOutline(nodes, size);
}

function stressShape(size: number, seed: bigint) {
  const s = stress(seed, size);
  const data = s.tasks.map((t, i): BenchTask => ({
    id: i + 1,
    text: t.head ? "Head" : `Task ${i + 1}`,
    duration: t.minutes / 480,
  }));
  const links = s.links.map((l, i): DatasetLink => ({
    id: i + 1,
    source: l.pred + 1,
    target: l.succ + 1,
    type: LINK_TYPE[l.kind],
  }));
  return { data, links };
}

/** Throw unless `size` is an integer from MIN_SIZE to MAX_SIZE and `seed` a u64. */
export function checkInputs(shape: string, size: number, seed: bigint): asserts shape is Shape {
  const known = [...SHAPES, ...Object.keys(KEYED)];
  if (!known.includes(shape)) throw new Error(`unknown shape ${JSON.stringify(shape)} (${known.join(", ")})`);
  if (!Number.isInteger(size) || size < MIN_SIZE || size > MAX_SIZE) throw new Error(`size ${size} is not an integer from ${MIN_SIZE} to ${MAX_SIZE}`);
  if (seed < 0n || seed >= 1n << 64n) throw new Error(`seed ${seed} is not a u64`);
}

/** The records for (shape, size, seed). */
export function buildDataset(shape: Shape, size: number, seed: bigint): BenchDataset {
  checkInputs(shape, size, seed);
  const keyOf = shape in KEYED ? KEYED[shape as keyof typeof KEYED] : undefined;
  const body = shape === "chain" ? chain(size) : shape === "stress" ? stressShape(size, seed) : tree4(size);
  if (shape !== "stress") placeLeafDays(START, body.data, body.links);
  if (keyOf) for (const t of body.data) t.key = keyOf(Number(t.id));
  return { shape, size, seed: String(seed), source: SOURCE[shape], start: START, ...body };
}

/** The bytes of dataset.json: fixed key order, one record per line. */
export function datasetJson(d: BenchDataset): string {
  const head = JSON.stringify({ shape: d.shape, size: d.size, seed: d.seed, source: d.source, start: d.start });
  const list = (rows: object[]) => (rows.length === 0 ? "[]" : `[\n${rows.map((r) => JSON.stringify(r)).join(",\n")}\n]`);
  return `${head.slice(0, -1)},\n"data":${list(d.data)},\n"links":${list(d.links)}}\n`;
}

const OPTIONS = ["shape", "size", "seed", "out"] as const;

/**
 * The four options as `--name value` pairs, each exactly once; undefined for anything else: an
 * unknown option, a repeat, a stray word, or a value that is missing or is itself an option.
 */
export function parseArgs(argv: string[]): Record<(typeof OPTIONS)[number], string> | undefined {
  const got = new Map<string, string>();
  for (let i = 0; i < argv.length; i += 2) {
    const name = argv[i].startsWith("--") ? argv[i].slice(2) : "";
    const value = argv[i + 1];
    if (!(OPTIONS as readonly string[]).includes(name) || got.has(name)) return undefined;
    if (value === undefined || value === "" || value.startsWith("--")) return undefined;
    got.set(name, value);
  }
  if (got.size !== OPTIONS.length) return undefined;
  return Object.fromEntries(got) as Record<(typeof OPTIONS)[number], string>;
}

function main(argv: string[]): number {
  const args = parseArgs(argv);
  if (!args || !/^\d+$/.test(args.seed)) {
    console.error("usage: node dataset.ts --shape <chain|tree4|stress|tree4seq|tree4long> --size <n> --seed <u64> --out <dir>");
    return EXIT_USAGE;
  }
  const { shape, size, seed, out } = args;
  let d: BenchDataset;
  try {
    checkInputs(shape, Number(size), BigInt(seed));
    d = buildDataset(shape, Number(size), BigInt(seed));
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_USAGE;
  }
  const json = datasetJson(d);
  const hash = createHash("sha256").update(json).digest("hex");
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "dataset.json"), json);
  writeFileSync(join(out, "dataset.json.sha256"), `${hash}  dataset.json\n`);
  console.log(`${d.shape} (${d.source}) size ${d.size} seed ${d.seed}: ${d.data.length} tasks, ${d.links.length} links, sha256 ${hash}`);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
