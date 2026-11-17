// Checks that CI runs every browserless test: each scripts/test-*, scripts/lib/*.test.ts, demos/**/test-*.mjs and demos/**/*.test.ts
// is named on a line of .github/workflows/ci.yml, other than a comment or an `echo` line, or is in `exempt` below with the reason it is not.
// Run from anywhere: node scripts/test-ci-wired.ts
// Needs Node 22.18 or later (type stripping). Exits 1 and prints one line per problem.
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");

// Tests that no named line of ci.yml runs, and why. A comment in ci.yml that names one does not run it.
const exempt: Record<string, string> = {
  "demos/screencast/test-clips.mjs": "needs ffmpeg",
  "demos/50k/test-measure-cleanup.mjs": "needs the build, Playwright Chromium and bwrap",
  "scripts/test-smoke-set.ts": "needs Playwright and the build",
  "scripts/test-api-docs-check.ts": "runs through `pnpm docs:check` in ci.yml",
};

const walk = (dir: string): string[] =>
  readdirSync(join(root, dir), { recursive: true, withFileTypes: true })
    .filter((e) => e.isFile() && !e.parentPath.includes("node_modules"))
    .map((e) => join(e.parentPath, e.name).slice(root.length + 1));

const tests = [
  ...walk("scripts").filter((p) => /^scripts\/(test-[^/]+|lib\/[^/]+\.test\.ts)$/.test(p)),
  ...walk("demos").filter((p) => /(^|\/)(test-[^/]+\.mjs|[^/]+\.test\.ts)$/.test(p)),
].sort();

const ci = readFileSync(join(root, ".github/workflows/ci.yml"), "utf8")
  .split("\n")
  .filter((line) => !/^(#|echo )/.test(line.trimStart()))
  .join("\n");
const namedIn = (text: string, path: string): boolean => new RegExp(`${path.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![\\w.-])`).test(text);
const named = (path: string): boolean => namedIn(ci, path);

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-ci-wired: FAIL ${line}`);
};

if (tests.length === 0) fail("found no test files: the glob is wrong");
for (const path of tests) {
  if (!named(path) && !(path in exempt)) fail(`${path} is not run by ci.yml and is not in the exempt list`);
}
for (const [path, reason] of Object.entries(exempt)) {
  if (!existsSync(join(root, path))) fail(`${path} is in the exempt list but does not exist`);
  else if (named(path)) fail(`${path} is exempt (${reason}) but ci.yml runs it: remove it from the exempt list`);
}

// The two steps that run the demos' browserless tests (a pull request that touches no browser spec, and `web (checks)`) must name the same tests.
const demoSteps = ci.split(/\n(?=\s*- [a-z]+: )/).filter((step) => namedIn(step, "demos/screencast/test-caption.mjs"));
const demoSets = demoSteps.map((step) => tests.filter((p) => p.startsWith("demos/") && namedIn(step, p)));
if (demoSteps.length !== 2) fail(`${demoSteps.length} ci.yml steps run demos/screencast/test-caption.mjs, expected the 2 demos test steps`);
else for (const [a, b] of [demoSets, [...demoSets].reverse()]) for (const p of a) if (!b.includes(p)) fail(`${p} is named in only one of the two demos test steps of ci.yml`);

if (failed) process.exit(1);
console.log("test-ci-wired: ok");
