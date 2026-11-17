// The DHTMLX side: its hooks (in the driver, and against a fake DHTMLX-like page), the page script
// (against a fake `gantt`), and the setup that refuses a work tree, pins the package, labels the
// build and makes DHTMLX's Google Fonts local. Nothing here installs or runs DHTMLX's code, so the
// test needs no network; the side itself runs against the real package installed by
// `dhtmlx-setup.ts`. The adapter's schedule is `demos/bench/adapt.test.ts`'s.

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, test } from "node:test";
import { fileURLToPath } from "node:url";
import { runInThisContext } from "node:vm";
import { loadSide } from "../drive.ts";
import { buildLabel, FONT_HOST, GANTT, indexHtml, LIBRARY, localCss, localFonts, packageJson, PACKAGES, parseArgs, type Fetcher } from "./dhtmlx-setup.ts";
import dhtmlx from "./dhtmlx.ts";

const here = dirname(fileURLToPath(import.meta.url));
const node = (script: string, args: string[]) => spawnSync(process.execPath, [join(here, script), ...args], { encoding: "utf8" });
const scratch = (prefix: string) => mkdtempSync(join(tmpdir(), prefix));
const sha256 = (b: string | Buffer) => createHash("sha256").update(b).digest("hex");
const g = globalThis as any;

const SAVED = ["window", "document", "gantt", "fetch", "location", "pageLog", "ganttDataDir", "ganttDataUrl", "ganttDatasetSha256", "ganttParsedAt"];
const before = Object.fromEntries(SAVED.map((k) => [k, g[k]]));
afterEach(() => {
  for (const k of SAVED) {
    if (before[k] === undefined) delete g[k];
    else g[k] = before[k];
  }
});

const HOOKS = ["ready", "scrollTarget", "leafBars", "dragState", "backend", "debugBuild", "build", "rowsOnScreen", "spotCheck", "settled", "datasetSha256"] as const;

test("the side exports its hooks, and the driver loads it", async () => {
  const side = await loadSide(join(here, "dhtmlx.ts"));
  assert.equal(side.name, "dhtmlx");
  for (const k of HOOKS) assert.equal(typeof side[k], "function", k);
  // Each hook goes to the page as its source text, which only an arrow function evaluates back to.
  for (const k of HOOKS) assert.doesNotThrow(() => new Function(`return (${side[k]!.toString()});`), k);
});

test("setup refuses a directory inside a git work tree or one not empty, and wrong arguments", () => {
  assert.deepEqual(parseArgs(["/x"]), { scratch: "/x" });
  for (const bad of [[], ["/x", "/y"], ["--help"]]) assert.equal(node("dhtmlx-setup.ts", bad).status, 64, JSON.stringify(bad));
  const repo = scratch("bench-dhtmlx-repo-");
  try {
    execFileSync("git", ["init", "-q", repo]);
    const r = node("dhtmlx-setup.ts", [join(repo, "side")]);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /refusing to put DHTMLX code at .*inside a git work tree/);
    assert.equal(existsSync(join(repo, "side")), false, "nothing written");
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
  const full = scratch("bench-dhtmlx-full-");
  writeFileSync(join(full, "old"), "");
  const r = node("dhtmlx-setup.ts", [full]);
  assert.equal(r.status, 1);
  assert.match(r.stderr, /is not empty/);
});

test("the package is pinned, the label carries both caveats, and the page loads DHTMLX's build then ours, nothing remote", () => {
  const deps = JSON.parse(packageJson()).dependencies;
  assert.deepEqual(deps, { "dhtmlx-gantt": "10.0.3" });
  assert.deepEqual(deps, { ...PACKAGES });
  // The label the table prints DHTMLX's numbers under: its drag reschedules nothing, and it is handed every date.
  assert.equal(buildLabel(), "DHTMLX Gantt 10.0.3 (MIT, no scheduling, no working-time calendars)");
  const html = indexHtml(buildLabel());
  assert.match(html, /<meta name="bench-build" content="DHTMLX Gantt 10\.0\.3 \(MIT, no scheduling, no working-time calendars\)">/);
  assert.match(html, /<link rel="stylesheet" href="dhtmlx\.css">/);
  assert.match(html, /<div id="gantt_here"><\/div>\n<script src="node_modules\/dhtmlx-gantt\/codebase\/dhtmlxgantt\.js"><\/script>\n<script src="page\.js"><\/script>/);
  assert.equal(LIBRARY, `node_modules/${GANTT}/codebase/dhtmlxgantt.js`);
  assert.doesNotMatch(html, /https?:/, "nothing remote");
});

