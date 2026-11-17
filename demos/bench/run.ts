// Run the matched pairs over a list of cells and print the one table.
//
//   node demos/bench/run.ts --sides <sides.json> --cells <shape:size>[,<shape:size>...] --out <dir> \
//     [--seed 20261020] [--pairs 8] [--wait-s 3600] [--max-load 4] [--driver demos/bench/drive.ts] [-- <drive options>]
//   node demos/bench/run.ts --help
//
// `sides.json` names the two sides: `[{ "name", "bundle", "probe", "path" }, ...]`. `bundle` is the
// directory `pair.ts` serves, `probe` what `drive.ts --probe` takes (`ours`, or a side module), and
// `path` the page to open in it, where `{shape}`, `{size}` and `{seed}` are replaced (the side's own
// adapter has put that cell's schedule in its bundle). Names are lower case letters, digits and `-`.
//
// For each cell (`<shape>-<size>`): write its schedule and SHA-256 to <out>/<cell>/dataset.json and
// dataset.json.sha256 (`dataset.ts`; a cell that already holds another hash is refused, so one cell
// never mixes two schedules), then run pairs numbered on from the last one there. A pair is the
// two sides back to back on a quiet box, the order flipped on every other pair (`pair.ts`), each
// side driven by `drive.ts --url <page> --probe <probe> --rows <size> --dataset-sha256 <the cell's>
// --out <dir>/result.json` with the same options after `--` for both, and its build written beside
// it as side.json (`build.ts`: bundle hash, probe, path, harness version), so the table never puts
// two builds of a side into one row. `--max-load` is `pair.ts`'s quiet cap on load1 (default 4),
// written in each wait.log; raise it only for a smoke run, as the test does. A cell stops once it has MIN_MATCHED matched pairs or
// `--pairs` attempts (at most MAX_ATTEMPTS count). Then the table (`table.ts`) is printed: the pairs,
// the matched-pair rule, refused results, and each side's numbers with the dataset hash.
// Exit code: the table's (0 a table; 1 no rows; 2 swap grew; 3 a result refused); 64 usage; 65 a
// cell holds another dataset; or the code `pair.ts` gives a side that failed (1, 70, 75).

import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { MAX_LOAD, SAMPLE_MS, pairOrder, readerFor, runSide, type Reader } from "../dhtmlx-compare/pair.ts";
import { MAX_ATTEMPTS, MIN_MATCHED, readPairs, render } from "../dhtmlx-compare/table.ts";
import { buildDataset, checkInputs, datasetJson } from "./dataset.ts";
import { harnessVersion, sideBuild } from "./build.ts";

export const EXIT_USAGE = 64;
export const EXIT_OTHER_DATASET = 65;
export const DEFAULT_SEED = "20261020";

export interface SideSpec {
  name: string;
  bundle: string;
  probe: string;
  path: string;
}

export interface Cell {
  shape: string;
  size: number;
}

export interface Options {
  sides: SideSpec[];
  cells: Cell[];
  out: string;
  seed: bigint;
  pairs: number;
  waitS: number;
  maxLoad: number;
  driver: string;
  driveArgs: string[];
}

const here = dirname(fileURLToPath(import.meta.url));
const VALUED = ["sides", "cells", "out", "seed", "pairs", "wait-s", "max-load", "driver"] as const;

/** The two sides in a sides.json text, or why it is not usable. */
export function parseSides(text: string): SideSpec[] | string {
  let list: unknown;
  try {
    list = JSON.parse(text);
  } catch (e) {
    return `sides: ${e instanceof Error ? e.message : String(e)}`;
  }
  if (!Array.isArray(list) || list.length !== 2) return "sides: a list of exactly two sides";
  for (const s of list) {
    for (const k of ["name", "bundle", "probe", "path"]) {
      if (typeof s?.[k] !== "string" || (k !== "path" && s[k] === "")) return `sides: each side needs a string "${k}"`;
    }
    if (!/^[a-z0-9][a-z0-9-]*$/.test(s.name)) return `sides: name ${JSON.stringify(s.name)} is not lower case letters, digits and -`;
  }
  if (list[0].name === list[1].name) return "sides: the two names are the same";
  return list as SideSpec[];
}

