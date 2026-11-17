import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));

/** A load.log whose load1 is `load` throughout, from unix second `start`; swap is `swap` kB at the first sample, `swapEnd` at the last. */
function log(load: number, swap = 100, swapEnd = swap, start = 1000): string {
  return [0, 3, 6].map((t, i) => `${start + t} ${load.toFixed(2)} 500000 ${i === 2 ? swapEnd : swap}`).join("\n") + "\n";
}

function put(root: string, cell: string, pair: string, variant: string, text: string): void {
  const dir = join(root, cell, pair, variant);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "load.log"), text);
}

function table(root: string) {
  const r = spawnSync(process.execPath, [join(here, "table.ts"), root], { encoding: "utf8" });
  return { code: r.status, out: r.stdout + r.stderr };
}

function tmp(): string {
  return mkdtempSync(join(tmpdir(), "table-test-"));
}

test("a pair 0.3 apart is matched, a pair 1.2 apart is not", () => {
  const root = tmp();
  putCell(root, "cell");
  putRun(root, "cell", "1", "a", 2.0, result());
  putRun(root, "cell", "1", "b", 2.3, result());
  putRun(root, "cell", "2", "a", 2.0, result());
  putRun(root, "cell", "2", "b", 3.2, result());
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /cell {2}1 .*diff 0\.30 matched$/m);
  assert.match(out, /cell {2}2 .*diff 1\.20 not matched$/m);
  assert.match(out, /cell: 1 matched of 2 attempts \(unmatched/);
  assert.match(out, process.platform === "linux" ? /^\/proc\/loadavg: \d/m : /^loadavg: \d/m);
});

test("the last line is /proc/loadavg where it exists and os.loadavg() where it does not", async () => {
  const { loadLine } = await import("./table.ts");
  assert.equal(loadLine("3.52 2.10 1.00 2/300 999\n", [9, 9, 9]), "/proc/loadavg: 3.52 2.10 1.00 2/300 999\n");
  assert.equal(loadLine(null, [2.5, 2, 1.25]), "loadavg: 2.50 2.00 1.25\n");
});

test("a swap figure that grows during a run exits 2", () => {
  const root = tmp();
  put(root, "cell", "1", "a", log(2.0, 100, 900));
  put(root, "cell", "1", "b", log(2.1));
  const { code, out } = table(root);
  assert.equal(code, 2, out);
  assert.match(out, /swap grew/);
});

test("an empty results directory exits 1 with no rows", () => {
  const { code, out } = table(tmp());
  assert.equal(code, 1, out);
  assert.match(out, /no rows/);
});

test("a results directory with no load.log exits 1", () => {
  const root = tmp();
  mkdirSync(join(root, "cell", "1", "a"), { recursive: true });
  const { code, out } = table(root);
  assert.equal(code, 1, out);
  assert.match(out, /no rows/);
});

test("pair.ts reads load1 and swap, and tells a job from the runner daemon", async () => {
  const p = await import("./pair.ts");
  assert.equal(p.parseLoad1("3.52 2.10 1.00 2/300 999\n"), 3.52);
  assert.equal(p.parseSwapUsedKb("SwapTotal:  1000 kB\nSwapFree:   400 kB\n"), 600);
  assert.equal(p.isRunnerJob("gitlab-runner\0run\0"), false);
  assert.equal(p.isRunnerJob("gitlab-runner\0exec\0shell\0"), true);
});

test("pair.ts writes wait.log and load.log that table.ts accepts", () => {
  const root = tmp();
  const bundle = join(root, "bundle");
  mkdirSync(bundle);
  writeFileSync(join(bundle, "index.html"), "<p>x</p>");
  const out = join(root, "out");
  const args = [join(here, "pair.ts"), "--out", out, "--cell", "c", "--pair", "1", "--a", `a=${bundle}`, "--b", `b=${bundle}`, "--wait-s", "60", "--max-load", "1000", "--", process.execPath, "-e", "0"];
  const r = spawnSync(process.execPath, args, { encoding: "utf8" });
  assert.equal(r.status, 0, r.stdout + r.stderr);
  const t = table(out);
  // This reads the real box: its swap may grow while the pair runs, which the table reports with exit 2.
  assert.ok(t.code === 0 || (t.code === 2 && /swap grew/.test(t.out)), t.out);
  assert.match(t.out, /c {2}1 /);
});

