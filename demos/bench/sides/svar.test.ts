// The SVAR Gantt side: its hooks (in the driver, and against a fake SVAR-like page), the page script
// (against fake React and SVAR modules), the setup that refuses a work tree, and the adapter that
// hands SVAR the same schedule as ours. Nothing here installs or runs SVAR's code, so the test needs
// no network; the side itself runs against the real package installed by `svar-setup.ts`.

import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { execFileSync, spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";
import { buildDataset, datasetJson, SHAPES } from "../dataset.ts";
import { loadSide } from "../drive.ts";
import { dhtmlxDates, toDhtmlx } from "./dhtmlx-adapt.ts";
import { expectedOf, LINK_TYPE, svarJson, toSvar, type SvarData } from "./svar-adapt.ts";
import * as setupModule from "./svar-setup.ts";
import { BUNDLE_ARGS, buildLabel, GANTT, indexHtml, packageJson, PACKAGES, parseArgs } from "./svar-setup.ts";
import svar from "./svar.ts";

const here = dirname(fileURLToPath(import.meta.url));
const node = (script: string, args: string[]) => spawnSync(process.execPath, [join(here, script), ...args], { encoding: "utf8" });
const scratch = (prefix: string) => mkdtempSync(join(tmpdir(), prefix));
const sha256 = (b: string | Buffer) => createHash("sha256").update(b).digest("hex");
const g = globalThis as any;

const SAVED = ["window", "document", "gantt", "fetch", "location", "pageLog", "ganttDataDir", "ganttDataUrl", "ganttDatasetSha256", "ganttProductionBuild"];
const before = Object.fromEntries(SAVED.map((k) => [k, g[k]]));
afterEach(() => {
  for (const k of SAVED) {
    if (before[k] === undefined) delete g[k];
    else g[k] = before[k];
  }
});

test("the side exports its hooks, and the driver loads it", async () => {
  const side = await loadSide(join(here, "svar.ts"));
  assert.equal(side.name, "svar");
  const hooks = ["ready", "scrollTarget", "leafBars", "dragState", "backend", "debugBuild", "build", "rowsOnScreen", "spotCheck", "datasetSha256"] as const;
  for (const k of hooks) assert.equal(typeof side[k], "function", k);
  // Each hook goes to the page as its source text, which only an arrow function evaluates back to.
  for (const k of hooks) assert.doesNotThrow(() => new Function(`return (${side[k]!.toString()});`), k);
});

test("setup refuses a directory inside a git work tree or one not empty, and wrong arguments", () => {
  assert.deepEqual(parseArgs(["/x"]), { scratch: "/x" });
  for (const bad of [[], ["/x", "/y"], ["--trial"]]) assert.equal(node("svar-setup.ts", bad).status, 64, JSON.stringify(bad));
  const repo = scratch("bench-svar-repo-");
  try {
    execFileSync("git", ["init", "-q", repo]);
    const r = node("svar-setup.ts", [join(repo, "side")]);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /refusing to put SVAR code at .*inside a git work tree/);
    assert.equal(existsSync(join(repo, "side")), false, "nothing written");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
  const full = scratch("bench-svar-full-");
  writeFileSync(join(full, "old"), "");
  const r = node("svar-setup.ts", [full]);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /is not empty/);
});

test("every package is pinned, the label says SVAR does not schedule, and the page is a production bundle of ours", () => {
  const deps = JSON.parse(packageJson()).dependencies;
  assert.deepEqual(deps, { ...PACKAGES });
  for (const v of Object.values(deps)) assert.match(v as string, /^\d+\.\d+\.\d+$/, "exact versions");
  // The label the table prints SVAR's numbers under carries that its drag reschedules nothing.
  assert.equal(buildLabel(), `SVAR React Gantt ${PACKAGES[GANTT]} (MIT, no scheduling)`);
  const html = indexHtml(buildLabel());
  assert.match(html, /<meta name="bench-build" content="SVAR React Gantt \d+\.\d+\.\d+ \(MIT, no scheduling\)">/);
  assert.match(html, /<div id="root"><\/div>\n<script type="module" src="page.js"><\/script>/);
  assert.ok(BUNDLE_ARGS.includes("--minify") && BUNDLE_ARGS.includes('--define:process.env.NODE_ENV="production"'));
});

