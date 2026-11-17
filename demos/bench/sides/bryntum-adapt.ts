// The benchmark schedule (`demos/bench/dataset.ts`) as the project data the Bryntum side loads.
//
//   node demos/bench/sides/bryntum-adapt.ts <dataset.json> <scratch-dir>
//
// Writes <scratch-dir>/cells/<shape>-<size>-<seed>/bryntum.json and its SHA-256 (`bryntum.json.sha256`,
// `sha256sum` format), the file the side's page (`bryntum-page.js`) fetches for `?cell=<shape>-<size>-<seed>`,
// with the dataset's own SHA-256 inside (`datasetSha256`), which the side reports to the table. The dataset is refused, and nothing is
// written, unless its `dataset.json.sha256` names these bytes, so the side loads the schedule that
// hash names. The output is a pure function of the dataset: the same records give the same bytes.
//
// The same schedule as ours (`demos/dhtmlx-compare/to-mspdi.ts`), not only the same records: the
// project calendar works Monday to Friday, 08:00 to 16:00, and nothing else (8 hours a day, 5 days a
// week); a leaf's duration is in hours (the dataset's working days times 8); a leaf's `start` is a
// start-no-earlier-than constraint at 08:00 of that day; every link keeps its type with no lag; the
// project starts at 08:00 of the dataset's `start` (`to-mspdi.ts`'s DEFAULT_START when it has none, as ours). Bryntum's own engine then places every task from
// its constraint and its links, as ours does, so the dates agree with the links on both sides.
// Summaries take their dates from their children and are expanded, so every task is a row.
// Exit code: 0 done; 1 the dataset is inconsistent or its hash does not match; 64 usage.

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_START } from "../../dhtmlx-compare/to-mspdi.ts";
import type { BenchDataset } from "../dataset.ts";

export const EXIT_BAD_DATASET = 1;
export const EXIT_USAGE = 64;

export const HOURS_PER_DAY = 8;
export const CALENDAR_ID = "work";
const DAY_START = "T08:00:00";
const DAY_MS = 86_400_000;
/** Bryntum's own default for how far a calendar may iterate: about 5 years. */
const DEFAULT_RANGE_DAYS = 5 * 365;

/** The dataset's link type ("0" FS, "1" SS, "2" FF, "3" SF) to Bryntum's DependencyType (0 SS, 1 SF, 2 FS, 3 FF). */
export const LINK_TYPE: Record<string, number> = { "0": 2, "1": 0, "2": 3, "3": 1 };

export interface BryntumTask {
  id: number | string;
  name: string;
  expanded?: true;
  children?: BryntumTask[];
  duration?: number;
  durationUnit?: "hour";
  constraintType?: "startnoearlierthan";
  constraintDate?: string;
  /** Only in `bryntum-dated.json`: the dates Bryntum's engine wrote back on a first pass. */
  startDate?: string;
  endDate?: string;
}

export interface BryntumProject {
  /** The SHA-256 of the dataset.json it was made from; the page takes it out before Bryntum sees the data. */
  datasetSha256?: string;
  startDate: string;
  calendar: string;
  hoursPerDay: number;
  daysPerWeek: number;
  daysPerMonth: number;
  maxCalendarRange: number;
  calendars: object[];
  tasks: BryntumTask[];
  dependencies: { id: number | string; fromTask: number | string; toTask: number | string; type: number; lag: 0 }[];
}

function day(date: string, what: string): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(date) || Number.isNaN(Date.parse(`${date}T00:00:00Z`))) {
    throw new Error(`${what} has start ${JSON.stringify(date)}, not a YYYY-MM-DD date`);
  }
  return `${date}${DAY_START}`;
}

/** Monday to Friday, 08:00 to 16:00; all other time is not working. */
export const CALENDAR = {
  id: CALENDAR_ID,
  name: "Mon-Fri 08:00-16:00",
  unspecifiedTimeIsWorking: false,
  intervals: [{ recurrentStartDate: "every weekday at 08:00", recurrentEndDate: "every weekday at 16:00", isWorking: true }],
};

/**
 * How far the calendar must iterate: from the project start past the latest leaf constraint plus
 * all the work done end to end, in calendar days (5 working days in 7, a week of slack). Bryntum's
 * 5-year default stops a long chain, and a schedule it cannot place would be no result at all.
 */
export function calendarRangeMs(d: Pick<BenchDataset, "start" | "data">): number {
  const t0 = Date.parse(`${d.start ?? DEFAULT_START}T00:00:00Z`);
  let latest = 0;
  let workDays = 0;
  for (const t of d.data) {
    if (t.start !== undefined) latest = Math.max(latest, (Date.parse(`${t.start}T00:00:00Z`) - t0) / DAY_MS);
    workDays += t.duration;
  }
  return Math.max(DEFAULT_RANGE_DAYS, Math.ceil(latest + (workDays * 7) / 5 + 7)) * DAY_MS;
}

