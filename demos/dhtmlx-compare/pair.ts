// Runs one pair of measurements on a quiet box.
//
//   node demos/dhtmlx-compare/pair.ts --out <dir> --cell <name> --pair <n> \
//     --a <name>=<bundle-dir> --b <name>=<bundle-dir> -- <command> [args]
//
// For each side (the order flips with the pair number, so neither side always goes first): wait
// until nothing busy runs, load1 is under --max-load, default MAX_LOAD, and swap used did not grow
// between two polls (polled every SAMPLE_MS, written to
// <out>/<cell>/<pair>/<side>/wait.log), serve the bundle on a free port, start the command with
// `{url}` replaced by the page's address, and sample load1, Chrome RSS and swap used every
// SAMPLE_MS into load.log until the command exits. `table.ts` reads the result.
// Runs on Linux (/proc; busy = a gitlab-runner build job) and macOS (os.loadavg(), `sysctl
// vm.swapusage`, `ps`; busy = a process named in BUSY_NAMES); it refuses any other platform.
// Exit code: 0 done; 1 the command exited nonzero (its code is printed); 64 usage; 70 the command
// could not start; 74 a reading of the box failed (a running command is stopped: SIGTERM, then
// SIGKILL, within 2 × KILL_MS);
// 75 the box never went quiet within --wait-s (no run was made).

import { execFileSync, spawn } from "node:child_process";
import { createReadStream, existsSync, mkdirSync, readdirSync, readFileSync, statSync, writeFileSync, appendFileSync } from "node:fs";
import { createServer } from "node:http";
import { loadavg as osLoadavg } from "node:os";
import { extname, join, normalize, resolve } from "node:path";
import { formatSample, loadLine } from "./table.ts";

export const MAX_LOAD = 4;
export const SAMPLE_MS = 3000;
export const EXIT_COMMAND_FAILED = 1;
export const EXIT_NO_START = 70;
export const EXIT_READER = 74;
/** How long a command stopped after a reader failure gets after SIGTERM, and again after SIGKILL, before the run exits 74 anyway. */
export const KILL_MS = 5000;
export const EXIT_NOT_QUIET = 75;

/** The one-minute load average, from the text of /proc/loadavg (Linux). */
export function parseLoad1(loadavg: string): number {
  const n = Number(loadavg.trim().split(/\s+/)[0]);
  if (!Number.isFinite(n)) throw new Error(`cannot read load1 from ${JSON.stringify(loadavg)}`);
  return n;
}

/** Swap in use, in kB, from the text of /proc/meminfo (Linux). */
export function parseSwapUsedKb(meminfo: string): number {
  const field = (name: string): number => {
    const m = meminfo.match(new RegExp(`^${name}:\\s+(\\d+) kB`, "m"));
    if (!m) throw new Error(`no ${name} in meminfo`);
    return Number(m[1]);
  };
  return field("SwapTotal") - field("SwapFree");
}

/**
 * True when a command line belongs to a CI build job, not to the runner daemon that waits for
 * one: it names gitlab-runner and one of the words a job's process carries (`exec`, `build`).
 * The daemon (`gitlab-runner run`) is always up and does not make the box busy.
 */
export function isRunnerJob(cmdline: string): boolean {
  const argv = cmdline.split("\0").filter(Boolean);
  if (!argv.some((a) => a.includes("gitlab-runner"))) return false;
  return argv.some((a) => a === "exec" || a === "build" || a.includes("/builds/"));
}

/**
 * Swap in use, in kB, from `sysctl vm.swapusage` on macOS:
 * `vm.swapusage: total = 2048.00M  used = 1024.50M  free = 1023.50M  (encrypted)`.
 */
