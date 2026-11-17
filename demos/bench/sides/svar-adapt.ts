// The benchmark schedule (`demos/bench/dataset.ts`) as the tasks and links the SVAR Gantt side loads.
//
//   node demos/bench/sides/svar-adapt.ts <dataset.json> <scratch-dir>
//
// Writes, into <scratch-dir>/cells/<shape>-<size>-<seed>/ (the directory the side's page,
// `svar-page.js`, reads for `?cell=<shape>-<size>-<seed>`), all or nothing, each with its SHA-256
// beside it (`sha256sum` format):
// - `svar.json`: `tasks` and `links` for SVAR's `Gantt`, and `datasetSha256`, the dataset's own
//   SHA-256, which the page takes out before SVAR sees the data and the side reports to the table;
// - `expected.json`: SAMPLE leaves, evenly spaced, with the dates handed to SVAR, which the side's
//   `spotCheck` compares with SVAR's own task store at every run's ready frame.
// The dataset is refused, and nothing is written, unless its `dataset.json.sha256` names its bytes.
// The output is a pure function of the dataset: the same records give the same bytes.
//
// The same schedule as ours, not only the same records. SVAR's MIT edition does not schedule:
// auto-scheduling and the working-time calendar are its PRO edition's (the package's readme, "PRO
// Edition"), so, as with the DHTMLX community edition, it draws the dates it is given. The dates
// handed to it are our engine's: the forward pass of `plan.ts` over the links and each leaf's
// start-no-earlier-than, on our calendar (Monday to Friday, 08:00 to 16:00). Each task gets its
// `start` and `end` as local wall-clock times (YYYY-MM-DDTHH:MM, which the page makes Dates of):
// SVAR keeps both and works out `duration` from them (with a start and a duration it would add
// calendar days, which skip no weekend). A leaf of no work is a `milestone` (its start only); a
// summary is a `summary` spanning its children, `open`, so every task is a row. Links keep their ids
// and get SVAR's type names ("e2s" finish-to-start, "s2s", "e2e", "s2e") with no lag. The task and
// link counts are read back out of `svar.json` and must equal the dataset's, or nothing is written.
// Exit code: 0 done; 1 the dataset, its hash or the counts disagree; 64 usage.

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { DEFAULT_START } from "../../dhtmlx-compare/to-mspdi.ts";
import type { BenchDataset } from "../dataset.ts";
import { finishKey, forwardPass, stampKey, workMinutes } from "../plan.ts";
import { cellDir, checkHash } from "./bryntum-adapt.ts";
import { sample } from "./bryntum-dates.ts";
import { writeAll } from "./dhtmlx-adapt.ts";

export const EXIT_MISMATCH = 1;
export const EXIT_USAGE = 64;

export const SVAR_FILE = "svar.json";
export const EXPECTED_FILE = "expected.json";

/** The dataset's link type ("0" FS, "1" SS, "2" FF, "3" SF) to SVAR's link type. */
export const LINK_TYPE: Record<string, string> = { "0": "e2s", "1": "s2s", "2": "e2e", "3": "s2e" };

export interface SvarTask {
  id: number | string;
  text: string;
  /** YYYY-MM-DDTHH:MM, local time. */
  start: string;
  /** YYYY-MM-DDTHH:MM, local time; none for a milestone. */
  end?: string;
  /** 0 at the root. */
  parent: number | string;
  type: "task" | "summary" | "milestone";
  open?: true;
}

export interface SvarLink {
  id: number | string;
  source: number | string;
  target: number | string;
  type: string;
}

export interface SvarData {
  datasetSha256?: string;
  tasks: SvarTask[];
  links: SvarLink[];
}

/** A sampled leaf and the dates handed to SVAR for it (a milestone's end is its start). */
export interface Expected {
  id: number | string;
  start: string;
  end: string;
}

const isRoot = (p: number | string | undefined) => p === undefined || String(p) === "0";

