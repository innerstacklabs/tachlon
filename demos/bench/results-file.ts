// Write the dated results file from the run directories `run.ts` wrote, so no hand-built file reaches the page.
//
// File name: `results/YYYY-MM-DD.json`; a second run on the same day is `results/YYYY-MM-DD-run2.json`, a third `-run3`, and so on.
// The page shows the file that the one line in `results/current` names, so a new run reaches the speed table by changing that line:
// `scripts/bench-table.ts` reads it by default, as does `packages/gantt/tests-node/site-page.test.ts`.
//
//   node demos/bench/results-file.ts --runs <dir> --head <commit> --date <YYYY-MM-DD> --notes <notes.json> --out <file>
//   node demos/bench/results-file.ts --help
//
// Every number and cell comes from `<runs>/<cell>/<pair>/<side>/{result.json,side.json,load.log,wait.log}` and
// `<cell>/dataset.json.sha256`, over the same matched pairs `table.ts` counts. The notes file holds the prose no run
// directory says: `story`, `machine`, `command`, `driverOptions`, `method`, `tree` and `box.{quietCap,wallClock}`.
// Exit code: 0 written; 1 refused, nothing written (a `side.json` anywhere under the runs directory whose harness is `unknown`,
// ends in `-dirty` or is not `--head`, a `--head` that is not a 40-digit commit, or a matched run with no figure for a median); 2 a side
// directory cannot be read or the notes lack a key; 64 usage.

import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";
import { MATCH_LOAD, MAX_ATTEMPTS, cellSha256, parseLoadLog, readPairs, type Pair } from "../dhtmlx-compare/table.ts";
import { median, type DriveResult } from "./results.ts";

export const USAGE = "usage: node demos/bench/results-file.ts --runs <dir> --head <commit> --date <YYYY-MM-DD> --notes <notes.json> --out <file>\n";
const max = (v: number[]) => Math.max(...v);
const frames = (r: DriveResult) => max((["scroll-y", "scroll-x", "drag"] as const).map((p) => r.phases[p]?.max ?? NaN));
/** Each median the page reads: its key, the number one run gives (NaN when its phase timed out or did not run: the file says null), and how the matched runs combine. */
const KEYS: [string, (r: DriveResult) => number, (v: number[]) => number][] = [
  ["loadMedianMs", (r) => r.phases.load?.ms ?? NaN, median], ["libraryMedianMs", (r) => r.phases.load?.libraryMs ?? NaN, median],
  ["dataToReadyMedianMs", (r) => r.phases.load?.dataToReadyMs ?? NaN, median], ["scrollYP95Ms", (r) => r.phases["scroll-y"]?.p95 ?? NaN, median],
  ["scrollXP95Ms", (r) => r.phases["scroll-x"]?.p95 ?? NaN, median], ["dragP95Ms", (r) => r.phases.drag?.p95 ?? NaN, median],
  ["longestFrameMaxMs", frames, max], ["memoryFootprintMB", (r) => (r.phases.memory?.afterLoad?.chromeBytes ?? NaN) / 1e6, median], ["inputBytes", (r) => r.inputBytes ?? NaN, median],
];
const PROSE = ["story", "machine", "command", "driverOptions", "method", "tree"];
const dirs = (p: string) => readdirSync(p, { withFileTypes: true }).filter((d) => d.isDirectory()).map((d) => d.name).sort();
const places = (v: number, n: number) => Number(v.toFixed(n));

