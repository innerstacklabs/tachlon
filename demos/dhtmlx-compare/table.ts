// Pairs the runs `pair.ts` wrote and prints the load-matched ones, and the benchmark's numbers.
//
//   node demos/dhtmlx-compare/table.ts <results-dir>
//   node demos/dhtmlx-compare/table.ts --help
//
// Layout: <results-dir>/<cell>/<pair>/<variant>/load.log, two variants per pair, and the driver's
// result.json beside each load.log when `demos/bench/run.ts` made the run. A pair is matched when
// the two runs' mean load1 differ by under MATCH_LOAD, neither run's swap grew, both runs wrote a result.json, and neither run's
// result is refused (`demos/bench/results.ts`: warnings, an error, a dataset hash missing or not the
// cell's, or the two sides' drags on different tasks of the dataset); a cell needs MIN_MATCHED matched pairs, within the first MAX_ATTEMPTS pairs (by pair
// number; later ones are listed and not counted), else it is reported "unmatched" and prints no numbers. A cell with results prints its
// dataset's SHA-256 (<cell>/dataset.json.sha256) and one row per side over its matched pairs, only
// when every run there shares one environment (Chrome, OS, GPU, refresh, driver options, harness
// version) and every run of a side one build (its bundle hash, probe and path: side.json).
// Exit 0: at least one pair was read. Exit 1: no rows (an empty table is a failure, not a result).
// Exit 2: a run's swap grew, so its pair is not counted. Exit 3: a result was refused, or a cell's
// matched runs came from different environments or a side's from different builds.

import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { loadavg } from "node:os";
import { join } from "node:path";
import { aligned, buildKey, environmentKey, HEADER, KIND_CAVEAT, KIND_LABEL, kinds, memoryTiming, pairRefusal, readResult, refusal, sideRow, type DriveResult, type MemoryKind } from "../bench/results.ts";

export const MATCH_LOAD = 0.5;
export const MIN_MATCHED = 4;
export const MAX_ATTEMPTS = 8;
export const EXIT_REFUSED = 3;

export interface Sample {
  t: number;
  load1: number;
  rssKb: number;
  swapKb: number;
}

export interface Run {
  variant: string;
  meanLoad1: number;
  swapGrewKb: number;
  samples: number;
  /** The first sample's time, unix seconds: which side of a pair ran first. */
  startT?: number;
  /** The driver's result, when the run wrote one. */
  result?: DriveResult | null;
  /** Why the result is not a result; empty when it is one or there is none. */
  refused?: string[];
}

export interface Pair {
  cell: string;
  pair: string;
  runs: Run[];
  diff: number | null;
  matched: boolean;
  /** Why the pair is not matched, when it is not for a reason the loads and verdict do not show. */
  why?: string;
}

/** One line of `load.log`: `<unix seconds> <load1> <chrome rss kB> <swap used kB>`. */
export function formatSample(s: Sample): string {
  return `${s.t} ${s.load1.toFixed(2)} ${s.rssKb} ${s.swapKb}`;
}

export function parseLoadLog(text: string): Sample[] {
  const out: Sample[] = [];
  for (const line of text.split("\n")) {
    const f = line.trim().split(/\s+/);
    if (f.length !== 4) continue;
    const [t, load1, rssKb, swapKb] = f.map(Number);
    if ([t, load1, rssKb, swapKb].some((n) => !Number.isFinite(n))) continue;
    out.push({ t, load1, rssKb, swapKb });
  }
  return out;
}

export function summarise(variant: string, samples: Sample[]): Run {
  const mean = samples.reduce((a, s) => a + s.load1, 0) / samples.length;
  const swapGrewKb = Math.max(0, samples[samples.length - 1].swapKb - samples[0].swapKb);
  return { variant, meanLoad1: mean, swapGrewKb, samples: samples.length, startT: samples[0].t };
}

function dirs(path: string): string[] {
  if (!existsSync(path)) return [];
  return readdirSync(path)
    .filter((n) => statSync(join(path, n)).isDirectory())
    .sort((a, b) => a.localeCompare(b, undefined, { numeric: true }));
}

/** The SHA-256 the run wrote beside the cell, or null when there is none. */
export function cellSha256(root: string, cell: string): string | null {
  const file = join(root, cell, "dataset.json.sha256");
  const hash = existsSync(file) ? readFileSync(file, "utf8").trim().split(/\s+/)[0] : "";
  return /^[0-9a-f]{64}$/.test(hash) ? hash : null;
}

