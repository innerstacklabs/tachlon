// What the table takes from the driver's result files (`drive.ts` writes one per side per pair).
//
// A run is refused, so no number of it reaches the table, when its result:
// - has `warnings` (headless, a software or unknown renderer, a debug build, a display rate off, an
//   input phase that did not do what it claims), or no `warnings` field at all, or no environment.
//   One exception: a side's trial build (it says it is not its production build, `debugBuild`, and
//   names a trial build, `sideBuild`). Its debug-build warning refuses only the columns a trial's
//   unminified bundle affects, library ready, the whole load and memory (more source held in the
//   renderer), which print "trial build"; the run is not refused for it;
// - does not report the dataset it loaded (`datasetSha256`), or reports another one than the cell's
//   (when load timed out, so the side never could say, the dataset the run asked for is checked);
// - has no side.json beside it (`build.ts`: the side's bundle hash, probe, path and harness version);
// - has a phase that threw (`error`) or whose probe never saw the row count (`not-ready`): either
//   may be our harness's fault as much as the side's, so it is never printed as the side's result.
// Only a phase stopped at --dnf-s (`timeout`) prints as "did not finish in N s"; a phase not run
// after it prints "not run".
//
// A pair is refused, both its runs, when both drags ran and they did not drag the same task of the
// dataset (each drag records the dataset id of the task it pressed on, `task`; none recorded counts
// as not the same): the two sides' drag numbers compare only when they moved the same task.
//
// The rest are summed up per side over the matched pairs of a cell, the same way on every side,
// in a row labelled with the side's name and the build it names (`sideBuild`): rows on screen;
// library ready, data to ready and data to settled (`demos/bench/load.ts`; the last only for a side
// whose ready frame can come before its load work is done) as medians; load as the median and the
// maximum of the runs' load times; each frame phase as the median of
// the runs' own p95 frame gap, the median share of its frames over 1.5 display periods and the longest frame of any run; memory as the median of the whole browser's memory after the input
// phases. A median of an even count is the mean of the middle two. No warm-up run is discarded:
// every run is a fresh browser and profile. Memory of two kinds is never compared.

import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { SideBuild } from "./build.ts";

/** How the driver read memory: PSS (Linux), the physical footprint (macOS), or the resident set. */
export type MemoryKind = "pss" | "footprint" | "rss";

export const KIND_LABEL: Record<MemoryKind, string> = { pss: "PSS", footprint: "footprint", rss: "RSS" };

/** What each kind of memory number counts and leaves out; the table prints the line for each kind it shows. */
export const KIND_CAVEAT: Record<MemoryKind, string> = {
  pss: "proportional set size (PSS, /proc/<pid>/smaps_rollup) summed over Chrome's processes, a shared page counted once in all; it leaves out GPU memory not mapped into a process (VRAM), so a side that keeps its schedule on the GPU can read low",
  footprint: "physical footprint (vmmap) summed over Chrome's processes; whether it counts GPU (IOAccelerator) memory is not yet measured on a Mac, so treat its GPU share as unverified",
  rss: "resident set (RSS) summed over Chrome's processes: a page shared by several counts once per process; it leaves out GPU memory not mapped into a process (VRAM)",
};

interface PhaseOut {
  dnf: boolean;
  cause?: string;
  afterS?: number;
  why?: string;
  ms?: number;
  libraryMs?: number | null;
  dataToReadyMs?: number | null;
  dataToSettledMs?: number | null;
  p95?: number;
  /** Gaps recorded, gaps over 1.5 display periods, and the worst gap (ms): `frames.ts`. */
  frames?: number;
  longGaps?: number;
  max?: number;
  /** A scroll phase: the distance it scrolled out, px (`phases.ts`). */
  scrolled?: number;
  /** The drag's task, as its dataset id; null when the side could not say. */
  task?: string | null;
  /** A drag: ms from the release to the side's commit, once what follows the task is rescheduled (`phases.ts`). */
  commitMs?: number;
  /** The memory read: what it counts, and when it began, seconds after the navigation (`drive.ts`). */
  afterLoad?: { kind?: MemoryKind; chromeBytes?: number; readAtS?: number };
}

