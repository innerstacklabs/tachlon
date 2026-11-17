// What the memory phase reads: the whole browser's memory, by process type, and the JS heap.
//
// A side may keep its schedule in JS objects, in WebAssembly memory (ours) or on the GPU. The JS heap
// counts only the first, so it is never the memory number on its own. The comparable one is the
// memory of Chrome's processes, summed: each side runs in the same fresh browser with one page.
// Chrome does not say which renderer process draws which page, so the renderers are summed (the
// spare ones are near empty and the same for every side), and so is the whole browser.
//
// Which number per process: summed RSS counts a page shared by several processes (the renderer and
// the GPU process share some) once per process, so a side can be skewed either way by how much it
// shares. So Linux reads PSS from /proc/<pid>/smaps_rollup (a shared page is split among its
// sharers, so the sum counts it once) and macOS the physical footprint from `vmmap --summary`.
// When that cannot be read for a process that is still running (vmmap refused by a hardened
// runtime, say), the sample throws, so the memory phase is an error the table refuses: it never
// falls back to RSS on those two. Other platforms read RSS, and `kind` says so; the table never
// compares two kinds. PSS leaves out GPU memory that is not mapped into a process (VRAM). Whether
// the macOS footprint counts the GPU's (IOAccelerator) memory is not yet measured on a Mac.
// The process list is CDP's `SystemInfo.getProcessInfo`; the heap is `Performance.getMetrics` after a
// garbage collection.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { platform } from "node:os";
import type { Cdp } from "./phases.ts";
import type { MemoryKind } from "./results.ts";

export interface MemorySample {
  /** What the byte counts below are: PSS, the physical footprint, or RSS. */
  kind: MemoryKind;
  /** Every Chrome process, summed, bytes: the number to compare. */
  chromeBytes: number;
  /** The renderer processes, summed, bytes. */
  rendererBytes: number;
  /** The GPU process, bytes (what it maps; not the GPU's own memory). */
  gpuProcessBytes: number;
  /** The page's JS heap in use, bytes. It leaves out WebAssembly and GPU memory. */
  jsHeapUsedBytes: number;
}

/** pid to resident kB, from `ps -o pid=,rss=` output. */
export function parsePsRss(text: string): Map<number, number> {
  const out = new Map<number, number>();
  for (const line of text.split("\n")) {
    const m = line.trim().match(/^(\d+)\s+(\d+)$/);
    if (m) out.set(Number(m[1]), Number(m[2]));
  }
  return out;
}

/** PSS in kB from the text of /proc/<pid>/smaps_rollup; null when it has no Pss line. */
export function parseSmapsRollup(text: string): number | null {
  const m = text.match(/^Pss:\s+(\d+) kB/m);
  return m ? Number(m[1]) : null;
}

/**
 * The physical footprint in kB from `vmmap --summary <pid>` (run in the C locale), whose line reads
 * `Physical footprint:         123.4M`; null when there is none. The peak line is not this one.
 */
export function parseFootprint(text: string): number | null {
  const m = text.match(/^Physical footprint:\s+(\d+(?:\.\d+)?)([BKMGT]?)\s*$/m);
  if (!m) return null;
  const scale: Record<string, number> = { "": 1 / 1024, B: 1 / 1024, K: 1, M: 1024, G: 1024 ** 2, T: 1024 ** 3 };
  return Math.round(Number(m[1]) * scale[m[2]]);
}

/** Resident kB of each pid still running. */
function rssKb(pids: number[]): Map<number, number> {
  if (platform() === "linux") {
    return perPid(pids, (pid) => {
      const m = readFileSync(`/proc/${pid}/status`, "utf8").match(/^VmRSS:\s+(\d+) kB/m);
      return m ? Number(m[1]) : null;
    });
  }
  try {
    return parsePsRss(execFileSync("ps", ["-o", "pid=,rss=", "-p", pids.join(",")], { encoding: "utf8" }));
  } catch (e) {
    // ps exits 1 when some pid has ended; what it printed still counts.
    return parsePsRss(String((e as { stdout?: string }).stdout ?? ""));
  }
}

/** `read` for each pid; a pid it throws on or returns null for is left out. */
function perPid(pids: number[], read: (pid: number) => number | null): Map<number, number> {
  const out = new Map<number, number>();
  for (const pid of pids) {
    try {
      const kb = read(pid);
      if (kb !== null) out.set(pid, kb);
    } catch {
      // the process ended, or this reader cannot see it
    }
  }
  return out;
}

/** PSS (Linux) or the physical footprint (macOS) of each pid; empty elsewhere. */
function preferredKb(pids: number[]): { kind: MemoryKind; kb: Map<number, number> } | null {
  if (platform() === "linux") return { kind: "pss", kb: perPid(pids, (pid) => parseSmapsRollup(readFileSync(`/proc/${pid}/smaps_rollup`, "utf8"))) };
  if (platform() === "darwin") {
    const env = { ...process.env, LC_ALL: "C" };
    return { kind: "footprint", kb: perPid(pids, (pid) => parseFootprint(execFileSync("vmmap", ["--summary", String(pid)], { encoding: "utf8", env, stdio: ["ignore", "pipe", "ignore"] }))) };
  }
  return null;
}

/**
 * One kind for every pid: PSS (Linux) or the physical footprint (macOS), and it must cover every
 * process RSS still sees running (one that ended between the two reads is left out of both), else
 * this throws; RSS on any other platform.
 */
export function memoryKb(pids: number[]): { kind: MemoryKind; kb: Map<number, number> } {
  const preferred = preferredKb(pids);
  const rss = rssKb(pids);
  if (preferred === null) return { kind: "rss", kb: rss };
  const missed = [...rss.keys()].filter((pid) => !preferred.kb.has(pid));
  if (missed.length > 0) throw new Error(`no ${preferred.kind} read for running process ${missed.join(", ")}`);
  return preferred;
}

/** Read the sample: `page` is the page's CDP session, `browser` the browser's. */
export async function sampleMemory(page: Cdp, browser: Cdp): Promise<MemorySample> {
  await page.send("HeapProfiler.collectGarbage");
  const { metrics } = (await page.send("Performance.getMetrics")) as { metrics: { name: string; value: number }[] };
  const heap = metrics.find((m) => m.name === "JSHeapUsedSize");
  if (heap === undefined) throw new Error("no JSHeapUsedSize in Performance.getMetrics");
  const { processInfo } = (await browser.send("SystemInfo.getProcessInfo")) as { processInfo: { type: string; id: number }[] };
  const { kind, kb } = memoryKb(processInfo.map((p) => p.id));
  if (kb.size === 0) throw new Error("no memory read for any Chrome process");
  const sum = (pick: (type: string) => boolean) => processInfo.filter((p) => pick(p.type)).reduce((n, p) => n + (kb.get(p.id) ?? 0) * 1024, 0);
  return {
    kind,
    chromeBytes: sum(() => true),
    rendererBytes: sum((t) => t === "renderer"),
    gpuProcessBytes: sum((t) => t.toLowerCase() === "gpu"),
    jsHeapUsedBytes: heap.value,
  };
}