const GOOGLE_CSS = `@font-face{font-family:Inter;font-weight:300;src:url(https://fonts.gstatic.com/s/inter/v20/AAA.ttf) format("truetype")}
@font-face{font-family:Inter;font-weight:400;src:url(https://fonts.gstatic.com/s/inter/v20/BBB.ttf) format("truetype")}.gantt_container{font-family:Inter}
@font-face{font-family:Inter;font-weight:500;src:url(https://fonts.gstatic.com/s/inter/v20/AAA.ttf)}`;

test("the stylesheet's Google Fonts become local files, each its pinned SHA-256; another hash or a path out of scratch is refused", async () => {
  const { css, files } = localCss(GOOGLE_CSS);
  assert.doesNotMatch(css, /fonts\.gstatic\.com|https?:/);
  assert.match(css, /url\(fonts\/inter\/v20\/AAA\.ttf\)/);
  assert.deepEqual(files, ["fonts/inter/v20/AAA.ttf", "fonts/inter/v20/BBB.ttf"], "each file once");
  const install = (text: string) => {
    const dir = scratch("bench-dhtmlx-fonts-");
    mkdirSync(join(dir, "node_modules", GANTT, "codebase"), { recursive: true });
    writeFileSync(join(dir, "node_modules", GANTT, "codebase", "dhtmlxgantt.css"), text);
    return dir;
  };
  const asked: string[] = [];
  const host = (bytes: Record<string, string>): Fetcher => async (url) => {
    asked.push(url);
    return { ok: true, status: 200, arrayBuffer: async () => new TextEncoder().encode(bytes[url] ?? "?").buffer as ArrayBuffer };
  };
  const files2 = { [`${FONT_HOST}inter/v20/AAA.ttf`]: "a", [`${FONT_HOST}inter/v20/BBB.ttf`]: "b" };
  const pins = { "fonts/inter/v20/AAA.ttf": sha256("a"), "fonts/inter/v20/BBB.ttf": sha256("b") };
  const dir = install(GOOGLE_CSS);
  await localFonts(dir, host(files2), pins);
  assert.deepEqual(asked, Object.keys(files2));
  assert.equal(readFileSync(join(dir, "fonts/inter/v20/BBB.ttf"), "utf8"), "b");
  assert.equal(readFileSync(join(dir, "dhtmlx.css"), "utf8"), css);
  const bad = install(GOOGLE_CSS);
  await assert.rejects(localFonts(bad, host({ ...files2, [`${FONT_HOST}inter/v20/BBB.ttf`]: "changed" }), pins), /BBB\.ttf has SHA-256 \w+, not the pinned/);
  assert.equal(existsSync(join(bad, "dhtmlx.css")), false, "no stylesheet over a refused font");
  const out = install("@font-face{src:url(https://fonts.gstatic.com/s/../../x.ttf)}");
  await assert.rejects(localFonts(out, host({}), { "fonts/../../x.ttf": sha256("?") }), /refusing to write fonts\/\.\.\/\.\.\/x\.ttf: outside the scratch directory/);
  await assert.rejects(localFonts(install("@font-face{src:url(https://fonts.gstatic.com/s/inter/v21/new.ttf)}"), host({}), pins), /is not a pinned font file/);
});

/** A fake DOM element: a box, and what its selectors find. */
class El {
  attrs: Record<string, string> = {};
  /** The element's own classes. */
  classes: string[] = [];
  /** The classes of the elements it sits in (its ancestors'), for a descendant selector. */
  within: string[] = [];
  rect: { left: number; top: number; width: number; height: number };
  constructor(rect: { left: number; top: number; width: number; height: number }, extra: Record<string, number> = {}) {
    this.rect = rect;
    Object.assign(this, extra);
  }
  getBoundingClientRect() {
    const { left, top, width, height } = this.rect;
    return { left, top, width, height, x: left, y: top, right: left + width, bottom: top + height };
  }
  getAttribute(name: string) {
    return this.attrs[name] ?? null;
  }
}

const DAY = new Date(2026, 2, 2, 8);
const LEAF = ".gantt_bars_area .gantt_task_line.gantt_bar_task";

/**
 * The elements of `all` a selector picks, with real selector semantics for the forms the hooks use:
 * descendant steps (matched against `within`), `.class` and `:not(.class)`. Any other form throws, so a hook
 * cannot move to a selector this fake does not understand and still pass.
 */