test("only the first 8 pairs of a cell count, and numbers sort as numbers", () => {
  const root = tmp();
  for (let n = 1; n <= 10; n++) {
    put(root, "cell", String(n), "a", log(2.0));
    put(root, "cell", String(n), "b", log(n > 8 ? 2.1 : 3.0));
  }
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /cell: 0 matched of 8 attempts, 2 past the cap of 8 not counted \(unmatched/);
});

function pairRun(extra: string[], cmd: string[]) {
  const root = tmp();
  const bundle = join(root, "bundle");
  mkdirSync(bundle);
  const args = [join(here, "pair.ts"), "--out", join(root, "out"), "--cell", "c", "--pair", "1", "--a", `a=${bundle}`, "--b", `b=${bundle}`, ...extra, "--", ...cmd];
  return spawnSync(process.execPath, args, { encoding: "utf8" });
}

test("pair.ts exits 70 when the command cannot start and 1 when it fails", () => {
  const missing = pairRun(["--wait-s", "60", "--max-load", "1000"], ["/nonexistent/measure"]);
  assert.equal(missing.status, 70, missing.stderr);
  assert.match(missing.stderr, /cannot start/);
  const failing = pairRun(["--wait-s", "60", "--max-load", "1000"], [process.execPath, "-e", "process.exit(3)"]);
  assert.equal(failing.status, 1, failing.stderr);
  assert.match(failing.stderr, /exit 1/);
});

const HASH = "ab".repeat(32);
const ENV = { chrome: "150.0.1", os: "linux 6.1", node: "v22", gpu: { vendor: "V", renderer: "R" }, viewport: { width: 1280, height: 800 }, dpr: 1, refreshHz: 60 };

type Opts = { loadMs?: number; p95?: number; mb?: number; kind?: string; warnings?: string[] | null; sha?: string | null; chrome?: string; phases?: Record<string, object> };

/** A driver result (`demos/bench/drive.ts`): load `loadMs`, every frame phase at p95 `p95`, memory `mb` MB of `kind`, dataset `sha`. */
function result(o: Opts = {}): object {
  const frames = { dnf: false, verified: true, frames: 240, p50: 16.7, p95: o.p95 ?? 17, longGaps: 0, droppedFrames: 0, max: 20 };
  const mem = { kind: o.kind ?? "pss", chromeBytes: (o.mb ?? 400) * 1e6, rendererBytes: 1e8, gpuProcessBytes: 1e8, jsHeapUsedBytes: 1e7 };
  const r: Record<string, unknown> = {
    tool: "drive", side: "x", rows: 10000, environment: { ...ENV, chrome: o.chrome ?? ENV.chrome }, options: { dnfS: 120, phases: ["load"] },
    requestedDatasetSha256: HASH,
    datasetSha256: o.sha === undefined ? HASH : o.sha,
    phases: { load: { dnf: false, ms: o.loadMs ?? 800 }, "scroll-y": frames, "scroll-x": frames, drag: { ...frames, task: "1", commitMs: 25 }, memory: { dnf: false, afterLoad: mem, afterScripts: mem }, ...o.phases },
  };
  if (o.warnings !== null) r.warnings = o.warnings ?? [];
  return r;
}

/** The cell's dataset hash file, as run.ts writes it. */
function putCell(root: string, cell: string, hash = HASH): void {
  mkdirSync(join(root, cell), { recursive: true });
  writeFileSync(join(root, cell, "dataset.json.sha256"), `${hash}  dataset.json\n`);
}

/** The side.json run.ts writes beside a run: the side's build. `null` writes none. */
const BUILD = { probe: "ours", path: "?sample={size}", bundleSha256: "11".repeat(32), harness: "c0ffee" };

/** One side of a pair: its load.log at load1 `load` from `start`, its driver result and its build. */
function putRun(root: string, cell: string, pair: string, side: string, load: number, r: object, start = 1000, swapEnd = 100, build: object | null = {}): void {
  put(root, cell, pair, side, log(load, 100, swapEnd, start));
  writeFileSync(join(root, cell, pair, side, "result.json"), JSON.stringify(r));
  if (build !== null) writeFileSync(join(root, cell, pair, side, "side.json"), JSON.stringify({ name: side, ...BUILD, ...build }));
}

/** Pairs 1..n of a cell, a then b on odd pairs and b then a on even, each side's result from `a(n)` and `b(n)`, a's build from `build(n)`. */
function pairs(root: string, cell: string, n: number, a: (k: number) => object = () => result(), b: (k: number) => object = () => result(), build: (k: number) => object | null = () => ({})): void {
  putCell(root, cell);
  for (let k = 1; k <= n; k++) {
    putRun(root, cell, String(k), "a", 2.0, a(k), k % 2 === 1 ? 1000 : 1010, 100, build(k));
    putRun(root, cell, String(k), "b", 2.1, b(k), k % 2 === 1 ? 1010 : 1000);
  }
}

