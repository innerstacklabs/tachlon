import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { DEFAULT_START, toMspdi, type Dataset } from "../dhtmlx-compare/to-mspdi.ts";
import { adapt, assertSameCounts, countDhtmlx, DHTMLX_CONFIG, dhtmlxDate, dhtmlxDates, dhtmlxJson, expectedOf, isoDate, toDhtmlx, writeAll, type DhtmlxData } from "./sides/dhtmlx-adapt.ts";
import { buildDataset, datasetJson, SHAPES, START } from "./dataset.ts";
import { keyOf, stampAt } from "./plan.ts";
import { toBryntum } from "./sides/bryntum-adapt.ts";

const here = dirname(fileURLToPath(import.meta.url));
const sha = (s: string) => createHash("sha256").update(s).digest("hex");
const parse = (json: string) => JSON.parse(json) as DhtmlxData;

/** MS Project's link `Type` (0 FF, 1 FS, 2 SF, 3 SS) to the dataset's and DHTMLX's ("0" FS, "1" SS, "2" FF, "3" SF). */
const FROM_MSPDI: Record<string, string> = { "1": "0", "3": "1", "0": "2", "2": "3" };

interface ChartTask { name: string; parent?: string; summary: boolean; milestone: boolean; hours: number; start?: string }

/** Our side's file (`toMspdi`, which `sides/ours-adapt.ts` writes) read back field by field: tasks by name, and links as `pred > succ type lag`. */
function readChart(xml: string): { tasks: Map<string, ChartTask>; links: string[] } {
  const tasks = new Map<string, ChartTask>();
  const byUid = new Map<string, string>();
  const preds: [string, string, string, string][] = [];
  const path: string[] = [];
  for (const [block] of xml.matchAll(/<Task>.*?<\/Task>/g)) {
    const field = (name: string) => block.match(new RegExp(`<${name}>([^<]*)</${name}>`))?.[1];
    const name = field("Name")!;
    const level = Number(field("OutlineLevel"));
    path.length = level - 1;
    const [, h, m] = field("Duration")!.match(/^PT(\d+)H(\d+)M0S$/)!;
    assert.ok(!tasks.has(name), `two tasks are named ${name}`);
    tasks.set(name, { name, parent: path.at(-1), summary: field("Summary") === "1", milestone: field("Milestone") === "1", hours: Number(h) + Number(m) / 60, start: field("ConstraintDate")?.slice(0, 16) });
    byUid.set(field("UID")!, name);
    path.push(name);
    for (const [, uid, type, lag] of block.matchAll(/<PredecessorLink><PredecessorUID>(\d+)<\/PredecessorUID><Type>(\d)<\/Type><LinkLag>(-?\d+)<\/LinkLag>/g)) preds.push([uid, name, type, lag]);
  }
  return { tasks, links: preds.map(([uid, succ, type, lag]) => `${byUid.get(uid)} > ${succ} ${FROM_MSPDI[type]} ${lag}`).sort() };
}

test("the DHTMLX file holds the dataset's task and link counts, for every shape and size", () => {
  for (const shape of SHAPES) {
    for (const size of [1000, 10000, 50000]) {
      const d = buildDataset(shape, size, 20261020n);
      assert.deepEqual(countDhtmlx(adapt(d).dhtmlx), { tasks: size, links: d.links.length }, `${shape} ${size}`);
    }
  }
});

/** Working minutes from a DHTMLX record's start to its end, on our calendar (a day end counts as the minute that ends the day). */
const workedMinutes = (start: string, t: { start_date: string; end_date: string }) => Math.ceil(keyOf(start, isoDate(t.end_date)) / 2) - Math.ceil(keyOf(start, isoDate(t.start_date)) / 2);

