// The benchmark driver: its numbers, its options, its fairness rules, and one full run against the
// hosted build (`apps/hosted/dist`, which `pnpm build` in `apps/hosted` writes, samples included;
// the `hosted` CI job runs this after its build). The runs are headless, so they are smoke runs:
// they check the driver works, not how fast a side is. The fixture side (written to a temporary
// directory) drives a small `data:` page whose `data-mode` makes it stall, throw or ignore input.

import assert from "node:assert/strict";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { after, test } from "node:test";
import { fileURLToPath } from "node:url";
import { harnessDnf, memoryWaitMs, parseArgs } from "./drive.ts";
import { splitLoad } from "./load.ts";
import { warningsFor } from "./env.ts";
import { percentile, stopRecorder, summarise } from "./frames.ts";
import { memoryKb, parseFootprint, parsePsRss, parseSmapsRollup } from "./memory.ts";
import { writeDataset } from "./run.ts";
import { adaptInto } from "./sides/ours-adapt.ts";
import { capped, checkSweep, PHASES, SCROLL_STEPS, SCROLL_X_VIEWS, SCROLL_Y_VIEWS, scrollY, type DrivenPage } from "./phases.ts";

const here = dirname(fileURLToPath(import.meta.url));
const hosted = resolve(here, "../../apps/hosted");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Run the CLI; its exit code, its stderr and the result it wrote, if any. */
function drive(args: string[], timeoutS: number) {
  const file = join(mkdtempSync(join(tmpdir(), "bench-drive-")), "result.json");
  const r = spawnSync(process.execPath, [join(here, "drive.ts"), "--memory-at-s", "0.1", ...args, "--out", file], { encoding: "utf8", timeout: timeoutS * 1000 });
  return { status: r.status, stderr: r.stderr, result: existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : undefined };
}

test("p50, p95, long gaps (over 1.5 periods) and dropped frames", () => {
  const gaps = [...Array(90).fill(16.7), ...Array(8).fill(20), 25, 50];
  assert.deepEqual(summarise(gaps, 60), { frames: 100, p50: 16.7, p95: 20, longGaps: 1, droppedFrames: 3, max: 50 });
  // 25 ms is 1.5 periods: not a long gap (over 1.5), but it rounds to two periods, one dropped; 50 ms drops two.
  assert.equal(summarise([30], 30).longGaps, 0, "at 30 Hz, 30 ms is one period");
  assert.equal(summarise([100], 60).droppedFrames, 5, "100 ms at 60 Hz is six periods: five dropped");
  assert.equal(percentile([], 50), Number.NaN);
  assert.equal(percentile([3, 1, 2], 50), 2);
});

test("the options: --url, --probe and --rows required, phases checked, numbers positive", () => {
  const ok = parseArgs(["--url", "http://x/", "--probe", "ours", "--rows", "50000"]);
  assert.ok(typeof ok === "object");
  assert.deepEqual([ok.phases, ok.rows, ok.dnfS, ok.refreshHz, ok.headless, ok.dragCommitS], [[...PHASES], 50000, 120, 60, false, 30]);
  // The memory read waits until `memoryAtS` after the navigation, not that long after load; a page already older is read at once.
  assert.deepEqual([ok.memoryAtS, memoryWaitMs(1_000_000, 1_000_400, 20), memoryWaitMs(1_000_000, 1_030_000, 20)], [20, 19_600, 0]);
  const picked = parseArgs(["--url", "u", "--probe", "p", "--rows", "1", "--phases", "load,drag", "--headless", "--dnf-s", "5"]);
  assert.ok(typeof picked === "object");
  assert.deepEqual([picked.phases, picked.headless, picked.dnfS], [["load", "drag"], true, 5]);
  const asked = parseArgs(["--url", "u", "--probe", "p", "--rows", "1", "--dataset-sha256", "ab".repeat(32)]);
  assert.ok(typeof asked === "object");
  assert.equal(asked.datasetSha256, "ab".repeat(32));
  for (const bad of [
    ["--probe", "ours", "--rows", "1"],
    ["--url", "u", "--rows", "1"],
    ["--url", "u", "--probe", "ours"],
    ["--url", "u", "--probe", "ours", "--rows", "0"],
    ["--url", "u", "--probe", "ours", "--rows", "1.5"],
    ["--url", "u", "--probe", "ours", "--rows", "1", "--phases", "load,spin"],
    ["--url", "u", "--probe", "ours", "--rows", "1", "--dnf-s", "0"],
    ["--url", "u", "--probe", "ours", "--rows", "1", "--url", "v"],
    ["--url", "--probe", "ours", "--rows", "1"],
    ["--url", "u", "--probe", "ours", "--rows", "1", "--colour", "red"],
    ["--url", "u", "--probe", "ours", "--rows", "1", "--dataset-sha256", "abc"],
  ]) {
    assert.equal(typeof parseArgs(bad), "string", bad.join(" "));
  }
  const r = spawnSync(process.execPath, [join(here, "drive.ts"), "--url", "u"], { encoding: "utf8" });
  assert.equal(r.status, 64);
  assert.match(r.stderr, /usage: node drive\.ts/);
});

