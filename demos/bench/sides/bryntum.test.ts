// The Bryntum side: its hooks, the setup that refuses without the reader's own trial or inside a
// work tree, the adapter that hands it the same schedule as ours, and the dates check that compares
// Bryntum's dates with ours. Nothing here installs or runs Bryntum's code, so the test needs no
// network and no trial; the comparison itself runs with the trial (`bryntum-dates.ts`).

import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { buildDataset, datasetJson, SHAPES } from "../dataset.ts";
import { loadSide } from "../drive.ts";
import { CALENDAR, countTasks, LINK_TYPE, toBryntum, type BryntumTask } from "./bryntum-adapt.ts";
import { assertTrial, diffDates, leafRecords, parseOurs, sample, withDates } from "./bryntum-dates.ts";
import { ALIAS, buildLabel, COMMUNITY, indexHtml, packageJson, parseArgs, TRIAL } from "./bryntum-setup.ts";

const here = dirname(fileURLToPath(import.meta.url));
const node = (script: string, args: string[]) => spawnSync(process.execPath, [join(here, script), ...args], { encoding: "utf8" });
const scratch = (prefix: string) => mkdtempSync(join(tmpdir(), prefix));

test("the side exports its hooks, and the driver loads it", async () => {
  const side = await loadSide(join(here, "bryntum.ts"));
  assert.equal(side.name, "bryntum");
  const hooks = ["ready", "scrollTarget", "leafBars", "dragState", "backend", "debugBuild", "build", "rowsOnScreen", "spotCheck", "settled", "datasetSha256"] as const;
  for (const k of hooks) assert.equal(typeof side[k], "function", k);
  // Each hook is sent to the page as its source text, which only an arrow function (or a plain
  // function expression) evaluates back to: a method's text does not. What each does is in
  // bryntum-fake.test.ts.
  for (const k of hooks) assert.doesNotThrow(() => new Function(`return (${side[k]!.toString()});`), k);
});

test("setup refuses without --trial, and installs nothing", () => {
  assert.match(parseArgs(["/x"]) as string, /without --trial/);
  assert.deepEqual(parseArgs(["--trial", "/x"]), { scratch: "/x" });
  assert.equal(typeof parseArgs(["--trial"]), "string");
  assert.equal(typeof parseArgs(["--trial", "/x", "/y"]), "string");
  const dir = join(scratch("bench-bryntum-"), "side");
  const r = node("bryntum-setup.ts", [dir]);
  assert.equal(r.status, 64);
  assert.match(r.stderr, /refusing to install without --trial: the Bryntum trial is the reader's own/);
  assert.equal(existsSync(dir), false, "nothing written");
});

test("setup refuses a scratch directory inside a git work tree", () => {
  const repo = scratch("bench-bryntum-repo-");
  try {
    execFileSync("git", ["init", "-q", repo]);
    const r = node("bryntum-setup.ts", ["--trial", join(repo, "side")]);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /refusing to put Bryntum code at .*inside a git work tree/);
    assert.equal(existsSync(join(repo, "side")), false);
  } finally {
    rmSync(repo, { recursive: true, force: true });
  }
});

test("the trial is pinned and installed under the name Bryntum's guide gives it", () => {
  assert.equal(TRIAL.name, "@bryntum/gantt-trial");
  assert.match(TRIAL.version, /^\d+\.\d+\.\d+$/);
  assert.deepEqual(JSON.parse(packageJson()).dependencies, { [ALIAS]: `npm:${TRIAL.name}@${TRIAL.version}` });
  assert.equal(buildLabel(), `Bryntum ${TRIAL.version} trial build`);
  assert.match(indexHtml(buildLabel()), /<meta name="bench-build" content="Bryntum \d+\.\d+\.\d+ trial build">/);
  assert.match(indexHtml(buildLabel()), /<script type="module" src="page.js"><\/script>/);
});

const leaves = (tasks: BryntumTask[]): BryntumTask[] => tasks.flatMap((t) => (t.children ? leaves(t.children) : [t]));
const all = (tasks: BryntumTask[]): BryntumTask[] => tasks.flatMap((t) => [t, ...all(t.children ?? [])]);

