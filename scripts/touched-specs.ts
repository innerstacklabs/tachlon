// Prints the browser specs a run owes, as lines ready to append to $GITHUB_OUTPUT. Run at the repo root,
// with origin/main fetched (on deploy, with PUSH_BEFORE set to the push's `before` commit):
//   node scripts/touched-specs.ts "$TIER" >> "$GITHUB_OUTPUT"
// The rule is in scripts/lib/touched-specs.ts. Needs Node 22.18 or later (type stripping). It prints:
//   web=none|checks|some|all   whether the `web` job runs, and on what
//   shards=<json array>     the legs of the `web` matrix: `["checks"]` when web is `checks`, else the four shards and `checks`
//   specs=<paths>           the spec paths it runs, relative to the chart folder (empty: no filter)
//   nopage_specs=<paths>    on fast, the spec files that hold no-page tests (the plan's class d)...
//   nopage_grep=<regex>     ...and a --grep that matches those tests, for the `smoke` job
//   hosted_grep=<regex>     the same specs as a --grep fragment, every regex metacharacter escaped, to join to `@smoke`
//   bench=true|false        whether the `hosted` job runs the benchmark driver test (false on every fast run, true on the others)
//   hosted_specs=<paths>    on fast, the hosted specs (relative to apps/hosted) to run besides @smoke, or `all` for the whole hosted suite
// Exits 2 and prints nothing to stdout when the tier is missing or unknown.
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync } from "node:fs";
import { BATCH_ONLY_SPECS, type Tier } from "./lib/ci-tier.ts";
import { WEB_ROOT } from "./lib/web-root.ts";
import { benchPlan, grepFor, hostedPlan, noPageTests, webPlan, webShards } from "./lib/touched-specs.ts";

const tier = process.argv[2] as Tier | undefined;
if (!tier || !["fast", "full", "nightly", "deploy"].includes(tier)) {
  console.error("touched-specs: usage: node scripts/touched-specs.ts <fast|full|nightly|deploy>");
  process.exit(2);
}
const specs = readdirSync(`${WEB_ROOT}/tests`).filter((f) => f.endsWith(".spec.ts")).sort();
// On deploy the diff against main is empty (HEAD is main), so the push's own commits are read: PUSH_BEFORE is the push event's `before`.
// A missing or all-zero value (a new branch) gives no range, and so no diff: the plan then runs every spec.
const before = process.env.PUSH_BEFORE ?? "";
const range = tier === "deploy" ? (/^0*$/.test(before) ? null : `${before}..HEAD`) : "origin/main...HEAD";
const diff = range === null ? null : spawnSync("git", ["diff", "--name-only", "--no-renames", range], { encoding: "utf8" });
const changed = diff?.status === 0 ? diff.stdout.split("\n").filter(Boolean) : null;
// The entry file's text before and after, read only when the diff holds it: `webPlan` runs one spec when its runtime code is the same.
const entryFile = `${WEB_ROOT}/src/index.ts`;
const gitText = (...args: string[]): string | null => {
  const run = spawnSync("git", args, { encoding: "utf8", maxBuffer: 1 << 24 });
  return run.status === 0 ? run.stdout : null;
};
const base = tier === "deploy" ? (range === null ? null : before) : (gitText("merge-base", "origin/main", "HEAD")?.trim() ?? null);
const readHead = (): string | null => {
  try {
    return readFileSync(entryFile, "utf8");
  } catch {
    return null;
  }
};
const entry = changed?.includes(entryFile) ? { base: base === null ? null : gitText("show", `${base}:${entryFile}`), head: readHead() } : null;
const plan = webPlan(tier, changed, specs, entry);
console.error(`touched-specs: web=${plan.web} (${plan.why})`);

const nopage = tier === "fast" ? specs.filter((s) => !BATCH_ONLY_SPECS.includes(s)).map((s) => ({ s, titles: noPageTests(readFileSync(`${WEB_ROOT}/tests/${s}`, "utf8")) })) : [];
const withTests = nopage.filter((n) => n.titles.length > 0);
console.error(`touched-specs: ${withTests.reduce((sum, n) => sum + n.titles.length, 0)} no-page test(s) in ${withTests.length} file(s)`);
const hostedSpecs = readdirSync("apps/hosted/tests").filter((f) => f.endsWith(".spec.ts")).sort();
const hosted = hostedPlan(tier, changed, hostedSpecs);
console.error(`touched-specs: hosted=${hosted.hosted} (${hosted.why})`);
const benchRun = benchPlan(tier);
console.error(`touched-specs: bench=${benchRun.bench} (${benchRun.why})`);
const paths = (names: string[]): string => names.map((n) => `tests/${n}`).join(" ");
console.log(`web=${plan.web}`);
console.log(`shards=${webShards(plan.web)}`);
console.log(`specs=${paths(plan.specs)}`);
console.log(`nopage_specs=${paths(withTests.map((n) => n.s))}`);
console.log(`nopage_grep=${grepFor(withTests.flatMap((n) => n.titles))}`);
console.log(`hosted_specs=${hosted.hosted === "all" ? "all" : paths(hosted.specs)}`);
console.log(`hosted_grep=${grepFor(hosted.specs)}`);
console.log(`bench=${benchRun.bench}`);
