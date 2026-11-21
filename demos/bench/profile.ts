// One command that shows where the time and memory go: builds (or reuses) the profile build, copies it to the Mac mini, serves it there, and opens one benchmark cell in Chromium n times. Usage and output: docs/PROFILING.md.
//
//   node demos/bench/profile.ts --cell chain-10000 --holder <lock name> [--runs 3] [--compare <ref>] [--out dir] [--host mini] [--memory-infra] [--drag] [--ghosts <n>] [--groups <k>] [--before-edit] [--critical-only]

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "./drive.ts";
import { readyScript } from "./load.ts";
import { type OpenSession, openSession, type Session } from "./profile-mini.ts";
import { type Args, type Drag, type DragRun, allocatorTable, clusterTable, compareTable, dragTable, pairedLine, flameSvg, foldStacks, holdGhosts, lockProblem, memoryDump, parseArgs, phaseRows, type Run, summaryTable, watchdog } from "./profile-format.ts";
import ours from "./sides/ours.ts";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "../..");
const SETTLE_MS = 20_000;
/** No session finished for this long, or the process not ended this long after its last session: `main` closes the session on the mini and exits 3. */
const STALL_MS = 900_000;
const AFTER_MS = 120_000;
const CATEGORIES = "devtools.timeline,v8,blink.user_timing,disabled-by-default-v8.cpu_profiler".split(",");
/** With `--memory-infra`, Chrome's own memory dump provider category: the one dump `runOnce` requests lands in the trace under it. */
const MEMORY_INFRA = "disabled-by-default-memory-infra";

const sh = (cwd: string, cmd: string, args: string[], env: NodeJS.ProcessEnv = process.env) => execFileSync(cmd, args, { cwd, env, stdio: ["ignore", "inherit", "inherit"] });

/** The commit sha `ref` names (a tag gives its commit, not the tag object), in `dir`: read once before any cell is built, so no cell compares against a later `origin/main`. Throws for a ref git does not know. */
export const baseline = (ref: string, dir = repo): string => execFileSync("git", ["-C", dir, "rev-parse", "--verify", `${ref}^{commit}`], { encoding: "utf8" }).trim();

/** The one line that fixes what this run needs and `env` or `root` lacks (null: nothing): `wasm-bindgen` on PATH, and `@playwright/test` resolvable from `root`. `main` asks before `build`, which takes minutes of the mini's queue. */
export function missingNeed(env: NodeJS.ProcessEnv = process.env, root = repo): string | null {
  if (spawnSync("wasm-bindgen", ["--version"], { stdio: "ignore", env }).error) return 'wasm-bindgen is not on PATH: run `export PATH="$HOME/.cargo/bin:$PATH"`, or install the version scripts/build-web.sh names';
  try {
    chromium(root);
  } catch (e) {
    return (e as Error).message;
  }
  return null;
}

/**
 * The cell's bundle from the profile build of `ref` (null: this checkout, uncommitted changes to tracked files included),
 * always built in a git worktree of its own, so the profile engine never lands in this checkout's `packages/gantt/src/wasm`.
 */