function select(all: El[], selector: string): El[] {
  const steps = selector.trim().split(/\s+/).map((step) => {
    const parts = step.match(/\.[\w-]+|:not\(\.[\w-]+\)/g) ?? [];
    if (parts.join("") !== step) throw new Error(`the fake DOM does not understand ${step} of ${selector}`);
    return { has: parts.filter((p) => p[0] === ".").map((p) => p.slice(1)), not: parts.filter((p) => p[0] === ":").map((p) => p.slice(6, -1)) };
  });
  const own = steps[steps.length - 1];
  const ancestors = steps.slice(0, -1);
  return all.filter(
    (el) =>
      own.has.every((c) => el.classes.includes(c)) &&
      own.not.every((c) => !el.classes.includes(c)) &&
      ancestors.every((a) => a.has.every((c) => el.within.includes(c)) && a.not.every((c) => !el.within.includes(c))),
  );
}

/** The classes DHTMLX Gantt 10.0.3 gives each kind of bar (recorded from its published build, headless Chrome, 2026-11-03). */
const BAR_CLASSES = {
  project: ["gantt_task_line", "gantt_project", "gantt_bar_project", "gantt_dependent_task"],
  task: ["gantt_task_line", "gantt_bar_task"],
  milestone: ["gantt_task_line", "gantt_milestone", "gantt_bar_milestone", "gantt_thin_task"],
};

/** A fake DHTMLX page: task 1's bar `barLeft` px into the bars area, where its start date puts it at 160 px. */
function page(o: { barLeft?: number; rows?: number; parsed?: boolean; unit?: string; meta?: string; script?: boolean } = {}) {
  const bar = new El({ left: 371 - 900 + (o.barLeft ?? 160), top: 73, width: 70, height: 24 }, { offsetLeft: o.barLeft ?? 160 });
  bar.attrs["data-task-id"] = "1";
  bar.classes = BAR_CLASSES.task;
  // A project's (summary) bar, the first row: the hook must leave it out.
  const project = new El({ left: 371 - 900 + 100, top: 37, width: 400, height: 24 });
  project.attrs["data-task-id"] = "9";
  project.classes = BAR_CLASSES.project;
  const milestone = new El({ left: 600, top: 109, width: 20, height: 20 });
  milestone.attrs["data-task-id"] = "2";
  milestone.classes = BAR_CLASSES.milestone;
  for (const el of [project, bar, milestone]) el.within = ["gantt_bars_area", "gantt_task"];
  // The timeline cell, and its data area: as wide as the whole axis, scrolled 900 px along.
  const cell = new El({ left: 371, top: 1, width: 893, height: 782 });
  const area = new El({ left: 371 - 900, top: 37, width: 20000, height: 746 }, { clientHeight: 746 });
  const ver = new El({ left: 1264, top: 37, width: 16, height: 746 }, { scrollHeight: 36000, clientHeight: 746 });
  const hor = new El({ left: 371, top: 783, width: 893, height: 16 }, { scrollWidth: 20000, clientWidth: 893 });
  const task = { id: 1, start_date: DAY, end_date: new Date(2026, 2, 3, 16) };
  const tasks = new Map<string, any>([["1", task], ["2", { id: 2, type: "milestone", start_date: new Date(2026, 2, 4, 16), end_date: new Date(2026, 2, 4, 16) }]]);
  g.window = g;
  g.ganttParsedAt = o.parsed === false ? undefined : 1234.5;
  g.gantt = {
    getTaskCount: () => o.rows ?? 1000,
    isTaskExists: (id: unknown) => tasks.has(String(id)),
    getTask: (id: unknown) => tasks.get(String(id)),
    posFromDate: (d: Date) => (d === DAY ? 160 : -1),
    getScrollState: () => ({ x: 900, y: 350 }),
    getScale: () => ({ unit: o.unit ?? "day", step: 1, col_width: 70 }),
    config: { row_height: 36 },
  };
  const found: Record<string, any> = { ".gantt_task": cell, ".gantt_task .gantt_data_area": area, ".gantt_ver_scroll": ver, ".gantt_hor_scroll": hor, [LEAF]: bar, ".gantt_task .gantt_row, .gantt_grid .gantt_row": new El(cell.rect) };
  const meta = o.meta === undefined ? null : { getAttribute: () => o.meta };
  g.document = {
    querySelector: (sel: string) => (sel.startsWith("meta") ? meta : sel.startsWith("script") ? (o.script === false ? null : {}) : (found[sel] ?? null)),
    querySelectorAll: (sel: string) => select([project, bar, milestone], sel),
  };
  return { task, tasks };
}