/** How far past `memoryAtS` a memory read may begin: the page was slow to settle, so a read past it is not the other side's 20 s. */
export const MEMORY_LATE_S = 2;

/** The memory timing the runs share, as the table's caption says it: the runs' `memoryAtS`, or that they predate it. */
export function memoryTiming(rs: DriveResult[]): string {
  const at = new Set(rs.map((r) => (r.options as { memoryAtS?: number } | null)?.memoryAtS));
  if (at.size > 1) return "runs read memory at different times";
  const [s] = at;
  return typeof s === "number" ? `read ${s} s after the page opened, settled` : "read right after load (legacy runs: not read after the page settled)";
}

/** The parts of a driver result the table reads. */
export interface DriveResult {
  environment: Record<string, unknown> | null;
  options: unknown;
  requestedDatasetSha256: string | null;
  datasetSha256: string | null;
  /** The bytes of the file the page loaded (the side's `inputBytes` hook), null when the side did not say. */
  inputBytes: number | null;
  /** The side.json beside the result, null when there is none. */
  build: SideBuild | null;
  warnings: string[];
  phases: Partial<Record<"load" | "scroll-y" | "scroll-x" | "drag" | "memory", PhaseOut>>;
}

/** The side.json in `dir`, or null when there is none or it does not read. */
function readBuild(dir: string): SideBuild | null {
  try {
    const b = JSON.parse(readFileSync(join(dir, "side.json"), "utf8"));
    return typeof b?.bundleSha256 === "string" && typeof b?.harness === "string" ? b : null;
  } catch {
    return null;
  }
}

/** The result in `file`, or null when there is none. An unreadable file is a refusal, never skipped. */
export function readResult(file: string): DriveResult | null {
  if (!existsSync(file)) return null;
  const build = readBuild(dirname(file));
  const hex = (v: unknown) => (typeof v === "string" ? v : null);
  try {
    const r = JSON.parse(readFileSync(file, "utf8"));
    const warnings = Array.isArray(r.warnings) ? r.warnings.map(String) : ["no warnings field: not a driver result"];
    return { environment: r.environment ?? null, options: r.options ?? null, requestedDatasetSha256: hex(r.requestedDatasetSha256), datasetSha256: hex(r.datasetSha256), inputBytes: typeof r.inputBytes === "number" ? r.inputBytes : null, build, warnings, phases: r.phases ?? {} };
  } catch (e) {
    return { environment: null, options: null, requestedDatasetSha256: null, datasetSha256: null, inputBytes: null, build, warnings: [`unreadable result: ${e instanceof Error ? e.message : String(e)}`], phases: {} };
  }
}

const DEBUG_BUILD = /^debug build: /;

/** A side's trial build: it says it is not its production build and names a trial build. */
export function isTrial(r: DriveResult): boolean {
  return r.environment?.debugBuild === true && /trial build/.test(String(r.environment?.sideBuild ?? ""));
}

/** Why a run is not a result; empty when it is one. `cellSha256` is the cell's dataset hash, null when none was written. */
export function refusal(r: DriveResult, cellSha256: string | null): string[] {
  const why = isTrial(r) ? r.warnings.filter((w) => !DEBUG_BUILD.test(w)) : [...r.warnings];
  if (r.environment === null) why.push("no environment: the run broke outside any phase");
  if (r.build === null) why.push("no side.json beside the result: the side's build is unknown");
  // A load that timed out never let the side say what it loaded: the dataset the run asked for stands in.
  const loadTimedOut = r.phases.load?.dnf === true && r.phases.load.cause === "timeout";
  const sha = loadTimedOut ? r.requestedDatasetSha256 : r.datasetSha256;
  if (cellSha256 === null) why.push("the cell has no dataset.json.sha256");
  else if (sha === null) why.push(loadTimedOut ? "the run did not record the dataset it asked for" : "the side did not report the dataset it loaded");
  else if (sha !== cellSha256) why.push(`the side ${loadTimedOut ? "was asked for" : "loaded"} dataset ${sha}, not the cell's ${cellSha256}`);
  const asked = (r.options as { memoryAtS?: number } | null)?.memoryAtS;
  const readAt = r.phases.memory?.afterLoad?.readAtS;
  if (typeof asked === "number" && typeof readAt === "number" && readAt - asked > MEMORY_LATE_S) {
    why.push(`memory: read began ${readAt} s after the page opened, over ${MEMORY_LATE_S} s past the ${asked} s asked (a slow load): not the other side's reading`);
  }
  const phases = Object.entries(r.phases) as [string, PhaseOut][];
  const timedOut = phases.some(([, p]) => p?.dnf && p.cause === "timeout");
  for (const [name, p] of phases) {
    if (!p?.dnf || p.cause === "timeout" || (p.cause === "not-run" && timedOut)) continue;
    why.push(`${name}: ${p.cause ?? "no cause"}: ${p.why ?? "no why"}`);
  }
  return why;
}

