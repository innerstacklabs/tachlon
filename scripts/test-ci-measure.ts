// Tests for scripts/lib/ci-measure.ts and the CLI. Node 22.18 or later, no dependencies, no network:
//   node --test scripts/test-ci-measure.ts
// The expected numbers for the fixture were worked out by hand from scripts/fixtures/ci-jobs-sample.json.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";
import { classOf, collect, dropCarried, percentile, peakOverlap, runnerKind, toRow } from "./lib/ci-measure.ts";
import type { ApiJob, JobsPage, Row, RunsPage, Source } from "./lib/ci-measure.ts";

const here = dirname(fileURLToPath(import.meta.url));
const FIXTURE = join(here, "fixtures/ci-jobs-sample.json");
const CLI = join(here, "ci-measure.ts");

interface Fixture {
  day: string;
  runs: RunsPage[];
  jobs: Record<string, JobsPage[]>;
}

const load = (): Fixture => JSON.parse(readFileSync(FIXTURE, "utf8")) as Fixture;

function sourceOf(f: Fixture): Source {
  return {
    runs: () => f.runs,
    jobs: (id) => {
      const pages = f.jobs[String(id)];
      if (!pages) throw new Error(`no jobs for run ${id}`);
      return pages;
    },
  };
}

function cli(args: string[]): { status: number | null; out: string; err: string } {
  const r = spawnSync("node", [CLI, ...args], { encoding: "utf8", env: { ...process.env, GITHUB_REPOSITORY: "" } });
  return { status: r.status, out: r.stdout, err: r.stderr };
}

const job = (over: Partial<ApiJob>): ApiJob => ({
  id: 1,
  run_id: 9,
  name: "rust",
  status: "completed",
  conclusion: "success",
  created_at: "2026-10-30T10:00:00Z",
  started_at: "2026-10-30T10:00:30Z",
  completed_at: "2026-10-30T10:01:31Z",
  runner_name: "hub-gantt1",
  labels: ["self-hosted", "tachlon-ci"],
  ...over,
});

test("the fixture command prints the hand-worked job-minutes, cancelled share and wait p90", () => {
  const r = cli(["--fixture", FIXTURE]);
  assert.equal(r.status, 0, r.err);
  assert.match(r.out, /^jobs: 11$/m);
  assert.match(r.out, /1 skipped and 1 re-run copies dropped/);
  assert.match(r.out, /^job-minutes: 25\.8$/m); // 1546 s
  assert.match(r.out, /^billable-minutes: 29$/m);
  assert.match(r.out, /^cancelled-minutes: 3\.0 \(11\.6% of job-minutes\)$/m); // 180 of 1546 s
  assert.match(r.out, /^abandoned-wait: 1 jobs never ran, 11\.0 min/m);
  assert.match(r.out, /^wait seconds, jobs that ran: n=10 p50=10 p90=600 max=1200 /m);
  assert.match(r.out, /^select job wait seconds: n=2 p50=1 p90=2 max=2$/m);
  assert.match(r.out, /^peak concurrency: 4 at 2026-10-30T10:05:01Z \(2026-10-30 03:05:01 MST\)/m);
  assert.match(r.out, /^peak queued: 5 at 2026-10-30T10:00:09Z /m);
  assert.match(r.out, /^minutes by class: deploy=1\.5 hosted=2\.0 rust=13\.0 select=0\.2 summary=0\.0 wasm=1\.0 web=8\.0$/m);
  assert.match(r.out, /^ {2}rust: n=2 p50=30 p90=600 max=600$/m);
  assert.match(r.out, /^ {2}dell-gantt3: n=4 p50=2 p90=1200 max=1200$/m);
  assert.match(r.out, /^ {2}\(GitHub-hosted\): n=1 p50=5 p90=5 max=5$/m); // not "GitHub Actions 7"
  assert.match(r.out, /^loadavg: /m);
});

test("nearest rank: the smallest value with at least p percent at or below it", () => {
  const ten = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
  assert.equal(percentile(ten, 50), 5);
  assert.equal(percentile(ten, 90), 9);
  assert.equal(percentile([1, 2], 90), 2);
  assert.equal(percentile([7], 50), 7);
  assert.equal(percentile([], 90), null);
});

test("an end frees its slot before a start in the same second takes one", () => {
  const a: [string, string] = ["2026-10-30T10:00:00Z", "2026-10-30T10:00:05Z"];
  const b: [string, string] = ["2026-10-30T10:00:05Z", "2026-10-30T10:00:09Z"];
  assert.equal(peakOverlap([a, b]).peak, 1);
  assert.equal(peakOverlap([a, ["2026-10-30T10:00:04Z", "2026-10-30T10:00:09Z"]]).peak, 2);
  assert.deepEqual(peakOverlap([]), { peak: 0, at: null });
});