test("ready: parsed, the asked-for rows are in, and a leaf's bar is drawn where its start date puts it", () => {
  g.window = g;
  assert.equal(dhtmlx.ready(1000), false, "no gantt yet");
  page();
  assert.equal(dhtmlx.ready(1000), true);
  assert.equal(dhtmlx.ready(999), false, "another row count");
  page({ parsed: false });
  assert.equal(dhtmlx.ready(1000), false, "parse has not returned");
  page({ barLeft: 162 });
  assert.equal(dhtmlx.ready(1000), false, "a bar 2 px from its date");
  page({ rows: 0 });
  assert.equal(dhtmlx.ready(1000), false, "the empty chart");
});

test("the scroll box is the timeline's data area clipped to the timeline; bars, drag state and the rest come from the page and DHTMLX's store", () => {
  const { task } = page({ meta: buildLabel() });
  assert.deepEqual(dhtmlx.scrollTarget(), { x: 371, y: 37, width: 893, height: 746, top: 350, left: 900, maxTop: 35254, maxLeft: 19107 }, "not the 20,000 px axis scrolled off to the left; both ranges are the scrollbars' scroll size less their own");
  assert.deepEqual(dhtmlx.leafBars(), {
    bars: [
      { x: -369, y: 73, width: 70, height: 24, id: "1", datasetId: "1" },
      { x: 600, y: 109, width: 20, height: 20, id: "2", datasetId: "2" },
    ],
    pxPerDay: 70,
  });
  const reported = dhtmlx.leafBars().bars.map((b) => b.id);
  assert.equal(reported.length, 2, "a project with a summary bar and two leaves reports two bars");
  assert.ok(!reported.includes("9"), "not the project's bar");
  assert.equal(dhtmlx.rowsOnScreen!(), 20);
  const t = { x: 0, y: 85, pxPerDay: 70, id: "1" };
  const was = dhtmlx.dragState(t);
  assert.equal(was, String(DAY.getTime()));
  task.start_date = new Date(2026, 2, 12, 8);
  assert.notEqual(dhtmlx.dragState(t), was, "the drop moved the task");
  assert.equal(dhtmlx.backend!(), "DOM");
  assert.equal(dhtmlx.build!(), "DHTMLX Gantt 10.0.3 (MIT, no scheduling, no working-time calendars)");
  assert.equal(dhtmlx.debugBuild!(), false, "the package's published build");
  page({ script: false });
  assert.equal(dhtmlx.debugBuild!(), true, "a page without it");
  assert.equal(dhtmlx.settled!(), 1234.5, "when parse returned");
  page({ parsed: false });
  assert.equal(dhtmlx.settled!(), null);
  page({ unit: "hour" });
  assert.ok(Number.isNaN((dhtmlx.leafBars() as { pxPerDay: number }).pxPerDay), "not the day scale");
});

test("the dataset hash is the one inside the file the page fetched, only when that file hashes to its sidecar", async () => {
  page();
  const inside = "ab".repeat(32);
  const body = `{"datasetSha256":"${inside}","config":{},\n"data":[],\n"links":[]}\n`;
  const serve = (text: string, sidecar: string) => (url: string) => {
    const t = url.endsWith(".sha256") ? sidecar : text;
    return Promise.resolve({ ok: true, text: async () => t, arrayBuffer: async () => new TextEncoder().encode(t).buffer });
  };
  assert.equal(await dhtmlx.datasetSha256!(), null, "the page fetched nothing");
  g.ganttDataUrl = "http://side.test/cells/c/dhtmlx.json";
  g.ganttDatasetSha256 = inside;
  g.fetch = serve(body, `${sha256(body)}  dhtmlx.json\n`);
  assert.equal(await dhtmlx.datasetSha256!(), inside);
  g.fetch = serve(body.replace("[]", "[ ]"), `${sha256(body)}  dhtmlx.json\n`);
  assert.equal(await dhtmlx.datasetSha256!(), null, "a file changed since its sidecar");
  g.ganttDatasetSha256 = "cd".repeat(32);
  g.fetch = serve(body, `${sha256(body)}  dhtmlx.json\n`);
  assert.equal(await dhtmlx.datasetSha256!(), null, "not the file the page loaded");
});

