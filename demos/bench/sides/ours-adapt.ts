// The benchmark schedule (`demos/bench/dataset.ts`) as the file our bench page hands to `mount(el, { tasks, links })`.
//
//   node demos/bench/sides/ours-adapt.ts <dataset.json> <bundle-dir>
//
// <bundle-dir> is the built bench page (`ours-vite.config.ts`, the directory `pair.ts` serves). Into its
// `cells/<shape>-<size>-<seed>/` this writes `ours.json` and its SHA-256 (`ours.json.sha256`, `sha256sum` format):
// `{ datasetSha256, tasks, links }`, the tasks and links in the shape `mount` takes (`PlainTask`, `PlainLink`).
// The dataset is refused, and nothing is written, unless its `dataset.json.sha256` names its bytes.
//
// The same JSON the DHTMLX side loads (`toDhtmlx`: every task dated with both its start and its end, the dates our
// engine computes, summaries spanning their children), in the shape `mount` reads: `name` for `text`, ISO dates, a
// link's `from` and `to` and its type as `FS`, `SS`, `FF` or `SF`. A leaf also carries its duration in working days,
// as the dataset has it, because `mount` would otherwise work a duration out from the calendar span of its two dates,
// which is longer across a weekend. `mount` reads each start as a start-no-earlier-than and the engine schedules the
// tasks on open, so ours solves the schedule it is given; the dates it is given are the ones it computes.
// Exit code: 0 done; 1 the dataset is inconsistent or its hash does not match; 64 usage.

import { createHash } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import type { PlainLink, PlainTask } from "../../../packages/gantt/src/adapters/plain.ts";
import type { Dataset } from "../../dhtmlx-compare/to-mspdi.ts";
import type { BenchDataset } from "../dataset.ts";
import { checkHash } from "./bryntum-adapt.ts";
import { isoDate, toDhtmlx, writeAll } from "./dhtmlx-adapt.ts";

export const EXIT_BAD_DATASET = 1;
export const EXIT_USAGE = 64;

/** What the bench page fetches. */
export interface OursData {
  datasetSha256: string;
  tasks: PlainTask[];
  links: PlainLink[];
}

const LINK_TYPES = ["FS", "SS", "FF", "SF"] as const;

/** The dataset as plain tasks and links, dated as `toDhtmlx` dates it. */
export function toPlain(d: Dataset, datasetSha256: string): OursData {
  const { data, links } = toDhtmlx(d, datasetSha256);
  const work = new Map(d.data.map((t) => [String(t.id), t.duration]));
  const keys = new Map(d.data.flatMap((t) => (t.key === undefined ? [] : [[String(t.id), t.key]])));
  const tasks = data.map((t): PlainTask => ({
    id: t.id,
    name: t.text,
    start: isoDate(t.start_date),
    end: isoDate(t.end_date),
    ...(String(t.parent) === "0" ? {} : { parent: t.parent }),
    ...(keys.has(String(t.id)) ? { key: keys.get(String(t.id)) } : {}),
    ...(t.type === undefined ? { duration: work.get(String(t.id)) } : { type: t.type }),
  }));
  return { datasetSha256, tasks, links: links.map((l) => ({ from: l.source, to: l.target, type: LINK_TYPES[Number(l.type)] })) };
}

/** The cell's directory under the bundle, as the page's `?cell=` names it. */
export const cellDir = (bundle: string, d: Pick<BenchDataset, "shape" | "size" | "seed">) => join(bundle, "cells", `${d.shape}-${d.size}-${d.seed}`);

/** Write the cell's `ours.json` and its hash into `bundle`, from the dataset at `input`; the directory and the data. */
export function adaptInto(input: string, bundle: string): { dir: string; data: OursData } {
  const bytes = readFileSync(input);
  checkHash(input, bytes);
  const dataset = JSON.parse(bytes.toString("utf8")) as BenchDataset;
  const data = toPlain(dataset, createHash("sha256").update(bytes).digest("hex"));
  if (data.tasks.length !== dataset.data.length || data.links.length !== dataset.links.length) {
    throw new Error(`ours holds ${data.tasks.length} tasks and ${data.links.length} links; the dataset has ${dataset.data.length} and ${dataset.links.length}`);
  }
  const dir = cellDir(bundle, dataset);
  mkdirSync(dir, { recursive: true });
  writeAll(dir, [["ours.json", `${JSON.stringify(data)}\n`]]);
  return { dir, data };
}

function main(argv: string[]): number {
  if (argv.length !== 2) {
    console.error("usage: node ours-adapt.ts <dataset.json> <bundle-dir>");
    return EXIT_USAGE;
  }
  try {
    const { dir, data } = adaptInto(argv[0], argv[1]);
    console.log(`${dir}: ${data.tasks.length} tasks, ${data.links.length} links, dataset sha256 ${data.datasetSha256}`);
    return 0;
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_BAD_DATASET;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
