// Check that Bryntum places the benchmark schedule where ours does, and write what the page's two
// variants need. Trial-gated: it runs the trial `bryntum-setup.ts` installed, so it needs it.
//
//   node demos/bench/sides/bryntum-dates.ts <dataset.json> <scratch-dir> [--chrome <path>]
//
// 1. Writes the cell's `bryntum.json` (`bryntum-adapt.ts`): `cells/<shape>-<size>-<seed>/` of the scratch directory.
// 2. Our dates: the file `to-mspdi.ts` writes for the dataset, scheduled by our engine
//    (`cargo run -p import --example mspdi_dates`).
// 3. Bryntum's dates: the page opened headless (dates need no GPU), and every task's start and end
//    read back once the initial commit has placed them.
// 4. Compares every leaf's start and finish, to the minute. A difference over one minute exits 1 and
//    writes nothing more: the setup is broken, and no run may give a number. Differences of one
//    minute are listed (docs/MEASURING.md, "The Bryntum side": a start-to-finish edge case).
// 5. Writes, beside it, `bryntum-dated.json` (the same tasks with the dates Bryntum's engine wrote back, the
//    `?dates` variant) and `expected.json` (SAMPLE leaves, evenly spaced, with our dates, which the
//    side's `spotCheck` compares at every run's ready frame).
// Exit code: 0 the schedules agree; 1 they do not, or no trial, or a step failed; 64 usage.

import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { launchRetrying } from "../../../scripts/lib/launch-retrying.ts";
import { toMspdi } from "../../dhtmlx-compare/to-mspdi.ts";
import { serve } from "../../dhtmlx-compare/pair.ts";
import { chromium } from "../drive.ts";
import type { BenchDataset } from "../dataset.ts";
import { adaptInto, bryntumJson, writeHashed, type BryntumProject, type BryntumTask } from "./bryntum-adapt.ts";
import { ALIAS, TRIAL } from "./bryntum-setup.ts";

export const EXIT_FAILED = 1;
export const EXIT_USAGE = 64;
/** Leaves in `expected.json`. */
export const SAMPLE = 64;
/** How long the first pass may take to place every task. */
const PASS_MS = 30 * 60_000;

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "../../..");

/** `[start, end]` per task, `YYYY-MM-DDTHH:MM` wall-clock. */
export type Dates = Map<string, [string, string]>;

/** Throw unless the pinned trial is installed in `dir`. */
export function assertTrial(dir: string): void {
  const pkg = join(dir, "node_modules", ALIAS, "package.json");
  const got = existsSync(pkg) ? JSON.parse(readFileSync(pkg, "utf8")) : null;
  if (got?.name !== TRIAL.name || got?.version !== TRIAL.version) {
    throw new Error(`no ${TRIAL.name}@${TRIAL.version} at ${dir}: run bryntum-setup.ts --trial ${dir} first`);
  }
}

/** Our engine's lines (`name<TAB>start<TAB>finish`) by name; names must be unique. */
export function parseOurs(tsv: string): Dates {
  const out: Dates = new Map();
  for (const line of tsv.split("\n")) {
    if (line === "") continue;
    const [name, start, end] = line.split("\t");
    if (out.has(name)) throw new Error(`two tasks are named ${JSON.stringify(name)}: the dates cannot be matched by name`);
    out.set(name, [start, end]);
  }
  return out;
}

const minutes = (s: string) => Date.parse(`${s.slice(0, 16)}:00Z`) / 60_000;

/** Each leaf whose start or finish differs, with the larger difference in minutes. */
export function diffDates(leaves: string[], ours: Dates, theirs: Dates): { name: string; minutes: number; ours: string[]; theirs: string[] }[] {
  const out = [];
  for (const name of leaves) {
    const a = ours.get(name);
    const b = theirs.get(name);
    if (a === undefined || b === undefined) throw new Error(`task ${JSON.stringify(name)} is missing from ${a === undefined ? "ours" : "Bryntum's"}`);
    const off = Math.max(Math.abs(minutes(a[0]) - minutes(b[0])), Math.abs(minutes(a[1]) - minutes(b[1])));
    if (off !== 0) out.push({ name, minutes: off, ours: a, theirs: b });
  }
  return out;
}

/** The project with every task's `startDate` and `endDate` from `dates` (by task id). */
export function withDates(p: BryntumProject, dates: Map<string, [string, string]>): BryntumProject {
  const dated = (tasks: BryntumTask[]): BryntumTask[] =>
    tasks.map((t) => {
      const d = dates.get(String(t.id));
      if (d === undefined) throw new Error(`no dates for task ${t.id}`);
      const out: BryntumTask = { ...t, startDate: d[0], endDate: d[1] };
      if (t.children) out.children = dated(t.children);
      return out;
    });
  return { ...p, tasks: dated(p.tasks) };
}