test("a headless run, a software or unknown renderer, a rate off --refresh-hz and a debug build are warned about", () => {
  const base = { headless: false, gpu: { vendor: "Apple", renderer: "ANGLE (Apple, Apple M2, OpenGL 4.1)" }, refreshHz: 60, measuredHz: 59.9, debugBuild: false };
  assert.deepEqual(warningsFor(base), []);
  assert.deepEqual(warningsFor({ ...base, debugBuild: null }), [], "a side that does not say is not warned about");
  assert.match(warningsFor({ ...base, headless: true }).join(), /headless/);
  assert.match(warningsFor({ ...base, gpu: { ...base.gpu, renderer: "ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device))" } }).join(), /software renderer/);
  assert.match(warningsFor({ ...base, gpu: { ...base.gpu, renderer: null } }).join(), /unknown acceleration/);
  assert.match(warningsFor({ ...base, measuredHz: 120 }).join(), /120 Hz/);
  assert.match(warningsFor({ ...base, debugBuild: true }).join(), /debug build/);
});

test("a sweep is verified only when it reached its target and came back", () => {
  assert.deepEqual(checkSweep(0, 1000, 0, 1000), { verified: true });
  assert.deepEqual(checkSweep(-12.8, 5107.2, -12.8, 5120), { verified: true });
  assert.equal(checkSweep(0, 400, 0, 1000).verified, false, "stopped short");
  assert.match(checkSweep(0, 1000, 300, 1000).why ?? "", /came back to 300/);
  assert.equal(checkSweep(0, 0, 0, 0).verified, false, "nothing to scroll");
  // 50k rows: the outward check keeps its scaled tolerance; the return is held to a fixed one, so 250 rows short is not back.
  assert.equal(checkSweep(0, 1_600_000 - 5000, 0, 1_600_000).verified, true, "out within 0.5%");
  assert.match(checkSweep(0, 1_600_000, 7000, 1_600_000).why ?? "", /came back to 7000/);
});

test("a memory sample that timed out is the harness's, refused with its reason, never the side's did-not-finish", () => {
  const late = { dnf: true, cause: "timeout", afterS: 120, why: "the phase did not finish" } as const;
  assert.deepEqual(harnessDnf(late, "the memory sample (CDP or vmmap)"), { dnf: true, cause: "harness", afterS: 120, why: "the harness: the memory sample (CDP or vmmap) did not answer in 120 s" });
  assert.equal(harnessDnf({ dnf: true, cause: "error", afterS: 1, why: "error: x" }, "y").cause, "error", "only a timeout is rewritten");
});

test("ps output parses to pid and resident kB", () => {
  assert.deepEqual([...parsePsRss("  216 120344\n  294  98120\n\n")], [[216, 120344], [294, 98120]]);
});

test("PSS from smaps_rollup and the physical footprint from vmmap parse to kB", () => {
  const rollup = "55d0c0de0000-7ffd00000000 ---p 00000000 00:00 0  [rollup]\nRss:              120344 kB\nPss:               88012 kB\nPss_Anon:          70000 kB\n";
  assert.equal(parseSmapsRollup(rollup), 88012);
  assert.equal(parseSmapsRollup("Rss: 1 kB\n"), null);
  const vmmap = "Process:         Chromium Helper (Renderer) [4242]\nPhysical footprint:         123.5M\nPhysical footprint (peak):  140.0M\n";
  assert.equal(parseFootprint(vmmap), 126464);
  assert.equal(parseFootprint("Physical footprint:         2.0G\n"), 2 * 1024 * 1024);
  assert.equal(parseFootprint("Physical footprint:         900K\n"), 900);
  assert.equal(parseFootprint("Physical footprint (peak):  140.0M\n"), null, "the peak is not the footprint");
});

