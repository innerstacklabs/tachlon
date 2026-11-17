// Checks the CI tier rule (see scripts/lib/ci-tier.ts). Run from anywhere:
//   node scripts/test-ci-tier.ts
// Needs Node 22.18 or later (type stripping). It does two things:
//   1. Runs the event-to-tier table of the CI plan (gs#1123) through `tierFor`, plus the refs and
//      events the plan does not name, which must run everything (full), never fewer jobs.
//   2. Runs the command `select` runs and reads its `tier=` line, and checks a missing argument.
// Exits 1 and prints one line per problem.
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { tierFor, type Tier } from "./lib/ci-tier.ts";

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-ci-tier: FAIL ${line}`);
};

// ---- 1. the table -----------------------------------------------------------------

const rows: Array<{ name: string; event: string; ref: string; tier: Tier }> = [
  // The plan's table.
  { name: "push to a feature branch", event: "push", ref: "refs/heads/feat/x", tier: "fast" },
  { name: "push to a ci branch", event: "push", ref: "refs/heads/ci/gs1123s3-tier-smoke", tier: "fast" },
  { name: "push to batch/x", event: "push", ref: "refs/heads/batch/repo-20261101-1", tier: "full" },
  { name: "manual run on a branch", event: "workflow_dispatch", ref: "refs/heads/feat/x", tier: "full" },
  { name: "manual run on main", event: "workflow_dispatch", ref: "refs/heads/main", tier: "full" },
  { name: "the nightly cron (the default branch is its ref)", event: "schedule", ref: "refs/heads/main", tier: "nightly" },
  { name: "push to main", event: "push", ref: "refs/heads/main", tier: "deploy" },
  { name: "push to release", event: "push", ref: "refs/heads/release", tier: "deploy" },
  // Names that look like a rule's branch and are not.
  { name: "a branch whose name ends in main", event: "push", ref: "refs/heads/feat/main", tier: "fast" },
  { name: "a branch named release-notes", event: "push", ref: "refs/heads/release-notes", tier: "fast" },
  { name: "a branch named batch with no slash", event: "push", ref: "refs/heads/batch", tier: "fast" },
  { name: "a branch with batch deeper in the name", event: "push", ref: "refs/heads/feat/batch/x", tier: "fast" },
  // What the plan does not name runs every job.
  { name: "a tag push", event: "push", ref: "refs/tags/v1", tier: "full" },
  { name: "a pull_request event", event: "pull_request", ref: "refs/pull/1/merge", tier: "full" },
  { name: "an event nobody planned for", event: "repository_dispatch", ref: "refs/heads/main", tier: "full" },
];
for (const row of rows) {
  const got = tierFor(row.event, row.ref);
  if (got !== row.tier) fail(`"${row.name}": ${row.event} ${row.ref} gave ${got}, wanted ${row.tier}`);
}

// ---- 2. the command select runs -----------------------------------------------------

const cli = join(dirname(fileURLToPath(import.meta.url)), "ci-tier.ts");
const run = (args: string[]) => spawnSync(process.execPath, [cli, ...args], { encoding: "utf8" });

const batch = run(["push", "refs/heads/batch/x"]);
if (batch.status !== 0 || batch.stdout !== "tier=full\n") {
  fail(`the command for a batch push: status ${batch.status}, stdout ${JSON.stringify(batch.stdout)}, wanted tier=full`);
}
const feature = run(["push", "refs/heads/feat/x"]);
if (feature.status !== 0 || feature.stdout !== "tier=fast\n") {
  fail(`the command for a feature push: status ${feature.status}, stdout ${JSON.stringify(feature.stdout)}, wanted tier=fast`);
}
for (const args of [[], ["push"]]) {
  const bad = run(args);
  if (bad.status !== 2 || bad.stdout !== "") {
    fail(`the command with ${args.length} argument(s): status ${bad.status}, stdout ${JSON.stringify(bad.stdout)}, wanted exit 2 and no stdout`);
  }
}

if (failed) process.exit(1);
console.log(`test-ci-tier: ok (${rows.length} table rows)`);