const tasksOf = (x: SvarData) => new Map(x.tasks.map((t) => [String(t.id), t]));

test("the same schedule as ours: every record, parent and link, dated as our engine dates it", () => {
  for (const shape of SHAPES) {
    const d = buildDataset(shape, 1000, 20261020n);
    const x = toSvar(d);
    assert.deepEqual(x.tasks.map((t) => [t.id, t.text, t.parent]), d.data.map((t) => [t.id, t.text, t.parent ?? 0]), `${shape}: ids, names, parents, order`);
    assert.deepEqual(x.links, d.links.map((l) => ({ id: l.id, source: l.source, target: l.target, type: LINK_TYPE[l.type] })), `${shape}: links`);
    // The dates are the ones handed to DHTMLX, which match our engine's to the minute (docs/MEASURING.md, "Handing the schedule to each side").
    const ours = dhtmlxDates(toDhtmlx(d));
    const parents = new Set(d.data.map((t) => String(t.parent ?? 0)));
    for (const t of x.tasks) {
      if (parents.has(String(t.id))) {
        assert.equal(t.type, "summary");
        assert.equal(t.open, true, "every row shown");
        continue;
      }
      const [s, e] = ours.get(t.text)!;
      assert.deepEqual([t.start, t.end ?? s], [s, e], `${shape} ${t.id}`);
      assert.equal(t.type, s === e && t.end === undefined ? "milestone" : "task", `${shape} ${t.id}: a leaf of no work is a milestone, with no end`);
    }
  }
  const stress = toSvar(buildDataset("stress", 1000, 20261020n));
  assert.ok(stress.tasks.some((t) => t.type === "milestone") && stress.tasks.some((t) => t.start.endsWith(":01")), "milestones and minute starts are covered");
  assert.deepEqual(new Set(stress.links.map((l) => l.type)), new Set(["e2s", "s2s", "e2e", "s2e"]));
  const tree = toSvar(buildDataset("tree4", 1000, 20261020n));
  const byId = tasksOf(tree);
  for (const t of tree.tasks.filter((t) => t.type === "summary")) {
    const kids = tree.tasks.filter((k) => String(k.parent) === String(t.id));
    assert.equal(t.start, kids.map((k) => k.start).sort()[0], `summary ${t.id} starts with its first child`);
    assert.equal(t.end, kids.map((k) => k.end!).sort().at(-1), `summary ${t.id} ends with its last child`);
    assert.ok(byId.has(String(t.id)));
  }
});

test("the adapter: refuses a dataset its hash does not name, writes the cell's files with the dataset's hash, the same bytes twice", () => {
  const dir = scratch("bench-svar-adapt-");
  const d = buildDataset("tree4", 1000, 20261020n);
  const input = join(dir, "dataset.json");
  const bytes = datasetJson(d);
  writeFileSync(input, bytes);
  writeFileSync(`${input}.sha256`, `${"0".repeat(64)}  dataset.json\n`);
  const out = join(dir, "side");
  const bad = node("svar-adapt.ts", [input, out]);
  assert.equal(bad.status, 1);
  assert.match(bad.stderr, /names 0{64}, not these bytes/);
  assert.equal(existsSync(out), false, "nothing written");
  assert.equal(node("svar-adapt.ts", [input]).status, 64);

  writeFileSync(`${input}.sha256`, `${sha256(bytes)}  dataset.json\n`);
  assert.equal(node("svar-adapt.ts", [input, out]).status, 0);
  const cell = join(out, "cells", "tree4-1000-20261020");
  const json = readFileSync(join(cell, "svar.json"), "utf8");
  assert.equal(json, svarJson(toSvar(d, sha256(bytes))));
  assert.equal(JSON.parse(json).datasetSha256, sha256(bytes));
  assert.equal(readFileSync(join(cell, "svar.json.sha256"), "utf8"), `${sha256(json)}  svar.json\n`);
  const expected = JSON.parse(readFileSync(join(cell, "expected.json"), "utf8"));
  assert.equal(expected.length, 64);
  assert.deepEqual(expected, expectedOf(toSvar(d)));
  const byId = tasksOf(JSON.parse(json));
  for (const e of expected) assert.deepEqual([e.start, e.end], [byId.get(String(e.id))!.start, byId.get(String(e.id))!.end], "the dates handed to SVAR");
  assert.equal(node("svar-adapt.ts", [input, out]).status, 0);
  assert.equal(readFileSync(join(cell, "svar.json"), "utf8"), json, "a pure function of the dataset");
});

