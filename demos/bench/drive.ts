// Drive any Gantt the same way, and write one JSON result.
//
//   node demos/bench/drive.ts --url <page> --probe <ours | path/to/side.ts> --rows <n> \
//     [--phases load,scroll-y,scroll-x,drag,memory] [--out <file>] [--headless] [--chrome <path>] \
//     [--width 1280] [--height 800] [--dpr 1] [--refresh-hz 60] [--dnf-s 120] [--dataset-sha256 <hex>] [--drag-commit-s 30] [--network fast-4g|cable]
//
// A fresh browser and profile per run; the GPU and the display rate are read on a blank page first
// (`env.ts`). Then the side's page runs the phases in order, with real input only (Playwright's
// mouse and wheel) and the same steps on every side: load (`load.ts`, until a frame shows `--rows`
// tasks, split at the page's `library-ready` and `data-start` marks), vertical and horizontal scroll
// and drag (`phases.ts`, each checked to have done what it claims; the drag records until the side
// says it is committed, up to `--drag-commit-s`), and memory (`memory.ts`: Chrome's resident set by process type, and the JS heap, after
// load (read `--memory-at-s`, default 20 s, after the navigation, the page idle: the number the table compares) and after the input phases). A side supplies its hooks in a module (`side.ts`); `--probe
// ours` is our own, from `window.gantt` (`sides/ours.ts`). `--phases` picks which are written; all run.
//
// A phase that runs past `--dnf-s` seconds or throws is written `{ "dnf": true, "afterS", "why" }`,
// never left out. Its input stops there, the page is closed, and every later phase is written as a
// dnf that names it: no number is taken from a page an unfinished phase may still be driving. The
// result always holds every phase. Frame numbers are `requestAnimationFrame` gaps: p50, p95, long
// gaps (over 1.5 display periods at `--refresh-hz`) and dropped frames (`frames.ts`). `warnings`
// names anything that makes the run not a result: headless, a software or unknown renderer, a
// display rate off `--refresh-hz`, a debug build, or a phase whose input did not do what it claims.
// `datasetSha256` is what the side says it loaded (its `datasetSha256` hook, asked after the memory
// phase, or just before a phase that did not finish closes the page), null when it does not say; `requestedDatasetSha256` is `--dataset-sha256`, the dataset the run asked for, recorded before
// load, so a load that timed out (and so never let the side say) is still tied to its dataset; `options` are the driver options every run in one table row must share.
//
// Headed by default: run it on the machine with the GPU and a display. Smooth scrolling and Chrome's
// background networking are off for every side, so a wheel event moves the view at once. Chrome is Playwright's own (the
// `@playwright/test` that `apps/hosted` or `web` installs), or `--chrome`, or `$CHROME`.
// Exit code: 0 a result was written (dnf phases included); 64 usage; 70 the browser did not start.

import { existsSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { platform, release } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { launchRetrying } from "../../scripts/lib/launch-retrying.ts";
import { probeBlank, warningsFor, type Environment } from "./env.ts";
import { load, SETTLE_MS } from "./load.ts";
import { applyNetwork, BROTLI_QUALITY, brotliProxy, isNetworkName, NETWORK_PROFILES, warmUp, type NetworkName } from "./network.ts";
import { sampleMemory, type MemorySample } from "./memory.ts";
import { capped, drag, DRAG_COMMIT_MS, NO_BAR, PHASES, scrollX, scrollY, type Dnf, type Phase } from "./phases.ts";
import type { Side } from "./side.ts";

export const EXIT_USAGE = 64;
export const EXIT_NO_BROWSER = 70;
/** Memory is read this long after the navigation: the page is idle and has settled, whatever the side did while it loaded. */
export const MEMORY_AT_S = 20;

/** Every side: a wheel event moves the view at once, not over an animation of the browser's; no background traffic of Chrome's own. */
export const COMMON_ARGS = ["--disable-smooth-scrolling", "--disable-background-networking"];
/** What a headless Chrome needs to draw WebGL at all: the software renderer, as the CI specs run. */
const SOFTWARE_ARGS = ["--ignore-gpu-blocklist", "--use-angle=swiftshader", "--enable-unsafe-swiftshader", "--enable-unsafe-webgpu"];

export interface Options {
  url: string;
  probe: string;
  rows: number;
  phases: Phase[];
  out?: string;
  datasetSha256?: string;
  headless: boolean;
  chrome?: string;
  width: number;
  height: number;
  dpr: number;
  refreshHz: number;
  dnfS: number;
  dragCommitS: number;
  /** The memory read after load is made this many seconds after the navigation, so the page has settled. */
  memoryAtS: number;
  /** A named slow link (`network.ts`) the page runs over, its files brotli-compressed; absent: the box's own link, files as the origin serves them. */
  network?: NetworkName;
}

const VALUED = ["url", "probe", "rows", "phases", "out", "chrome", "width", "height", "dpr", "refresh-hz", "dnf-s", "dataset-sha256", "drag-commit-s", "memory-at-s", "network"] as const;

/** The options, or a reason they are not usable. Each option at most once; `--headless` takes no value. */
export function parseArgs(argv: string[]): Options | string {
  const got = new Map<string, string>();
  let headless = false;
  for (let i = 0; i < argv.length; i++) {
    const name = argv[i].startsWith("--") ? argv[i].slice(2) : "";
    if (name === "headless" && !headless) {
      headless = true;
      continue;
    }
    if (!(VALUED as readonly string[]).includes(name) || got.has(name)) return `unknown or repeated ${JSON.stringify(argv[i])}`;
    const value = argv[++i];
    if (value === undefined || value === "" || value.startsWith("--")) return `--${name} needs a value`;
    got.set(name, value);
  }
  const url = got.get("url");
  const probe = got.get("probe");
  const rows = got.get("rows");
  if (url === undefined || probe === undefined || rows === undefined) return "--url, --probe and --rows are required";
  if (!/^[1-9]\d*$/.test(rows)) return "--rows is not a positive whole number";
  const phases = (got.get("phases") ?? PHASES.join(",")).split(",");
  const bad = phases.find((p) => !(PHASES as readonly string[]).includes(p));
  if (bad !== undefined) return `unknown phase ${JSON.stringify(bad)} (${PHASES.join(", ")})`;
  const num = (name: string, fallback: number): number => {
    const v = got.get(name);
    return v === undefined ? fallback : /^\d+(\.\d+)?$/.test(v) && Number(v) > 0 ? Number(v) : Number.NaN;
  };
  const o: Options = {
    url, probe, rows: Number(rows), phases: phases as Phase[], headless,
    width: num("width", 1280), height: num("height", 800), dpr: num("dpr", 1), refreshHz: num("refresh-hz", 60), dnfS: num("dnf-s", 120),
    dragCommitS: num("drag-commit-s", DRAG_COMMIT_MS / 1000), memoryAtS: num("memory-at-s", MEMORY_AT_S),
  };
  if (got.has("out")) o.out = got.get("out");
  const sha = got.get("dataset-sha256");
  if (sha !== undefined) {
    if (!/^[0-9a-f]{64}$/.test(sha)) return "--dataset-sha256 is not a SHA-256 in hex";
    o.datasetSha256 = sha;
  }
  const network = got.get("network");
  if (network !== undefined) {
    if (!isNetworkName(network)) return `--network is not a profile (${Object.keys(NETWORK_PROFILES).join(", ")})`;
    if (!url.startsWith("http:")) return "--network needs an http: --url";
    o.network = network;
  }
  const chrome = got.get("chrome") ?? process.env.CHROME;
  if (chrome) o.chrome = chrome;
  const nan = (["width", "height", "dpr", "refreshHz", "dnfS", "dragCommitS", "memoryAtS"] as const).find((k) => Number.isNaN(o[k]));
  return nan === undefined ? o : `--${nan.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`)} is not a positive number`;
}

const here = dirname(fileURLToPath(import.meta.url));

/** The side a `--probe` names: `ours`, or a module path whose default export is a Side. */
export async function loadSide(probe: string): Promise<Side> {
  const file = probe === "ours" ? join(here, "sides", "ours.ts") : resolve(probe);
  const side = (await import(pathToFileURL(file).href)).default as Side;
  for (const k of ["ready", "scrollTarget", "leafBars", "dragState"] as const) {
    if (typeof side?.[k] !== "function") throw new Error(`${probe}: the default export has no ${k} hook`);
  }
  return side;
}

/** Playwright's chromium, from whichever of the repo's two packages has it installed. */
export function chromium(): any {
  const root = resolve(here, "../..");
  for (const pkg of ["apps/hosted", "packages/gantt"]) {
    if (!existsSync(join(root, pkg, "node_modules", "@playwright", "test"))) continue;
    return createRequire(join(root, pkg, "package.json"))("@playwright/test").chromium;
  }
  throw new Error("no @playwright/test: run pnpm install in apps/hosted or packages/gantt");
}

/** Every phase a dnf with `why`: the result when the run could not even start. */
const allDnf = (why: string) => Object.fromEntries(PHASES.map((p) => [p, { dnf: true, cause: "error", afterS: 0, why } satisfies Dnf]));

/** Ask an optional hook, or null when the side has none or it fails. */
async function ask<T>(page: any, fn: (() => T) | undefined): Promise<T | null> {
  if (fn === undefined) return null;
  return page.evaluate(fn).catch(() => null);
}

/** A reading of the harness's own that ran out of time: refused with the harness as the cause, never printed as the side's "did not finish". */
export function harnessDnf<T extends object>(m: T | Dnf, what: string): T | Dnf {
  if (!("dnf" in m) || m.dnf !== true || m.cause !== "timeout") return m;
  return { ...m, cause: "harness", why: `the harness: ${what} did not answer in ${m.afterS} s` };
}

/** How long to idle before the memory read: until `atS` seconds after the navigation (epoch ms `origin`), none once that has passed. */
export function memoryWaitMs(origin: number, now: number, atS: number): number {
  return Math.max(0, origin + atS * 1000 - now);
}

/** Run every phase on one page, in order, and return the result object. */
export async function drive(o: Options, side: Side, browser: any, pageUrl = o.url): Promise<object> {
  const context = await browser.newContext({ viewport: { width: o.width, height: o.height }, deviceScaleFactor: o.dpr });
  const blank = await probeBlank(context);
  const page = await context.newPage();
  const cdp = await context.newCDPSession(page);
  await cdp.send("Performance.enable");
  if (o.network) await applyNetwork(cdp, o.network);
  const browserCdp = await browser.newBrowserCDPSession();
  const limit = o.dnfS * 1000;
  const phases: Partial<Record<Phase, object>> = {};
  /** The first phase that did not finish; nothing runs on the page after it. */
  let stopped: Phase | undefined;
  // The side's dataset is asked last, after every memory sample, or just before a stopped phase
  // closes the page: a hook may read the whole file it loaded (ours hashes it), which is not the
  // side's memory. At most once, and capped, so a busy page cannot hold the run.
  let datasetSha256: unknown = null;
  let datasetAsked = false;
  let inputBytes: number | null = null;
  /** An optional hook, capped like every other ask: null when it fails or does not answer in time. */
  const askCapped = async <T>(fn: (() => T) | undefined): Promise<T | null> => {
    const v = await capped(() => ask(page, fn), limit);
    return v !== null && typeof v === "object" && (v as Dnf).dnf === true ? null : (v as T | null);
  };
  const askDataset = async () => {
    if (datasetAsked) return;
    datasetAsked = true;
    const v = await askCapped(side.datasetSha256);
    datasetSha256 = typeof v === "string" ? v : null;
    const bytes = await askCapped(side.inputBytes);
    inputBytes = typeof bytes === "number" ? bytes : null;
  };
  let afterLoad: (MemorySample & { readAtS: number }) | undefined;
  const sample = async () => harnessDnf(await capped(() => sampleMemory(cdp, browserCdp), limit), "the memory sample (CDP or vmmap)");
  const memoryPhase = async () => {
    const afterScripts = await sample();
    return "dnf" in afterScripts ? afterScripts : { dnf: false as const, afterLoad: afterLoad!, afterScripts };
  };
  const step = async <T extends object>(phase: Phase, run: (signal: AbortSignal) => Promise<T | Dnf>, limitMs = limit): Promise<T | Dnf> => {
    const r: T | Dnf = stopped ? { dnf: true, cause: "not-run", afterS: 0, why: `not run: ${stopped} did not finish` } : await capped(run, limitMs);
    if ((r as Dnf).dnf === true && stopped === undefined) {
      // A drag that found no bar sent no drag input: memory is still read before the page closes.
      if (phase === "drag" && (r as Dnf).why.startsWith(NO_BAR) && phases.memory === undefined) phases.memory = await memoryPhase();
      if (phase !== "load") await askDataset();
      stopped = phase;
      // Ends whatever the unfinished phase still waits on in the page; it sends no more input.
      await page.close().catch(() => undefined);
    }
    phases[phase] = r;
    return r;
  };

  // Load's own deadline is --dnf-s; the cap above it only catches a page that never answers at all.
  await step("load", () => load(page, side, pageUrl, o.rows, limit), limit + SETTLE_MS + 10_000);
  const sideBackend = stopped ? null : await askCapped(side.backend);
  const debugBuild = stopped ? null : await askCapped(side.debugBuild);
  const sideBuild = stopped ? null : await askCapped(side.build);
  const rowsOnScreen = stopped ? null : await askCapped(side.rowsOnScreen);
  if (!stopped) {
    // Idle until `memoryAtS` after the navigation: the number is the settled one, the same moment on every side.
    const origin = Number(await page.evaluate("performance.timeOrigin"));
    await new Promise((r) => setTimeout(r, memoryWaitMs(origin, Date.now(), o.memoryAtS)));
    const began = Date.now();
    const m = await sample();
    if ("dnf" in m) phases.memory = m;
    else afterLoad = { ...m, readAtS: Math.round(began - origin) / 1000 }; // when the read began: the table refuses one made late (`results.ts`)
  }
  await step("scroll-y", (signal) => scrollY(page, side, o.refreshHz, o.height, signal));
  await step("scroll-x", (signal) => scrollX(page, side, o.refreshHz, o.width, signal));
  await step("drag", (signal) => drag(page, side, o.refreshHz, signal, o.dragCommitS * 1000));
  // The sample inside is capped at `limit` and says the harness did not answer; this cap only backs it up.
  if (phases.memory === undefined) await step("memory", memoryPhase, limit + 10_000);
  if (!stopped) await askDataset();
  await context.close();

  const env: Environment = {
    chrome: String(browser.version()),
    ...blank,
    viewport: { width: o.width, height: o.height },
    refreshHz: o.refreshHz,
    headless: o.headless,
    os: `${platform()} ${release()}`,
    node: process.version,
    sideBackend,
    debugBuild,
    sideBuild,
    rowsOnScreen,
  };
  const unverified = Object.entries(phases).filter(([, r]) => (r as { verified?: boolean }).verified === false);
  return {
    tool: "drive",
    url: o.url,
    side: side.name,
    rows: o.rows,
    date: new Date().toISOString(),
    environment: env,
    // What the table checks is equal on every run it combines, beside the environment.
    options: { width: o.width, height: o.height, dpr: o.dpr, refreshHz: o.refreshHz, dnfS: o.dnfS, memoryAtS: o.memoryAtS, headless: o.headless, phases: o.phases, ...(o.network && { network: o.network, brotliQuality: BROTLI_QUALITY }) },
    requestedDatasetSha256: o.datasetSha256 ?? null,
    datasetSha256: typeof datasetSha256 === "string" ? datasetSha256 : null,
    inputBytes,
    warnings: [...warningsFor(env), ...unverified.map(([p, r]) => `${p}: ${(r as { why?: string }).why}`)],
    phases: Object.fromEntries(o.phases.map((p) => [p, phases[p]])),
  };
}

async function main(argv: string[]): Promise<number> {
  const o = parseArgs(argv);
  if (typeof o === "string") {
    console.error(`${o}\nusage: node drive.ts --url <page> --probe <ours|side.ts> --rows <n> [--phases ${PHASES.join(",")}] [--out file] [--headless] [--chrome path] [--width 1280] [--height 800] [--dpr 1] [--refresh-hz 60] [--dnf-s 120] [--dataset-sha256 hex] [--drag-commit-s 30] [--memory-at-s 20] [--network ${Object.keys(NETWORK_PROFILES).join("|")}]`);
    return EXIT_USAGE;
  }
  let side: Side;
  try {
    side = await loadSide(o.probe);
  } catch (e) {
    console.error(e instanceof Error ? e.message : String(e));
    return EXIT_USAGE;
  }
  let browser: any;
  let proxy: Awaited<ReturnType<typeof brotliProxy>> | undefined;
  const at = new URL(o.url);
  try {
    const launch: Record<string, unknown> = { headless: o.headless, args: [...COMMON_ARGS, ...(o.headless ? SOFTWARE_ARGS : [])] };
    if (o.chrome) launch.executablePath = o.chrome;
    else launch.channel = "chromium";
    if (o.network) {
      proxy = await brotliProxy(o.url);
      at.host = new URL(proxy.url).host;
      await warmUp(proxy, () => launchRetrying(() => chromium().launch(launch)), at.href);
    }
    browser = await launchRetrying(() => chromium().launch(launch));
  } catch (e) {
    proxy?.close();
    console.error(`the browser did not start: ${e instanceof Error ? e.message : String(e)}`);
    return EXIT_NO_BROWSER;
  }
  let result: object;
  try {
    result = await drive(o, side, browser, at.href);
  } catch (e) {
    // Never a blank result: a run that broke outside any phase still says so in every phase.
    const why = `error: ${e instanceof Error ? e.message.split("\n")[0] : String(e)}`;
    result = { tool: "drive", url: o.url, side: side.name, rows: o.rows, date: new Date().toISOString(), environment: null, warnings: [why], phases: allDnf(why) };
  } finally {
    proxy?.close();
    await browser.close().catch(() => undefined);
  }
  if (proxy?.late.length) (result as { warnings?: string[] }).warnings?.push(`network: ${proxy.late.length} file(s) were compressed during the run, not before it: ${proxy.late.join(", ")}`);
  const json = `${JSON.stringify(result, null, 2)}\n`;
  if (o.out) writeFileSync(o.out, json);
  else process.stdout.write(json);
  return 0;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