/** The dataset as SVAR's tasks and links: same ids, parents and order, dated by our engine. */
export function toSvar(d: Pick<BenchDataset, "start" | "data" | "links">, datasetSha256?: string): SvarData {
  const start = d.start ?? DEFAULT_START;
  const es = forwardPass(start, d.data, d.links);
  const byId = new Map(d.data.map((t) => [String(t.id), t]));
  const kids = new Map<string, string[]>();
  for (const t of d.data) {
    if (isRoot(t.parent)) continue;
    const p = String(t.parent);
    if (!byId.has(p)) throw new Error(`task ${t.id} has unknown parent ${t.parent}`);
    kids.set(p, [...(kids.get(p) ?? []), String(t.id)]);
  }
  const span = new Map<string, [number, number]>();
  const spanOf = (id: string): [number, number] => {
    const known = span.get(id);
    if (known) return known;
    const below = kids.get(id);
    const s: [number, number] = below
      ? below.map(spanOf).reduce(([a, b], [c, e]) => [Math.min(a, c), Math.max(b, e)])
      : [es.get(id)!, finishKey(es.get(id)!, workMinutes(byId.get(id)!))];
    span.set(id, s);
    return s;
  };
  const tasks = d.data.map((t): SvarTask => {
    const id = String(t.id);
    const [s, f] = spanOf(id);
    const type = kids.has(id) ? "summary" : workMinutes(t) === 0 ? "milestone" : "task";
    return {
      id: t.id,
      text: t.text,
      start: stampKey(start, s),
      ...(type === "milestone" ? {} : { end: stampKey(start, f) }),
      parent: t.parent ?? 0,
      type,
      ...(type === "summary" ? { open: true as const } : {}),
    };
  });
  const links = d.links.map((l): SvarLink => {
    const type = LINK_TYPE[String(l.type)];
    if (type === undefined) throw new Error(`link ${l.id} has unknown type ${JSON.stringify(l.type)}`);
    return { id: l.id, source: l.source, target: l.target, type };
  });
  return { ...(datasetSha256 === undefined ? {} : { datasetSha256 }), tasks, links };
}

/** SAMPLE leaves, evenly spaced in the dataset's order, with the dates handed to SVAR. */
export function expectedOf(x: SvarData): Expected[] {
  return sample(x.tasks.filter((t) => t.type !== "summary")).map((t) => ({ id: t.id, start: t.start, end: t.end ?? t.start }));
}

/** The bytes of svar.json: fixed key order, one record per line. */
export function svarJson(x: SvarData): string {
  const list = (rows: object[]) => (rows.length === 0 ? "[]" : `[\n${rows.map((r) => JSON.stringify(r)).join(",\n")}\n]`);
  return `{"datasetSha256":${JSON.stringify(x.datasetSha256)},\n"tasks":${list(x.tasks)},\n"links":${list(x.links)}}\n`;
}

const sha256 = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");

/**
 * Write the cell's `svar.json` and `expected.json` (each with its `.sha256`) from the dataset at
 * `input`, refused unless its `.sha256` names its bytes and the counts read back out of `svar.json`
 * are the dataset's. Returns the cell's directory, the counts and svar.json's hash.
 */
export function adaptInto(input: string, scratch: string): { dir: string; tasks: number; links: number; hash: string } {
  const bytes = readFileSync(input);
  checkHash(input, bytes);
  const dataset = JSON.parse(bytes.toString("utf8")) as BenchDataset;
  const x = toSvar(dataset, sha256(bytes));
  const json = svarJson(x);
  const back = JSON.parse(json) as SvarData;
  if (back.tasks.length !== dataset.data.length || back.links.length !== dataset.links.length) {
    throw new Error(`svar holds ${back.tasks.length} tasks and ${back.links.length} links; the dataset has ${dataset.data.length} and ${dataset.links.length}`);
  }
  const dir = cellDir(scratch, dataset);
  mkdirSync(dir, { recursive: true });
  writeAll(dir, [
    [SVAR_FILE, json],
    [EXPECTED_FILE, `${JSON.stringify(expectedOf(x))}\n`],
  ]);
  return { dir, tasks: back.tasks.length, links: back.links.length, hash: sha256(json) };
}

function main(argv: string[]): number {
  if (argv.length !== 2 || argv.some((a) => a.startsWith("--"))) {
    console.error("usage: node svar-adapt.ts <dataset.json> <scratch-dir>");
    return EXIT_USAGE;
  }
  try {
    const r = adaptInto(argv[0], argv[1]);
    console.log(`${r.dir}: ${r.tasks} tasks, ${r.links} links, ${SVAR_FILE} sha256 ${r.hash}`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_MISMATCH;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