export function parseSwapUsage(text: string): number {
  // The whole token must be a C-locale number: `1024,50M` (a decimal-comma locale) is refused,
  // never read as 1024 bytes. `macReader` runs sysctl with LC_ALL=C so it prints the dot form.
  const token = text.match(/\bused = (\S+)/)?.[1] ?? "";
  const m = token.match(/^(\d+(?:\.\d+)?)([KMGT]?)$/);
  if (!m) throw new Error(`cannot read swap used from ${JSON.stringify(text)}`);
  const scale: Record<string, number> = { "": 1 / 1024, K: 1, M: 1024, G: 1024 ** 2, T: 1024 ** 3 };
  return Math.round(Number(m[1]) * scale[m[2]]);
}

/** One process: its executable's name and its RSS in kB. */
export type Proc = { name: string; rssKb: number };

/**
 * Processes from `ps -axww -o rss=,comm=` on macOS: RSS in kB, then the executable's path. Where
 * ps cannot read a process's arguments it prints the kernel name in parentheses, `(cargo)`; the
 * name is then the part inside them.
 */
export function parsePs(text: string): Proc[] {
  const out: Proc[] = [];
  for (const line of text.split("\n")) {
    const m = line.match(/^\s*(\d+)\s+(.+?)\s*$/);
    if (!m) continue;
    const kernel = m[2].match(/^\((.+)\)$/);
    const name = kernel ? kernel[1] : m[2].slice(m[2].lastIndexOf("/") + 1);
    out.push({ name, rssKb: Number(m[1]) });
  }
  // `ps -ax` always lists launchd and ps itself: no row at all is a misparse, never a quiet box.
  if (out.length === 0) throw new Error(`cannot read any process from ps: ${JSON.stringify(text.slice(0, 200))}`);
  return out;
}

/**
 * The macOS busy test: a process with one of these names is a build that would load the box. The
 * Mac has no gitlab-runner job to look for, so the test goes by the executable's name.
 */
export const BUSY_NAMES = ["cargo", "rustc", "xcodebuild"];

export function isBusyName(name: string): boolean {
  return BUSY_NAMES.includes(name);
}

/** What the runner reads from the box, one implementation per platform. */
export type Reader = {
  load1(): number;
  swapKb(): number;
  chromeRssKb(): number;
  /** The name of what keeps the box busy, or null when nothing does. */
  busy(): string | null;
  /** The load line printed last. */
  loadText(): string;
};

const isChrome = (name: string) => /chrom/i.test(name);

function linuxProcs(): (Proc & { cmdline: string })[] {
  const out = [];
  for (const pid of readdirSync("/proc").filter((n) => /^\d+$/.test(n))) {
    try {
      const cmdline = readFileSync(`/proc/${pid}/cmdline`, "utf8");
      const name = readFileSync(`/proc/${pid}/comm`, "utf8").trim();
      const m = readFileSync(`/proc/${pid}/status`, "utf8").match(/^VmRSS:\s+(\d+) kB/m);
      out.push({ name, cmdline, rssKb: m ? Number(m[1]) : 0 });
    } catch {
      // the process ended while we read it
    }
  }
  return out;
}

const read = (p: string) => readFileSync(p, "utf8");

/** The Linux busy name in wait.log's `busy=` field: one word, so the line splits on spaces. */
export const RUNNER_JOB = "gitlab-runner-job";

/** Linux: /proc/loadavg, /proc/meminfo, and a gitlab-runner build job as the busy test. */
export function linuxReader(): Reader {
  return {
    load1: () => parseLoad1(read("/proc/loadavg")),
    swapKb: () => parseSwapUsedKb(read("/proc/meminfo")),
    chromeRssKb: () => linuxProcs().filter((p) => isChrome(p.name)).reduce((a, p) => a + p.rssKb, 0),
    busy: () => (linuxProcs().some((p) => isRunnerJob(p.cmdline)) ? RUNNER_JOB : null),
    loadText: () => loadLine(read("/proc/loadavg"), []),
  };
}

type Run = (cmd: string, args: string[], env: Record<string, string>) => string;
const runCommand: Run = (cmd, args, env) => execFileSync(cmd, args, { encoding: "utf8", env: { ...process.env, ...env } });