type Rect = { left: number; top: number; width: number; height: number };
/** An element with a box, data attributes, scroll offsets and the children a selector finds. */
class El {
  dataset: Record<string, string> = {};
  scrollTop = 0;
  scrollLeft = 0;
  scrollHeight = 0;
  scrollWidth = 0;
  clientWidth = 0;
  clientHeight = 0;
  parentElement: El | null = null;
  rect: Rect;
  found: Record<string, El[]>;
  constructor(rect: Rect, found: Record<string, El[]> = {}) {
    this.rect = rect;
    this.found = found;
  }
  getBoundingClientRect() {
    const { left, top, width, height } = this.rect;
    return { left, top, width, height, x: left, y: top, right: left + width, bottom: top + height };
  }
  querySelector(sel: string) {
    return this.found[sel]?.[0] ?? null;
  }
  querySelectorAll(sel: string) {
    return this.found[sel] ?? [];
  }
}

const DAY = new Date(2026, 2, 2, 8);
const LEAF = ".wx-bars .wx-bar.wx-task";

/** A fake page: task 1's bar `barLeft` px into the bars, where its start date (two days into the scale) puts it at 80 px. */
function page(o: { barLeft?: number; rows?: number; unit?: string; meta?: string } = {}) {
  const bars = new El({ left: 445, top: 73, width: 11200, height: 35000 });
  const bar = new El({ left: 445 + (o.barLeft ?? 80), top: 108, width: 80, height: 28 });
  bar.parentElement = bars;
  bar.dataset.id = "1";
  const summary = new El({ left: 445, top: 73, width: 400, height: 28 });
  const scale = new El({ left: 445, top: 0, width: 11200, height: 73 });
  const chart = new El({ left: 445, top: 0, width: 835, height: 800 }, { ".wx-scale": [scale] });
  Object.assign(chart, { clientWidth: 820, clientHeight: 785, scrollLeft: 120, scrollWidth: 20000 });
  const gantt = new El({ left: 0, top: 0, width: 1280, height: 800 }, { ".wx-chart": [chart] });
  Object.assign(gantt, { scrollTop: 350, scrollHeight: 35072, clientHeight: 800 });
  const task = { id: 1, type: "task", start: DAY, end: new Date(2026, 2, 3, 16) };
  const tasks = new Map<unknown, any>([[1, task]]);
  g.window = g;
  g.gantt = {
    getState: () => ({ _tasks: { length: o.rows ?? 1000 }, cellHeight: 35, _scales: { lengthUnit: o.unit ?? "day", lengthUnitWidth: 40, start: new Date(2026, 1, 28) } }),
    getTask: (id: unknown) => tasks.get(id),
  };
  const found: Record<string, any> = { ".wx-gantt": gantt, ".wx-gantt .wx-row": new El(bars.rect), [LEAF]: bar, ".wx-gantt .wx-chart": chart };
  const meta = o.meta === undefined ? null : { getAttribute: () => o.meta };
  g.document = {
    querySelector: (sel: string) => (sel.startsWith("meta") ? meta : (found[sel] ?? null)),
    querySelectorAll: (sel: string) => (sel === ".wx-bars .wx-bar:not(.wx-summary)" ? [bar] : sel === ".wx-bars .wx-bar" ? [summary, bar] : []),
  };
  return { task, tasks };
}

test("ready: the asked-for rows are in, and a leaf's bar is drawn where its start date puts it", () => {
  g.window = g;
  assert.equal(svar.ready(1000), false, "no Gantt yet");
  page();
  assert.equal(svar.ready(1000), true);
  assert.equal(svar.ready(999), false, "another row count");
  page({ barLeft: 82 });
  assert.equal(svar.ready(1000), false, "a bar 2 px from its date: drawn before the data");
  page({ rows: 0 });
  assert.equal(svar.ready(1000), false, "the empty Gantt");
  page({ unit: "hour" });
  assert.equal(svar.ready(1000), false, "not the day scale this side reads");
});

