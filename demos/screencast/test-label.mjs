// Checks the recording's source label with no browser: node demos/screencast/test-label.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { BRANCH_PATTERN, DETACHED_BRANCH, sourceLabel } from "./label.mjs";

// A recording on main keeps today's text, with or without the branch field.
assert.equal(sourceLabel({ branch: "main", commit: "f586337" }), "from `main` at `f586337`");
assert.equal(sourceLabel({ commit: "f586337-dirty" }), "from `main` at `f586337-dirty`");

// A recording on a branch names the branch and its base commit, and never says main.
const label = sourceLabel({ branch: "feat/110-theme-tokens", commit: "637f5fa" });
assert.equal(label, "from branch `feat/110-theme-tokens` based on `637f5fa`");
assert.ok(!label.includes("`main`"));
assert.equal(
  sourceLabel({ branch: "fix/125-screencast-label", commit: "637f5fa-dirty" }),
  "from branch `fix/125-screencast-label` based on `637f5fa-dirty`",
);
// A branch that merely contains "main" is a branch.
assert.equal(sourceLabel({ branch: "maintenance", commit: "abc1234" }), "from branch `maintenance` based on `abc1234`");

// A name that would break the README's backticks is refused, not written.
for (const bad of ["a`b", "a b", "", "a;b", "$(x)"]) {
  assert.throws(() => sourceLabel({ branch: bad, commit: "abc1234" }), /not allowed/);
}
// A detached HEAD is labelled as such, with the base commit.
assert.equal(sourceLabel({ branch: DETACHED_BRANCH, commit: "abc1234" }), "from detached HEAD based on `abc1234`");
assert.equal(sourceLabel({ branch: DETACHED_BRANCH, commit: "abc1234-dirty" }), "from detached HEAD based on `abc1234-dirty`");
// ... and no branch name can be taken for it.
assert.ok(!BRANCH_PATTERN.test(DETACHED_BRANCH));

// run.sh with fake tools: a fake git and fake build tools on the PATH, so no browser, no build, no ssh and no real git
// run. Every fake appends one line (its name, then its arguments, tab-separated) to a log; ssh and rsync fail, so a run
// that reaches the Mac path shows up in the log.
const work = mkdtempSync(join(tmpdir(), "test-label-"));
try {
  const fakes = join(work, "fakes");
  mkdirSync(fakes);
  const log = join(work, "log");
  const record = '#!/bin/sh\nprintf "%s" "${0##*/}" >> "$FAKE_LOG"; for a in "$@"; do printf "\\t%s" "$a" >> "$FAKE_LOG"; done; printf "\\n" >> "$FAKE_LOG"\n';
  const fake = (name, body) => {
    writeFileSync(join(fakes, name), record + body);
    chmodSync(join(fakes, name), 0o755);
  };
  for (const name of ["cargo", "wasm-bindgen", "bash", "pnpm", "node"]) fake(name, "exit 0\n");
  for (const name of ["ssh", "rsync"]) fake(name, "exit 97\n");
  fake("git", [
    'case "$1" in',
    "  fetch|diff) exit 0 ;;",
    '  symbolic-ref) if [ -n "$FAKE_BRANCH" ]; then echo "$FAKE_BRANCH"; exit 0; fi; exit 1 ;;',
    "  merge-base) echo 0123456789abcdef0123456789abcdef01234567 ;;",
    "  rev-parse) echo abc1234 ;;",
    "  *) exit 99 ;;",
    "esac",
    "",
  ].join("\n"));

  const realBash = (process.env.PATH ?? "").split(":").map((dir) => join(dir, "bash")).find((path) => existsSync(path));
  assert.ok(realBash, "bash is not on the PATH");
  const runSh = fileURLToPath(new URL("./run.sh", import.meta.url));
  const run = (args, branch) => {
    writeFileSync(log, "");
    const result = spawnSync(realBash, [runSh, ...args], {
      encoding: "utf8",
      env: { PATH: `${fakes}:/usr/bin:/bin`, HOME: work, FAKE_LOG: log, FAKE_BRANCH: branch ?? "" },
    });
    const lines = readFileSync(log, "utf8").split("\n").filter(Boolean).map((line) => line.split("\t"));
    return { ...result, lines, tools: lines.map((line) => line[0]) };
  };
  const records = (lines) => lines.filter((line) => line[0] === "node" && line[1] === "demos/screencast/record.mjs");

  // --local on a detached HEAD runs both recordings, labelled detached with the base commit.
  const detachedLocal = run(["--local"]);
  assert.equal(detachedLocal.status, 0, detachedLocal.stderr);
  const detachedRuns = records(detachedLocal.lines);
  assert.equal(detachedRuns.length, 2, "the story and the feature scenes are both recorded");
  for (const line of detachedRuns) {
    assert.equal(line[line.indexOf("--branch") + 1], DETACHED_BRANCH);
    assert.equal(line[line.indexOf("--commit") + 1], "abc1234");
    assert.equal(line[line.indexOf("--out") + 1], "demos/screencast/draft");
  }
  assert.match(detachedLocal.stdout, /draft only: demos\/screencast\/draft\//);
  assert.ok(!detachedLocal.tools.includes("ssh") && !detachedLocal.tools.includes("rsync"), "--local never touches the Mac");

  // --local on a branch is unchanged.
  const branchLocal = run(["--local"], "feat/653-x");
  assert.equal(branchLocal.status, 0, branchLocal.stderr);
  assert.equal(records(branchLocal.lines).length, 2);
  for (const line of records(branchLocal.lines)) assert.equal(line[line.indexOf("--branch") + 1], "feat/653-x");

  // Every other run still refuses a detached HEAD, before any build and before the Mac.
  for (const args of [[], ["--upload"]]) {
    const refused = run(args);
    assert.equal(refused.status, 1, `run.sh ${args.join(" ")} on a detached HEAD`);
    assert.match(refused.stderr, /HEAD is detached/);
    assert.deepEqual([...new Set(refused.tools)], ["git"], `run.sh ${args.join(" ")} ran only git before it refused`);
  }

  // --local still refuses a branch name the README label cannot carry.
  const badName = run(["--local"], "a b");
  assert.equal(badName.status, 1);
  assert.match(badName.stderr, /characters the README label cannot carry/);
  assert.equal(records(badName.lines).length, 0);
} finally {
  rmSync(work, { recursive: true, force: true });
}

console.log("test-label: ok");