test("a job that never ran goes in the abandoned-wait column, in either shape the API gives", () => {
  // The shape the API returns today: started_at is set to created_at, there is no runner.
  const sameInstant = toRow(job({ conclusion: "cancelled", started_at: "2026-10-30T10:00:00Z", completed_at: "2026-10-30T10:10:00Z", runner_name: null }), "push");
  // The shape the plan describes: no started_at at all.
  const noStart = toRow(job({ conclusion: "cancelled", started_at: null, completed_at: "2026-10-30T10:10:00Z", runner_name: null }), "push");
  for (const row of [sameInstant, noStart]) {
    assert.ok(row);
    assert.equal(row.abandoned_wait_s, 600);
    assert.equal(row.wait_s, null);
    assert.equal(row.run_s, 0);
    assert.equal(row.billable_min, 0);
    assert.equal(row.runner_name, null);
  }
});

test("a job that ran has a wait, a run time and ceil(run / 60) billable minutes; skipped is dropped", () => {
  const row = toRow(job({}), "pull_request");
  assert.ok(row);
  assert.deepEqual([row.day, row.event, row.class, row.runner_kind, row.wait_s, row.run_s, row.billable_min, row.abandoned_wait_s], ["2026-10-30", "pull_request", "rust", "fleet", 30, 61, 2, null]);
  assert.equal(toRow(job({ conclusion: "skipped" }), "push"), null);
});

test("class and runner kind come from the job name and labels", () => {
  assert.deepEqual(["web (3)", "ci-summary", "select", "deploy", "lint"].map(classOf), ["web", "summary", "select", "deploy", "other"]);
  assert.equal(runnerKind(["self-hosted", "tachlon-ci"]), "fleet");
  assert.equal(runnerKind(["self-hosted", "ci-select"]), "fleet");
  assert.equal(runnerKind(["ubuntu-latest"]), "hosted");
  assert.equal(runnerKind(["self-hosted", "vendor-x64"]), "vendor");
  assert.equal(runnerKind(["vendor-x64"]), "vendor");
});

test("a complete read has no truncation reason; a re-run copy is dropped and a re-executed job is kept", () => {
  const f = load();
  const c = collect(f.day, sourceOf(f));
  assert.deepEqual(c.truncated, []);
  assert.equal(c.rows.length, 11);
  assert.equal(c.requests, 4);
  assert.equal(c.carried, 1); // job 5007 is a copy of job 5003 listed by a re-run attempt
});

test("a failed, capped or short read is truncated, never complete", () => {
  const failed = load();
  delete failed.jobs["1002"];
  assert.match(collect(failed.day, sourceOf(failed)).truncated.join("\n"), /run 1002: jobs read failed: no jobs for run 1002/);

  const short = load();
  short.jobs["1001"][0].total_count = 8;
  assert.match(collect(short.day, sourceOf(short)).truncated.join("\n"), /run 1001: jobs read 7 of 8/);

  const capped = load();
  capped.runs[0].total_count = 1001;
  assert.match(collect(capped.day, sourceOf(capped)).truncated.join("\n"), /capped: 1001 runs match/);

  const open = load();
  open.jobs["1003"][0].jobs[1].status = "in_progress";
  const c = collect(open.day, sourceOf(open));
  assert.equal(c.open, 1);
  assert.match(c.truncated.join("\n"), /1 jobs not finished/);

  const broken: Source = { runs: () => { throw new Error("HTTP 502"); }, jobs: () => [] };
  assert.match(collect("2026-10-30", broken).truncated.join("\n"), /runs read failed: HTTP 502/);
});

test("the CLI exits 2 and still prints the report for a truncated day, 1 for bad arguments", () => {
  const f = load();
  delete f.jobs["1002"];
  const file = join(mkdtempSync(join(tmpdir(), "ci-measure-")), "truncated.json");
  writeFileSync(file, JSON.stringify(f));
  const cut = cli(["--fixture", file]);
  assert.equal(cut.status, 2);
  assert.match(cut.out, /^TRUNCATED: this is not a complete day$/m);
  assert.match(cut.err, /truncated/);
  assert.equal(cli([]).status, 1);
  assert.equal(cli(["--day", "yesterday"]).status, 1);
  assert.match(cli(["--day", "2026-10-30"]).err, /no repository/);
});

test("of rows that share run, name, runner, start and end only the lowest job id stays; a real re-execution and a job that never ran stay", () => {
  const row = (job_id: number, over: Partial<ApiJob> = {}) => toRow(job({ id: job_id, ...over }), "push") as Row;
  const first = row(10);
  const copy = row(11, { created_at: "2026-10-30T12:00:00Z" }); // a re-run attempt, same times and runner
  const again = row(12, { created_at: "2026-10-30T12:00:00Z", started_at: "2026-10-30T12:00:05Z", completed_at: "2026-10-30T12:03:00Z" });
  const never = [13, 14].map((id) => row(id, { conclusion: "cancelled", started_at: null, runner_name: null }));
  const out = dropCarried([copy, ...never, again, first]);
  assert.equal(out.carried, 1);
  assert.deepEqual(out.rows.map((r) => r.job_id), [10, 12, 13, 14]);
});
