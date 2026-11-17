// Hands one benchmark schedule to the DHTMLX community side, as the same schedule our chart opens.
//
//   node demos/bench/sides/dhtmlx-adapt.ts <dataset.json> <out-dir>
//
// <dataset.json> is what `dataset.ts` writes; it is refused, and nothing is written, unless its
// `dataset.json.sha256` names its bytes, so the side loads the schedule that hash names. Our own
// side's file is `ours-adapt.ts`'s (MS Project XML from `demos/dhtmlx-compare/to-mspdi.ts`), and
// Bryntum's is `bryntum-adapt.ts`'s; this writes the DHTMLX one, a pure function of the dataset,
// each with its SHA-256 beside it (`sha256sum` format), all written together or not at all:
// - `dhtmlx.json`: what a DHTMLX community page loads. `data` and `links` go to `gantt.parse`;
//   `datasetSha256` is the dataset's hash, which the page takes out and the side reports, as
//   Bryntum's adapter does; `config` is what the page sets first (`gantt.config[key] = value`:
//   `date_format` "%d-%m-%Y %H:%i", `duration_unit` "minute");
// - `expected.json`: SAMPLE leaves, evenly spaced, with the dates handed to DHTMLX, which the side's
//   `spotCheck` compares with DHTMLX's own task store at every run's ready frame.
// Write them into the cell's directory of the side's scratch directory, `<scratch>/cells/<shape>-<size>-<seed>`.
//
// The same schedule as ours (`toMspdi` of the same dataset), not only the same records. DHTMLX's
// community edition (MIT) does not schedule and has no working-time calendars: its readme's
// "Community vs PRO" table marks "Working-time calendars (setWorkTime, custom work calendars)" PRO
// only, and in 10.0.3 `setWorkTime` returns true and changes nothing (Saturday and 22:00 are work
// time, `calculateEndDate` adds calendar minutes). So, as with SVAR's MIT edition, it is handed both
// dates of every task and draws them: the dates our engine computes, the forward pass of `plan.ts`
// over the dataset's links and its leaves' starts (start-no-earlier-than, as in our file), on our
// calendar (Monday to Friday, 08:00 to 16:00). With a start and an end, DHTMLX keeps both and works
// out the duration from them, in calendar minutes; no work-time setting is made, so nothing of
// DHTMLX's moves a date. Per task: `start_date` and `end_date` (minutes, because most `stress` tasks
// start a minute past the hour, at 08:01 and the like); `parent` the dataset's, 0 at the root;
// `open`, so every task is a row. A leaf of no work is `type: "milestone"` (ours:
// `<Milestone>1</Milestone>`), its end its start. A summary is `type: "project"`, given its
// children's span (from their earliest start to their latest finish), which is also the span DHTMLX
// rolls a project up to.
// Links keep their ids and type codes, which are DHTMLX's already ("0" finish-to-start, "1"
// start-to-start, "2" finish-to-finish, "3" start-to-finish), with lag 0. A dataset with no `start`
// starts on `to-mspdi.ts`'s DEFAULT_START on both sides.
// The task and link counts are read back out of the output, not from the input, and must equal the
// dataset's, or nothing is written.
// Exit code: 0 done; 1 the dataset, its hash or the counts disagree (the message says how); 64 usage.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_START, type Dataset } from "../../dhtmlx-compare/to-mspdi.ts";
import { finishKey, forwardPass, stampKey, workMinutes } from "../plan.ts";
import { checkHash } from "./bryntum-adapt.ts";
import { sample } from "./bryntum-dates.ts";

export const EXIT_MISMATCH = 1;
export const EXIT_USAGE = 64;

/** The output file name, inside <out-dir>. */
export const DHTMLX_FILE = "dhtmlx.json";

/** The sample of leaves the side's spot check reads. */
export const EXPECTED_FILE = "expected.json";

/** `gantt.config` keys the page sets before `gantt.parse`. No `work_time`: the community edition has no calendars. */
export const DHTMLX_CONFIG = { date_format: "%d-%m-%Y %H:%i", duration_unit: "minute" } as const;

export interface DhtmlxTask {
  id: number | string;
  text: string;
  /** `%d-%m-%Y %H:%i`. */
  start_date: string;
  /** `%d-%m-%Y %H:%i`; a milestone's is its start. */
  end_date: string;
  /** 0 at the root. */
  parent: number | string;
  open: true;
  type?: "project" | "milestone";
}