test("our file read back holds, per task, the DHTMLX record's parent, working time, type, start and links", () => {
  const seen = new Set<string>();
  for (const shape of SHAPES) {
    const d = buildDataset(shape, 1000, 20261020n);
    const a = adapt(d);
    const chart = readChart(toMspdi(d));
    assert.equal(chart.tasks.size, d.data.length, `${shape}: tasks in our file`);
    const x = parse(a.dhtmlx);
    const text = new Map(x.data.map((t) => [String(t.id), t.text]));
    for (const t of x.data) {
      const c = chart.tasks.get(t.text);
      assert.ok(c, `${shape}: ${t.text} is not in our file`);
      assert.equal(t.parent === 0 ? undefined : text.get(String(t.parent)), c.parent, `${shape} ${t.text}: parent`);
      assert.equal(t.type === "project", c.summary, `${shape} ${t.text}: summary`);
      assert.equal(t.type === "milestone", c.milestone, `${shape} ${t.text}: milestone`);
      if (!c.summary) assert.equal(workedMinutes(d.start, t), c.hours * 60, `${shape} ${t.text}: working minutes from start to end`);
      // A start the file gives is a start-no-earlier-than our engine keeps (the links never move it).
      if (c.start !== undefined) assert.equal(t.start_date, dhtmlxDate(c.start), `${shape} ${t.text}: start`);
      if (t.type === "milestone") seen.add("milestone");
    }
    const links = x.links.map((l) => `${text.get(String(l.source))} > ${text.get(String(l.target))} ${l.type} ${l.lag}`).sort();
    assert.deepEqual(links, chart.links, `${shape}: links`);
    for (const l of x.links) seen.add(`type ${l.type}`);
  }
  assert.deepEqual([...seen].sort(), ["milestone", "type 0", "type 1", "type 2", "type 3"], "stress covers all four link types and milestones");
});

// Our engine's dates for each leaf, `name<TAB>start<TAB>finish` sorted, from
//   node demos/bench/dataset.ts --shape <shape> --size 1000 --seed 20261020 --out <dir>
//   node demos/dhtmlx-compare/to-mspdi.ts <dir>/dataset.json <dir>/chart.xml
//   cargo run -q --release -p import --example mspdi_dates -- <dir>/chart.xml
// (leaves only: that example prints a summary's own row, not its children's span). Checked the same
// way on 2026-11-03 for every shape at 10,000 and 50,000 tasks: every leaf agrees to the minute.
const ENGINE_LEAF_DATES: Record<string, string> = {
  chain: "9d0884f4f73a841d77dadd5bd0ee7e4c20e4736b712a4bea690cd1582beabf87",
  tree4: "0067ebc34620fd5b9bd67b4aa9436beece8b6b63143544cdce1fab297158b1eb",
  stress: "e181fe6df70bfac7ee7cfbcd8dba5d9d78d026a8e433572af2e1d74189aa2dd1",
};

// Both dates are handed to DHTMLX (its community edition has no working-time calendar to work an
// end out on), and the side's spot check reads a sample of them back out of DHTMLX's store at every run.
test("the dates handed to DHTMLX are where our engine places every leaf, start and finish, on every shape", () => {
  for (const shape of SHAPES) {
    const d = buildDataset(shape, 1000, 20261020n);
    const x = parse(adapt(d).dhtmlx);
    const parents = new Set(x.data.map((t) => String(t.parent)));
    const dates = dhtmlxDates(x);
    const lines = x.data.filter((t) => !parents.has(String(t.id))).map((t) => `${t.text}\t${dates.get(t.text)!.join("\t")}`).sort();
    assert.equal(sha(`${lines.join("\n")}\n`), ENGINE_LEAF_DATES[shape], shape);
  }
});

test("a chain's leaves follow each other: leaf k starts 2(k - 1) working days in, so a 1,000-task chain ends in 2033", () => {
  const x = parse(adapt(buildDataset("chain", 1000, 1n)).dhtmlx);
  const leaf = (k: number) => x.data.find((t) => t.text === `Task ${k}`)!;
  for (const k of [1, 2, 3, 4, 500, 999]) assert.equal(leaf(k).start_date, dhtmlxDate(stampAt(START, 2 * (k - 1) * 480)), `Task ${k}`);
  assert.equal(leaf(3).start_date, "06-03-2026 08:00"); // Friday, after Monday-Tuesday and Wednesday-Thursday
  assert.equal(leaf(4).start_date, "10-03-2026 08:00"); // Tuesday: the weekend is skipped
  assert.equal(workedMinutes(START, x.data.find((t) => t.text === "Project")!), 999 * 2 * 480);
  assert.equal(leaf(4).end_date, "11-03-2026 16:00", "Tuesday to Wednesday, two working days");
});