test("this process's own memory is read as PSS on Linux, never a mix of kinds", { skip: process.platform !== "linux" }, () => {
  const { kind, kb } = memoryKb([process.pid]);
  assert.equal(kind, "pss");
  assert.ok((kb.get(process.pid) ?? 0) > 0);
});

/** A page that answers every call after `ms`, counting the wheel events it is sent. */
function fakePage(ms: number) {
  const sent = { wheels: 0 };
  const page: DrivenPage = {
    goto: async () => undefined,
    waitForLoadState: async () => undefined,
    evaluate: async (fn: unknown) => {
      await sleep(ms);
      return (fn === stopRecorder ? [] : { x: 0, y: 0, width: 100, height: 100, top: 0, left: 0, maxTop: 100_000, maxLeft: 100_000 }) as any;
    },
    keyboard: { down: async () => undefined, up: async () => undefined },
    mouse: { move: async () => undefined, down: async () => undefined, up: async () => undefined, wheel: async () => void sent.wheels++ },
  };
  return { page, sent };
}
const fakeSide = { name: "fake", ready: () => true, scrollTarget: () => ({}) as any, leafBars: () => ({ bars: [], pxPerDay: 1 }), dragState: () => "" };

test("a phase past its limit is a dnf, and it sends no more input after it", async () => {
  const { page, sent } = fakePage(5);
  const r = await capped((signal) => scrollY(page, fakeSide, 60, 800, signal), 150);
  assert.deepEqual(r, { dnf: true, cause: "timeout", afterS: 0.15, why: "the phase did not finish" });
  const at = sent.wheels;
  assert.ok(at > 0 && at < SCROLL_STEPS, `${at} wheel events before the limit`);
  await sleep(400);
  assert.equal(sent.wheels, at, "the timed-out phase kept sending wheel events into the page");
});

test("a phase that throws is a dnf with the error as why", async () => {
  const r = await capped(async () => {
    throw new Error("hook failed\nat somewhere");
  }, 1000);
  assert.equal(r.dnf, true);
  assert.equal((r as { why: string }).why, "error: hook failed");
});

// The fixture side: a page that shows `data-rows` tasks and, by `data-mode`, stalls or throws in its
// scroll hook, or ignores all input (its view never moves and no drag changes anything). In "late"
// mode the page sets the two load marks, says its load work is done 2.5 s in (`settled`), and commits
// a drag 1.5 s after the release, and at the release makes an object only a weak reference holds:
// its dataset hook answers only once a garbage collection after the drag (the memory phase's) has
// freed it. In "badspot" its spot check finds a wrong date.
const FIXTURE_SIDE = `export default {
  name: "fixture",
  ready: (rows) => Number(document.body.dataset.rows) === rows,
  scrollTarget: () => {
    if (document.body.dataset.mode === "stall") return new Promise(() => {});
    if (document.body.dataset.mode === "throw") throw new Error("no scroll box here");
    return { x: 0, y: 0, width: 400, height: 300, top: 0, left: 0, maxTop: 5000, maxLeft: 100000 };
  },
  leafBars: () => ({ bars: document.body.dataset.mode === "nobar" ? [] : [{ x: 90, y: 95, width: 20, height: 10, id: "7" }], pxPerDay: 10 }),
  backend: () => (document.body.dataset.mode === "hang" ? new Promise(() => {}) : null),
  debugBuild: () => (document.body.dataset.mode === "hang" ? new Promise(() => {}) : null),
  dragState: (target) => (document.body.dataset.state ?? "unchanged") + " " + target.id,
  spotCheck: async () => (document.body.dataset.mode === "badspot" ? "task 7 is a day late" : null),
  settled: () => (document.body.dataset.mode === "late" ? (window.benchSettledAt ?? null) : 0),
  datasetSha256: () => (window.benchAfterDrag !== undefined && window.benchAfterDrag.deref() === undefined ? "cd".repeat(32) : null),
};
`;
const fixture = (() => {
  const dir = mkdtempSync(join(tmpdir(), "bench-side-"));
  writeFileSync(join(dir, "fixture.mjs"), FIXTURE_SIDE);
  return join(dir, "fixture.mjs");
})();
const LATE = `<script>
performance.mark("library-ready");
performance.mark("data-start");
addEventListener("mouseup", () => {
  window.benchAfterDrag = new WeakRef({});
  setTimeout(() => (document.body.dataset.state = "moved"), 1500);
});
setTimeout(() => (window.benchSettledAt = performance.now()), 2500);
</script>`;
const page = (mode: string) =>
  `data:text/html,${encodeURIComponent(`<body data-rows="10" data-mode="${mode}"><p>fixture</p>${mode === "late" ? LATE : ""}</body>`)}`;
