// The pure parts of demos/bench/profile.ts: its arguments, the phase table and the before/after table.

import assert from "node:assert/strict";
import { test } from "node:test";
import { readFileSync } from "node:fs";
import { allocatorTable, clusterTable, compareTable, foldStacks, holdGhosts, isOwnDir, lockProblem, memoryDump, ownPids, pairedLine, parseArgs, phaseRows, type Run } from "../../../demos/bench/profile-format.ts";
import { expectedMinutes } from "../../../demos/bench/profile.ts";

const run = (ready: number, peak = 100 * 1048576): Run => ({ marks: { "library-ready": 300, "data-start": 301, "mount-start": 500 }, readyMs: ready, peakLive: peak, allocations: 5000, memoryBuffer: 256 * 1048576, gpuProcess: 64 * 1048576 });

test("arguments: a cell is required, the rest default, and anything else is refused", () => {
  assert.deepEqual(parseArgs(["--cell", "chain-10000", "--runs", "5", "--compare", "main"]), { cell: "chain-10000", runs: 5, compare: "main", out: "profile-out", host: "mini", chrome: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", seed: "20261020", memoryInfra: false, drag: false, ghosts: 0, groups: 0, beforeEdit: false, criticalOnly: false, buildOnly: false, holder: null });
  assert.equal(parseArgs(["--cell", "chain-10", "--compare", "main", "--drag"])?.drag, true);
  assert.deepEqual([parseArgs(["--cell", "tree4-50000", "--drag", "--runs", "3"])?.drag, parseArgs(["--cell", "tree4-50000", "--drag"])?.compare], [true, null]);
  for (const bad of [[], ["--cell", "chain"], ["--cell", "chain-10", "--runs", "0"], ["--cell", "chain-10", "--cell", "chain-20"], ["--cell", "chain-10", "--nope", "1"], ["--cell", "chain-10", "--runs"], ["--cell", "chain-10", "--memory-infra", "--memory-infra"], ["--cell", "chain-10", "--drag", "--drag"], ["--cell", "chain-10", "--compare", "main", "--drag", "--drag"]]) assert.equal(parseArgs(bad), undefined);
});

test("--build-only is a flag with no value, given once, and it needs no --compare", () => {
  assert.equal(parseArgs(["--cell", "tree4-50000", "--build-only"])?.buildOnly, true);
  assert.equal(parseArgs(["--cell", "tree4-50000", "--build-only", "--runs", "2"])?.runs, 2);
  assert.equal(parseArgs(["--cell", "tree4-50000", "--build-only", "--build-only"]), undefined);
});

test("--holder names the task that holds the mini's lock, and a run with the lock free or held by another task is refused", () => {
  assert.equal(parseArgs(["--cell", "tree4-50000", "--holder", "tl1743l"])?.holder, "tl1743l");
  assert.equal(parseArgs(["--cell", "tree4-50000", "--holder"]), undefined);
  assert.match(lockProblem("free", "tl1743l")!, /free/);
  assert.match(lockProblem("tl1767b 3h5m (stale)", "tl1743l")!, /held by tl1767b/);
  assert.match(lockProblem("tl1743l 12m", null)!, /--holder/);
});

test("a run whose --holder is the one `who` names goes on, whatever the lock's age", () => {
  assert.equal(lockProblem("tl1743l 12m", "tl1743l"), undefined);
  assert.equal(lockProblem("tl1743l 3h5m (stale)", "tl1743l"), undefined);
});

test("the phase table splits the load at the page's marks and ends at the ready frame", () => {
  assert.deepEqual(phaseRows(run(2000)), [["navigation to library ready", 300], ["library ready to data start", 1], ["data start to mount", 199], ["mount to ready frame", 1500], ["navigation to ready frame (total)", 2000]]);
});

test("the compare table gives each median and the change", () => {
  const t = compareTable([run(2000), run(2200), run(2100)], [run(1000, 50 * 1048576)], ["main", "branch"]);
  assert.match(t, /\| navigation to ready frame \(total\) \| 2,?100 \(2,?000 to 2,?200\) \| 1,?000 \(1,?000 to 1,?000\) \| -52.4% \| ms \|/);
  assert.match(t, /\| engine peak live \| 100 \(100 to 100\) \| 50 \(50 to 50\) \| -50% \| MB \|/);
});

test("the compare table shows each side's spread beside its median, and the change stays on the medians", () => {
  const t = compareTable([run(2000), run(2200), run(2100)], [run(900), run(1100), run(1000)], ["main", "branch"]);
  assert.match(t, /\| navigation to ready frame \(total\) \| 2,?100 \(2,?000 to 2,?200\) \| 1,?000 \(900 to 1,?100\) \| -52.4% \| ms \|/);
});

test("a flame graph folds the profile Chrome writes on its profiler thread, found by the profile's id", () => {
  const meta = { name: "thread_name", pid: 1, tid: 1, ph: "M", args: { name: "CrRendererMain" } };
  const chunk = (pid: number, id: string, samples: number[], tid = 7) => ({ name: "ProfileChunk", pid, tid, id, ph: "P", args: { data: { cpuProfile: { nodes, samples } } } });
  const nodes = [{ id: 1, callFrame: { functionName: "(root)" }, children: [2] }, { id: 2, callFrame: { functionName: "app::open" } }];
  // The v8:ProfEvntProc thread (tid 7) holds two profiles, one in two chunks; another process holds a bigger one.
  const folded = foldStacks([meta, chunk(1, "0x1", [2, 2]), chunk(1, "0x1", [1]), chunk(1, "0x2", [1]), chunk(9, "0x3", [2, 2, 2, 2])]);
  assert.deepEqual([...folded], [["(root);app::open", 2], ["(root)", 1]]);
  // A spare renderer (its own CrRendererMain, no profile) listed before the real one does not empty the graph.
  const spare = { ...meta, pid: 5 };
  assert.deepEqual([...foldStacks([spare, meta, chunk(1, "0x1", [2, 2]), chunk(5, "0x4", [2])])], [["(root);app::open", 2]]);
});

test("a leftover is a process or directory with the command's marker, and nothing else", () => {
  const ps = [
    "  101 /usr/bin/python3 -u -m http.server 0 --bind 127.0.0.1 --directory /var/folders/x/T/gantt-profile.a1B2c3",
    "  102 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome --user-data-dir=/var/folders/x/T/gantt-profile.d4E5f6 about:blank",
    "  103 /Applications/Google Chrome.app/Contents/MacOS/Google Chrome --remote-debugging-port=9222",
    "  104 ssh -N gantt-profile.z9 mini",
    "  105 vim /tmp/gantt-profile.notes.txt",
    "  106 /tmp/gantt-profile.a1B2c3/helper --type=gpu-process",
  ].join("\n");
  assert.deepEqual(ownPids(ps), ["101", "102", "106"]);
  assert.ok(isOwnDir("/var/folders/x/T/gantt-profile.a1B2c3"));
  for (const no of ["/var/folders/x/T/tmp.a1B2c3", "/tmp/gantt-profile.a/b", "/tmp/gantt-profile.", "/", "/tmp/xgantt-profile.a"]) assert.ok(!isOwnDir(no), no);
});

// profile-memory-dump.fixture.json: the events of one real memory dump (Chrome, `Tracing.requestMemoryDump`), cut to the Browser, GPU and Renderer processes' top-level allocators.
const dump = JSON.parse(readFileSync(new URL("../../../demos/bench/profile-memory-dump.fixture.json", import.meta.url), "utf8")).traceEvents;

test("a memory dump gives the GPU and renderer processes' top-level allocators by size, and nothing of the browser process", () => {
  const d = memoryDump(dump, "0x2");
  assert.deepEqual(Object.keys(d).sort(), ["gpu", "renderer"]);
  assert.equal(d.gpu.malloc, 0x79d8439);
  assert.equal(d.gpu.gpu, 0xdbe180);
  assert.equal(d.renderer.v8, 0x3aa430);
  assert.ok(!("discardable" in d.gpu), "an allocator with no size of its own is not listed");
  // A spare renderer with less in it, inside the same dump, is passed over.
  const spare = [{ name: "process_name", ph: "M", pid: 7, tid: 0, args: { name: "Renderer" } }, { ph: "v", name: "periodic_interval", pid: 7, tid: 0, ts: 134626009762, args: { dumps: { allocators: { v8: { attrs: { size: { value: "10" } } } } } } }];
  assert.deepEqual(memoryDump([...spare, ...dump], "0x2").renderer, d.renderer);
});

test("a memory dump is the one whose guid was asked for, not the one with the most allocators, and a missing guid throws", () => {
  // An earlier dump (guid 0x1) of the same processes: every top-level allocator 16 MB, and nine more of them, so it would win on size and on count.
  const shift = (e: any) => ({ ...e, ts: e.ts - 1_000_000, ...(e.id2 && { id2: { local: "0x1" } }), ...(e.args?.dump_guid && { args: { ...e.args, dump_guid: "0x1" } }) });
  const fat = (e: any) => (e.ph !== "v" || !e.args.dumps.allocators ? e : { ...e, args: { dumps: { allocators: Object.fromEntries([...Object.keys(e.args.dumps.allocators), ...Array.from({ length: 9 }, (_, i) => `extra${i}`)].map((k) => [k, { attrs: { size: { value: "1000000" } } }])) } } });
  const earlier = dump.filter((e: any) => e.name === "GlobalMemoryDump" || e.ph === "v").map(shift).map(fat);
  assert.deepEqual(memoryDump([...earlier, ...dump], "0x2"), memoryDump(dump, "0x2"));
  assert.equal(memoryDump([...earlier, ...dump], "0x1").gpu.malloc, 0x1000000);
  assert.throws(() => memoryDump(dump, "0x9"), /no memory dump 0x9/);
});

test("the allocator table lists each process's allocators largest first, with a column per set of runs", () => {
  const d = memoryDump(dump, "0x2");
  const withDump = (allocators: typeof d): Run => ({ ...run(1000), allocators });
  const lines = allocatorTable([[withDump(d), withDump(d)], [withDump({ ...d, gpu: { ...d.gpu, malloc: 0x79d8439 * 2 } })]], ["before", "after"]).split("\n");
  assert.deepEqual(lines.slice(0, 3), ["| process | allocator | before (MB) | after (MB) |", "|---|---|---|---|", "| gpu | malloc | 121.8 | 243.7 |"]);
  assert.deepEqual(lines.filter((l) => l.startsWith("| gpu")).map((l) => l.split(" | ")[1]), ["malloc", "gpu", "skia", "shared_memory", "cc"]);
  assert.ok(lines.includes("| renderer | malloc | 6.2 | 6.2 |"));
});

test("--groups takes a positive count and only with --drag; absent it is 0", () => {
  assert.equal(parseArgs(["--cell", "tree4-50000", "--drag", "--groups", "12"])?.groups, 12);
  assert.equal(parseArgs(["--cell", "tree4-50000", "--drag"])?.groups, 0);
  for (const bad of [["--groups", "0", "--drag"], ["--groups", "x", "--drag"], ["--groups"], ["--groups", "12"], ["--groups", "2", "--groups", "3", "--drag"]]) assert.equal(parseArgs(["--cell", "tree4-50000", ...bad]), undefined);
});

test("--before-edit is a flag with no value, given once, and only with --drag", () => {
  assert.equal(parseArgs(["--cell", "tree4-50000", "--drag", "--before-edit"])?.beforeEdit, true);
  for (const bad of [["--before-edit"], ["--drag", "--before-edit", "--before-edit"]]) assert.equal(parseArgs(["--cell", "tree4-50000", ...bad]), undefined);
});

test("--ghosts takes a count, and holdGhosts holds week-late starts until that many tasks would move", () => {
  assert.equal(parseArgs(["--cell", "tree4-50000", "--ghosts", "500"])?.ghosts, 500);
  for (const bad of [["--ghosts"], ["--ghosts", "x"], ["--ghosts", "5", "--ghosts", "6"]]) assert.equal(parseArgs(["--cell", "chain-10", ...bad]), undefined);
  // Task 0 is a summary; tasks 1 to 4 and 5 to 9 are two chains, so slipping a task moves it and the ones after it. Asking for 7 skips tasks 2 to 4, which move no new task.
  const held = new Map<number, string>(), asked: number[] = [];
  const tail = (t: number) => Array.from({ length: (t < 5 ? 5 : 10) - t }, (_, i) => t + i);
  const rows = (ts: number[]) => ts.map((task) => ({ task, start_now: `d${task}` }));
  const app = {
    table_rows: () => JSON.stringify(Array.from({ length: 10 }, (_, i) => [i])),
    what_moves: (t: Uint32Array) => (asked.push(t[0]), JSON.stringify(t[0] === 0 ? { ok: false } : { ok: true, moved: rows(tail(t[0])) })),
    propose_move: (t: number, start: string) => (held.set(t, start), JSON.stringify({ ok: true, moved: rows([...new Set([...held.keys()].flatMap(tail))].sort()) })),
  };
  assert.equal(holdGhosts(app, 7), 9);
  assert.deepEqual(asked, [0, 1, 5]); // the tasks the first change moves are not asked about
  assert.deepEqual([...held], [[1, "d1"], [5, "d5"]]);
  held.clear();
  assert.throws(() => holdGhosts(app, 20), /2 held changes move only 9 of the 20 tasks/);
});

// Runs whose open time is `ms` (`run` above puts mount-start at 500).
const opens = (...ms: number[]) => ms.map((m) => run(m + 500));

test("a shift in how many runs land in each cluster shows as run counts, while the change inside each cluster stays near 0", () => {
  const before = opens(300, 310, 320, 330, 660, 670, 680, 690, 700, 710, 720, 730);
  const after = opens(305, 315, 325, 335, 305, 315, 325, 335, 665, 675, 685, 695);
  assert.match(compareTable(before, after, ["main", "branch"]), /\| mount to ready frame \| 675 \(300 to 730\) \| 330 \(305 to 695\) \| -51\.1% \| ms \|/);
  const t = clusterTable(before, after, ["main", "branch"]);
  assert.match(t, /split at 498 ms/);
  assert.match(t, /\| fast \(under 498 ms\) \| 4 \/ 8 \| 315 \(300 to 330\) \| 320 \(305 to 335\) \| \+1\.6% \|/);
  assert.match(t, /\| slow \(from 498 ms\) \| 8 \/ 4 \| 695 \(660 to 730\) \| 680 \(665 to 695\) \| -2\.2% \|/);
});

test("runs in one cluster are not split, and a cluster of one side only shows no change", () => {
  assert.equal(clusterTable(opens(560, 580, 600, 590, 570, 585), opens(575, 595, 565, 590, 580, 600), ["a", "b"]), "");
  assert.match(clusterTable(opens(300, 310, 320, 330), opens(660, 670, 680, 690), ["a", "b"]), /\| fast \(under 495 ms\) \| 4 \/ 0 \| 315 \(300 to 330\) \| n\/a \| n\/a \|/);
  assert.match(clusterTable(opens(300, 540, 550, 560), opens(610, 615, 620, 625), ["a", "b"]), /split at 585 ms/); // a gap of 8.5% of the median is enough
});

test("a state that lasts a few runs is in both runs of a pair, so the paired median shows the code and the whole-run medians show the mix", () => {
  const [before, after] = [opens(700, 700, 700, 300, 300, 300), opens(700, 700, 300, 300, 300, 300)];
  assert.match(compareTable(before, after, ["a", "b"]), /\| mount to ready frame \| 500 \(300 to 700\) \| 300 \(300 to 700\) \| -40% \|/);
  assert.match(pairedLine(before, after), /median of the ratios \+0% over 6 pairs/);
  assert.match(pairedLine(before, opens(770, 770, 330, 330, 330, 330)), /\+10% over 6 pairs/);
});

test("a --drag run's expected time is about 4 minutes per session on each side, and a run without --drag has none", () => {
  const minutes = (...argv: string[]) => expectedMinutes(parseArgs(["--cell", "tree4-50000", ...argv])!);
  assert.deepEqual([minutes("--drag", "--compare", "main", "--runs", "10"), minutes("--drag", "--runs", "10"), minutes("--runs", "10")], [80, 40, null]);
});