test("the scroll box, the bars and the drag state come from the page and SVAR's store", () => {
  const { task } = page({ meta: "SVAR React Gantt 2.8.2" });
  assert.deepEqual(svar.scrollTarget(), { x: 445, y: 73, width: 820, height: 712, top: 350, left: 120, maxTop: 34272, maxLeft: 19180 }, "the chart below its scale");
  assert.deepEqual(svar.leafBars(), { bars: [{ x: 525, y: 108, width: 80, height: 28, id: "1", datasetId: "1" }], pxPerDay: 40 }, "no summary's bar");
  assert.equal(svar.rowsOnScreen!(), 20);
  const t = { x: 565, y: 122, pxPerDay: 40, id: "1" };
  const was = svar.dragState(t);
  assert.equal(was, String(DAY.getTime()));
  task.start = new Date(2026, 2, 12, 8);
  assert.notEqual(svar.dragState(t), was, "the drop moved the task");
  assert.equal(svar.backend!(), "DOM");
  assert.equal(svar.build!(), "SVAR React Gantt 2.8.2");
  assert.equal(svar.debugBuild!(), true, "a page that did not say it is a production build");
  g.ganttProductionBuild = true;
  assert.equal(svar.debugBuild!(), false);
});

test("the dataset hash is the one inside the file the page fetched, only when that file hashes to its sidecar", async () => {
  page();
  const inside = "ab".repeat(32);
  const body = `{"datasetSha256":"${inside}","tasks":[],"links":[]}\n`;
  const serve = (text: string, sidecar: string) => (url: string) => {
    const t = url.endsWith(".sha256") ? sidecar : text;
    return Promise.resolve({ ok: true, text: async () => t, arrayBuffer: async () => new TextEncoder().encode(t).buffer });
  };
  assert.equal(await svar.datasetSha256!(), null, "the page fetched nothing");
  g.ganttDataUrl = "http://side.test/cells/c/svar.json";
  g.ganttDatasetSha256 = inside;
  g.fetch = serve(body, `${sha256(body)}  svar.json\n`);
  assert.equal(await svar.datasetSha256!(), inside);
  g.fetch = serve(body.replace("[]", "[ ]"), `${sha256(body)}  svar.json\n`);
  assert.equal(await svar.datasetSha256!(), null, "a file changed since its sidecar");
  g.ganttDatasetSha256 = "cd".repeat(32);
  g.fetch = serve(body, `${sha256(body)}  svar.json\n`);
  assert.equal(await svar.datasetSha256!(), null, "not the file the page loaded");
});

