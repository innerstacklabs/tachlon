// The pure parts of `profile.ts`: its arguments, the phase table, the before/after table and the flame graph.

export interface Args {
  cell: string;
  runs: number;
  compare: string | null;
  out: string;
  host: string;
  chrome: string;
  seed: string;
  memoryInfra: boolean;
  /** Also measure a drag on each side (one side without `--compare`). */
  drag: boolean;
  /** `--ghosts <n>`: hold a proposal that moves at least this many tasks while the cell is measured; 0 holds none. */
  ghosts: number;
  /** `--groups <k>`: the drag page groups its stress fixture by a `Phase` code with this many values (the page's `?groups=`); 0 is the plain fixture. */
  groups: number;
  /** `--build-only`: build the bundles, print where they are and stop; nothing runs on the mini. */
  buildOnly: boolean;
  /** `--holder <name>`: the name this task took the mini's lock under; the command runs on the mini only while that is the lock's holder. */
  holder: string | null;
}

/** Why a run must not start on the mini, or undefined: `who` is the lock command's answer (`free`, or `<holder> <age>`), `holder` the `--holder` name. */
export const lockProblem = (who: string, holder: string | null): string | undefined => {
  const held = who.trim().split(" ")[0];
  if (holder === null) return `pass --holder <the name this task took the mini's lock under> (the lock's holder is ${held})`;
  return held === holder ? undefined : held === "free" ? `the mini's lock is free: take it as ${holder} first` : `the mini's lock is held by ${held}, not ${holder}`;
};

const DEFAULTS = { ghosts: "0", groups: "0", runs: "3", out: "profile-out", host: "mini", chrome: "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", seed: "20261020" };

/** `--cell <shape>-<size>` plus optional `--runs --compare --out --host --chrome --seed` `--ghosts` `--groups` and the flags `--memory-infra`, `--build-only` and `--drag`; undefined for anything else. */
export function parseArgs(argv: string[]): Args | undefined {
  const got = new Map<string, string>();
  const flags = new Set<string>();
  for (let i = 0; i < argv.length; i += 2) {
    if (argv[i] === "--memory-infra" || argv[i] === "--drag" || argv[i] === "--build-only") {
      if (flags.has(argv[i])) return undefined;
      flags.add(argv[i]);
      i--; // the flag has no value: the loop's step of 2 then moves on by one
      continue;
    }
    const name = argv[i].startsWith("--") ? argv[i].slice(2) : "";
    const value = argv[i + 1];
    if (!["cell", "compare", "holder", ...Object.keys(DEFAULTS)].includes(name) || got.has(name)) return undefined;
    if (value === undefined || value === "" || value.startsWith("--")) return undefined;
    got.set(name, value);
  }
  const cell = got.get("cell");
  const o = { ...DEFAULTS, ...Object.fromEntries(got) };
  if (!cell || !/^[a-z0-9]+-\d+$/.test(cell) || !/^[1-9]\d*$/.test(o.runs) || !/^\d+$/.test(o.seed) || !/^\d+$/.test(o.ghosts) || !/^[1-9]\d*$/.test(got.get("groups") ?? "1") || (got.has("groups") && !flags.has("--drag"))) return undefined;
  return { cell, runs: Number(o.runs), compare: got.get("compare") ?? null, out: o.out, host: o.host, chrome: o.chrome, seed: o.seed, memoryInfra: flags.has("--memory-infra"), drag: flags.has("--drag"), ghosts: Number(o.ghosts), groups: Number(o.groups), buildOnly: flags.has("--build-only"), holder: got.get("holder") ?? null };
}

/**
 * Hold a proposal in the page (`app` is `window.gantt.app`) until at least `n` tasks would move, and answer how many do: each change is a week-late start (what `what_moves` tries) of one of the first
 * 1,000 rows. The first change is the first row that moves (a ghost on the top rows, which are on screen); each later one must move enough tasks no held change moves yet that the changes left,
 * 10 at most in the app, can reach `n`. It throws when it finds too few. It runs in the page, so it names nothing outside itself.
 */