export function build(ref: string | null, a: Args): string {
  const sha = execFileSync("git", ["-C", repo, "rev-parse", ref ?? "HEAD"], { encoding: "utf8" }).trim();
  const patch = ref === null ? execFileSync("git", ["-C", repo, "diff", "HEAD", "--binary"], { encoding: "utf8", maxBuffer: 1 << 28 }) : "";
  const cache = join(homedir(), ".cache/gantt-profile", `${sha}${patch === "" ? "" : "-dirty"}-${a.cell}`);
  if (patch === "" && existsSync(join(cache, "bundle/ours-page.html")) && (!a.drag || existsSync(join(cache, "bundle/measure/index.html")))) return join(cache, "bundle");
  const tree = join(cache, "tree");
  if (existsSync(tree)) execFileSync("git", ["-C", repo, "worktree", "remove", "--force", tree], { stdio: "inherit" });
  execFileSync("git", ["-C", repo, "worktree", "add", "--detach", tree, sha], { stdio: "inherit" });
  try {
    if (patch !== "") execFileSync("git", ["-C", tree, "apply", "--binary"], { input: patch, stdio: ["pipe", "inherit", "inherit"] });
    const env = { ...process.env, CARGO_TARGET_DIR: process.env.CARGO_TARGET_DIR ?? join(homedir(), ".cache/cargo-target/gantt/profile") };
    const bundle = join(cache, "bundle");
    const [shape, size] = a.cell.split("-");
    sh(tree, "bash", ["scripts/build-web.sh", "--keep-names", "--no-bundle"], env);
    // The hosted app and the chrome load the built package (gantt#1090): each installs it, and the package is built first.
    for (const dir of ["packages/gantt", "apps/chrome", "apps/hosted"]) sh(join(tree, dir), "pnpm", ["install", "--frozen-lockfile"]);
    sh(join(tree, "packages/gantt"), "pnpm", ["build:lib"]);
    sh(join(tree, "apps/hosted"), "pnpm", ["exec", "vite", "build", "--config", "../../demos/bench/sides/ours-vite.config.ts", "--outDir", bundle]);
    sh(tree, "node", ["demos/bench/dataset.ts", "--shape", shape, "--size", size, "--seed", a.seed, "--out", join(cache, "data")]);
    sh(tree, "node", ["demos/bench/sides/ours-adapt.ts", join(cache, "data/dataset.json"), bundle]);
    // `--drag`: the developer page, whose `?measure=1` script `demos/50k/measure.mjs` drives, next to the bench page (relative URLs, so it runs from a subfolder).
    if (a.drag) sh(join(tree, "packages/gantt"), "pnpm", ["exec", "vite", "build", "--base", "./", "--outDir", join(bundle, "measure")]);
    return bundle;
  } finally {
    execFileSync("git", ["-C", repo, "worktree", "remove", "--force", tree], { stdio: "inherit" });
  }
}

/** `--ghosts`: hold the proposal in the page's chart (`holdGhosts`), which draws a ghost outline for each moved task on screen; how many tasks it moves. */
const hold = async (page: any, n: number): Promise<number> => {
  const t0 = Date.now();
  const moved: number = await page.evaluate(`(${holdGhosts})(window.gantt.app, ${n})`);
  console.log(`held a proposal that moves ${moved} tasks (${Date.now() - t0} ms)`);
  return moved;
};

/** One run in a fresh browser: load the cell with the trace on, wait SETTLE_MS after the ready frame, read the numbers, write the run's folder. */
export async function runOnce(session: Session, url: string, rows: number, dir: string, memoryInfra = false, ghosts = 0): Promise<Run> {
  mkdirSync(dir, { recursive: true });
  const { browser } = session;
  const page = browser.contexts()[0].pages()[0];
  const cdp = await browser.newBrowserCDPSession();
  const trace = join(dir, "trace.json");
  await browser.startTracing(page, { categories: memoryInfra ? [...CATEGORIES, MEMORY_INFRA] : CATEGORIES });
  await page.goto(url, { waitUntil: "commit", timeout: 120_000 });
  const ready: number | false = await page.evaluate(readyScript(ours, rows, 300_000));
  if (ready === false) throw new Error(`${url}: not ready with ${rows} rows`);
  const origin: number = await page.evaluate(() => performance.timeOrigin);
  const held = ghosts > 0 ? await hold(page, ghosts) : undefined;
  await page.waitForTimeout(SETTLE_MS);
  // The first mark of each name, as load.ts reads them: mount() sets its own `library-ready` later, which must not replace the page's.
  const marks: Record<string, number> = await page.evaluate(() => Object.fromEntries(["library-ready", "data-start", "mount-start"].flatMap((n) => { const m = performance.getEntriesByName(n, "mark")[0]; return m === undefined ? [] : [[n, m.startTime]]; })));
  const stats: number[] = await page.evaluate(() => (window as any).gantt.app.test_alloc_stats?.() ?? []);
  const { processInfo } = (await cdp.send("SystemInfo.getProcessInfo")) as { processInfo: { type: string; id: number }[] };
  const kb = await session.footprintKb(processInfo.map((p) => p.id));
  const gpu = processInfo.filter((p) => p.type.toLowerCase() === "gpu").reduce((n, p) => n + (kb.get(p.id) ?? 0) * 1024, 0);
  // The dump is requested at the same moment as the footprint read, in every process at once, and arrives in the trace when it stops.
  const dump = memoryInfra ? ((await cdp.send("Tracing.requestMemoryDump", { levelOfDetail: "light" })) as { dumpGuid: string; success: boolean }) : undefined;
  if (dump && !dump.success) throw new Error("Chrome did not take the memory dump");
  writeFileSync(trace, (await browser.stopTracing()).toString());
  const events = JSON.parse(readFileSync(trace, "utf8")).traceEvents ?? [];
  const allocators = dump ? memoryDump(events, dump.dumpGuid) : undefined;
  if (allocators && !allocators.gpu) throw new Error("the memory dump has no GPU process");
  const folded = foldStacks(events);
  writeFileSync(join(dir, "flame.folded"), [...folded].map(([s, n]) => `${s} ${n}`).join("\n") + "\n");
  writeFileSync(join(dir, "flame.svg"), flameSvg(folded));
  const run: Run = { marks, readyMs: ready - origin, peakLive: stats[0] ?? null, allocations: stats[1] ?? null, memoryBuffer: stats[3] ?? null, gpuProcess: gpu, ...(held !== undefined && { ghosts: held }), ...(allocators && { allocators }) };
  writeFileSync(join(dir, "run.json"), JSON.stringify({ ...run, phases: phaseRows(run) }, null, 2) + "\n");
  return run;
}