export interface DhtmlxLink {
  id: number | string;
  source: number | string;
  target: number | string;
  type: string;
  lag: 0;
}

export interface DhtmlxData {
  datasetSha256?: string;
  config: typeof DHTMLX_CONFIG;
  data: DhtmlxTask[];
  links: DhtmlxLink[];
}

/** The record counts one side loads. */
export interface Counts {
  tasks: number;
  links: number;
}

/** YYYY-MM-DDTHH:MM to DHTMLX's `DD-MM-YYYY HH:MM`. */
export const dhtmlxDate = (at: string) => `${at.slice(8, 10)}-${at.slice(5, 7)}-${at.slice(0, 4)} ${at.slice(11, 16)}`;

/** Working minutes `m` of a key: a day end counts as the minute that ends the day. */
const minuteOf = (key: number) => Math.ceil(key / 2);

/** The dataset in the DHTMLX community format: same ids, parents and links, in the same order, dated. */
export function toDhtmlx(d: Dataset, datasetSha256?: string): DhtmlxData {
  const start = d.start ?? DEFAULT_START;
  const es = forwardPass(start, d.data, d.links);
  const byId = new Map(d.data.map((t) => [String(t.id), t]));
  const kids = new Map<string, string[]>();
  for (const t of d.data) {
    if (t.parent === undefined || String(t.parent) === "0") continue;
    const p = String(t.parent);
    if (!byId.has(p)) throw new Error(`task ${t.id} has unknown parent ${t.parent}`);
    kids.set(p, [...(kids.get(p) ?? []), String(t.id)]);
  }
  const span = new Map<string, [number, number]>();
  const spanOf = (id: string): [number, number] => {
    const known = span.get(id);
    if (known) return known;
    const below = kids.get(id);
    const t = byId.get(id)!;
    const s = below
      ? below.map(spanOf).reduce(([a, b], [c, e]) => [Math.min(a, c), Math.max(b, e)])
      : ([es.get(id)!, finishKey(es.get(id)!, workMinutes(t))] as [number, number]);
    span.set(id, s);
    return s;
  };
  const data = d.data.map((t): DhtmlxTask => {
    const id = String(t.id);
    const [s, f] = spanOf(id);
    const minutes = minuteOf(f) - minuteOf(s);
    const at = dhtmlxDate(stampKey(start, s));
    const row: DhtmlxTask = { id: t.id, text: t.text, start_date: at, end_date: minutes === 0 ? at : dhtmlxDate(stampKey(start, f)), parent: t.parent ?? 0, open: true };
    if (kids.has(id)) row.type = "project";
    else if (minutes === 0) row.type = "milestone";
    return row;
  });
  const links = d.links.map((l): DhtmlxLink => ({ id: l.id, source: l.source, target: l.target, type: String(l.type), lag: 0 }));
  return { ...(datasetSha256 === undefined ? {} : { datasetSha256 }), config: DHTMLX_CONFIG, data, links };
}

/** DHTMLX's `DD-MM-YYYY HH:MM` back to YYYY-MM-DDTHH:MM. */
export const isoDate = (d: string) => `${d.slice(6, 10)}-${d.slice(3, 5)}-${d.slice(0, 2)}T${d.slice(11)}`;

/**
 * Each task's start and finish (YYYY-MM-DDTHH:MM) by name, as handed to DHTMLX. The side's spot
 * check reads DHTMLX's store against a sample of them at every run (`expected.json`).
 */
export function dhtmlxDates(x: DhtmlxData): Map<string, [string, string]> {
  return new Map(x.data.map((t) => [t.text, [isoDate(t.start_date), isoDate(t.end_date)]]));
}

/** A sampled leaf and the dates handed to DHTMLX for it (YYYY-MM-DDTHH:MM; a milestone's end is its start). */
export interface Expected {
  id: number | string;
  start: string;
  end: string;
}

/** SAMPLE leaves, evenly spaced in the dataset's order, with the dates handed to DHTMLX. */
export function expectedOf(x: DhtmlxData): Expected[] {
  return sample(x.data.filter((t) => t.type !== "project")).map((t) => ({ id: t.id, start: isoDate(t.start_date), end: isoDate(t.end_date) }));
}