export const holdGhosts = (app: { table_rows(first: number, last: number): string; what_moves(tasks: Uint32Array): string; propose_move(task: number, start: string): string }, n: number): number => {
  type Moved = { task: number; start_now: string };
  const rows: number[] = (JSON.parse(app.table_rows(0, 1000)) as number[][]).map((r) => r[0]);
  let moved = new Set<number>(), held = 0;
  for (let i = 0; moved.size < n; i++) {
    const t = rows[i];
    if (t === undefined || held === 10) throw new Error(`${held} held changes move only ${moved.size} of the ${n} tasks asked for`);
    if (moved.has(t)) continue;
    const slip: Moved[] | undefined = JSON.parse(app.what_moves(Uint32Array.of(t))).moved; // none for a summary or a started task
    const start = slip?.find((m) => m.task === t)?.start_now;
    if (!start || slip!.filter((m) => !moved.has(m.task)).length < (held === 0 ? 1 : Math.ceil((n - moved.size) / (10 - held)))) continue;
    moved = new Set<number>((JSON.parse(app.propose_move(t, start)).moved as Moved[]).map((m) => m.task));
    held++;
  }
  return moved.size;
};

export const median = (v: number[]): number => {
  const s = [...v].sort((a, b) => a - b);
  return s.length === 0 ? NaN : s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};

/** What one run measured. Times are ms from the navigation, on the page's clock; sizes are bytes. */
export interface Run {
  marks: Record<string, number>;
  readyMs: number;
  peakLive: number | null;
  allocations: number | null;
  memoryBuffer: number | null;
  gpuProcess: number;
  /** With `--ghosts`: how many tasks the held proposal moves. */
  ghosts?: number;
  /** With `--memory-infra`: the GPU and renderer processes' memory by top-level allocator. */
  allocators?: Allocators;
}

/** Bytes by top-level allocator name, per process type (`gpu`, `renderer`). */
export type Allocators = Record<string, Record<string, number>>;

/** The phase rows of one run: the time each step took, in order, then the total. A mark the page did not set is skipped. */
export function phaseRows(r: Run): [string, number][] {
  const m = r.marks;
  const rows: [string, number][] = [];
  let at = 0;
  for (const [label, name] of [["navigation to library ready", "library-ready"], ["library ready to data start", "data-start"], ["data start to mount", "mount-start"]] as const) {
    if (m[name] === undefined) continue;
    rows.push([label, m[name] - at]);
    at = m[name];
  }
  rows.push([at === 0 ? "navigation to ready frame" : "mount to ready frame", r.readyMs - at], ["navigation to ready frame (total)", r.readyMs]);
  return rows;
}

/** Every number of a run as `[name, value, unit]`, the phase rows first. */
export function metrics(r: Run): [string, number | null, string][] {
  const mb = (n: number | null) => (n === null ? null : n / 1048576);
  return [
    ...phaseRows(r).map(([k, v]): [string, number, string] => [k, v, "ms"]),
    ["engine peak live", mb(r.peakLive), "MB"],
    ["engine allocations", r.allocations, "calls"],
    ["memory.buffer", mb(r.memoryBuffer), "MB"],
    ["GPU process at 20 s", mb(r.gpuProcess), "MB"],
  ];
}

const fmt = (n: number | null) => (n === null || Number.isNaN(n) ? "n/a" : n >= 1000 ? Math.round(n).toLocaleString("en-US") : String(Math.round(n * 10) / 10));

/** The median of each metric over `runs`, as a markdown table. */
export function summaryTable(runs: Run[]): string {
  const cols = runs.map(metrics);
  return ["| metric | median | unit |", "|---|---|---|", ...cols[0].map(([k, , u], i) => `| ${k} | ${fmt(median(cols.map((c) => c[i][1] ?? NaN)))} | ${u} |`)].join("\n");
}

/** Two sets of runs, interleaved when taken, as a before/after markdown table: each side's cell is `median (min to max)` over its runs (a run without the number left out), the change is between the medians. */
export function compareTable(before: Run[], after: Run[], labels: [string, string]): string {
  const [b, a] = [before.map(metrics), after.map(metrics)];
  const rows = b[0].map(([k, , u], i) => {
    const [vb, va] = [b, a].map((cs) => cs.map((c) => c[i][1] ?? NaN).filter((n) => !Number.isNaN(n)));
    const [x, y] = [median(vb), median(va)];
    const cell = (v: number[], m: number) => (v.length ? `${fmt(m)} (${fmt(Math.min(...v))} to ${fmt(Math.max(...v))})` : "n/a");
    const change = Number.isNaN(x) || Number.isNaN(y) || x === 0 ? "n/a" : `${y >= x ? "+" : ""}${Math.round(((y - x) / x) * 1000) / 10}%`;
    return `| ${k} | ${cell(vb, x)} | ${cell(va, y)} | ${change} | ${u} |`;
  });
  return [`| metric | ${labels[0]} | ${labels[1]} | change | unit |`, "|---|---|---|---|---|", ...rows].join("\n");
}