/** Why a pair's two runs do not compare, null when they do: both drags ran, on different tasks of the dataset or on one not recorded. */
export function pairRefusal(runs: { side: string; result: DriveResult }[]): string | null {
  const drags = runs.map(({ side, result }) => ({ side, drag: result.phases.drag }));
  if (drags.length !== 2 || drags.some(({ drag }) => drag === undefined || drag.dnf)) return null;
  const unnamed = drags.filter(({ drag }) => typeof drag!.task !== "string").map(({ side }) => side);
  if (unnamed.length > 0) return `drag: ${unnamed.join(" and ")} did not record which task of the dataset it dragged`;
  const [a, b] = drags;
  return a.drag!.task === b.drag!.task ? null : `drag: the sides dragged different tasks of the dataset (${a.side} task ${a.drag!.task}, ${b.side} task ${b.drag!.task})`;
}

/** What must be equal on every run of a cell, both sides: the machine, the browser, the driver's options and the harness version. */
export function environmentKey(r: DriveResult): string {
  const e = r.environment ?? {};
  const pick = (k: string) => e[k] ?? null;
  return JSON.stringify({ chrome: pick("chrome"), os: pick("os"), node: pick("node"), gpu: pick("gpu"), viewport: pick("viewport"), dpr: pick("dpr"), refreshHz: pick("refreshHz"), options: r.options, harness: r.build?.harness ?? null });
}

/** What must be equal on every run of one side's row: its bundle's hash, its probe, its page path and the build it names. */
export function buildKey(r: DriveResult): string {
  return JSON.stringify({ bundleSha256: r.build?.bundleSha256 ?? null, probe: r.build?.probe ?? null, path: r.build?.path ?? null, sideBuild: r.environment?.sideBuild ?? null });
}

/** The median (the mean of the middle two for an even count); NaN for none. */
export function median(values: number[]): number {
  if (values.length === 0) return Number.NaN;
  const s = [...values].sort((a, b) => a - b);
  const m = Math.floor(s.length / 2);
  return s.length % 2 === 1 ? s[m] : (s[m - 1] + s[m]) / 2;
}

const fixed = (v: number) => (Number.isFinite(v) ? v.toFixed(1) : "-");

type Read = number | { timeoutS: number } | "not-run" | null;

/** What `phase` gives on one run: its number by `num`, a timeout, not run, or null when absent. */
function read(r: DriveResult, phase: keyof DriveResult["phases"], num: (p: PhaseOut) => number | undefined): Read {
  const p = r.phases[phase];
  if (p === undefined) return null;
  if (p.dnf) return p.cause === "timeout" ? { timeoutS: p.afterS ?? Number.NaN } : p.cause === "not-run" ? "not-run" : null;
  return num(p) ?? null;
}

/** A column: `stat` of the runs' numbers; "did not finish in N s (n/m)" or "not run (n/m)" when any run has none. */
function column(rs: DriveResult[], got: Read[], stat: (v: number[]) => number): string {
  const timeouts = got.filter((v): v is { timeoutS: number } => typeof v === "object" && v !== null);
  if (timeouts.length > 0) return `did not finish in ${Math.max(...timeouts.map((t) => t.timeoutS))} s (${timeouts.length}/${rs.length})`;
  const notRun = got.filter((v) => v === "not-run").length;
  if (notRun > 0) return `not run (${notRun}/${rs.length})`;
  const nums = got.filter((v): v is number => typeof v === "number");
  return nums.length === rs.length ? fixed(stat(nums)) : "-";
}