const runFixture = (mode: string, rows = "10", dnfS = "3", more: string[] = []) =>
  drive(["--url", page(mode), "--probe", fixture, "--rows", rows, "--headless", "--dnf-s", dnfS, ...more], 300);

test("a page without the asked-for row count never gets ready: every phase a dnf, the later ones naming load", () => {
  const { status, stderr, result } = runFixture("still", "11");
  assert.equal(status, 0, stderr);
  assert.deepEqual(Object.keys(result.phases), [...PHASES]);
  assert.deepEqual(result.phases.load, { dnf: true, cause: "not-ready", afterS: 3, why: "no ready frame with 11 tasks" });
  for (const p of PHASES.slice(1)) assert.deepEqual(result.phases[p], { dnf: true, cause: "not-run", afterS: 0, why: "not run: load did not finish" }, p);
});

test("input that does nothing is not verified, and the run says so in its warnings", () => {
  const { status, stderr, result } = runFixture("still", "10", "60", ["--drag-commit-s", "2"]);
  assert.equal(status, 0, stderr);
  const { "scroll-y": y, "scroll-x": x, drag } = result.phases;
  assert.equal(result.phases.load.dnf, false);
  // The fixture's rows scroll 5,000 px at most, less than 50 heights: the target is that range, and says so in `scrolled` (0 here: nothing moved).
  assert.deepEqual([y.verified, y.why, y.target, y.scrolled], [false, "went 0 px of 5000", 5000, 0]);
  assert.deepEqual(y.wheelEvents, { out: SCROLL_STEPS, back: 0 }, "a view that never moves stops after SCROLL_STEPS events, not at the cap");
  assert.deepEqual([x.verified, x.target], [false, SCROLL_X_VIEWS * 1280], "scroll-x goes by the driver's viewport, not the side's box");
  assert.equal(drag.verified, false);
  for (const k of ["frames", "p50", "p95", "max", "longGaps", "droppedFrames", "commitMs"]) assert.equal(drag[k], undefined, `an unverified drag has no ${k}`);
  assert.match(result.warnings.join("\n"), /scroll-y: went 0 px of 5000/);
  assert.match(result.warnings.join("\n"), /drag: the drag state stayed "unchanged 7" for 2 s after the release: no bar moved/);
  // No marks on this page: load is not split.
  assert.deepEqual([result.phases.load.libraryMs, result.phases.load.dataToReadyMs, result.phases.load.spotChecked], [null, null, true]);
  assert.equal(result.phases.memory.dnf, false);
  // What the table compares across the runs of one row, and the dataset the side says it loaded (the fixture does not say).
  assert.deepEqual(result.options, { width: 1280, height: 800, dpr: 1, refreshHz: 60, dnfS: 60, memoryAtS: 0.1, headless: true, phases: [...PHASES] });
  assert.equal(result.datasetSha256, null);
  assert.equal(result.requestedDatasetSha256, null, "no --dataset-sha256 given");
});