/** Where the values fall into a fast and a slow cluster: the middle of their largest gap, when that gap is at least 6% of their median and leaves 3 or more values on each side; else undefined. */
export function splitAt(values: number[]): number | undefined {
  const s = [...values].sort((a, b) => a - b);
  let at: number | undefined, gap = 0;
  for (let i = 3; i <= s.length - 3; i++) if (s[i] - s[i - 1] > gap) [gap, at] = [s[i] - s[i - 1], Math.round((s[i] + s[i - 1]) / 2)];
  return gap >= 0.06 * median(s) ? at : undefined;
}

/** The ready-frame time of a run after its last mark (`mount to ready frame`), the number that lands in two clusters. */
const openMs = (r: Run): number => phaseRows(r).at(-2)![1];

/** The open time of two sets of runs read by cluster, split at one point for both sides, with the number of runs of each side in each cluster; "" when the runs have no two clusters. A cluster with runs of one side only shows no change. */
export function clusterTable(before: Run[], after: Run[], labels: [string, string]): string {
  const [b, a] = [before.map(openMs), after.map(openMs)];
  const at = splitAt([...b, ...a]);
  if (at === undefined) return "";
  const rows = [["fast", (x: number) => x < at], ["slow", (x: number) => x >= at]].map(([name, in_]) => {
    const [vb, va] = [b, a].map((v) => v.filter(in_ as (x: number) => boolean));
    const [x, y] = [median(vb), median(va)];
    const cell = (v: number[], m: number) => (v.length ? `${fmt(m)} (${fmt(Math.min(...v))} to ${fmt(Math.max(...v))})` : "n/a");
    const change = vb.length && va.length ? `${y >= x ? "+" : ""}${Math.round(((y - x) / x) * 1000) / 10}%` : "n/a";
    return `| ${name} (${name === "fast" ? "under" : "from"} ${fmt(at)} ms) | ${vb.length} / ${va.length} | ${cell(vb, x)} | ${cell(va, y)} | ${change} |`;
  });
  return [`mount to ready frame by cluster, split at ${fmt(at)} ms (the largest gap of both sides' runs together):`, "", `| cluster | runs (${labels[0]} / ${labels[1]}) | ${labels[0]} median (min to max) | ${labels[1]} median (min to max) | change |`, "|---|---|---|---|---|", ...rows].join("\n");
}

/** The open time read in pairs: the median over the runs of the second side's time divided by the first side's of the same number, taken one after the other, so a state of the machine that lasts a few runs is in both. */
export function pairedLine(before: Run[], after: Run[]): string {
  const ratios = before.map((r, i) => openMs(after[i]) / openMs(r)).filter(Number.isFinite);
  const m = median(ratios);
  return `mount to ready frame in pairs (each run against the run of the other side taken next to it): median of the ratios ${m >= 1 ? "+" : ""}${Math.round((m - 1) * 1000) / 10}% over ${ratios.length} pairs`;
}

/** One scripted drag's p50 and p95, in ms, of the frame interval and of the response (pointer event to the frame that shows its recompute). */
export interface Drag { interval: [number, number]; response: [number, number] }
/** The two drags `--drag` runs on each side: the head task's, and the first critical task's (the page's `dragTarget`). */
export const DRAGS = [["head", "head-task drag"], ["critical", "critical-path drag"]] as const;
export type DragRun = Record<(typeof DRAGS)[number][0], Drag>;

/** One or two sets of drag runs (two: interleaved when taken): per drag and measure, each side's median p50 and median p95 (with the min to max of the runs' p95) and, with two, whether the second side's median p95 lies inside the first's spread. */
export function dragTable(before: DragRun[], after: DragRun[] | undefined, labels: [string, string?]): string {
  const side = (runs: DragRun[], key: keyof DragRun, m: keyof Drag) => {
    const p95 = runs.map((r) => r[key][m][1]);
    return { p50: median(runs.map((r) => r[key][m][0])), p95: median(p95), min: Math.min(...p95), max: Math.max(...p95) };
  };
  const sets = after === undefined ? [before] : [before, after];
  const head = ["drag (ms)", "runs", ...sets.flatMap((_, i) => [`${labels[i]} p50`, `${labels[i]} p95 (spread)`]), ...(after === undefined ? [] : [`${labels[1]} p95 vs ${labels[0]} spread`])];
  const rows = DRAGS.flatMap(([key, name]) => (["interval", "response"] as const).map((m) => {
    const s = sets.map((runs) => side(runs, key, m));
    const cell = (x: (typeof s)[0]) => `${fmt(x.p50)} | ${fmt(x.p95)} (${fmt(x.min)} to ${fmt(x.max)})`;
    const verdict = s.length === 2 ? [s[1].p95 >= s[0].min && s[1].p95 <= s[0].max ? "within" : "outside"] : [];
    return `| ${[`${name}, ${m === "interval" ? "frame interval" : "response"}`, sets.map((runs) => runs.length).join("/"), ...s.map(cell), ...verdict].join(" | ")} |`;
  }));
  return [`| ${head.join(" | ")} |`, `|${"---|".repeat(head.length)}`, ...rows].join("\n");
}