test("the page's settings: minutes and the date format, no work time (the community edition has no calendars); every task carries both dates", () => {
  const d = buildDataset("stress", 1000, 1n);
  const x = parse(adapt(d).dhtmlx);
  assert.deepEqual(x.config, { date_format: "%d-%m-%Y %H:%i", duration_unit: "minute" });
  assert.deepEqual(x.config, DHTMLX_CONFIG);
  assert.equal("workTime" in x, false, "no setWorkTime calls: inert in the MIT edition, so none are made");
  for (const t of x.data) {
    assert.deepEqual(Object.keys(t).filter((k) => k.endsWith("_date") || k === "duration"), ["start_date", "end_date"], `${t.text}: both dates, no duration for DHTMLX to work an end out from`);
    if (t.type === "milestone") assert.equal(t.end_date, t.start_date, `${t.text}: a milestone ends where it starts`);
  }
  assert.match(toMspdi(d), /<DayType>2<\/DayType><DayWorking>1<\/DayWorking><WorkingTimes><WorkingTime><FromTime>08:00:00<\/FromTime><ToTime>16:00:00<\/ToTime>/);
  // Stress durations are 2 hours and up, in minutes, and most stress tasks start a minute past the
  // hour (08:01), which the minute format keeps.
  assert.ok(x.data.some((t) => t.type === undefined && workedMinutes(d.start, t) === 120));
  const big = parse(adapt(buildDataset("stress", 10000, 20261020n)).dhtmlx);
  assert.ok(big.data.filter((t) => !t.start_date.endsWith(":00")).length > big.data.length / 2);
});

test("a duration that is missing, negative, not a number or not whole minutes is refused on both sides", () => {
  for (const duration of [undefined, -1, Number.NaN, "2"] as unknown as number[]) {
    const d: Dataset = { start: START, data: [{ id: 1, text: "A", duration }], links: [] };
    assert.throws(() => toMspdi(d), /task 1 has duration/, String(duration));
    assert.throws(() => toDhtmlx(d), /task 1 has duration/, String(duration));
  }
  assert.throws(() => toDhtmlx({ start: START, data: [{ id: 1, text: "A", duration: 0.01 }], links: [] }), /not a whole number of minutes/);
});

test("a dataset with no start starts on DEFAULT_START on every side: ours, DHTMLX and Bryntum", () => {
  const d: Dataset = { data: [{ id: 1, text: "A", duration: 1 }], links: [] };
  assert.match(toMspdi(d), new RegExp(`<StartDate>${DEFAULT_START}T08:00:00</StartDate>`));
  assert.equal(toDhtmlx(d).data[0].start_date, dhtmlxDate(`${DEFAULT_START}T08:00`));
  assert.equal(toBryntum(d as Parameters<typeof toBryntum>[0]).startDate, `${DEFAULT_START}T08:00:00`);
});

test("the same dataset gives the same bytes", () => {
  for (const shape of SHAPES) assert.deepEqual(adapt(buildDataset(shape, 5000, 7n)), adapt(buildDataset(shape, 5000, 7n)), shape);
});

test("expected.json: 64 leaves evenly spaced, no project, with the dates handed to DHTMLX", () => {
  const x = toDhtmlx(buildDataset("tree4", 1000, 20261020n));
  const e = expectedOf(x);
  assert.equal(e.length, 64);
  const leaves = x.data.filter((t) => t.type !== "project");
  assert.deepEqual(e[0], { id: leaves[0].id, start: isoDate(leaves[0].start_date), end: isoDate(leaves[0].end_date) });
  assert.equal(e.at(-1)!.id, leaves.at(-1)!.id);
  const ids = new Set(x.data.filter((t) => t.type === "project").map((t) => t.id));
  assert.ok(e.every((t) => !ids.has(t.id)));
});

test("an output that lost a record is refused", () => {
  const d = buildDataset("chain", 1000, 1n);
  const x = toDhtmlx(d);
  const want = { tasks: d.data.length, links: d.links.length };
  const short = dhtmlxJson({ ...x, data: x.data.slice(1) });
  assert.throws(() => assertSameCounts(want, { dhtmlx: countDhtmlx(short) }), /dhtmlx holds 999 tasks and 998 links; the dataset has 1000 and 998/);
  const noLink = dhtmlxJson({ ...x, links: x.links.slice(1) });
  assert.throws(() => assertSameCounts(want, { dhtmlx: countDhtmlx(noLink) }), /dhtmlx holds 1000 tasks and 997 links/);
});

/** File operations whose `n`th call of `op` throws. */
function failingOn(op: "write" | "rename", n: number) {
  let calls = 0;
  const fail = () => {
    if (++calls === n) throw new Error(`${op} ${n} failed`);
  };
  return {
    write: (p: string, t: string) => (op === "write" && fail(), writeFileSync(p, t)),
    rename: (a: string, b: string) => (op === "rename" && fail(), renameSync(a, b)),
  };
}

const listing = (dir: string) => Object.fromEntries(readdirSync(dir).sort().map((f) => [f, readFileSync(join(dir, f), "utf8")]));