test("a drag committed 1.5 s after the release is recorded until it is, and load splits at the page's marks", () => {
  const { status, stderr, result } = runFixture("late", "10", "60");
  assert.equal(status, 0, stderr);
  const { load, drag } = result.phases;
  assert.equal(drag.verified, true, drag.why);
  assert.ok(drag.commitMs >= 1400 && drag.commitMs < 5000, `committed after ${drag.commitMs} ms`);
  // 60 moves, then the frames until the commit and DRAG_AFTER_FRAMES more: the 1.5 s wait is in the frames.
  assert.ok(drag.frames >= 60 + 60 + 10, `${drag.frames} frames recorded`);
  assert.equal(result.warnings.filter((w: string) => /^drag/.test(w)).length, 0);
  assert.ok(load.libraryMs > 0 && load.libraryMs <= load.ms, `library ${load.libraryMs} of ${load.ms} ms`);
  assert.ok(load.dataToReadyMs >= 0 && load.dataToReadyMs < load.ms, `data to ready ${load.dataToReadyMs} of ${load.ms} ms`);
  // Ready at once, the side's own load work done 2.5 s in: the input phases waited for it.
  assert.ok(load.settledMs >= 2500 && load.settledMs > load.ms, `settled at ${load.settledMs} ms, ready at ${load.ms} ms`);
  assert.ok(load.dataToSettledMs > load.dataToReadyMs, `data to settled ${load.dataToSettledMs} ms, to ready ${load.dataToReadyMs} ms`);
});

test("the side's dataset is asked after the memory phase, so a hook that reads its file is not in the memory numbers", () => {
  // The fixture answers only after a garbage collection that came after the drag: the memory phase's.
  const { status, stderr, result } = runFixture("late", "10", "60");
  assert.equal(status, 0, stderr);
  assert.equal(result.phases.memory.dnf, false);
  assert.equal(result.datasetSha256, "cd".repeat(32));
});

test("a spot check that finds a wrong date ends the run at load: no number from a broken setup", () => {
  const { status, stderr, result } = runFixture("badspot");
  assert.equal(status, 0, stderr);
  assert.deepEqual(result.phases.load, { dnf: true, cause: "error", afterS: 0, why: "spot check: task 7 is a day late" });
  for (const p of PHASES.slice(1)) assert.equal(result.phases[p].why, "not run: load did not finish", p);
});

test("load splits at the marks on the page's clock", () => {
  assert.deepEqual(splitLoad(1000, 4000, { library: 1800, data: 2500 }, true), { dnf: false, ms: 3000, libraryMs: 800, dataToReadyMs: 1500, spotChecked: true, settledMs: null, dataToSettledMs: null });
  assert.deepEqual(splitLoad(1000, 4000, { library: 1800, data: 2500 }, true, 4600), { dnf: false, ms: 3000, libraryMs: 800, dataToReadyMs: 1500, spotChecked: true, settledMs: 3600, dataToSettledMs: 2100 });
  assert.deepEqual(splitLoad(1000, 4000, { library: null, data: null }, false), { dnf: false, ms: 3000, libraryMs: null, dataToReadyMs: null, spotChecked: false, settledMs: null, dataToSettledMs: null });
});

test("a phase that stalls is a dnf at the limit, and nothing after it is measured on that page", () => {
  const { status, stderr, result } = runFixture("stall");
  assert.equal(status, 0, stderr);
  assert.equal(result.phases.load.dnf, false);
  assert.deepEqual(result.phases["scroll-y"], { dnf: true, cause: "timeout", afterS: 3, why: "the phase did not finish" });
  for (const p of ["scroll-x", "drag", "memory"]) assert.deepEqual(result.phases[p], { dnf: true, cause: "not-run", afterS: 0, why: "not run: scroll-y did not finish" }, p);
});

test("a hook that throws gives a dnf with why, never a blank result", () => {
  const { status, stderr, result } = runFixture("throw");
  assert.equal(status, 0, stderr);
  assert.equal(result.phases["scroll-y"].dnf, true);
  assert.match(result.phases["scroll-y"].why, /^error: .*no scroll box here/);
  assert.equal(result.phases["scroll-y"].cause, "error");
  assert.equal(result.phases.memory.why, "not run: scroll-y did not finish");
});