const HEADER_RE = /^ {2}side +pairs +first +rows on screen +library median ms +data to ready median ms +data to settled median ms +load median ms +load max ms/m;

test("a result with warnings is refused: its pair is not counted and the table exits 3", () => {
  const root = tmp();
  putCell(root, "cell");
  putRun(root, "cell", "1", "a", 2.0, result({ warnings: ["software renderer: SwiftShader"] }));
  putRun(root, "cell", "1", "b", 2.1, result());
  const { code, out } = table(root);
  assert.equal(code, 3, out);
  assert.match(out, /refused: cell\/1\/a: software renderer: SwiftShader/);
  assert.match(out, /cell {2}1 .*refused/);
  assert.match(out, /cell: 0 matched of 1 attempts \(unmatched/);
});

test("a matched cell prints each side's median and max over its matched pairs, who ran first, and the dataset hash", () => {
  const root = tmp();
  pairs(root, "tree4-10000", 4, (k) => result({ loadMs: 100 * k, p95: 16 + k, mb: 300 }), (k) => result({ loadMs: 1000 * k, p95: 30, mb: 500 }));
  // Load 3.0 against 2.0: not matched, so its numbers are not in the table.
  putRun(root, "tree4-10000", "5", "a", 2.0, result({ loadMs: 99999 }));
  putRun(root, "tree4-10000", "5", "b", 3.0, result());
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, new RegExp(`tree4-10000: dataset sha256 ${HASH}`));
  assert.match(out, /tree4-10000: 4 matched of 5 attempts$/m);
  assert.match(out, /^ {2}a +4 +2 +- +- +- +- +250\.0 +400\.0 +18\.5 +0\.0 +20\.0 +- +18\.5 +0\.0 +20\.0 +- +18\.5 +0\.0 +20\.0 +25\.0 +300\.0 MB PSS$/m);
  assert.match(out, /^ {2}b +4 +2 +- +- +- +- +2500\.0 +4000\.0 +30\.0 +0\.0 +20\.0 +- +30\.0 +0\.0 +20\.0 +- +30\.0 +0\.0 +20\.0 +25\.0 +500\.0 MB PSS$/m);
  assert.doesNotMatch(out, /99999/);
  assert.match(out, /no warm-up run discarded/);
  assert.match(out, /memory: .*leaves out GPU memory/);
});

test("in a cell with results, a pair where neither run has result.json is not matched, and the table still prints", () => {
  const root = tmp();
  pairs(root, "cell", 4);
  put(root, "cell", "5", "a", log(2.0));
  put(root, "cell", "5", "b", log(2.1));
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /cell {2}5 .*not matched: no result\.json from a and b$/m);
  assert.match(out, /cell: 4 matched of 5 attempts$/m);
});

test("a cell where no run has result.json matches no pair: a missing result is unknown, never matched", () => {
  const root = tmp();
  for (let k = 1; k <= 4; k++) {
    put(root, "cell", String(k), "a", log(2.0));
    put(root, "cell", String(k), "b", log(2.1));
  }
  const { out } = table(root);
  assert.match(out, /cell: 0 matched of 4 attempts \(unmatched: needs 4\)$/m);
  for (let k = 1; k <= 4; k++) assert.match(out, new RegExp(`cell {2}${k} .*not matched: no result\\.json from a and b$`, "m"));
});

test("a pair where one run has no result.json says why it is not matched", () => {
  const root = tmp();
  putCell(root, "cell");
  putRun(root, "cell", "1", "a", 2.0, result());
  put(root, "cell", "1", "b", log(2.1));
  assert.match(table(root).out, /cell {2}1 .*diff 0\.10 not matched: no result\.json from b$/m);
});

test("a pair whose swap grew is not matched and its numbers stay out, inside a cell that still has 4", () => {
  const root = tmp();
  pairs(root, "cell", 5, () => result({ loadMs: 100 }), (k) => result({ loadMs: k === 3 ? 99999 : 200 }));
  putRun(root, "cell", "3", "b", 2.1, result({ loadMs: 99999 }), 1010, 900);
  const { code, out } = table(root);
  assert.equal(code, 2, out);
  assert.match(out, /cell {2}3 .*swap grew/);
  assert.match(out, /cell: 4 matched of 5 attempts$/m);
  assert.match(out, /^ {2}b +4 +\d +- +- +- +- +200\.0 +200\.0 /m);
  assert.doesNotMatch(out, /99999/);
});