test("writing is all or nothing: a failed write or rename leaves the directory as it was", () => {
  const files: [string, string][] = [["dhtmlx.json", "new a"], ["other.json", "new b"]];
  for (const [op, n] of [["write", 3], ["rename", 1], ["rename", 3], ["rename", 4]] as const) {
    const empty = mkdtempSync(join(tmpdir(), "bench-adapt-"));
    assert.throws(() => writeAll(empty, files, failingOn(op, n)), new RegExp(`${op} ${n} failed`));
    assert.deepEqual(readdirSync(empty), [], `${op} ${n}, empty directory`);
    // With an earlier set there: renames 1 and 2 move it aside, 3 to 6 put the new set in place.
    const full = mkdtempSync(join(tmpdir(), "bench-adapt-"));
    writeAll(full, [["dhtmlx.json", "old a"], ["other.json", "old b"]]);
    const before = listing(full);
    assert.throws(() => writeAll(full, files, failingOn(op, op === "rename" ? n + 2 : n)));
    assert.deepEqual(listing(full), before, `${op} ${n}, earlier set kept`);
  }
  const dir = mkdtempSync(join(tmpdir(), "bench-adapt-"));
  writeAll(dir, [["dhtmlx.json", "old"]]);
  writeAll(dir, [["dhtmlx.json", "new"]]);
  assert.deepEqual(readdirSync(dir).sort(), ["dhtmlx.json", "dhtmlx.json.sha256"]);
  assert.equal(readFileSync(join(dir, "dhtmlx.json"), "utf8"), "new");
});

function datasetDir(): string {
  const dir = mkdtempSync(join(tmpdir(), "bench-adapt-"));
  const json = datasetJson(buildDataset("tree4", 1000, 5n));
  writeFileSync(join(dir, "dataset.json"), json);
  writeFileSync(join(dir, "dataset.json.sha256"), `${sha(json)}  dataset.json\n`);
  return dir;
}

const cli = (...args: string[]) => spawnSync(process.execPath, [join(here, "sides", "dhtmlx-adapt.ts"), ...args], { encoding: "utf8" });

test("the CLI writes dhtmlx.json and expected.json with their SHA-256s and the dataset's hash inside, and prints the counts", () => {
  const dir = datasetDir();
  const out = join(dir, "out");
  const r = cli(join(dir, "dataset.json"), out);
  assert.equal(r.status, 0, r.stderr);
  const datasetHash = sha(readFileSync(join(dir, "dataset.json"), "utf8"));
  const a = adapt(buildDataset("tree4", 1000, 5n), datasetHash);
  assert.equal(readFileSync(join(out, "dhtmlx.json"), "utf8"), a.dhtmlx);
  assert.equal(readFileSync(join(out, "dhtmlx.json.sha256"), "utf8"), `${sha(a.dhtmlx)}  dhtmlx.json\n`);
  assert.equal(parse(a.dhtmlx).datasetSha256, datasetHash);
  assert.equal(readFileSync(join(out, "expected.json"), "utf8"), a.expected);
  assert.deepEqual(readdirSync(out).sort(), ["dhtmlx.json", "dhtmlx.json.sha256", "expected.json", "expected.json.sha256"]);
  assert.match(r.stdout, new RegExp(`dataset sha256 ${datasetHash}: 1000 tasks, ${a.counts.links} links in dhtmlx.json, expected.json beside it`));
});

test("the CLI refuses a dataset whose hash does not match, or has none, and writes nothing", () => {
  const dir = datasetDir();
  writeFileSync(join(dir, "dataset.json.sha256"), `${"0".repeat(64)}  dataset.json\n`);
  let r = cli(join(dir, "dataset.json"), join(dir, "out"));
  assert.equal(r.status, 1);
  assert.match(r.stderr, /dataset\.json\.sha256 names 0{64}, not these bytes/);
  const bare = mkdtempSync(join(tmpdir(), "bench-adapt-"));
  writeFileSync(join(bare, "dataset.json"), datasetJson(buildDataset("chain", 1000, 1n)));
  r = cli(join(bare, "dataset.json"), join(bare, "out"));
  assert.equal(r.status, 1);
  assert.match(r.stderr, /dataset\.json\.sha256 is missing/);
  assert.deepEqual(readdirSync(dir).sort(), ["dataset.json", "dataset.json.sha256"]);
  assert.deepEqual(readdirSync(bare), ["dataset.json"]);
});

test("the CLI exits 64 on a wrong number of arguments", () => {
  assert.equal(cli().status, 64);
  assert.equal(cli("a.json").status, 64);
  assert.equal(cli("a.json", "out", "extra").status, 64);
  assert.equal(cli("--help", "out").status, 64);
});