test("a backend or debugBuild hook that never answers is capped like every other ask: the run goes on", () => {
  // Two asks capped at --dnf-s 3, then the input phases: well inside a minute. An uncapped ask waits until the run is killed
  // (Playwright closes the browser on SIGTERM, so the run then still writes a result and exits 0: the clock is the check).
  const t0 = Date.now();
  const { status, stderr, result } = drive(["--url", page("hang"), "--probe", fixture, "--rows", "10", "--headless", "--dnf-s", "3", "--phases", "load"], 90);
  assert.ok(Date.now() - t0 < 60_000, `the run took ${Math.round((Date.now() - t0) / 1000)} s`);
  assert.equal(status, 0, stderr);
  assert.equal(result.phases.load.dnf, false);
  assert.deepEqual([result.environment.sideBackend, result.environment.debugBuild], [null, null]);
});

test("a drag with no bar on screen still has its memory read before the page closes", () => {
  const { status, stderr, result } = runFixture("nobar", "10", "60");
  assert.equal(status, 0, stderr);
  assert.match(result.phases.drag.why, /^no bar on screen to drag: /);
  assert.equal(result.phases.memory.dnf, false, result.phases.memory.why);
});

/** A free port on 127.0.0.1. */
function freePort(): Promise<number> {
  return new Promise((done, fail) => {
    const probe = createServer();
    probe.once("error", fail);
    probe.listen(0, "127.0.0.1", () => {
      const { port } = probe.address() as { port: number };
      probe.close(() => done(port));
    });
  });
}

const servers: ChildProcess[] = [];
after(() => {
  for (const server of servers) if (server.pid !== undefined) process.kill(-server.pid, "SIGTERM");
});

/** Serve `dist` (default `apps/hosted/dist`) with `vite preview`, as the hosted specs do; the page's address. */
async function startHosted(dist = join(hosted, "dist"), page = ""): Promise<string> {
  assert.ok(existsSync(join(hosted, "dist", "index.html")), "no apps/hosted/dist: run `pnpm build` in apps/hosted first");
  const port = await freePort();
  const vite = join(hosted, "node_modules", "vite", "bin", "vite.js");
  const server = spawn(process.execPath, [vite, "preview", "--outDir", dist, "--host", "127.0.0.1", "--port", String(port), "--strictPort"], { cwd: hosted, detached: true, stdio: "ignore" });
  servers.push(server);
  const url = `http://127.0.0.1:${port}/`;
  for (const stop = Date.now() + 60_000; Date.now() < stop; await sleep(250)) {
    if (await fetch(`${url}${page}`).then((r) => r.ok, () => false)) return url;
  }
  throw new Error(`vite preview did not answer on ${url}`);
}

test("the hosted build, driven through window.gantt, gives one JSON with all five phases, each verified", async () => {
  const url = `${await startHosted()}?sample=demo-50k`;
  // The software renderer draws 50,000 rows slowly; this only checks the run completes and its input took.
  const { status, stderr, result } = drive(["--url", url, "--probe", "ours", "--rows", "50000", "--headless", "--dnf-s", "300"], 1500);
  assert.equal(status, 0, stderr);
  assert.deepEqual([result.tool, result.side, result.rows], ["drive", "ours", 50000]);
  assert.deepEqual(Object.keys(result.phases), [...PHASES]);
  for (const p of PHASES) assert.equal(result.phases[p].dnf, false, `${p}: ${JSON.stringify(result.phases[p])}`);

  const { load, "scroll-y": y, "scroll-x": x, drag, memory } = result.phases;
  assert.ok(load.ms > 0, "load time");
  // The wasm instantiated before the schedule loaded; demo-50k is generated in the page, so there is no fetch to time.
  assert.ok(load.libraryMs > 0 && load.libraryMs < load.ms, `library ready at ${load.libraryMs} of ${load.ms} ms`);
  assert.equal(load.dataToReadyMs, null);
  assert.ok(drag.commitMs >= 0, "the drag says when it was committed");
  for (const [name, phase] of [["scroll-y", y], ["scroll-x", x], ["drag", drag]]) {
    assert.equal(phase.verified, true, `${name}: ${phase.why}`);
    assert.ok(phase.frames > 0 && phase.p50 > 0 && phase.p95 >= phase.p50, `${name}: frames ${phase.frames}, p50 ${phase.p50}, p95 ${phase.p95}`);
    assert.ok(Number.isInteger(phase.longGaps) && Number.isInteger(phase.droppedFrames), `${name}: long gaps and dropped frames`);
  }
  // The checks behind `verified`: down to the last rows and back, along the time axis and back.
  assert.ok(y.target === SCROLL_Y_VIEWS * 800 && Math.abs(y.turn - y.from - y.target) <= y.target * 0.005, `scroll-y reached ${y.turn} of ${y.target}`);
  assert.equal(x.target, SCROLL_X_VIEWS * 1280);
  for (const when of ["afterLoad", "afterScripts"]) {
    const m = memory[when];
    assert.ok(m.chromeBytes > m.rendererBytes && m.rendererBytes > m.jsHeapUsedBytes && m.jsHeapUsedBytes > 0, `${when}: ${JSON.stringify(m)}`);
    assert.equal(m.kind, process.platform === "linux" ? "pss" : process.platform === "darwin" ? "footprint" : "rss", `${when}: the kind read`);
  }
  assert.equal(memory.heapBytes, undefined);

  const env = result.environment;
  assert.match(env.chrome, /^\d+\./);
  assert.deepEqual([env.viewport, env.dpr, env.refreshHz, env.headless, env.debugBuild], [{ width: 1280, height: 800 }, 1, 60, true, false]);
  assert.ok(env.rowsOnScreen > 10, `${env.rowsOnScreen} rows on screen`);
  assert.match(env.sideBackend, /^(WebGPU|WebGL2)$/);
  assert.ok("renderer" in env.gpu && "vendor" in env.gpu);
  assert.match(result.warnings.join("\n"), /headless/);
  assert.equal(result.datasetSha256, null, "demo-50k is the chart's own schedule, not a benchmark dataset");
  assert.deepEqual(result.warnings.filter((w: string) => /^(scroll|drag)/.test(w)), [], "every input phase verified");
});

