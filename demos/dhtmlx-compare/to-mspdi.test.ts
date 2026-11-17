import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { toMspdi, type Dataset, type DatasetLink, type DatasetTask } from "./to-mspdi.ts";

const here = dirname(fileURLToPath(import.meta.url));

/** 1,000 leaf tasks in one chain with one break: 998 links (CONFIGURATION.md, flat chain). */
function flatChain(): Dataset {
  const tasks: DatasetTask[] = [];
  const links: DatasetLink[] = [];
  for (let i = 1; i <= 1000; i++) tasks.push({ id: i, text: `Task ${i}`, duration: 1 + (i % 5) });
  for (let i = 1; i < 1000; i++) if (i !== 500) links.push({ id: i, source: i, target: i + 1, type: "0" });
  return { data: tasks, links };
}

/**
 * 1,000 records in a 4-level tree: 1 project, 9 phases, 90 packages, 900 leaf tasks; 891 links, all
 * between leaves (the app drops a link with a summary at either end).
 */
function projectTree(): Dataset {
  const tasks: DatasetTask[] = [];
  const leaves: number[] = [];
  let id = 0;
  const add = (text: string, duration: number, parent?: number): number => {
    id++;
    tasks.push(parent === undefined ? { id, text, duration } : { id, text, duration, parent });
    return id;
  };
  const project = add("Project", 0);
  for (let p = 1; p <= 9; p++) {
    const phase = add(`Phase ${p}`, 0, project);
    for (let k = 1; k <= 10; k++) {
      const pkg = add(`Package ${p}.${k}`, 0, phase);
      for (let t = 1; t <= 10; t++) leaves.push(add(`Task ${p}.${k}.${t}`, 1 + (t % 4), pkg));
    }
  }
  const links: DatasetLink[] = [];
  for (let i = 0; i < 891; i++) links.push({ id: i + 1, source: leaves[i], target: leaves[i + 1], type: "0" });
  return { data: tasks, links };
}

const count = (xml: string, re: RegExp): number => (xml.match(re) ?? []).length;

test("a 1,000-record flat chain becomes 1,000 tasks and 998 links", () => {
  const xml = toMspdi(flatChain());
  assert.equal(count(xml, /<Task>/g), 1000);
  assert.equal(count(xml, /<PredecessorLink>/g), 998);
  assert.equal(count(xml, /<Summary>1<\/Summary>/g), 0);
});

test("a 1,000-record project tree becomes 1,000 tasks, with summaries counted, and 891 links", () => {
  const xml = toMspdi(projectTree());
  assert.equal(count(xml, /<Task>/g), 1000);
  assert.equal(count(xml, /<PredecessorLink>/g), 891);
  assert.ok(count(xml, /<Summary>1<\/Summary>/g) > 0);
  assert.equal(count(xml, /<OutlineLevel>4<\/OutlineLevel>/g) > 0, true);
});

test("a child follows its parent and sits one outline level deeper", () => {
  const xml = toMspdi({
    data: [
      { id: 7, text: "Parent", duration: 0 },
      { id: 9, text: "Child", duration: 3, parent: 7 },
    ],
    links: [],
  });
  assert.match(xml, /<UID>1<\/UID><Name>Parent<\/Name><OutlineLevel>1<\/OutlineLevel><Summary>1<\/Summary>/);
  assert.match(xml, /<UID>2<\/UID><Name>Child<\/Name><OutlineLevel>2<\/OutlineLevel><Summary>0<\/Summary><Duration>PT24H0M0S<\/Duration>/);
});

test("a link refers to the predecessor's file UID, not the harness id", () => {
  const xml = toMspdi({
    data: [
      { id: 50, text: "A", duration: 1 },
      { id: 60, text: "B", duration: 1 },
    ],
    links: [{ id: 1, source: 50, target: 60, type: "0" }],
  });
  assert.match(xml, /<UID>2<\/UID>[^]*<PredecessorLink><PredecessorUID>1<\/PredecessorUID><Type>1<\/Type>/);
});