/** `runs` runs of each bundle, each copied to the mini and served there, interleaved (A B A B ...), in `out/<label>/run-<k>`. */
/** The open or open-ing session's closer, so a signal can run it before the process exits. */
let closer: (() => Promise<void>) | undefined;
/** Set by the signal handler: it, not a failure that the interrupted run throws, decides the exit. */
let interrupted = false;
/** Started by `main` around the runs; `say` marks it. */
let dog: ReturnType<typeof watchdog> | undefined;
const say = (text: string, limit?: number) => (console.error(text), dog?.mark(text, limit));
/** The open session's close, for at most 20 s: what a signal and a stall run before they exit. */
const closeSoon = () => Promise.race([closer?.(), new Promise((r) => setTimeout(r, 20_000))]);

export async function profile(a: Args, bundles: [string, string][], open: OpenSession = openSession, last = false): Promise<Map<string, Run[]>> {
  const rows = Number(a.cell.split("-")[1]);
  return interleaved(a, bundles, open, (session, label, k) => runOnce(session, `${session.baseUrl}ours-page.html?cell=${a.cell}-${a.seed}`, rows, join(a.out, label, `run-${k}`), a.memoryInfra, a.ghosts), last);
}

/** `a.runs` sessions of each bundle, interleaved (A B A B ...), each a fresh browser on the mini handed to `each`; the results by label. */
export async function interleaved<T>(a: Args, bundles: [string, string][], open: OpenSession, each: (session: Session, label: string, k: number) => Promise<T>, last = false): Promise<Map<string, T[]>> {
  const out = new Map(bundles.map(([label]) => [label, [] as T[]]));
  for (let k = 1; k <= a.runs; k++) {
    for (const [label, bundle] of bundles) {
      const session = await open({ host: a.host, chrome: a.chrome, bundle, register: (c) => (closer = c) });
      try {
        out.get(label)!.push(await each(session, label, k));
        say(`${label} session ${k}/${a.runs} done`, last && k === a.runs && label === bundles[bundles.length - 1][0] ? AFTER_MS : undefined);
      } finally {
        await session.close();
        closer = undefined;
      }
    }
  }
  return out;
}

/** The page's own scripted run (`?measure=1`, the one `demos/50k/measure.mjs` reads) shortened to 4 s drags and 3 repeats, as the frame reads of gantt#1680 were. */
const DRAG_RUN = "measure=1&dragMs=4000&repeats=3&editRepeats=0";

/** How long `dragOnce` waits for the page's result (and, with `--ghosts`, for its first frame): one scripted run is three 4 s drags and an idle-frame calibration, which ends well inside this. */
export const DRAG_WAIT_MS = 120_000;

/** How long the stall line waits for the page's message line: a page with a stuck main thread never answers `evaluate`, which has no timeout of its own. */
export const READ_MS = 5_000;