/** sysctl formats numbers by LC_NUMERIC; the C locale makes the decimal point a dot. */
const C_LOCALE = { LC_ALL: "C" };

/** macOS: `os.loadavg()`, `sysctl vm.swapusage`, and `ps` for Chrome RSS and the busy test. */
export function macReader(run: Run = runCommand, loadavg: () => number[] = osLoadavg): Reader {
  const procs = () => parsePs(run("ps", ["-axww", "-o", "rss=,comm="], C_LOCALE));
  return {
    load1: () => loadavg()[0],
    swapKb: () => parseSwapUsage(run("sysctl", ["vm.swapusage"], C_LOCALE)),
    chromeRssKb: () => procs().filter((p) => isChrome(p.name)).reduce((a, p) => a + p.rssKb, 0),
    busy: () => procs().find((p) => isBusyName(p.name))?.name ?? null,
    loadText: () => loadLine(null, loadavg()),
  };
}

/** The reader for a `process.platform` value; any other platform is refused. */
export function readerFor(platform: string): Reader {
  if (platform === "linux") return linuxReader();
  if (platform === "darwin") return macReader();
  throw new Error(`no reader for ${platform}: pair.ts runs on Linux and macOS`);
}

function sample(r: Reader) {
  return { t: Math.floor(Date.now() / 1000), load1: r.load1(), rssKb: r.chromeRssKb(), swapKb: r.swapKb() };
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/**
 * Poll until the box is quiet: nothing busy, load1 under `maxLoad`, and swap used not grown since
 * the poll before (so the first poll never counts as quiet); false when `limitS` seconds pass first.
 */
async function waitQuiet(r: Reader, log: string, limitS: number, maxLoad: number, sampleMs: number): Promise<boolean> {
  const stop = Date.now() + limitS * 1000;
  let lastSwap: number | undefined;
  for (;;) {
    const busy = r.busy();
    const load1 = r.load1();
    const swap = r.swapKb();
    const steady = lastSwap !== undefined && swap <= lastSwap;
    lastSwap = swap;
    const quiet = busy === null && load1 < maxLoad && steady;
    appendFileSync(log, `${Math.floor(Date.now() / 1000)} load1=${load1.toFixed(2)} max=${maxLoad} swapKb=${swap} busy=${busy ?? "none"} ${quiet ? "quiet" : "wait"}\n`);
    if (quiet) return true;
    if (Date.now() >= stop) return false;
    await sleep(sampleMs);
  }
}

const TYPES: Record<string, string> = {
  ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".css": "text/css",
  ".json": "application/json", ".wasm": "application/wasm", ".svg": "image/svg+xml", ".png": "image/png",
};

export function serve(root: string): Promise<{ url: string; close: () => void }> {
  const server = createServer((req, res) => {
    const rel = normalize(decodeURIComponent((req.url ?? "/").split("?")[0])).replace(/^(\.\.[/\\])+/, "");
    let file = join(root, rel);
    if (existsSync(file) && statSync(file).isDirectory()) file = join(file, "index.html");
    if (!file.startsWith(root) || !existsSync(file)) {
      res.writeHead(404).end();
      return;
    }
    res.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" });
    createReadStream(file).pipe(res);
  });
  return new Promise((resolve) =>
    server.listen(0, "127.0.0.1", () => {
      const port = (server.address() as { port: number }).port;
      resolve({ url: `http://127.0.0.1:${port}/`, close: () => server.close() });
    }),
  );
}

/** One side: wait for a quiet box, serve `bundle`, run `cmd` (`{url}` replaced) and sample into `dir`/load.log. */
export async function runSide(r: Reader, dir: string, bundle: string, cmd: string[], waitS: number, maxLoad = MAX_LOAD, sampleMs = SAMPLE_MS, killMs = KILL_MS): Promise<number> {
  mkdirSync(dir, { recursive: true });
  if (!(await waitQuiet(r, join(dir, "wait.log"), waitS, maxLoad, sampleMs))) return EXIT_NOT_QUIET;
  const srv = await serve(resolve(bundle));
  const loadLog = join(dir, "load.log");
  writeFileSync(loadLog, "");
  // A reader that throws (a sysctl or ps it cannot read) stops the run with its own exit code, never the command's.
  let failed: unknown;
  const take = (): boolean => {
    try {
      appendFileSync(loadLog, formatSample(sample(r)) + "\n");
      return true;
    } catch (e) {
      failed ??= e;
      return false;
    }
  };
  const readerFailed = () => {
    srv.close();
    process.stderr.write(`reading the box failed: ${failed instanceof Error ? failed.message : String(failed)}\n`);
    return EXIT_READER;
  };
  if (!take()) return readerFailed();
  const child = spawn(cmd[0], cmd.slice(1).map((a) => a.replaceAll("{url}", srv.url)), { stdio: "inherit" });
  const done = new Promise<number>((r) => {
    child.on("error", (e) => {
      process.stderr.write(`cannot start ${cmd[0]}: ${e.message}\n`);
      r(EXIT_NO_START);
    });
    child.on("exit", (c) => r(c === 0 ? 0 : EXIT_COMMAND_FAILED));
  });
  let running = true;
  let gaveUp: (code: number) => void = () => undefined;
  const given = new Promise<number>((r) => (gaveUp = r));
  const alive = () => child.exitCode === null && child.signalCode === null;
  const sampler = (async () => {
    while (running) {
      await sleep(sampleMs);
      if (!running || take()) continue;
      // SIGTERM, then SIGKILL, each given killMs: a command that ignores SIGTERM cannot hold the run.
      for (const signal of ["SIGTERM", "SIGKILL"] as const) {
        if (alive()) child.kill(signal);
        await Promise.race([done, sleep(killMs)]);
      }
      if (alive()) child.unref();
      gaveUp(EXIT_READER);
      return;
    }
  })();
  const code = await Promise.race([done, given]);
  running = false;
  await sampler;
  if (failed !== undefined || !take()) return readerFailed();
  srv.close();
  return code;
}

/** The pair's order: as given on an odd pair number, flipped on an even one, so neither side always goes first. */
export function pairOrder<T>(pair: number, sides: T[]): T[] {
  return pair % 2 === 0 ? [...sides].reverse() : [...sides];
}

async function main(): Promise<void> {
  const argv = process.argv.slice(2);
  const split = argv.indexOf("--");
  const opts = split < 0 ? argv : argv.slice(0, split);
  const cmd = split < 0 ? [] : argv.slice(split + 1);
  const get = (name: string) => opts[opts.indexOf(`--${name}`) + 1];
  const [out, cell, pair, a, b] = ["out", "cell", "pair", "a", "b"].map(get);
  if (!out || !cell || !pair || !a || !b || cmd.length === 0 || !a.includes("=") || !b.includes("=")) {
    process.stderr.write("usage: node pair.ts --out D --cell C --pair N --a name=dir --b name=dir [--wait-s 3600] [--max-load 4] -- cmd {url}\n");
    process.exit(64);
  }
  const waitS = Number(get("wait-s") ?? 3600);
  const maxLoad = Number(get("max-load") ?? MAX_LOAD);
  const r = readerFor(process.platform);
  const sides = pairOrder(Number(pair), [a, b].map((s) => s.split("=") as [string, string]));
  for (const [name, bundle] of sides) {
    const code = await runSide(r, join(out, cell, pair, name), bundle, cmd, waitS, maxLoad);
    if (code !== 0) {
      process.stderr.write(`${name}: exit ${code}\n`);
      process.exit(code);
    }
  }
  process.stdout.write(r.loadText());
}

if (import.meta.main) await main();
