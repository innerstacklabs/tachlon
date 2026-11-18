// Prints whether a run needs the code jobs, as lines ready to append to $GITHUB_OUTPUT. Run at the repo root, with origin/main fetched:
//   node scripts/docs-only.ts "$TIER" >> "$GITHUB_OUTPUT"
// The rule is in scripts/lib/docs-only.ts. Needs Node 22.18 or later (type stripping). It prints:
//   code=true|false     whether rust, wasm, smoke, web, hosted and hosted-browser run
//   browser=true|false  on code=false, whether wasm and hosted-browser run anyway (book/src/SUMMARY.md changed)
//   why=<one line>
// A diff, a file or a search it cannot read prints code=true, never less. Exits 2 and prints nothing to stdout when the tier is missing.
import { spawnSync } from "node:child_process";
import { lstatSync, readFileSync } from "node:fs";
import { docsOnly, type DocsPlan } from "./lib/docs-only.ts";

const tier = process.argv[2];
if (!tier) {
  console.error("docs-only: usage: node scripts/docs-only.ts <fast|full|nightly|deploy>");
  process.exit(2);
}
const git = (...args: string[]) => spawnSync("git", args, { encoding: "utf8", maxBuffer: 1 << 26 });
let plan: DocsPlan;
try {
  if (tier !== "fast") plan = { code: true, browser: false, why: `the ${tier} tier runs every job` };
  else {
    const diff = git("diff", "--name-only", "--no-renames", "-z", "origin/main...HEAD");
    if (diff.status !== 0) throw new Error(`could not diff against main: ${diff.stderr.trim()}`);
    // The four chapter tests are not a reader: the `book` job runs them on a run that skips the code jobs.
    const named = (path: string): boolean => {
      const found = git("grep", "-l", "-F", "-e", path, "--", "crates", "packages", "apps", "demos");
      if (found.status !== 0 && found.status !== 1) throw new Error(`git grep exited ${found.status}: ${found.stderr.trim()}`);
      return found.stdout.split("\n").some((line) => line !== "" && !/(^|\/)book-[^/]*\.test\.ts$/.test(line));
    };
    const read = (path: string): string | null => {
      const stat = lstatSync(path, { throwIfNoEntry: false });
      if (stat === undefined) return null;
      if (!stat.isFile()) throw new Error(`${path} is not a regular file`);
      return readFileSync(path, "utf8");
    };
    plan = docsOnly(diff.stdout.split("\0").filter(Boolean), named, read);
  }
} catch (error) {
  plan = { code: true, browser: false, why: `could not judge: ${error}` };
}
const why = plan.why.replace(/\s+/g, " ");
console.error(`select: code=${plan.code} browser=${plan.browser} (${why})`);
console.log(`code=${plan.code}\nbrowser=${plan.browser}\nwhy=${why}`);
