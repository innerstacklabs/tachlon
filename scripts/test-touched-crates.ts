// Checks which workspace crates' tests a run owes (see scripts/lib/touched-crates.ts). Run from the repo root:
//   node scripts/test-touched-crates.ts
// Needs Node 22.18 or later (type stripping), and `cargo` for the last two checks. A table of tiers and changed files against the
// packages the `rust` job's test step gets, a check of the `cargo metadata` reader, and a scan that the table knows every crate
// test that reads outside its own crate folder. Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { readdirSync, readFileSync, statSync } from "node:fs";
import { cratesPlan, workspaceCrates, type Crate } from "./lib/touched-crates.ts";
import type { Tier } from "./lib/ci-tier.ts";

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-touched-crates: FAIL ${line}`);
};

// The workspace as `cargo metadata` shows it: app depends on the other four (xer in dev-dependencies), import and timeline on engine, import on xer.
const CRATES: Crate[] = [
  { name: "engine", dir: "crates/engine", deps: [] },
  { name: "xer", dir: "crates/xer", deps: [] },
  { name: "timeline", dir: "crates/timeline", deps: ["engine"] },
  { name: "import", dir: "crates/import", deps: ["engine", "xer"] },
  { name: "app", dir: "crates/app", deps: ["engine", "import", "timeline", "xer"] },
];
const ALL = "all";

const rows: Array<{ name: string; tier: Tier; changed: string[] | null; want: string[] | "all" }> = [
  { name: "a change in the top crate runs that crate only", tier: "fast", changed: ["crates/app/src/lib.rs", "crates/app/tests/insert.rs"], want: ["app"] },
  { name: "a middle crate runs itself and its dependents", tier: "fast", changed: ["crates/timeline/src/scene.rs"], want: ["timeline", "app"] },
  { name: "the core crate runs every crate that depends on it", tier: "fast", changed: ["crates/engine/src/cpm.rs"], want: ["engine", "timeline", "import", "app"] },
  { name: "a dev-dependency edge counts as a dependent", tier: "fast", changed: ["crates/xer/src/lib.rs"], want: ["xer", "import", "app"] },
  { name: "two crates run the union, each once", tier: "fast", changed: ["crates/import/src/a.rs", "crates/import/src/b.rs", "crates/timeline/src/c.rs"], want: ["timeline", "import", "app"] },
  { name: "a file outside every crate runs the crate whose tests read the repo", tier: "fast", changed: ["packages/gantt/src/find.ts", "docs/PLAN.md"], want: ["app"] },
  { name: "a crate change and an outside file run the crate and app", tier: "fast", changed: ["docs/PLAN.md", "crates/timeline/src/scene.rs"], want: ["timeline", "app"] },
  { name: "a fixture of the chart folder is also read by import", tier: "fast", changed: ["packages/gantt/tests/fixtures/one-project.xer"], want: ["import", "app"] },
  { name: "a fixture of app is also read by import", tier: "fast", changed: ["crates/app/tests/fixtures/baselined.xml"], want: ["import", "app"] },
  { name: "the token stylesheet is also read by timeline", tier: "fast", changed: ["packages/gantt/src/styles/tokens.css"], want: ["timeline", "app"] },
  { name: "another stylesheet is not", tier: "fast", changed: ["packages/gantt/src/styles/toolbar.css"], want: ["app"] },
  { name: "Cargo.lock runs everything", tier: "fast", changed: ["Cargo.lock"], want: ALL },
  { name: "the workspace manifest runs everything", tier: "fast", changed: ["Cargo.toml", "crates/app/src/lib.rs"], want: ALL },
  { name: "the toolchain file runs everything", tier: "fast", changed: ["rust-toolchain.toml"], want: ALL },
  { name: "the nextest settings run everything", tier: "fast", changed: [".config/nextest.toml"], want: ALL },
  { name: "the workflow runs everything", tier: "fast", changed: [".github/workflows/ci.yml"], want: ALL },
  { name: "a folder under crates/ that is no crate runs everything", tier: "fast", changed: ["crates/newcrate/src/lib.rs"], want: ALL },
  { name: "a diff that could not be read runs everything", tier: "fast", changed: null, want: ALL },
  { name: "an empty diff runs everything", tier: "fast", changed: [], want: ALL },
  { name: "full runs everything", tier: "full", changed: ["crates/app/src/lib.rs"], want: ALL },
  { name: "nightly runs everything", tier: "nightly", changed: ["crates/app/src/lib.rs"], want: ALL },
  { name: "deploy runs everything", tier: "deploy", changed: ["crates/app/src/lib.rs"], want: ALL },
];
for (const row of rows) {
  const got = cratesPlan(row.tier, row.changed, CRATES);
  const gotNames = got.rust === "all" ? ALL : got.packages;
  if (JSON.stringify(gotNames) !== JSON.stringify(row.want)) fail(`"${row.name}": got ${JSON.stringify(gotNames)} (${got.why}), wanted ${JSON.stringify(row.want)}`);
}
const noApp = cratesPlan("fast", ["docs/PLAN.md"], CRATES.filter((c) => c.name !== "app"));
if (noApp.rust !== "all") fail(`a workspace with no app crate and an outside file: got ${JSON.stringify(noApp.packages)}, wanted all`);
const oddName = cratesPlan("fast", ["crates/a/src/lib.rs"], [{ name: "a;b", dir: "crates/a", deps: [] }, ...CRATES]);
if (oddName.rust !== "all") fail(`a package name a shell would split is passed on: ${JSON.stringify(oddName.packages)}`);

// The reader of `cargo metadata` output: a dependency counts only when it has a path in the workspace, of any kind, and a crate never depends on itself.
const root = "/w";
const pkg = (name: string, deps: Array<{ name: string; path: string | null }>) => ({ name, manifest_path: `${root}/crates/${name}/Cargo.toml`, dependencies: deps.map((d) => ({ ...d, kind: null })) });
const read = workspaceCrates({ workspace_root: root, packages: [pkg("a", [{ name: "a", path: `${root}/crates/a` }]), pkg("b", [{ name: "a", path: `${root}/crates/a` }, { name: "serde", path: null }])] });
if (JSON.stringify(read) !== JSON.stringify([{ name: "a", dir: "crates/a", deps: [] }, { name: "b", dir: "crates/b", deps: ["a"] }])) fail(`workspaceCrates read ${JSON.stringify(read)}`);

// The real workspace: the graph is what the table above assumes, and no test of another crate reads a file outside its own folder, which
// would be an input the plan does not know (the three tests of import and theme.rs of timeline are the ones it does).
const meta = spawnSync("cargo", ["metadata", "--no-deps", "--format-version", "1", "--offline"], { encoding: "utf8", maxBuffer: 1 << 26 });
if (meta.status !== 0) fail(`cargo metadata exited ${meta.status}: ${meta.stderr.trim()}`);
else {
  const real = workspaceCrates(JSON.parse(meta.stdout));
  const shape = (cs: Crate[]): string => JSON.stringify(cs.map((c) => [c.name, c.dir, [...c.deps].sort()]).sort());
  if (shape(real) !== shape(CRATES)) fail(`the workspace is ${shape(real)}, the table above assumes ${shape(CRATES)}`);
}
const reaches = /\.\.\/\.\.\/|join\("\.\."\)|"\/\.\.\//;
const readers: string[] = [];
const walk = (dir: string): void => {
  for (const name of readdirSync(dir)) {
    const path = `${dir}/${name}`;
    if (statSync(path).isDirectory()) walk(path);
    else if (name.endsWith(".rs") && reaches.test(readFileSync(path, "utf8"))) readers.push(path);
  }
};
for (const c of CRATES.filter((c) => c.name !== "app")) walk(c.dir);
const want = ["crates/import/tests/parse_snapshot.rs", "crates/import/tests/xer_export.rs", "crates/import/tests/xer_export_tasks.rs", "crates/timeline/tests/theme.rs"];
if (JSON.stringify(readers.sort()) !== JSON.stringify(want)) fail(`files outside app that reach out of their crate folder are ${JSON.stringify(readers)}, the plan knows ${JSON.stringify(want)}: add the folder they read to EXTRA_READERS in scripts/lib/touched-crates.ts`);

// The command `select` runs: the full tier prints rust=all without reading the diff, and a missing tier is a usage error.
const run = (...args: string[]) => spawnSync("node", ["scripts/touched-crates.ts", ...args], { encoding: "utf8" });
const full = run("full");
if (full.status !== 0 || !full.stdout.split("\n").includes("rust=all") || !full.stdout.split("\n").includes("rust_packages=")) fail(`touched-crates full printed ${JSON.stringify(full.stdout)} (exit ${full.status})`);
const bad = run();
if (bad.status !== 2 || bad.stdout !== "") fail(`touched-crates with no tier exited ${bad.status} and printed ${JSON.stringify(bad.stdout)}`);

if (failed) process.exit(1);
console.log(`test-touched-crates: ok (${rows.length} plan rows)`);