/** One drag scenario (`head` or `critical`, the page's `dragTarget`) in the session's browser: its p50 and p95 frame interval and drag response. A wait that times out throws what the page was doing: its address, message line, errors and failed requests. */
export async function dragOnce(session: Session, target: string, ghosts = 0, groups = 0, beforeEdit = false): Promise<Drag> {
  const page = session.browser.contexts()[0].pages()[0];
  const url = `${session.baseUrl}measure/index.html?${DRAG_RUN}${groups > 0 ? `&groups=${groups}` : ""}${beforeEdit ? "&beforeEdit=1" : ""}&dragTarget=${target}`;
  const seen: string[] = [];
  const on: [string, (x: any) => void][] = [
    ["pageerror", (e) => seen.push(`page error: ${e.message ?? e}`)],
    ["console", (m) => m.type() === "error" && seen.push(`console error: ${m.text()}`)],
    ["requestfailed", (r) => seen.push(`request failed: ${r.url()} ${r.failure()?.errorText}`)],
  ];
  const stalled = async (e: Error): Promise<never> => {
    let timer: NodeJS.Timeout | undefined;
    const unavailable = new Promise<string>((done) => (timer = setTimeout(done, READ_MS, "(unavailable)")));
    // `#overlay` is the page's message line: a measured run's progress or a failed start. The status line is hidden and empty here.
    const read = page.evaluate(() => document.getElementById("overlay")?.textContent ?? "").catch(() => "(unavailable)");
    const line: string = await Promise.race([read, unavailable]).finally(() => clearTimeout(timer));
    throw new Error(`the ${target} drag run gave no result: ${e.message.split("\n")[0]}; page ${url}; message line ${JSON.stringify(line)}; ${seen.length ? seen.join("; ") : "no console errors, page errors or failed requests"}`);
  };
  for (const [name, f] of on) page.on(name, f);
  try {
    await page.goto(url);
    // The script waits for the refresh rate (about 120 frames) before its first step, so the proposal, held on the first frame the chart is up, is on screen for all of it.
    if (ghosts > 0) await page.waitForFunction(() => (window as any).gantt?.ready === true, null, { timeout: DRAG_WAIT_MS, polling: "raf" }).catch(stalled).then(() => hold(page, ghosts));
    const result = await (await page.waitForFunction(() => (window as any).gantt?.measurement ?? false, null, { timeout: DRAG_WAIT_MS, polling: 1000 }).catch(stalled)).jsonValue();
    if (result.error) throw new Error(`the ${target} drag run failed: ${result.error}`);
    return { interval: [result.drag.p50, result.drag.p95], response: [result.dragResponseMs.p50, result.dragResponseMs.p95] };
  } finally {
    for (const [name, f] of on) page.off(name, f);
  }
}

/** `--drag`: the head-task and critical-path drags of each bundle (`--critical-only`: the critical-path drag alone), `a.runs` times, interleaved. Each run's folder gets `drag.json`: the p95 of each drag's frame intervals, in ms. */
export const dragProfile = (a: Args, bundles: [string, string][], open: OpenSession = openSession) =>
  interleaved(a, bundles, open, async (session, label, k): Promise<DragRun> => {
    const head = a.criticalOnly ? undefined : await dragOnce(session, "head", a.ghosts, a.groups, a.beforeEdit);
    if (head) say(`${label} session ${k}/${a.runs} head drag done`);
    const critical = await dragOnce(session, "critical", a.ghosts, a.groups, a.beforeEdit);
    say(`${label} session ${k}/${a.runs} critical drag done`);
    const dir = join(a.out, label, `run-${k}`);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "drag.json"), JSON.stringify({ frameIntervalP95Ms: { ...(head && { head: head.interval[1] }), critical: critical.interval[1] } }, null, 2) + "\n");
    return { head, critical };
  }, true);

/** What `main` measures on the mini: the open runs (not with `--critical-only`) and, with `--drag`, the drags. */
export async function measure(a: Args, bundles: [string, string][], open: OpenSession = openSession) {
  const runs = a.criticalOnly ? undefined : await profile(a, bundles, open, !a.drag);
  return { runs, drags: a.drag ? await dragProfile(a, bundles, open) : undefined };
}