/** The memory kinds the runs read, in a stable order. */
export function kinds(rs: DriveResult[]): MemoryKind[] {
  const all = rs.map((r) => r.phases.memory?.afterLoad?.kind).filter((k): k is MemoryKind => k !== undefined && k in KIND_LABEL);
  return (Object.keys(KIND_LABEL) as MemoryKind[]).filter((k) => all.includes(k));
}

export const HEADER = ["side", "pairs", "first", "rows on screen", "library median ms", "data to ready median ms", "data to settled median ms", "load median ms", "load max ms", "scroll-y p95 median ms", "scroll-y long frames median %", "scroll-y longest frame max ms", "scroll-y distance median px", "scroll-x p95 median ms", "scroll-x long frames median %", "scroll-x longest frame max ms", "scroll-x distance median px", "drag p95 median ms", "drag long frames median %", "drag longest frame max ms", "drag commit median ms", "memory median"];

/** The cell a trial build's runs print in the columns its unminified bundle affects. */
export const TRIAL_CELL = "trial build";

/**
 * One side's row: its name, its runs, how many of their pairs it ran first, then each column.
 * `showMemory` is false when the cell's sides read memory of more than one kind.
 */
export function sideRow(side: string, rs: DriveResult[], first: number, showMemory: boolean): string[] {
  const max = (v: number[]) => Math.max(...v);
  /** A frame phase's three columns: the median p95, the median share of frames over 1.5 periods (%), the longest frame of any run. */
  const frames = (phase: "scroll-y" | "scroll-x" | "drag") => [
    column(rs, rs.map((r) => read(r, phase, (p) => p.p95)), median),
    column(rs, rs.map((r) => read(r, phase, (p) => (p.frames && p.longGaps !== undefined ? (100 * p.longGaps) / p.frames : undefined))), median),
    column(rs, rs.map((r) => read(r, phase, (p) => p.max)), max),
  ];
  /** The distance a scroll phase covered, px: its target is the smaller of the fixed distance and the side's own range, so this says which. */
  const distance = (phase: "scroll-y" | "scroll-x") => column(rs, rs.map((r) => read(r, phase, (p) => p.scrolled)), median);
  const loads = rs.map((r) => read(r, "load", (p) => p.ms));
  const trial = rs.some(isTrial);
  const loadPart = (k: "libraryMs" | "dataToReadyMs" | "dataToSettledMs") => column(rs, rs.map((r) => read(r, "load", (p) => p[k] ?? undefined)), median);
  const label = rs[0]?.environment?.sideBuild;
  const onScreen = column(rs, rs.map((r) => (typeof r.environment?.rowsOnScreen === "number" ? r.environment.rowsOnScreen : null)), median);
  const k = kinds(rs);
  let memory = "-";
  if (showMemory && k.length <= 1) {
    const c = column(rs, rs.map((r) => read(r, "memory", (p) => (p.afterLoad?.chromeBytes === undefined ? undefined : p.afterLoad.chromeBytes / 1e6))), median);
    memory = /^\d/.test(c) && k.length === 1 ? `${c} MB ${KIND_LABEL[k[0]]}` : /^\d/.test(c) ? "-" : c;
  }
  const whole = (stat: (v: number[]) => number) => (trial ? TRIAL_CELL : column(rs, loads, stat));
  return [
    typeof label === "string" ? `${side}: ${label}` : side,
    String(rs.length),
    String(first),
    onScreen,
    trial ? TRIAL_CELL : loadPart("libraryMs"),
    loadPart("dataToReadyMs"),
    loadPart("dataToSettledMs"),
    whole(median),
    whole(max),
    ...frames("scroll-y"),
    distance("scroll-y"),
    ...frames("scroll-x"),
    distance("scroll-x"),
    ...frames("drag"),
    column(rs, rs.map((r) => read(r, "drag", (p) => p.commitMs)), median),
    trial ? TRIAL_CELL : memory,
  ];
}

/** Rows padded to their columns' widths, each line indented two spaces. */
export function aligned(rows: string[][]): string[] {
  const widths = rows[0].map((_, i) => Math.max(...rows.map((r) => r[i].length)));
  return rows.map((r) => `  ${r.map((c, i) => (i === r.length - 1 ? c : c.padEnd(widths[i]))).join("  ")}`);
}