/**
 * Every pair under `root`. A pair is matched when its two runs' mean load1 are within MATCH_LOAD,
 * neither run's swap grew, and neither result is refused (`refusal`, against the cell's dataset
 * hash; `pairRefusal`, both runs when their drags moved different tasks). A run with no
 * result.json is unknown, never matched: its pair says which run has none.
 */
export function readPairs(root: string): Pair[] {
  const pairs: Pair[] = [];
  for (const cell of dirs(root)) {
    const sha = cellSha256(root, cell);
    for (const pair of dirs(join(root, cell))) {
      const runs: Run[] = [];
      for (const variant of dirs(join(root, cell, pair))) {
        const log = join(root, cell, pair, variant, "load.log");
        if (!existsSync(log)) continue;
        const samples = parseLoadLog(readFileSync(log, "utf8"));
        if (samples.length === 0) continue;
        const result = readResult(join(root, cell, pair, variant, "result.json"));
        runs.push({ ...summarise(variant, samples), result, refused: result === null ? [] : refusal(result, sha) });
      }
      if (runs.length === 0) continue;
      const results = runs.flatMap((r) => (r.result == null ? [] : [{ side: r.variant, result: r.result }]));
      const unlike = pairRefusal(results);
      if (unlike !== null) for (const r of runs) r.refused = [...(r.refused ?? []), unlike];
      const diff = runs.length === 2 ? Math.abs(runs[0].meanLoad1 - runs[1].meanLoad1) : null;
      const clean = !runs.some(isRefused) && runs.every((r) => r.swapGrewKb === 0);
      const missing = runs.filter((r) => r.result == null).map((r) => r.variant);
      const why = missing.length > 0 ? `no result.json from ${missing.join(" and ")}` : undefined;
      pairs.push({ cell, pair, runs, diff, matched: diff !== null && diff < MATCH_LOAD && clean && why === undefined, why });
    }
  }
  return pairs;
}

const isRefused = (r: Run) => (r.refused ?? []).length > 0;

/** The side that ran first in a pair (the earlier first sample), or null on a tie. */
function ranFirst(p: Pair): string | null {
  if (p.runs.length !== 2 || p.runs[0].startT === p.runs[1].startT) return null;
  return [...p.runs].sort((a, b) => (a.startT ?? 0) - (b.startT ?? 0))[0].variant;
}

/** A cell's numbers: one row per side over its counted matched pairs, or why there are none; `refused` when the runs do not combine. */
function cellNumbers(c: string, counted: Pair[], n: number, shown: Set<MemoryKind>): { lines: string[]; refused: boolean } {
  if (n < MIN_MATCHED) return { lines: [`  no numbers: ${n} matched pairs, ${MIN_MATCHED} needed`], refused: false };
  const matched = counted.filter((p) => p.matched);
  const all = matched.flatMap((p) => p.runs.map((r) => r.result!));
  const envs = [...new Set(all.map(environmentKey))];
  if (envs.length > 1) {
    return { lines: [`${c}: environments differ, not combined into one row:`, ...envs.map((e) => `  ${e}`), "  no numbers"], refused: true };
  }
  const sides = [...new Set(matched.flatMap((p) => p.runs.map((r) => r.variant)))].sort();
  const bySide = sides.map((s) => matched.flatMap((p) => p.runs.filter((r) => r.variant === s).map((r) => r.result!)));
  const rebuilt = sides.flatMap((s, i) => {
    const builds = [...new Set(bySide[i].map(buildKey))];
    return builds.length > 1 ? [`${c}: side ${s} ran more than one build, not combined into one row:`, ...builds.map((b) => `  ${b}`)] : [];
  });
  if (rebuilt.length > 0) return { lines: [...rebuilt, "  no numbers"], refused: true };
  const firsts = sides.map((s) => matched.filter((p) => ranFirst(p) === s).length);
  const k = kinds(all);
  const lines: string[] = [];
  if (k.length > 1) lines.push(`${c}: memory not comparable (${k.map((x) => KIND_LABEL[x]).join(" and ")})`);
  else k.forEach((x) => shown.add(x));
  lines.push(...aligned([HEADER, ...sides.map((s, i) => sideRow(s, bySide[i], firsts[i], k.length <= 1))]));
  // Each side loads its own file of the one dataset (the hash printed above the table): the sizes say what each parsed.
  const bytes = (rs: DriveResult[]) => (rs[0]?.inputBytes == null ? "not recorded" : `${rs[0].inputBytes.toLocaleString("en-US")} bytes`);
  lines.push(`  input files: ${sides.map((s, i) => `${s} ${bytes(bySide[i])}`).join(", ")}`);
  return { lines, refused: false };
}