test("a link to an unknown task is an error", () => {
  assert.throws(
    () => toMspdi({ data: [{ id: 1, text: "A", duration: 1 }], links: [{ id: 1, source: 1, target: 2, type: "0" }] }),
    /unknown task 2/,
  );
});

test("the dataset's start and a leaf's start are kept; a summary's start and a bad date are errors", () => {
  const xml = toMspdi({ start: "2026-03-01", data: [{ id: 1, text: "A", duration: 1, start: "2027-02-28" }, { id: 2, text: "B", duration: 1 }], links: [] });
  assert.match(xml, /<StartDate>2026-03-01T08:00:00<\/StartDate>/);
  assert.match(xml, /<Name>A<\/Name>.*<ConstraintType>4<\/ConstraintType><ConstraintDate>2027-02-28T08:00:00<\/ConstraintDate>/);
  assert.doesNotMatch(xml, /<Name>B<\/Name>[^\n]*Constraint/);
  assert.match(toMspdi({ data: [{ id: 1, text: "A", duration: 1 }], links: [] }), /<StartDate>2026-01-05T08:00:00<\/StartDate>/);
  const summary = { data: [{ id: 1, text: "P", duration: 0, start: "2026-03-01" }, { id: 2, text: "A", duration: 1, parent: 1 }], links: [] };
  assert.throws(() => toMspdi(summary), /summary task 1 has a start/);
  assert.throws(() => toMspdi({ data: [{ id: 1, text: "A", duration: 1, start: "1 March" }], links: [] }), /not a YYYY-MM-DD date/);
});

test("an impossible date is refused, at the dataset's start or a leaf's, and a leap day is kept", () => {
  const leaf = (start: string): Dataset => ({ data: [{ id: 1, text: "A", duration: 1, start }], links: [] });
  for (const bad of ["2026-02-31", "2026-02-29"]) {
    assert.throws(() => toMspdi({ start: bad, data: [{ id: 1, text: "A", duration: 1 }], links: [] }), /the dataset has start/);
    assert.throws(() => toMspdi(leaf(bad)), /task 1 has start/);
    assert.throws(() => toMspdi(leaf(`${bad}T09:00`)), /not a YYYY-MM-DD date/);
  }
  assert.match(toMspdi({ start: "2028-02-29", data: [{ id: 1, text: "A", duration: 1 }], links: [] }), /<StartDate>2028-02-29T08:00:00<\/StartDate>/);
  assert.match(toMspdi(leaf("2028-02-29")), /<ConstraintDate>2028-02-29T08:00:00<\/ConstraintDate>/);
});

test("a name with markup is escaped", () => {
  const xml = toMspdi({ data: [{ id: 1, text: "A & <B>", duration: 1 }], links: [] });
  assert.match(xml, /<Name>A &amp; &lt;B&gt;<\/Name>/);
});

test("the same dataset gives the same bytes", () => {
  assert.equal(toMspdi(flatChain()), toMspdi(flatChain()));
});

test("the CLI writes the file and a .sha256 holding its hash", () => {
  const dir = mkdtempSync(join(tmpdir(), "to-mspdi-test-"));
  const input = join(dir, "in.json");
  const out = join(dir, "out.xml");
  writeFileSync(input, JSON.stringify(flatChain()));
  const r = spawnSync(process.execPath, [join(here, "to-mspdi.ts"), input, out], { encoding: "utf8" });
  assert.equal(r.status, 0, r.stderr);
  const hash = createHash("sha256").update(readFileSync(out)).digest("hex");
  assert.equal(readFileSync(`${out}.sha256`, "utf8").trim(), `${hash}  out.xml`);
  assert.match(r.stdout, /1000 tasks, 998 links/);
});

test("the CLI exits 64 with no arguments", () => {
  const r = spawnSync(process.execPath, [join(here, "to-mspdi.ts")], { encoding: "utf8" });
  assert.equal(r.status, 64);
});