test("the Community tier (MIT, bundled, no --trial) gets its leaf's start as a start date, as it has no constraints", () => {
  assert.deepEqual(parseArgs(["--community", "/x"]), { scratch: "/x", community: true });
  assert.deepEqual(Object.keys(JSON.parse(packageJson(true)).dependencies), [COMMUNITY.name, "esbuild"]);
  assert.match(buildLabel(true), /^Bryntum 8\.\d+\.\d+ Community \(MIT\)/);
  assert.doesNotMatch(indexHtml(buildLabel(true), true), /importmap/);
  const d = buildDataset("tree4", 1000, 20261020n);
  const [trial, community] = [toBryntum(d), toBryntum(d, true)].map((p) => leaves(p.tasks).filter((t) => t.constraintType !== undefined || t.startDate !== undefined));
  assert.equal(community.length, trial.length);
  assert.ok(community.every((t) => t.constraintType === undefined && t.startDate !== undefined));
});

test("the same schedule as ours: Mon-Fri 08:00-16:00, hours, start-no-earlier-than at 08:00, every link", () => {
  assert.deepEqual(CALENDAR.intervals, [{ recurrentStartDate: "every weekday at 08:00", recurrentEndDate: "every weekday at 16:00", isWorking: true }]);
  assert.equal(CALENDAR.unspecifiedTimeIsWorking, false, "only the weekday hours work");
  for (const shape of SHAPES) {
    const d = buildDataset(shape, 1000, 20261020n);
    const p = toBryntum(d);
    assert.equal(p.calendar, CALENDAR.id);
    assert.deepEqual([p.hoursPerDay, p.daysPerWeek], [8, 5]);
    assert.equal(p.startDate, `${d.start}T08:00:00`);
    assert.equal(countTasks(p.tasks), d.data.length, `${shape}: every record is a task`);
    assert.equal(p.dependencies.length, d.links.length, `${shape}: every link`);
    const byId = new Map(all(p.tasks).map((t) => [String(t.id), t]));
    for (const t of d.data) {
      const b = byId.get(String(t.id))!;
      if (b.children) {
        assert.equal(b.expanded, true);
        assert.equal(b.duration, undefined, "a summary takes its dates from its children");
        continue;
      }
      assert.deepEqual([b.duration, b.durationUnit], [t.duration * 8, "hour"], `${shape} task ${t.id}`);
      if (t.start === undefined) assert.equal(b.constraintType, undefined);
      else assert.deepEqual([b.constraintType, b.constraintDate], ["startnoearlierthan", `${t.start}T08:00:00`]);
    }
    d.links.forEach((l, i) => {
      assert.deepEqual(p.dependencies[i], { id: l.id, fromTask: l.source, toTask: l.target, type: LINK_TYPE[l.type], lag: 0 });
    });
    // The calendar can reach past the latest constraint plus all the work end to end.
    const days = d.data.reduce((n, t) => n + t.duration, 0);
    assert.ok(p.maxCalendarRange >= (days * 7) / 5 * 86_400_000, `${shape}: calendar range`);
    assert.ok(leaves(p.tasks).every((t) => t.durationUnit === "hour"), `${shape}: every leaf has its hours`);
  }
  // Finish-to-start, start-to-start, finish-to-finish, start-to-finish to Bryntum's EndToStart 2, StartToStart 0, EndToEnd 3, StartToEnd 1.
  assert.deepEqual(LINK_TYPE, { "0": 2, "1": 0, "2": 3, "3": 1 });
  assert.ok(new Set(buildDataset("stress", 1000, 20261020n).links.map((l) => l.type)).size === 4, "stress has all four link types");
});