/** The project data for `d`: tasks nested under their parents in the dataset's order. */
export function toBryntum(d: Pick<BenchDataset, "start" | "data" | "links">, community = false): BryntumProject {
  const byId = new Map<string, BryntumTask>();
  const hasChild = new Set(d.data.flatMap((t) => (t.parent === undefined || t.parent === 0 || t.parent === "0" ? [] : [String(t.parent)])));
  for (const t of d.data) {
    if (byId.has(String(t.id))) throw new Error(`duplicate task ${t.id}`);
    const summary = hasChild.has(String(t.id));
    if (summary && t.start !== undefined) throw new Error(`summary task ${t.id} has a start; only a leaf can`);
    const b: BryntumTask = { id: t.id, name: t.text };
    if (summary) {
      b.expanded = true;
      b.children = [];
    } else {
      b.duration = t.duration * HOURS_PER_DAY;
      b.durationUnit = "hour";
      // The Community tier has no constraints (and no working calendar): the leaf's start is given as its start date.
      if (t.start !== undefined && community) b.startDate = day(t.start, `task ${t.id}`);
      else if (t.start !== undefined) {
        b.constraintType = "startnoearlierthan";
        b.constraintDate = day(t.start, `task ${t.id}`);
      }
    }
    byId.set(String(t.id), b);
  }
  const roots: BryntumTask[] = [];
  for (const t of d.data) {
    const b = byId.get(String(t.id))!;
    if (t.parent === undefined || t.parent === 0 || t.parent === "0") roots.push(b);
    else {
      const p = byId.get(String(t.parent));
      if (p === undefined) throw new Error(`task ${t.id} has unknown parent ${t.parent}`);
      p.children!.push(b);
    }
  }
  const dependencies = d.links.map((l) => {
    for (const end of [l.source, l.target]) if (!byId.has(String(end))) throw new Error(`link ${l.id} names unknown task ${end}`);
    const type = LINK_TYPE[String(l.type)];
    if (type === undefined) throw new Error(`link ${l.id} has unknown type ${JSON.stringify(l.type)}`);
    return { id: l.id, fromTask: l.source, toTask: l.target, type, lag: 0 as const };
  });
  return {
    startDate: day(d.start ?? DEFAULT_START, "the dataset"),
    calendar: CALENDAR_ID,
    hoursPerDay: HOURS_PER_DAY,
    daysPerWeek: 5,
    daysPerMonth: 20,
    maxCalendarRange: calendarRangeMs(d),
    calendars: [CALENDAR],
    tasks: roots,
    dependencies,
  };
}

/** Every task in the nested data, summaries included. */
export function countTasks(tasks: BryntumTask[]): number {
  return tasks.reduce((n, t) => n + 1 + countTasks(t.children ?? []), 0);
}

/** The bytes of bryntum.json. */
export const bryntumJson = (p: BryntumProject) => `${JSON.stringify(p)}\n`;

const sha256 = (bytes: string | Buffer) => createHash("sha256").update(bytes).digest("hex");

/** Throw unless `<file>.sha256` exists and names the file's bytes. */
export function checkHash(file: string, bytes: Buffer): void {
  const sidecar = `${file}.sha256`;
  if (!existsSync(sidecar)) throw new Error(`${sidecar} is missing: write the dataset with demos/bench/dataset.ts`);
  const named = readFileSync(sidecar, "utf8").trim().split(/\s+/)[0];
  if (named !== sha256(bytes)) throw new Error(`${sidecar} names ${named}, not these bytes (${sha256(bytes)})`);
}

/** The cell's directory under the scratch directory, as the page's `?cell=` names it: `cells/<shape>-<size>-<seed>`. */
export const cellDir = (scratch: string, d: Pick<BenchDataset, "shape" | "size" | "seed">) => join(scratch, "cells", `${d.shape}-${d.size}-${d.seed}`);

/**
 * Write the cell's `bryntum.json` and its hash from the dataset at `input` (refused unless its
 * `.sha256` names its bytes), with that dataset hash inside as `datasetSha256`, which the side
 * reports so the table can check it is the cell's. Returns the dataset, the project and the cell's directory.
 */
export function adaptInto(input: string, scratch: string, community = false): { dataset: BenchDataset; project: BryntumProject; hash: string; dir: string } {
  const bytes = readFileSync(input);
  checkHash(input, bytes);
  const dataset = JSON.parse(bytes.toString("utf8")) as BenchDataset;
  const project: BryntumProject = { datasetSha256: sha256(bytes), ...toBryntum(dataset, community) };
  const tasks = countTasks(project.tasks);
  if (tasks !== dataset.data.length || project.dependencies.length !== dataset.links.length) {
    throw new Error(`bryntum holds ${tasks} tasks and ${project.dependencies.length} links; the dataset has ${dataset.data.length} and ${dataset.links.length}`);
  }
  const dir = cellDir(scratch, dataset);
  mkdirSync(dir, { recursive: true });
  const hash = writeHashed(dir, "bryntum.json", bryntumJson(project));
  return { dataset, project, hash, dir };
}

/** Write `<dir>/<name>` and `<name>.sha256` (`sha256sum` format); the hash. */
export function writeHashed(dir: string, name: string, text: string): string {
  const hash = sha256(text);
  writeFileSync(join(dir, name), text);
  writeFileSync(join(dir, `${name}.sha256`), `${hash}  ${name}\n`);
  return hash;
}

function main(argv: string[]): number {
  const community = argv[0] === "--community";
  const args = community ? argv.slice(1) : argv;
  if (args.length !== 2) {
    console.error("usage: node bryntum-adapt.ts [--community] <dataset.json> <scratch-dir>");
    return EXIT_USAGE;
  }
  try {
    const { project, hash, dir } = adaptInto(args[0], args[1], community);
    console.log(`${dir}: ${countTasks(project.tasks)} tasks, ${project.dependencies.length} links, sha256 ${hash}`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_BAD_DATASET;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