/** The bytes of dhtmlx.json: fixed key order, one record per line. */
export function dhtmlxJson(x: DhtmlxData): string {
  const list = (rows: object[]) => (rows.length === 0 ? "[]" : `[\n${rows.map((r) => JSON.stringify(r)).join(",\n")}\n]`);
  const head = JSON.stringify({ datasetSha256: x.datasetSha256, config: x.config });
  return `${head.slice(0, -1)},\n"data":${list(x.data)},\n"links":${list(x.links)}}\n`;
}

/** The records `gantt.parse` reads from the DHTMLX JSON. */
export function countDhtmlx(json: string): Counts {
  const x = JSON.parse(json) as DhtmlxData;
  return { tasks: x.data.length, links: x.links.length };
}

/** Throw unless every output holds the dataset's task and link counts. */
export function assertSameCounts(want: Counts, sides: Record<string, Counts>): void {
  for (const [side, got] of Object.entries(sides)) {
    if (got.tasks !== want.tasks || got.links !== want.links) {
      throw new Error(`${side} holds ${got.tasks} tasks and ${got.links} links; the dataset has ${want.tasks} and ${want.links}`);
    }
  }
}

export interface Adapted {
  dhtmlx: string;
  expected: string;
  counts: Counts;
}

/** The DHTMLX file for `d`, checked: the counts read back from it equal the dataset's. */
export function adapt(d: Dataset, datasetSha256?: string): Adapted {
  const x = toDhtmlx(d, datasetSha256);
  const dhtmlx = dhtmlxJson(x);
  const counts = { tasks: d.data.length, links: d.links.length };
  assertSameCounts(counts, { dhtmlx: countDhtmlx(dhtmlx) });
  return { dhtmlx, expected: `${JSON.stringify(expectedOf(x))}\n`, counts };
}

const sha256 = (s: string | Buffer) => createHash("sha256").update(s).digest("hex");

/** The file operations `writeAll` uses; a test swaps one for one that fails. */
export interface WriteOps {
  write: (path: string, text: string) => void;
  rename: (from: string, to: string) => void;
}

const FS: WriteOps = { write: writeFileSync, rename: renameSync };

/**
 * Write every file and its `.sha256` all or nothing: each under a temporary name, then any file
 * already there moved aside, then each renamed into place. A failure at any step puts back what was
 * there before (the earlier files, or none) and removes the temporary files.
 */
export function writeAll(dir: string, files: [string, string][], ops: WriteOps = FS): void {
  const all = files.flatMap(([name, text]): [string, string][] => [[name, text], [`${name}.sha256`, `${sha256(text)}  ${name}\n`]]);
  const at = (name: string) => join(dir, name);
  const tmp = (name: string) => join(dir, `.${name}.tmp`);
  const old = (name: string) => join(dir, `.${name}.old`);
  const aside: string[] = [];
  const placed: string[] = [];
  try {
    for (const [name, text] of all) ops.write(tmp(name), text);
    for (const [name] of all) {
      if (!existsSync(at(name))) continue;
      ops.rename(at(name), old(name));
      aside.push(name);
    }
    for (const [name] of all) {
      ops.rename(tmp(name), at(name));
      placed.push(name);
    }
  } catch (e) {
    for (const name of placed) rmSync(at(name), { force: true });
    for (const name of aside) renameSync(old(name), at(name));
    for (const [name] of all) rmSync(tmp(name), { force: true });
    throw e;
  }
  for (const name of aside) rmSync(old(name), { force: true });
}

function main(argv: string[]): number {
  if (argv.length !== 2 || argv.some((a) => a.startsWith("--"))) {
    console.error("usage: node dhtmlx-adapt.ts <dataset.json> <out-dir>");
    return EXIT_USAGE;
  }
  const [input, out] = argv;
  try {
    const bytes = readFileSync(input);
    checkHash(input, bytes);
    const hash = sha256(bytes);
    const a = adapt(JSON.parse(bytes.toString("utf8")) as Dataset, hash);
    mkdirSync(out, { recursive: true });
    writeAll(out, [
      [DHTMLX_FILE, a.dhtmlx],
      [EXPECTED_FILE, a.expected],
    ]);
    console.log(`dataset sha256 ${hash}: ${a.counts.tasks} tasks, ${a.counts.links} links in ${DHTMLX_FILE}, ${EXPECTED_FILE} beside it`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_MISMATCH;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