/**
 * The memory dump `guid` (the `dumpGuid` that `Tracing.requestMemoryDump` returned) in a Chrome trace, as top-level allocator sizes of
 * the GPU and renderer processes. Chrome does not put the guid on the dump's `ph: "v"` events (their `id` is `0x0`, and an explicit
 * request is named `periodic_interval` like a periodic one); it puts it on the `GlobalMemoryDump` async event, whose begin and end bracket the
 * dump's `v` events by time. Throws when the trace has no such dump. Of the `v` events of one process in it, the one with the most allocators
 * is the dump's. Only a top-level name that has a `size` is listed. Allocators overlap (`shared_memory` and `malloc` hold bytes others
 * also count), so the rows do not add up to the process. A spare renderer is passed over: the one with the most bytes is kept.
 */
export function memoryDump(events: TraceEvent[], guid: string): Allocators {
  const start = events.find((e) => e.name === "GlobalMemoryDump" && e.ph === "b" && e.args?.dump_guid === guid);
  const end = events.find((e) => e.name === "GlobalMemoryDump" && e.ph === "e" && e.id2?.local === start?.id2?.local);
  if (!start || !end || start.ts === undefined || end.ts === undefined) throw new Error(`the trace has no memory dump ${guid}`);
  const kind = new Map(events.filter((e) => e.name === "process_name").map((e) => [e.pid, { "GPU Process": "gpu", Renderer: "renderer" }[e.args?.name as string]]));
  const best = new Map<number, Record<string, any>>();
  for (const e of events) {
    const al = e.ph === "v" && (e.ts ?? -1) >= start.ts && (e.ts ?? -1) <= end.ts ? e.args?.dumps?.allocators : undefined;
    if (al && Object.keys(al).length > Object.keys(best.get(e.pid) ?? {}).length) best.set(e.pid, al);
  }
  const out: Allocators = {};
  const sum = (r: Record<string, number>) => Object.values(r).reduce((a, b) => a + b, 0);
  for (const [pid, al] of best) {
    const type = kind.get(pid);
    if (!type) continue;
    const row = Object.fromEntries(Object.entries(al).filter(([k, a]) => !k.includes("/") && a.attrs?.size).map(([k, a]) => [k, parseInt(a.attrs.size.value, 16)]));
    if (!out[type] || sum(row) > sum(out[type])) out[type] = row;
  }
  return out;
}

/** The median MB of each allocator over the runs, per process, one column per set of runs, largest first (by the first column). */
export function allocatorTable(sets: Run[][], labels: string[]): string {
  const mb = (runs: Run[], p: string, a: string) => median(runs.flatMap((r) => (r.allocators?.[p]?.[a] === undefined ? [] : [r.allocators[p][a] / 1048576])));
  const rows = ["gpu", "renderer"].flatMap((p) => [...new Set(sets.flatMap((rs) => rs.flatMap((r) => Object.keys(r.allocators?.[p] ?? {}))))].map((a) => ({ p, a, v: sets.map((rs) => mb(rs, p, a)) })).sort((x, y) => (y.v[0] || 0) - (x.v[0] || 0)));
  return [`| process | allocator | ${labels.map((l) => `${l} (MB)`).join(" | ")} |`, `|---|---|${labels.map(() => "---|").join("")}`, ...rows.map(({ p, a, v }) => `| ${p} | ${a} | ${v.map(fmt).join(" | ")} |`)].join("\n");
}

interface CpuNode { id: number; parent?: number; callFrame: { functionName: string; url?: string }; children?: number[] }
interface TraceEvent { id?: string; id2?: { local?: string }; ts?: number; name: string; pid: number; tid: number; ph: string; args: any }

/**
 * Folded stacks (`a;b;c count`) of the renderer main thread's CPU profile in a Chrome trace. Chrome writes the chunks on its
 * own `v8:ProfEvntProc` thread, so a profile is found by its `id` (in any renderer process (a spare one may come first), not by a
 * thread: of those profiles, the one with the most samples is the main thread's.
 */