/** `shape:size,...` to cells, each checked as `dataset.ts` checks it, or why not. */
export function parseCells(text: string, seed: bigint): Cell[] | string {
  const cells: Cell[] = [];
  for (const item of text.split(",")) {
    const m = item.match(/^([a-z0-9]+):(\d+)$/);
    if (!m) return `cells: ${JSON.stringify(item)} is not shape:size`;
    try {
      checkInputs(m[1], Number(m[2]), seed);
    } catch (e) {
      return `cells: ${e instanceof Error ? e.message : String(e)}`;
    }
    if (cells.some((c) => c.shape === m[1] && c.size === Number(m[2]))) return `cells: ${item} is repeated`;
    cells.push({ shape: m[1], size: Number(m[2]) });
  }
  return cells;
}

/** The options, or a reason they are not usable. Everything after `--` goes to the driver as it is. */
export function parseArgs(argv: string[], read: (path: string) => string = (p) => readFileSync(p, "utf8")): Options | string {
  const split = argv.indexOf("--");
  const own = split < 0 ? argv : argv.slice(0, split);
  const got = new Map<string, string>();
  for (let i = 0; i < own.length; i += 2) {
    const name = own[i].startsWith("--") ? own[i].slice(2) : "";
    const value = own[i + 1];
    if (!(VALUED as readonly string[]).includes(name) || got.has(name)) return `unknown or repeated ${JSON.stringify(own[i])}`;
    if (value === undefined || value === "" || value.startsWith("--")) return `--${name} needs a value`;
    got.set(name, value);
  }
  for (const k of ["sides", "cells", "out"]) if (!got.has(k)) return "--sides, --cells and --out are required";
  const seedText = got.get("seed") ?? DEFAULT_SEED;
  if (!/^\d+$/.test(seedText)) return "--seed is not a u64";
  const seed = BigInt(seedText);
  const whole = (name: string, fallback: number, max = Infinity): number | string => {
    const v = got.get(name);
    if (v === undefined) return fallback;
    return /^[1-9]\d*$/.test(v) && Number(v) <= max ? Number(v) : `--${name} is not a whole number from 1${max < Infinity ? ` to ${max}` : ""}`;
  };
  const pairs = whole("pairs", MAX_ATTEMPTS, MAX_ATTEMPTS);
  const waitS = whole("wait-s", 3600);
  if (typeof pairs === "string") return pairs;
  if (typeof waitS === "string") return waitS;
  const maxLoadText = got.get("max-load") ?? String(MAX_LOAD);
  if (!/^\d+(\.\d+)?$/.test(maxLoadText) || Number(maxLoadText) <= 0) return "--max-load is not a positive number";
  let sidesText: string;
  try {
    sidesText = read(got.get("sides")!);
  } catch (e) {
    return `sides: ${e instanceof Error ? e.message : String(e)}`;
  }
  const sides = parseSides(sidesText);
  if (typeof sides === "string") return sides;
  const cells = parseCells(got.get("cells")!, seed);
  if (typeof cells === "string") return cells;
  const driver = got.get("driver") ?? join(here, "drive.ts");
  return { sides, cells, out: got.get("out")!, seed, pairs, waitS, maxLoad: Number(maxLoadText), driver, driveArgs: split < 0 ? [] : argv.slice(split + 1) };
}

export const cellName = (c: Cell) => `${c.shape}-${c.size}`;

/** The page to open on a side for a cell: its path with the cell's values put in, under `{url}`. */
export function pagePath(path: string, c: Cell, seed: bigint): string {
  return `{url}${path.replace(/^\/+/, "").replaceAll("{shape}", c.shape).replaceAll("{size}", String(c.size)).replaceAll("{seed}", String(seed))}`;
}