/** SAMPLE leaves evenly spaced through the dataset's order (all of them when there are fewer). */
export function sample<T>(leaves: T[], n = SAMPLE): T[] {
  if (leaves.length <= n) return leaves;
  return Array.from({ length: n }, (_, i) => leaves[Math.floor((i * (leaves.length - 1)) / (n - 1))]);
}

/** Bryntum's dates for every task, by id and by name, from the page served out of `dir`, for `cell`. */
async function bryntumDates(dir: string, cell: string, rows: number, chrome: string | undefined): Promise<{ byId: Map<string, [string, string]>; byName: Dates }> {
  const srv = await serve(resolve(dir));
  const browser = await launchRetrying(() => chromium().launch(chrome ? { executablePath: chrome } : { channel: "chromium" }));
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    await page.goto(`${srv.url}?cell=${cell}`);
    await page.waitForFunction((n: number) => (window as any).ganttScheduledAt !== undefined && (window as any).gantt.taskStore.count === n, rows, { timeout: PASS_MS, polling: 500 });
    const rowsRead: [string, string, string, string][] = await page.evaluate(() => {
      const z = (n: number) => String(n).padStart(2, "0");
      const f = (d: Date) => `${d.getFullYear()}-${z(d.getMonth() + 1)}-${z(d.getDate())}T${z(d.getHours())}:${z(d.getMinutes())}:${z(d.getSeconds())}`;
      const out: [string, string, string, string][] = [];
      (window as any).gantt.taskStore.traverse((t: any) => out.push([String(t.id), t.name, f(t.startDate), f(t.endDate)]));
      return out;
    });
    return {
      byId: new Map(rowsRead.map(([id, , s, e]) => [id, [s, e]])),
      byName: new Map(rowsRead.map(([, name, s, e]) => [name, [s.slice(0, 16), e.slice(0, 16)]])),
    };
  } finally {
    await browser.close();
    srv.close();
  }
}

async function main(argv: string[]): Promise<number> {
  const at = argv.indexOf("--chrome");
  const chrome = at >= 0 ? argv[at + 1] : process.env.CHROME;
  const rest = at >= 0 ? argv.filter((_, i) => i !== at && i !== at + 1) : argv;
  if (rest.length !== 2 || (at >= 0 && chrome === undefined)) {
    console.error("usage: node bryntum-dates.ts <dataset.json> <scratch-dir> [--chrome <path>]");
    return EXIT_USAGE;
  }
  const [input, scratch] = rest;
  try {
    assertTrial(scratch);
    const { dataset, project, dir } = adaptInto(input, scratch);
    const cell = basename(dir);
    const leaves = leafRecords(dataset);
    const xml = join(mkdtempSync(join(tmpdir(), "bench-dates-")), "chart.xml");
    writeFileSync(xml, toMspdi(dataset));
    const tsv = execFileSync("cargo", ["run", "-q", "--release", "-p", "import", "--example", "mspdi_dates", "--", xml], { cwd: root, encoding: "utf8", maxBuffer: 1 << 28 });
    const ours = parseOurs(tsv);
    const theirs = await bryntumDates(scratch, cell, dataset.data.length, chrome);
    const diffs = diffDates(leaves.map((t) => t.text), ours, theirs.byName);
    const far = diffs.filter((d) => d.minutes > 1);
    console.log(`${leaves.length} leaves compared: ${leaves.length - diffs.length} agree, ${diffs.length - far.length} differ by one minute, ${far.length} by more`);
    for (const d of (far.length > 0 ? far : diffs).slice(0, 5)) console.log(`  ${d.name}: ours ${d.ours.join(" to ")}, Bryntum ${d.theirs.join(" to ")}`);
    if (far.length > 0) throw new Error("Bryntum does not place this schedule where ours does: no run may give a number from it");
    writeHashed(dir, "bryntum-dated.json", bryntumJson(withDates(project, theirs.byId)));
    const expected = sample(leaves).map((t) => ({ id: t.id, start: ours.get(t.text)![0], end: ours.get(t.text)![1] }));
    writeFileSync(join(dir, "expected.json"), `${JSON.stringify(expected)}\n`);
    console.log(`wrote bryntum-dated.json and expected.json (${expected.length} tasks) in ${dir}`);
    return 0;
  } catch (e) {
    console.error(`bryntum-dates: ${e instanceof Error ? e.message : String(e)}`);
    return EXIT_FAILED;
  }
}

/** The dataset's leaves (records no other names as parent), in its order. */
export function leafRecords(d: Pick<BenchDataset, "data">): BenchDataset["data"] {
  const parents = new Set(d.data.flatMap((t) => (t.parent === undefined ? [] : [String(t.parent)])));
  return d.data.filter((t) => !parents.has(String(t.id)));
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
