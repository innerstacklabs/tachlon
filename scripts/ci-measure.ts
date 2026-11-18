// Measures one UTC day of CI use from the GitHub Actions API (see scripts/lib/ci-measure.ts). It only
// reads. Run from the repo root, with Node 22.18 or later (type stripping):
//   node scripts/ci-measure.ts --day 2026-10-30 [--repo owner/name] [--rows-out rows.jsonl]
//   node scripts/ci-measure.ts --fixture scripts/fixtures/ci-jobs-sample.json
// The live read needs `gh` signed in with read access to the repo. It makes one call for the day's runs
// and one per run for its jobs (`filter=all` keeps rerun attempts). Exit 0 for a complete day, 1 for bad
// arguments or an unreadable fixture, 2 for a capped, failed or unfinished read (the report still prints).
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { collect, formatReport } from "./lib/ci-measure.ts";
import type { JobsPage, RunsPage, Source } from "./lib/ci-measure.ts";

const USAGE = "usage: node scripts/ci-measure.ts (--day YYYY-MM-DD [--repo owner/name] | --fixture file) [--rows-out file]";

function arg(name: string): string | undefined {
  const at = process.argv.indexOf(name);
  return at < 0 ? undefined : process.argv[at + 1];
}

function fail(message: string): never {
  console.error(`ci-measure: ${message}`);
  process.exit(1);
}

/** One `gh api --paginate --slurp` call: the array of pages, or a thrown error naming why not. */
function ghPages<T>(path: string): T[] {
  const r = spawnSync("gh", ["api", "--paginate", "--slurp", path], { encoding: "utf8", maxBuffer: 256 * 1024 * 1024 });
  if (r.error) throw r.error;
  if (r.status !== 0) throw new Error(`gh api ${path} exited ${r.status}: ${r.stderr.trim().split("\n")[0]}`);
  const pages = JSON.parse(r.stdout) as T[];
  if (!Array.isArray(pages)) throw new Error(`gh api ${path} did not return pages`);
  return pages;
}

function liveSource(repo: string): Source {
  return {
    runs: (day) => ghPages<RunsPage>(`repos/${repo}/actions/runs?created=${day}&per_page=100`),
    jobs: (id) => ghPages<JobsPage>(`repos/${repo}/actions/runs/${id}/jobs?filter=all&per_page=100`),
  };
}

function fixtureSource(file: string): { day: string; source: Source } {
  const f = JSON.parse(readFileSync(file, "utf8")) as { day: string; runs: RunsPage[]; jobs: Record<string, JobsPage[]> };
  if (typeof f.day !== "string" || !Array.isArray(f.runs) || typeof f.jobs !== "object") fail(`${file} is not a fixture (day, runs, jobs)`);
  return {
    day: f.day,
    source: {
      runs: () => f.runs,
      jobs: (id) => {
        const pages = f.jobs[String(id)];
        if (!pages) throw new Error(`the fixture has no jobs for run ${id}`);
        return pages;
      },
    },
  };
}

const fixture = arg("--fixture");
let day = arg("--day");
let source: Source;
if (fixture !== undefined) {
  const f = fixtureSource(fixture);
  day = f.day;
  source = f.source;
} else {
  if (day === undefined || !/^\d{4}-\d{2}-\d{2}$/.test(day)) fail(USAGE);
  const repo = arg("--repo") || process.env.GITHUB_REPOSITORY;
  if (!repo) fail("no repository: pass --repo owner/name or set GITHUB_REPOSITORY");
  if (!/^[\w.-]+\/[\w.-]+$/.test(repo)) fail(`bad --repo ${repo}`);
  source = liveSource(repo);
}

const result = collect(day as string, source);
let loadavg = "unreadable";
try {
  loadavg = readFileSync("/proc/loadavg", "utf8");
} catch {
  // not Linux: the report says so rather than printing a made-up load
}
console.log(formatReport(day as string, result, loadavg));

const rowsOut = arg("--rows-out");
if (rowsOut !== undefined) writeFileSync(rowsOut, result.rows.map((r) => JSON.stringify(r)).join("\n") + (result.rows.length > 0 ? "\n" : ""));
if (result.truncated.length > 0) {
  console.error(`ci-measure: truncated: ${result.truncated.length} problem(s); not a complete day`);
  process.exit(2);
}
