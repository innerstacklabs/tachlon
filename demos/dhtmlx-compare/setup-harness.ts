// Clone the DHTMLX benchmark harness into a scratch directory outside any repo, swap its trial
// import for the MIT Community package, and install it. Nothing of DHTMLX's is ever written into
// a work tree of ours: the scratch path is refused if it sits inside one.
//
//   node demos/dhtmlx-compare/setup-harness.ts [scratch-dir]
//
// Default scratch dir: $TMPDIR/dhtmlx-harness-<sha7>. `--help` prints the usage and does nothing
// else; any other argument that starts with `-` exits 64.

import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";

export const HARNESS_REPO = "https://github.com/DHTMLX/js-gantt-benchmarks";

// The latest commit of the harness's default branch, read with
// `git ls-remote <repo> HEAD` (sha) and `git log -1 --format=%cI` (commit date 2026-09-16)
// on the date below.
export const HARNESS_COMMIT = {
  sha: "d16c261bae3b46a8f7ca59cfdbb7c54acd7ae847",
  dateRead: "2026-10-30",
} as const;

const TRIAL = "@dhx/trial-gantt";
const COMMUNITY = "dhtmlx-gantt";

/** Files of the harness that name the trial package, relative to the clone root. */
const IMPORT_FILE = join("dhtmlx", "src", "components", "GanttChart.js");
const PACKAGE_JSON = join("dhtmlx", "package.json");

/** Rewrite every `@dhx/trial-gantt` specifier (bare or sub-path) to `dhtmlx-gantt`. */
export function rewriteImports(source: string): string {
  return source.split(TRIAL).join(COMMUNITY);
}

/** Replace the trial dependency key with the Community one, keeping the version. */
export function rewritePackageJson(text: string): string {
  const pkg = JSON.parse(text) as Record<string, Record<string, string> | undefined>;
  for (const section of ["dependencies", "devDependencies"]) {
    const deps = pkg[section];
    if (deps && TRIAL in deps) {
      const version = deps[TRIAL];
      delete deps[TRIAL];
      deps[COMMUNITY] = version;
    }
  }
  return JSON.stringify(pkg, null, 2) + "\n";
}

/** Throw when `target` (existing or not) lies inside a git work tree; `what` names the vendor code refused. */
export function assertOutsideWorkTree(target: string, what = "DHTMLX code"): void {
  let dir = resolve(target);
  while (!existsSync(dir)) dir = dirname(dir);
  const r = spawnSync("git", ["-C", dir, "rev-parse", "--is-inside-work-tree"], {
    encoding: "utf8",
  });
  if (r.status === 0 && r.stdout.trim() === "true") {
    throw new Error(
      `refusing to put ${what} at ${resolve(target)}: it is inside a git work tree`,
    );
  }
}

function run(cmd: string, args: string[], cwd: string): void {
  console.log(`$ ${cmd} ${args.join(" ")}  (in ${cwd})`);
  execFileSync(cmd, args, { cwd, stdio: "inherit" });
}

export function setupHarness(scratch: string): void {
  assertOutsideWorkTree(scratch);
  if (existsSync(scratch)) throw new Error(`${scratch} already exists; remove it first`);

  run("git", ["clone", "--quiet", HARNESS_REPO, scratch], tmpdir());
  run("git", ["checkout", "--quiet", HARNESS_COMMIT.sha], scratch);

  const importFile = join(scratch, IMPORT_FILE);
  writeFileSync(importFile, rewriteImports(readFileSync(importFile, "utf8")));
  const pkgFile = join(scratch, PACKAGE_JSON);
  writeFileSync(pkgFile, rewritePackageJson(readFileSync(pkgFile, "utf8")));

  // The harness's lockfile and .npmrc pin the trial package on DHTMLX's registry; drop them and
  // lock the Community package from npmjs so the harness's own `npm ci` has a matching lock.
  const dhtmlxDir = join(scratch, "dhtmlx");
  rmSync(join(dhtmlxDir, ".npmrc"), { force: true });
  rmSync(join(dhtmlxDir, "package-lock.json"), { force: true });
  run("npm", ["install", "--package-lock-only", "--ignore-scripts"], dhtmlxDir);

  run("npm", ["ci"], scratch);
  run("npm", ["run", "setup"], scratch);
  console.log(`\nHarness ready at ${scratch} (commit ${HARNESS_COMMIT.sha.slice(0, 7)}).`);
  console.log("It holds DHTMLX code: do not copy it into a repo.");
}

// Run as a CLI only when executed directly, not when the test imports it.
export const USAGE =
  "usage: node demos/dhtmlx-compare/setup-harness.ts [scratch-dir]\n" +
  "  clones the DHTMLX benchmark harness at its pinned commit into scratch-dir (default\n" +
  "  $TMPDIR/dhtmlx-harness-<sha7>), swaps its trial package for the MIT Community one, installs it;\n" +
  "  refuses a scratch-dir inside a git work tree or one that exists\n";

if (process.argv[1] && resolve(process.argv[1]) === resolve(new URL(import.meta.url).pathname)) {
  const arg = process.argv[2];
  if (arg === "--help") {
    process.stdout.write(USAGE);
    process.exit(0);
  }
  if (arg?.startsWith("-") || process.argv.length > 3) {
    process.stderr.write(USAGE);
    process.exit(64);
  }
  const scratch = resolve(
    process.argv[2] ?? join(tmpdir(), `dhtmlx-harness-${HARNESS_COMMIT.sha.slice(0, 7)}`),
  );
  try {
    setupHarness(scratch);
  } catch (e) {
    console.error(`setup-harness: ${(e as Error).message}`);
    process.exit(1);
  }
}