/** The command `pair.ts` runs for one side of one pair; `sha` is the cell's dataset hash, recorded by the driver before load. */
export function sideCommand(o: Options, s: SideSpec, c: Cell, dir: string, sha: string): string[] {
  const url = pagePath(s.path, c, o.seed);
  return [process.execPath, o.driver, "--url", url, "--probe", s.probe, "--rows", String(c.size), "--dataset-sha256", sha, "--out", join(dir, "result.json"), ...o.driveArgs];
}

/** Write the cell's dataset and hash; the hash, or null when the cell already holds another one. */
export function writeDataset(dir: string, c: Cell, seed: bigint): string | null {
  checkInputs(c.shape, c.size, seed);
  const json = datasetJson(buildDataset(c.shape, c.size, seed));
  const hash = createHash("sha256").update(json).digest("hex");
  const shaFile = join(dir, "dataset.json.sha256");
  if (existsSync(shaFile) && readFileSync(shaFile, "utf8").trim().split(/\s+/)[0] !== hash) return null;
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "dataset.json"), json);
  writeFileSync(shaFile, `${hash}  dataset.json\n`);
  return hash;
}

/** The highest pair number already in a cell's directory, 0 when none. */
function lastPair(dir: string): number {
  if (!existsSync(dir)) return 0;
  return Math.max(0, ...readdirSync(dir).filter((n) => /^\d+$/.test(n)).map(Number));
}

/**
 * Run every cell's pairs with `reader` (the box's load, swap and busy test, polled every
 * `sampleMs`), writing each side's build (side.json) beside its run, then render the table.
 */
export async function runCells(o: Options, reader: Reader, harness = harnessVersion(), sampleMs = SAMPLE_MS): Promise<{ text: string; code: number }> {
  const builds = new Map(o.sides.map((s) => [s.name, sideBuild(s, harness)]));
  for (const c of o.cells) {
    const dir = join(o.out, cellName(c));
    const sha = writeDataset(dir, c, o.seed);
    if (sha === null) return { text: `${dir} holds another dataset's results; use another --out\n`, code: EXIT_OTHER_DATASET };
    const counted = () => readPairs(o.out).filter((p) => p.cell === cellName(c)).slice(0, MAX_ATTEMPTS);
    for (let n = lastPair(dir) + 1; ; n++) {
      const sofar = counted();
      if (sofar.length >= o.pairs || sofar.filter((p) => p.matched).length >= MIN_MATCHED) break;
      for (const s of pairOrder(n, o.sides)) {
        const sideDir = join(dir, String(n), s.name);
        mkdirSync(sideDir, { recursive: true });
        writeFileSync(join(sideDir, "side.json"), `${JSON.stringify(builds.get(s.name))}\n`);
        const code = await runSide(reader, sideDir, s.bundle, sideCommand(o, s, c, sideDir, sha), o.waitS, o.maxLoad, sampleMs);
        if (code !== 0) return { text: `${cellName(c)} pair ${n} ${s.name}: exit ${code}\n`, code };
      }
    }
  }
  return render(readPairs(o.out), o.out);
}

export const USAGE = `usage: node demos/bench/run.ts --sides <sides.json> --cells <shape:size,...> --out <dir> [--seed ${DEFAULT_SEED}] [--pairs ${MAX_ATTEMPTS}] [--wait-s 3600] [--max-load ${MAX_LOAD}] [--driver drive.ts] [-- <drive options>]\n`;

async function main(argv: string[]): Promise<number> {
  if (argv[0] === "--help") {
    process.stdout.write(USAGE);
    return 0;
  }
  const o = parseArgs(argv);
  if (typeof o === "string") {
    process.stderr.write(`${o}\n${USAGE}`);
    return EXIT_USAGE;
  }
  const reader = readerFor(process.platform);
  const { text, code } = await runCells(o, reader);
  (code === 0 ? process.stdout : process.stderr).write(text);
  process.stdout.write(reader.loadText());
  return code;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