test("on a tree4 schedule the driver finds a task bar to drag on our side, and the drag is verified", async () => {
  // The bench page built as the harness builds it (ours-vite.config.ts), with the tree4 cell adapted into it as run.ts and ours-adapt.ts do.
  const root = mkdtempSync(join(tmpdir(), "bench-tree4-"));
  const bundle = join(root, "bundle");
  const built = spawnSync("pnpm", ["exec", "vite", "build", "--config", join(here, "sides/ours-vite.config.ts"), "--outDir", bundle], { cwd: hosted, encoding: "utf8" });
  assert.equal(built.status, 0, built.stdout + built.stderr);
  const sha = writeDataset(join(root, "cell"), { shape: "tree4", size: 1000 }, 20261020n)!;
  adaptInto(join(root, "cell", "dataset.json"), bundle);
  const url = `${await startHosted(bundle, "ours-page.html")}ours-page.html?cell=tree4-1000-20261020`;
  const { status, stderr, result } = drive(["--url", url, "--probe", "ours", "--rows", "1000", "--headless", "--dnf-s", "300", "--dataset-sha256", sha], 1500);
  assert.equal(status, 0, stderr);
  const { drag, load } = result.phases;
  // The bench page marks library-ready before data-start (as the DHTMLX page does), so the library time holds no wait for the file.
  assert.ok(load.libraryMs <= load.ms - load.dataToReadyMs + 0.1, `library-ready at ${load.libraryMs} ms, data-start at ${load.ms - load.dataToReadyMs} ms`);
  assert.equal(drag.dnf, false, `drag: ${drag.why}`);
  assert.equal(drag.verified, true, `drag: ${drag.why}`);
  assert.ok(drag.frames > 0 && drag.commitMs >= 0, `drag: frames ${drag.frames}, committed after ${drag.commitMs} ms`);
  // Task 1.1.1, the dataset's leaf 1, is the first task bar in row order; its 2-day bar is zoomed in on until it is wide enough.
  assert.equal(drag.task, "1", "the dragged task, as its dataset id");
  assert.ok(drag.zoomSteps > 0 && drag.scrollSteps === 0, `zoom steps ${drag.zoomSteps}, scroll steps ${drag.scrollSteps}`);
  assert.ok(drag.pxPerDay * 2 >= 6, `a 2-day bar at ${drag.pxPerDay} px a day is wide enough to press`);
  assert.equal(result.phases.memory.dnf, false, "the phase after the drag ran");
  assert.equal(result.datasetSha256, sha, "the page loaded the tree4 cell");
});