test("a refused outlier inside a matched cell is left out of the numbers", () => {
  const root = tmp();
  pairs(root, "cell", 5, (k) => result({ loadMs: k === 2 ? 99999 : 100, warnings: k === 2 ? ["debug build: the side says this page is a development build"] : [] }));
  const { code, out } = table(root);
  assert.equal(code, 3, out);
  assert.match(out, /cell: 4 matched of 5 attempts$/m);
  assert.match(out, /^ {2}a +4 +\d +- +- +- +- +100\.0 +100\.0 /m);
  assert.doesNotMatch(out, /99999/);
  assert.match(out, /refused: cell\/2\/a: debug build/);
});

test("a cell without a recorded hash, or a side that loaded another dataset or did not say, prints no numbers", () => {
  const missing = tmp();
  for (let k = 1; k <= 4; k++) {
    putRun(missing, "cell", String(k), "a", 2.0, result());
    putRun(missing, "cell", String(k), "b", 2.0, result());
  }
  const m = table(missing);
  assert.equal(m.code, 3, m.out);
  assert.match(m.out, /cell: dataset sha256 not recorded: no numbers/);
  assert.match(m.out, /refused: cell\/1\/a: the cell has no dataset\.json\.sha256/);
  assert.doesNotMatch(m.out, HEADER_RE);

  const other = tmp();
  pairs(other, "cell", 4, (k) => result({ sha: k === 1 ? "cd".repeat(32) : HASH }), (k) => result({ sha: k === 2 ? null : HASH }));
  const o = table(other);
  assert.equal(o.code, 3, o.out);
  assert.match(o.out, new RegExp(`refused: cell/1/a: the side loaded dataset ${"cd".repeat(32)}, not the cell's ${HASH}`));
  assert.match(o.out, /refused: cell\/2\/b: the side did not report the dataset it loaded/);
  assert.match(o.out, /cell: 2 matched of 4 attempts \(unmatched/);
  assert.doesNotMatch(o.out, HEADER_RE);
});

test("only a timeout prints as did not finish; a phase that threw or never got ready is refused with its why", () => {
  const timeout = { "scroll-y": { dnf: true, cause: "timeout", afterS: 120, why: "the phase did not finish" } };
  const notRun = (p: string) => ({ [p]: { dnf: true, cause: "not-run", afterS: 0, why: "not run: scroll-y did not finish" } });
  const slow = tmp();
  pairs(slow, "cell", 4, () => result(), () => result({ phases: { ...timeout, ...notRun("scroll-x"), ...notRun("drag"), ...notRun("memory") } }));
  const s = table(slow);
  assert.equal(s.code, 0, s.out);
  assert.match(s.out, /^ {2}b +4 .*(did not finish in 120 s \(4\/4\) +){4}(not run \(4\/4\) +){8}not run \(4\/4\)$/m);

  const broken = tmp();
  const threw = { "scroll-y": { dnf: true, cause: "error", afterS: 0.2, why: "error: no scroll box here" } };
  const notReady = { load: { dnf: true, cause: "not-ready", afterS: 120, why: "no ready frame with 10000 tasks" } };
  pairs(broken, "cell", 4, (k) => result(k === 1 ? { phases: threw } : {}), (k) => result(k === 2 ? { phases: notReady } : {}));
  const b = table(broken);
  assert.equal(b.code, 3, b.out);
  assert.match(b.out, /refused: cell\/1\/a: scroll-y: error: error: no scroll box here/);
  assert.match(b.out, /refused: cell\/2\/b: load: not-ready: no ready frame with 10000 tasks/);
  assert.doesNotMatch(b.out, /did not finish in/);
  assert.doesNotMatch(b.out, HEADER_RE);
});

test("runs from different environments are not combined into one row", () => {
  const root = tmp();
  pairs(root, "cell", 4, (k) => result({ chrome: k === 4 ? "151.0.0" : ENV.chrome }));
  const { code, out } = table(root);
  assert.equal(code, 3, out);
  assert.match(out, /cell: environments differ, not combined into one row:/);
  assert.match(out, /151\.0\.0/);
  assert.doesNotMatch(out, HEADER_RE);
});

test("an unmatched cell prints no side row", () => {
  const root = tmp();
  pairs(root, "few", 1);
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /few: 1 matched of 1 attempts \(unmatched: needs 4\)\n {2}no numbers/);
  assert.doesNotMatch(out, HEADER_RE);
  assert.doesNotMatch(out, /^ {2}a /m);
});