/** The results file's text, or why none is written: code 1 a harness of any run that is not `head` (a 40-digit commit, so never `unknown` or `-dirty`), code 2 an unreadable run directory or notes. */
export function resultsFile(root: string, head: string, date: string, notesText: string): { json: string } | { code: number; why: string } {
  if (!/^[0-9a-f]{40}$/.test(head)) return { code: 1, why: `--head ${head} is not a 40-digit commit` };
  const logs = new Map<string, { load1: number[]; start: number }>();
  const load1: number[] = [], start: number[] = [];
  let notes: Record<string, unknown>, names: string[], pairs: Pair[];
  try {
    notes = JSON.parse(notesText);
    const box = notes?.box as Record<string, unknown> | null | undefined;
    for (const k of [...PROSE, "box.quietCap", "box.wallClock"]) if (typeof (k.startsWith("box.") ? box?.[k.slice(4)] : notes?.[k]) !== "string") throw new Error(`notes: ${k} must be a string`);
    // Every side directory, whatever its pair number or logs: its side.json is the harness check, then its files must read.
    for (const c of dirs(root)) for (const p of dirs(join(root, c))) for (const s of dirs(join(root, c, p))) {
      const dir = join(root, c, p, s), harness = JSON.parse(readFileSync(join(dir, "side.json"), "utf8")).harness;
      if (harness !== head) return { code: 1, why: `${c}/${p}/${s}: harness ${harness}, not ${head}` };
      JSON.parse(readFileSync(join(dir, "result.json"), "utf8"));
      logs.set(dir, { load1: parseLoadLog(readFileSync(join(dir, "load.log"), "utf8")).map((l) => l.load1), start: Number(readFileSync(join(dir, "wait.log"), "utf8").trim().split("\n").pop()!.match(/load1=([\d.]+)/)![1]) });
    }
    const all = readPairs(root);
    names = [...new Set(all.map((p) => p.cell))];
    pairs = names.flatMap((c) => all.filter((p) => p.cell === c).slice(0, MAX_ATTEMPTS));
    if (pairs.length === 0) throw new Error("no run with a load.log");
    for (const c of names) if (cellSha256(root, c) === null) throw new Error(`${c}: no dataset.json.sha256`);
    for (const p of pairs) for (const r of p.runs) {
      const l = logs.get(join(root, p.cell, p.pair, r.variant))!;
      load1.push(...l.load1);
      start.push(l.start);
    }
  } catch (e) {
    return { code: 2, why: e instanceof Error ? e.message : String(e) };
  }
  const cells: Record<string, object> = {}, medians: Record<string, object> = {}, missing: string[] = [];
  for (const c of names) {
    const mine = pairs.filter((p) => p.cell === c), matched = mine.filter((p) => p.matched);
    const note = mine.filter((p) => !p.matched).map((p) => `pair ${p.pair} not matched: load1 means differ by ${p.diff?.toFixed(2)} (limit ${MATCH_LOAD}), or a result refused or swap grew; kept and listed, not counted`).join("; ");
    cells[c] = { datasetSha256: cellSha256(root, c), pairsRun: mine.length, matched: matched.length, ...(note ? { note } : {}) };
    const sides = [...new Set(matched.flatMap((p) => p.runs.map((r) => r.variant)))].sort();
    medians[c] = Object.fromEntries(sides.map((s) => {
      const rs = matched.flatMap((p) => p.runs.filter((r) => r.variant === s).map((r) => r.result!));
      return [s, Object.fromEntries(KEYS.map(([k, one, combine]) => [k, rs.some((r) => Number.isNaN(one(r))) ? (missing.push(`${c} ${s} ${k}`), null) : places(combine(rs.map(one)), 1)]))];
    }));
  }
  if (missing.length > 0) return { code: 1, why: `no figure for ${missing.join(", ")}: a phase timed out or did not run in a matched run, and the page cannot read a figure that is not there` };
  const sides = ["ours", ...new Set(pairs.flatMap((p) => p.runs.map((r) => r.variant)))].filter((s, i, a) => a.indexOf(s) === i);
  const { quietCap, wallClock } = notes.box as Record<string, string>;
  const box = { quietCap, load1Samples: { count: load1.length, min: Math.min(...load1), mean: places(load1.reduce((a, v) => a + v, 0) / load1.length, 2), max: max(load1) }, highestLoad1AtStart: max(start), wallClock };
  const { story, machine, command, driverOptions, method, tree } = notes;
  return { json: `${JSON.stringify({ date, story, machine, harness: head, ours: head, command, driverOptions, method, sides, tree, cells, box, medians }, null, 2)}\n` };
}

function main(argv: string[]): number {
  if (argv[0] === "--help") return process.stdout.write(USAGE), 0;
  const { values: v } = parseArgs({ args: argv, strict: false, options: { runs: { type: "string" }, head: { type: "string" }, date: { type: "string" }, notes: { type: "string" }, out: { type: "string" } } });
  if (![v.runs, v.head, v.date, v.notes, v.out].every((x) => typeof x === "string") || !/^\d{4}-\d\d-\d\d$/.test(v.date as string)) return process.stderr.write(USAGE), 64;
  let notes = "";
  try {
    notes = readFileSync(v.notes as string, "utf8");
  } catch (e) {
    return process.stderr.write(`notes: ${e instanceof Error ? e.message : String(e)}\n`), 2;
  }
  const got = resultsFile(v.runs as string, v.head as string, v.date as string, notes);
  if ("json" in got) return writeFileSync(v.out as string, got.json), 0;
  return process.stderr.write(`${got.why}\n`), got.code;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