/** The open tables of `runs` (`--memory-infra` adds the allocator table). */
const openTables = (a: Args, runs: Map<string, Run[]>, labels: string[]): string => {
  const sets = a.compare === null ? [runs.get("run")!] : [runs.get("before")!, runs.get("after")!];
  const table = a.compare === null ? summaryTable(sets[0]) : compareTable(sets[0], sets[1], [labels[0], labels[1]]);
  const clusters = a.compare === null ? "" : `${clusterTable(sets[0], sets[1], [labels[0], labels[1]])}\n\n${pairedLine(sets[0], sets[1])}`.trim();
  return [table, clusters, a.memoryInfra ? allocatorTable(sets, labels) : ""].filter(Boolean).join("\n\n");
};

async function main(argv: string[]): Promise<number> {
  if (argv.includes("--help")) return console.log("usage: node demos/bench/profile.ts --cell <shape>-<size> --holder <lock name> [--runs <n>] [--compare <ref>] [--out <dir>] [--host <ssh host>] [--chrome <path on the host>] [--seed <u64>] [--memory-infra] [--drag] [--ghosts <n>] [--groups <k>] [--before-edit] [--critical-only] [--build-only]"), 0;
  // The mini's Chrome and page server have no terminal and get no SIGHUP, so a signal here runs the session's close (the same
  // one `profile` awaits), for at most 20 s, then exits 130.
  for (const sig of ["SIGINT", "SIGTERM", "SIGHUP"] as const) {
    process.once(sig, () => {
      interrupted = true;
      void closeSoon().finally(() => process.exit(130));
    });
  }
  const a = parseArgs(argv);
  if (!a) return console.error("usage: node demos/bench/profile.ts --cell <shape>-<size> --holder <lock name> [--runs <n>] [--compare <ref>] [--out <dir>] [--host mini] [--memory-infra] [--drag] [--ghosts <n>] [--groups <k>] [--before-edit] [--critical-only] [--build-only]"), 64;
  const need = missingNeed();
  if (need) return console.error(`profile: ${need}; nothing was built`), 2;
  // The ref is resolved to a sha once, here, before anything is built or run, and printed: pass that sha as `--compare` to compare another cell against the same code.
  const base = a.compare === null ? null : baseline(a.compare);
  if (base !== null) say(`baseline: ${a.compare} = ${base}`);
  const bundles: [string, string][] = base === null ? [["run", build(null, a)]] : [["before", build(base, a)], ["after", build(null, a)]];
  if (a.buildOnly) return bundles.forEach(([label, bundle]) => console.log(`${label}: ${bundle}`)), 0;
  // Only the task that holds the mini's lock measures on it (lifeos `scripts/mini-lock.ts`): a lock that is free, another task's or unreadable stops the run before anything is copied there.
  const who = spawnSync("node", [join(homedir(), "lifeos/scripts/mini-lock.ts"), "who"], { encoding: "utf8", env: { ...process.env, MINI_LOCK_HOST: a.host } });
  const refused = who.status === 0 ? lockProblem(who.stdout, a.holder) : `the mini's lock cannot be read: ${who.stdout}${who.stderr}`.trim();
  if (refused) return console.error(`${refused}; nothing ran on the mini`), 2;
  const stop = async (e: unknown) => {
    if (interrupted) await new Promise(() => undefined); // the handler is closing the session and will exit 130
    throw e;
  };
  dog = watchdog(STALL_MS, (last, ms) => {
    interrupted = true;
    console.error(`no session finished for ${ms / 60_000} minutes; last: ${last || "none"}`);
    void closeSoon().finally(() => process.exit(3));
  });
  const { runs, drags } = await measure(a, bundles).catch(stop);
  dog.stop();
  const labels = a.compare === null ? ["median"] : [a.compare, "this checkout"];
  const opens = [base === null ? "" : `baseline: ${a.compare} = ${base}`, runs && openTables(a, runs, labels)].filter(Boolean).join("\n\n");
  const drag = drags && `${a.groups > 0 ? `drag fixture: grouped by Phase, ${a.groups} groups\n\n` : ""}${a.compare === null ? dragTable(drags.get("run")!, undefined, ["median"]) : dragTable(drags.get("before")!, drags.get("after")!, [labels[0], labels[1]])}`;
  const text = [opens, drag].filter(Boolean).join("\n\n");
  writeFileSync(join(a.out, "summary.md"), `${text}\n`);
  console.log(text);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exit(await main(process.argv.slice(2)));