test("the adapter writes the same bytes for the same dataset, and refuses a dataset its hash does not name", () => {
  const dir = scratch("bench-bryntum-adapt-");
  try {
    const json = datasetJson(buildDataset("tree4", 1000, 1n));
    writeFileSync(join(dir, "dataset.json"), json);
    const scratchDir = join(dir, "side");
    mkdirSync(scratchDir);
    // The cell's files go under cells/<shape>-<size>-<seed>, the page's ?cell=.
    const out = join(scratchDir, "cells", "tree4-1000-1");
    let r = node("bryntum-adapt.ts", [join(dir, "dataset.json"), scratchDir]);
    assert.equal(r.status, 1, "no .sha256 beside it");
    assert.match(r.stderr, /dataset\.json\.sha256 is missing/);
    writeFileSync(join(dir, "dataset.json.sha256"), `${"0".repeat(64)}  dataset.json\n`);
    r = node("bryntum-adapt.ts", [join(dir, "dataset.json"), scratchDir]);
    assert.equal(r.status, 1, "a hash of other bytes");
    assert.equal(existsSync(join(out, "bryntum.json")), false, "nothing written");
    execFileSync(process.execPath, [join(here, "..", "dataset.ts"), "--shape", "tree4", "--size", "1000", "--seed", "1", "--out", dir]);
    r = node("bryntum-adapt.ts", [join(dir, "dataset.json"), scratchDir]);
    assert.equal(r.status, 0, r.stderr);
    assert.match(r.stdout, /cells\/tree4-1000-1: 1000 tasks, 891 links, sha256 [0-9a-f]{64}\n$/);
    const first = readFileSync(join(out, "bryntum.json"), "utf8");
    // The source dataset's hash is in the file, so the side reports the dataset it was made from.
    const datasetHash = readFileSync(join(dir, "dataset.json.sha256"), "utf8").split(" ")[0];
    assert.equal(JSON.parse(first).datasetSha256, datasetHash);
    assert.match(readFileSync(join(out, "bryntum.json.sha256"), "utf8"), /^[0-9a-f]{64} {2}bryntum\.json\n$/);
    assert.equal(node("bryntum-adapt.ts", [join(dir, "dataset.json"), scratchDir]).status, 0);
    assert.equal(readFileSync(join(out, "bryntum.json"), "utf8"), first, "same bytes");
    assert.equal(node("bryntum-adapt.ts", [join(dir, "dataset.json")]).status, 64);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test("the dates check: ours by name, differences in minutes, the dated variant, the sample, the trial gate", () => {
  const ours = parseOurs("Task 1\t2026-03-02T08:00\t2026-03-03T16:00\nTask 2\t2026-03-04T08:00\t2026-03-05T16:00\n");
  assert.deepEqual(ours.get("Task 2"), ["2026-03-04T08:00", "2026-03-05T16:00"]);
  assert.throws(() => parseOurs("A\tx\ty\nA\tx\ty\n"), /two tasks are named "A"/);
  const theirs = new Map<string, [string, string]>([["Task 1", ["2026-03-02T08:00", "2026-03-03T16:00"]], ["Task 2", ["2026-03-04T08:01", "2026-03-05T16:00"]]]);
  assert.deepEqual(diffDates(["Task 1", "Task 2"], ours, theirs).map((d) => [d.name, d.minutes]), [["Task 2", 1]]);
  assert.throws(() => diffDates(["Task 3"], ours, theirs), /missing/);

  const d = buildDataset("tree4", 1000, 1n);
  const leaves = leafRecords(d);
  assert.equal(leaves.length, d.data.filter((t) => t.start !== undefined).length, "the leaves are the records with a start");
  const picked = sample(leaves);
  assert.equal(picked.length, 64);
  assert.deepEqual([picked[0], picked[63]], [leaves[0], leaves[leaves.length - 1]]);
  assert.deepEqual(sample([1, 2, 3]), [1, 2, 3]);

  const p = toBryntum(d);
  const dates = new Map(all(p.tasks).map((t) => [String(t.id), ["2026-03-02T08:00:00", "2026-03-03T16:00:00"] as [string, string]]));
  const dated = all(withDates(p, dates).tasks);
  assert.equal(dated.length, d.data.length);
  assert.ok(dated.every((t) => t.startDate === "2026-03-02T08:00:00" && t.endDate === "2026-03-03T16:00:00"), "every task carries its dates");
  assert.ok(dated.filter((t) => !t.children).every((t) => t.durationUnit === "hour"), "and keeps its duration and constraint");
  assert.throws(() => withDates(p, new Map()), /no dates for task/);

  const dir = scratch("bench-bryntum-dates-");
  try {
    assert.throws(() => assertTrial(dir), /run bryntum-setup\.ts --trial/);
    const r = node("bryntum-dates.ts", [join(dir, "dataset.json"), dir]);
    assert.equal(r.status, 1);
    assert.match(r.stderr, /no @bryntum\/gantt-trial@[\d.]+ at /);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