export function foldStacks(events: TraceEvent[]): Map<string, number> {
  const renderers = new Set(events.filter((e) => e.name === "thread_name" && e.args?.name === "CrRendererMain").map((e) => e.pid));
  const chunks = new Map<string, TraceEvent[]>();
  for (const e of events) {
    if (e.name !== "ProfileChunk" || (renderers.size > 0 && !renderers.has(e.pid))) continue;
    const id = `${e.pid}:${(e as any).id ?? e.args?.data?.id ?? ""}`;
    chunks.set(id, [...(chunks.get(id) ?? []), e]);
  }
  const count = (c: TraceEvent[]) => c.reduce((n, e) => n + (e.args?.data?.cpuProfile?.samples?.length ?? 0), 0);
  const profile = [...chunks.values()].sort((a, b) => count(b) - count(a))[0] ?? [];
  const nodes = new Map<number, CpuNode>();
  const parent = new Map<number, number>();
  const samples: number[] = [];
  for (const e of profile) {
    const p = e.args?.data?.cpuProfile;
    for (const n of (p?.nodes ?? []) as CpuNode[]) {
      nodes.set(n.id, n);
      for (const c of n.children ?? []) parent.set(c, n.id);
      if (n.parent !== undefined) parent.set(n.id, n.parent);
    }
    samples.push(...(p?.samples ?? []));
  }
  const folded = new Map<string, number>();
  for (const s of samples) {
    const stack: string[] = [];
    for (let id: number | undefined = s; id !== undefined; id = parent.get(id)) stack.unshift(nodes.get(id)?.callFrame.functionName || "(anonymous)");
    const key = stack.join(";");
    folded.set(key, (folded.get(key) ?? 0) + 1);
  }
  return folded;
}

interface Frame { name: string; value: number; kids: Map<string, Frame> }

/** An SVG flame graph (root at the top, width = samples) of folded stacks. */
export function flameSvg(folded: Map<string, number>): string {
  const root: Frame = { name: "all", value: 0, kids: new Map() };
  for (const [stack, n] of folded) {
    let f = root;
    f.value += n;
    for (const name of stack.split(";")) {
      if (!f.kids.has(name)) f.kids.set(name, { name, value: 0, kids: new Map() });
      f = f.kids.get(name)!;
      f.value += n;
    }
  }
  const W = 1200;
  const out: string[] = [];
  let depth = 0;
  const esc = (s: string) => s.replace(/[&<>"]/g, (c) => `&#${c.charCodeAt(0)};`);
  const draw = (f: Frame, x: number, d: number) => {
    depth = Math.max(depth, d);
    const w = (f.value / Math.max(1, root.value)) * W;
    if (w < 0.5) return;
    out.push(`<g><title>${esc(f.name)} (${f.value} samples)</title><rect x="${x.toFixed(1)}" y="${d * 16}" width="${w.toFixed(1)}" height="15" fill="hsl(${(f.name.length * 37) % 50 + 5},80%,60%)"/>${w > 40 ? `<text x="${(x + 2).toFixed(1)}" y="${d * 16 + 11}" font-size="10">${esc(f.name.slice(0, Math.floor(w / 6)))}</text>` : ""}</g>`);
    let cx = x;
    for (const k of f.kids.values()) {
      draw(k, cx, d + 1);
      cx += (k.value / Math.max(1, root.value)) * W;
    }
  };
  draw(root, 0, 0);
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${(depth + 1) * 16}" font-family="monospace">${out.join("")}</svg>\n`;
}

/** The marker every process and directory the command makes on the mini carries: a temp directory named `gantt-profile.<id>`. */
const OWN_PATH = /\/gantt-profile\.\w+(?=\/|\s|$)/;
/** A directory the command made (and the only kind it removes). */
export const isOwnDir = (path: string): boolean => /^\/[\w./-]*\/gantt-profile\.\w+$/.test(path);
/** The pids in `ps -axo pid=,command=` output whose command line holds a marked path: leftovers of an earlier run. A process without it is never listed. */
export function ownPids(ps: string): string[] {
  return ps.split("\n").flatMap((line) => {
    const m = line.trim().match(/^(\d+)\s+(.*)$/);
    return m && OWN_PATH.test(m[2]) ? [m[1]] : [];
  });
}

/** A timer for a run that may hang, armed on creation: `mark(text)` restarts it (for `limit` ms, default `ms`) and keeps the text; when it runs out, `onStall(lastText, limit)` is called once. */
export function watchdog(ms: number, onStall: (last: string, limit: number) => void): { mark(text: string, limit?: number): void; stop(): void } {
  let last = "";
  let timer: NodeJS.Timeout;
  const arm = (limit: number) => (clearTimeout(timer), (timer = setTimeout(() => onStall(last, limit), limit)));
  arm(ms);
  return { mark: (text, limit = ms) => ((last = text), arm(limit)), stop: () => clearTimeout(timer) };
}