export function render(pairs: Pair[], root = ""): { text: string; code: number } {
  if (pairs.length === 0) return { text: "no rows: no load.log found\n", code: 1 };
  const lines: string[] = [];
  const grew = pairs.flatMap((p) =>
    p.runs.filter((r) => r.swapGrewKb > 0).map((r) => `${p.cell}/${p.pair}/${r.variant}`),
  );
  const refused = pairs.flatMap((p) => p.runs.filter(isRefused).map((r) => `refused: ${p.cell}/${p.pair}/${r.variant}: ${r.refused!.join("; ")}`));
  for (const p of pairs) {
    const loads = p.runs.map((r) => `${r.variant} ${r.meanLoad1.toFixed(2)}`).join("  ");
    const verdict = p.runs.some(isRefused) ? "refused" : p.runs.some((r) => r.swapGrewKb > 0) ? "swap grew" : p.matched ? "matched" : p.why ? `not matched: ${p.why}` : "not matched";
    const d = p.diff === null ? "incomplete" : `diff ${p.diff.toFixed(2)} ${verdict}`;
    lines.push(`${p.cell}  ${p.pair}  ${loads}  ${d}`);
  }
  const cells = [...new Set(pairs.map((p) => p.cell))];
  const shown = new Set<MemoryKind>();
  let cellRefused = false;
  for (const c of cells) {
    const all = pairs.filter((p) => p.cell === c);
    const counted = all.slice(0, MAX_ATTEMPTS);
    const n = counted.filter((p) => p.matched).length;
    const extra = all.length > counted.length ? `, ${all.length - counted.length} past the cap of ${MAX_ATTEMPTS} not counted` : "";
    const withResults = all.some((p) => p.runs.some((r) => r.result != null));
    const sha = cellSha256(root, c);
    if (withResults) lines.push(`${c}: dataset sha256 ${sha ?? "not recorded: no numbers"}`);
    lines.push(`${c}: ${n} matched of ${counted.length} attempts${extra}${n < MIN_MATCHED ? ` (unmatched: needs ${MIN_MATCHED})` : ""}`);
    if (withResults) {
      const got = cellNumbers(c, counted, n, shown);
      lines.push(...got.lines);
      cellRefused ||= got.refused;
    }
  }
  if (cells.some((c) => pairs.some((p) => p.cell === c && p.runs.some((r) => r.result != null)))) {
    lines.push("each side's runs: a fresh browser each, no warm-up run discarded; a median of an even count is the mean of the middle two");
  }
  const results = pairs.flatMap((p) => p.runs.flatMap((r) => (r.result == null ? [] : [r.result])));
  for (const k of shown) lines.push(`memory: ${KIND_CAVEAT[k]}; ${memoryTiming(results)}`);
  lines.push(...refused);
  if (grew.length > 0) lines.push(`swap grew in: ${grew.join(", ")} (those pairs are not matched)`);
  const code = grew.length > 0 ? 2 : refused.length > 0 || cellRefused ? EXIT_REFUSED : 0;
  return { text: lines.join("\n") + "\n", code };
}

export const USAGE = "usage: node demos/dhtmlx-compare/table.ts <results-dir>\n";

function main(): void {
  const root = process.argv[2];
  if (root === "--help") {
    process.stdout.write(USAGE);
    process.exit(0);
  }
  if (!root || root.startsWith("-")) {
    process.stderr.write(USAGE);
    process.exit(64);
  }
  const { text, code } = render(readPairs(root), root);
  (code === 0 ? process.stdout : process.stderr).write(text);
  process.stdout.write(loadLine(existsSync("/proc/loadavg") ? readFileSync("/proc/loadavg", "utf8") : null, loadavg()));
  process.exit(code);
}

/** The load line printed last: /proc/loadavg on Linux, `os.loadavg()` where that file is absent (macOS). */
export function loadLine(procLoadavg: string | null, avg: number[]): string {
  return procLoadavg !== null ? `/proc/loadavg: ${procLoadavg}` : `loadavg: ${avg.map((n) => n.toFixed(2)).join(" ")}\n`;
}

if (import.meta.main) main();