test("the spot check passes SVAR's dates to the minute and names a task that is off, or a missing file", async () => {
  const { tasks } = page();
  tasks.set(2, { id: 2, type: "milestone", start: new Date(2026, 2, 4, 16) });
  g.ganttDataDir = "cells/stress-1000-1/";
  let asked = "";
  const serve = (body: unknown, ok = true) => (url: string) => {
    asked = url;
    return Promise.resolve({ ok, json: async () => body });
  };
  const milestone = { id: 2, start: "2026-03-04T16:00", end: "2026-03-04T16:00" };
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:00", end: "2026-03-03T16:00" }, milestone]);
  assert.equal(await svar.spotCheck!(), null);
  assert.equal(asked, "cells/stress-1000-1/expected.json");
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:01", end: "2026-03-03T16:00" }]);
  assert.match((await svar.spotCheck!())!, /^task 1 is 2026-03-02T08:00 to 2026-03-03T16:00, the schedule's is 2026-03-02T08:01 to 2026-03-03T16:00$/);
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:00", end: "2026-03-04T16:00" }]);
  assert.match((await svar.spotCheck!())!, /^task 1 is 2026-03-02T08:00 to 2026-03-03T16:00/, "an end a day off");
  g.fetch = serve([{ ...milestone, end: "2026-03-05T16:00" }]);
  assert.match((await svar.spotCheck!())!, /^task 2 is 2026-03-04T16:00 to 2026-03-04T16:00/, "a milestone is its start");
  g.fetch = serve([]);
  assert.match((await svar.spotCheck!())!, /no tasks/);
  g.fetch = serve(null, false);
  assert.match((await svar.spotCheck!())!, /no expected\.json/);
});

/** Fake React (one root, function components, useState, useEffect once) and a fake SVAR that logs each Gantt it is given. */
const FAKES: Record<string, string> = {
  "react/index.js": `let slots = [], at = 0, pending = [];
export const createElement = (type, props, ...children) => ({ type, props: { ...props, children } });
export function useState(v) { const k = at++; if (!(k in slots)) slots[k] = v; return [slots[k], (n) => { slots[k] = n; globalThis.rerender(); }]; }
export function useEffect(fn) { const k = at++; if (!(k in slots)) { slots[k] = true; pending.push(fn); } }
const walk = (el) => { if (typeof el.type === "function") return walk(el.type(el.props)); el.type.seen?.(el.props); el.props.children.forEach(walk); };
export function render(el) { at = 0; walk(el); const run = pending; pending = []; run.forEach((f) => f()); }`,
  "react-dom/client.js": `import { render } from "react";
export const createRoot = () => ({ render: (el) => { globalThis.rerender = () => render(el); render(el); } });`,
  // As SVAR's Gantt does (its Gantt.jsx): a mount (the first render, or a new \`key\`) inits the store from
  // the props in that render and calls \`init\` once; a later render with another \`init\` re-inits the store.
  "@svar-ui/react-gantt/index.js": `let last;
export const Willow = { seen: (p) => globalThis.pageLog.push({ willow: p }) };
export const Gantt = { seen: (p) => {
  const mount = last === undefined || last.key !== p.key;
  globalThis.pageLog.push({ gantt: p, mount });
  if (mount) p.init({ api: p.key });
  else if (p.init !== last.init) globalThis.pageLog.push({ reinit: p.key });
  last = p;
} };`,
};

async function runPage(search: string, production: boolean) {
  const dir = scratch("bench-svar-page-");
  writeFileSync(join(dir, "package.json"), '{"type":"module"}\n');
  for (const [file, text] of Object.entries(FAKES)) {
    const pkg = join(dir, "node_modules", dirname(file));
    mkdirSync(pkg, { recursive: true });
    writeFileSync(join(pkg, "package.json"), `{"type":"module","exports":{".":"./index.js","./client":"./client.js"}}\n`);
    writeFileSync(join(dir, "node_modules", file), text);
  }
  copyFileSync(join(here, "svar-page.js"), join(dir, "page.js"));
  g.pageLog = [];
  g.window = g;
  g.location = { search, href: "http://side.test/" };
  g.document = { getElementById: (id: string) => ({ id }) };
  g.fetch = (url: string) => {
    g.pageLog.push({ fetch: url });
    const tasks = [{ id: 1, text: "A", start: "2026-03-02T08:01", end: "2026-03-03T16:00", parent: 0, type: "task" }, { id: 2, text: "M", start: "2026-03-04T16:00", parent: 0, type: "milestone" }];
    return Promise.resolve({ json: async () => ({ datasetSha256: "ef".repeat(32), tasks, links: [{ id: 1, source: 1, target: 2, type: "e2s" }] }) });
  };
  const env = process.env.NODE_ENV;
  process.env.NODE_ENV = production ? "production" : "development";
  performance.clearMarks();
  const real = performance.mark.bind(performance);
  (performance as any).mark = (name: string) => (g.pageLog.push({ mark: name }), real(name));
  try {
    await import(`${pathToFileURL(join(dir, "page.js")).href}?${encodeURIComponent(search)}`);
    for (let i = 0; i < 20 && !g.pageLog.some((e: any) => e.gantt?.tasks.length > 0); i++) await new Promise((r) => setImmediate(r));
    g.rerender(); // a later render of the page, as any state change of React's makes
  } finally {
    (performance as any).mark = real;
    process.env.NODE_ENV = env;
  }
  return g.pageLog as any[];
}

test("the page: library ready, the empty Gantt, then data start, the cell's file, and a new Gantt mounted with the Dates, without the hash", async () => {
  const log = await runPage("?cell=stress-1000-1", true);
  const at = (pred: (e: any) => boolean, from = 0) => log.findIndex((e, i) => i >= from && pred(e));
  const empty = at((e) => e.gantt !== undefined);
  const loaded = at((e) => e.gantt?.tasks.length > 0);
  const order = [at((e) => e.mark === "library-ready"), empty, at((e) => e.mark === "data-start"), at((e) => e.fetch !== undefined), loaded];
  assert.ok(order.every((i, k) => i >= 0 && (k === 0 || i > order[k - 1])), `the order: ${JSON.stringify(log.map((e) => Object.keys(e)[0]))}`);
  const [first, full] = [log[empty].gantt, log[loaded].gantt];
  assert.deepEqual([first.tasks, first.links, first.cellWidth, first.cellHeight], [[], [], 40, 35], "made empty, as SVAR's own benchmark sizes it");
  // SVAR's first-mount path, as its benchmark loads (a new \`key\`): the store is made from the data in
  // that first render, not by an update of the empty Gantt (a render, a commit, an effect, a render).
  assert.equal(log[empty].mount, true);
  assert.equal(log[loaded].mount, true, "the data arrives in a new mount, not an update of the empty Gantt");
  assert.notEqual(full.key, first.key);
  assert.deepEqual(log.filter((e) => e.reinit !== undefined), [], "init is one function: no render re-inits the store");
  assert.equal(log.filter((e) => e.gantt?.tasks.length > 0 && !e.mount).length, log.filter((e) => e.gantt?.tasks.length > 0).length - 1, "later renders are updates");
  assert.equal(log[order[3]].fetch, "http://side.test/cells/stress-1000-1/svar.json");
  assert.equal(full.tasks[0].start.getTime(), new Date("2026-03-02T08:01").getTime(), "Dates, local time");
  assert.equal(full.tasks[0].end.getTime(), new Date("2026-03-03T16:00").getTime());
  assert.equal(full.tasks[1].end, undefined, "a milestone has its start only");
  assert.equal(full.datasetSha256, undefined, "SVAR never sees the hash");
  assert.deepEqual(full.links, [{ id: 1, source: 1, target: 2, type: "e2s" }]);
  assert.deepEqual([g.gantt, g.ganttDatasetSha256, g.ganttDataUrl, g.ganttDataDir, g.ganttProductionBuild], [{ api: full.key }, "ef".repeat(32), "http://side.test/cells/stress-1000-1/svar.json", "cells/stress-1000-1/", true]);
  await runPage("?cell=chain-1000-7", false);
  assert.equal(g.ganttProductionBuild, false, "a development build says so");
});

const CDN_CSS = `@font-face{font-family:"Open Sans";src:url(https://cdn.svar.dev/fonts/open-sans/regular.woff2) format("woff2"),url(https://cdn.svar.dev/fonts/open-sans/regular.woff) format("woff")}
.wx-willow-theme{font-family:Roboto}@font-face{font-family:Roboto;src:url(https://cdn.svar.dev/fonts/roboto/500.woff2)}`;

test("the built page loads nothing from cdn.svar.dev: SVAR's fonts and icons are served from scratch", async () => {
  const s = setupModule as any;
  assert.equal(typeof s.localCss, "function", "svar-setup.ts rewrites SVAR's stylesheet to local fonts");
  const { css, files } = s.localCss(CDN_CSS);
  assert.doesNotMatch(css, /cdn\.svar\.dev/);
  assert.match(css, /url\(fonts\/open-sans\/regular\.woff2\)/);
  assert.deepEqual(files, ["fonts/open-sans/regular.woff2", "fonts/open-sans/regular.woff", "fonts/roboto/500.woff2"], "each file the stylesheet names, fetched once");
  assert.deepEqual(s.iconFiles('src: url("./wx-icons.woff?5c1d") format("woff"),\nurl("./wx-icons.woff2?5c1d") format("woff2");'), ["fonts/wxi/wx-icons.woff", "fonts/wxi/wx-icons.woff2"]);
  const html = indexHtml(buildLabel());
  assert.doesNotMatch(html, /cdn\.svar\.dev|https?:/, "nothing remote");
  assert.match(html, /<link rel="stylesheet" href="svar\.css">\n<link rel="stylesheet" href="fonts\/wxi\/wx-icons\.css">/, "SVAR's stylesheet and its icons, local");
  // The theme's own font links (its \`fonts\` prop, on by default) would load from the CDN: the page turns them off.
  const log = await runPage("?cell=tree4-1000-1", true);
  assert.ok(log.some((e) => e.willow !== undefined) && log.filter((e) => e.willow !== undefined).every((e) => e.willow.fonts === false), "Willow without its CDN links");
});