test("the spot check passes DHTMLX's dates to the minute and names a task that is off, or a missing file", async () => {
  page();
  g.ganttDataDir = "cells/stress-1000-1/";
  let asked = "";
  const serve = (body: unknown, ok = true) => (url: string) => {
    asked = url;
    return Promise.resolve({ ok, json: async () => body });
  };
  const milestone = { id: 2, start: "2026-03-04T16:00", end: "2026-03-04T16:00" };
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:00", end: "2026-03-03T16:00" }, milestone]);
  assert.equal(await dhtmlx.spotCheck!(), null);
  assert.equal(asked, "cells/stress-1000-1/expected.json");
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:01", end: "2026-03-03T16:00" }]);
  assert.equal(await dhtmlx.spotCheck!(), "task 1 is 2026-03-02T08:00 to 2026-03-03T16:00, the schedule's is 2026-03-02T08:01 to 2026-03-03T16:00");
  // What DHTMLX's community edition would make of a working-minute duration: calendar minutes, so an evening end.
  g.fetch = serve([{ id: 1, start: "2026-03-02T08:00", end: "2026-03-02T16:00" }]);
  assert.match((await dhtmlx.spotCheck!())!, /^task 1 is 2026-03-02T08:00 to 2026-03-03T16:00/, "an end moved");
  g.fetch = serve([{ ...milestone, end: "2026-03-05T16:00" }]);
  assert.match((await dhtmlx.spotCheck!())!, /^task 2 is 2026-03-04T16:00 to 2026-03-04T16:00/, "a milestone ends where it starts");
  g.fetch = serve([{ id: 9, start: "2026-03-02T08:00", end: "2026-03-03T16:00" }]);
  assert.match((await dhtmlx.spotCheck!())!, /^task 9 is none to none/, "a task DHTMLX does not hold");
  g.fetch = serve([]);
  assert.match((await dhtmlx.spotCheck!())!, /no tasks/);
  g.fetch = serve(null, false);
  assert.match((await dhtmlx.spotCheck!())!, /no expected\.json/);
});

test("the page: library ready, the chart made, data start, the cell's file, its config, then parse without the hash", async () => {
  const log: any[] = [];
  g.window = g;
  g.location = { search: "?cell=stress-1000-1", href: "http://side.test/" };
  const config: Record<string, unknown> = {};
  g.gantt = {
    config: new Proxy(config, { set: (c, k, v) => ((c[k as string] = v), log.push({ set: k }), true) }),
    init: (id: string) => log.push({ init: id }),
    parse: (x: unknown) => log.push({ parse: x }),
  };
  const file = { datasetSha256: "ef".repeat(32), config: { date_format: "%d-%m-%Y %H:%i", duration_unit: "minute" }, data: [{ id: 1 }], links: [{ id: 1 }] };
  g.fetch = (url: string) => (log.push({ fetch: url }), Promise.resolve({ json: async () => structuredClone(file) }));
  const real = performance.mark.bind(performance);
  (performance as any).mark = (name: string) => (log.push({ mark: name }), real(name));
  try {
    runInThisContext(readFileSync(join(here, "dhtmlx-page.js"), "utf8"));
    for (let i = 0; i < 20 && g.ganttParsedAt === undefined; i++) await new Promise((r) => setImmediate(r));
  } finally {
    (performance as any).mark = real;
  }
  const at = (pred: (e: any) => boolean) => log.findIndex(pred);
  const order = [at((e) => e.mark === "library-ready"), at((e) => e.init !== undefined), at((e) => e.mark === "data-start"), at((e) => e.fetch !== undefined), at((e) => e.set === "date_format"), at((e) => e.parse !== undefined)];
  assert.ok(order.every((i, k) => i >= 0 && (k === 0 || i > order[k - 1])), `the order: ${JSON.stringify(log.map((e) => Object.values(e)[0]))}`);
  assert.equal(log[order[1]].init, "gantt_here");
  assert.equal(log[order[3]].fetch, "http://side.test/cells/stress-1000-1/dhtmlx.json");
  assert.deepEqual(log[order[5]].parse, { data: file.data, links: file.links }, "DHTMLX never sees the hash");
  // DHTMLX's large-data guidance, set before the chart is made; no work-time setting (the edition has none).
  const beforeInit = log.slice(0, order[1]).map((e) => e.set);
  for (const k of ["smart_rendering", "smart_scales", "autosize", "duration_step", "columns"]) assert.ok(beforeInit.includes(k), k);
  assert.deepEqual([config.smart_rendering, config.smart_scales, config.autosize, config.duration_step, config.duration_unit], [true, true, false, 1, "minute"]);
  assert.equal("work_time" in config, false);
  assert.deepEqual((config.columns as { name: string }[]).map((c) => c.name), ["text", "start_date", "duration"]);
  assert.deepEqual([g.ganttDatasetSha256, g.ganttDataUrl, g.ganttDataDir, typeof g.ganttParsedAt], ["ef".repeat(32), "http://side.test/cells/stress-1000-1/dhtmlx.json", "cells/stress-1000-1/", "number"]);
});

test("nothing of DHTMLX's is in this repository", () => {
  assert.deepEqual(readdirSync(here).filter((f) => /dhtmlxgantt|^codebase$/.test(f)), []);
});