test("memory of two kinds is not compared, and a result with no warnings field is refused", () => {
  const root = tmp();
  pairs(root, "mixed", 4, () => result({ kind: "pss" }), () => result({ kind: "rss" }));
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /mixed: memory not comparable \(PSS and RSS\)/);
  assert.match(out, /^ {2}a +4 +\d +- +- +- +- +800\.0 +800\.0 +17\.0 +0\.0 +20\.0 +- +17\.0 +0\.0 +20\.0 +- +17\.0 +0\.0 +20\.0 +25\.0 +-$/m);

  const bare = tmp();
  pairs(bare, "cell", 1, () => result({ warnings: null }));
  const b = table(bare);
  assert.equal(b.code, 3, b.out);
  assert.match(b.out, /refused: cell\/1\/a: no warnings field/);
});

test("a load that timed out prints as did not finish, tied to the dataset the run asked for", () => {
  const timedOut = (requested: string | null) => ({
    ...result({ sha: null }),
    requestedDatasetSha256: requested,
    phases: {
      load: { dnf: true, cause: "timeout", afterS: 120, why: "the page did not answer" },
      ...Object.fromEntries(["scroll-y", "scroll-x", "drag", "memory"].map((p) => [p, { dnf: true, cause: "not-run", afterS: 0, why: "not run: load did not finish" }])),
    },
  });
  const root = tmp();
  pairs(root, "cell", 4, () => result(), () => timedOut(HASH));
  const { code, out } = table(root);
  assert.equal(code, 0, out);
  assert.match(out, /^ {2}b +4 +\d +- +(did not finish in 120 s \(4\/4\) +){5}not run \(4\/4\)/m);

  const unasked = tmp();
  pairs(unasked, "cell", 1, () => result(), () => timedOut(null));
  const u = table(unasked);
  assert.equal(u.code, 3, u.out);
  assert.match(u.out, /refused: cell\/1\/b: the run did not record the dataset it asked for/);
});

test("a side rebuilt between pairs, or another harness version, is not combined into one row", () => {
  const rebuilt = tmp();
  pairs(rebuilt, "cell", 4, () => result(), () => result(), (k) => (k === 3 ? { bundleSha256: "22".repeat(32) } : {}));
  const r = table(rebuilt);
  assert.equal(r.code, 3, r.out);
  assert.match(r.out, /cell: side a ran more than one build, not combined into one row:/);
  assert.match(r.out, /2222/);
  assert.doesNotMatch(r.out, HEADER_RE);

  const harness = tmp();
  pairs(harness, "cell", 4, () => result(), () => result(), (k) => (k === 4 ? { harness: "beef" } : {}));
  const h = table(harness);
  assert.equal(h.code, 3, h.out);
  assert.match(h.out, /cell: environments differ/);
  assert.doesNotMatch(h.out, HEADER_RE);

  const unknown = tmp();
  pairs(unknown, "cell", 1, () => result(), () => result(), () => null);
  const n = table(unknown);
  assert.equal(n.code, 3, n.out);
  assert.match(n.out, /refused: cell\/1\/a: no side\.json beside the result/);
});

test("a pair whose two sides dragged different tasks of the dataset is refused, both runs, with why", () => {
  const drag = (task: string | null) => ({ drag: { dnf: false, verified: true, frames: 240, p50: 16.7, p95: 17, longGaps: 0, droppedFrames: 0, max: 20, task } });
  const root = tmp();
  // Pairs 1 to 4 drag task 1 on both sides; pair 5 drags task 2 on b; pair 6 records no task on a.
  pairs(root, "cell", 6, (k) => result({ phases: drag(k === 6 ? null : "1") }), (k) => result({ phases: drag(k === 5 ? "2" : "1") }));
  const { code, out } = table(root);
  assert.equal(code, 3, out);
  for (const side of ["a", "b"]) assert.match(out, new RegExp(`refused: cell/5/${side}: drag: the sides dragged different tasks of the dataset \\(a task 1, b task 2\\)`));
  assert.match(out, /refused: cell\/6\/a: drag: a did not record which task of the dataset it dragged/);
  assert.match(out, /cell {2}5 .*refused/);
  assert.match(out, /cell: 4 matched of 6 attempts/);
});
