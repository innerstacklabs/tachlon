import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";
import { expect } from "@playwright/test";

// The journey time report (`journey-time.mjs`, gs#495) flags what it cannot time (gs#597). None of these tests opens a
// browser: each writes a Playwright JSON result, runs the real script on it and reads what it prints. A skipped test does
// not count as run, a count of journey tests other than two is flagged, an unwritable job summary does not fail the step,
// and the script still runs when started through a symlink or from a path with a space in it.

const SCRIPT = fileURLToPath(new URL("../tests/journey-time.mjs", import.meta.url));

type Result = { status: string; duration: number };

/** A Playwright JSON result with one journey spec per `[title, test status, results]`. */
function resultJson(tests: Array<[string, string, Result[]]>): string {
  const specs = tests.map(([title, status, results]) => ({ title, file: "journey.spec.ts", tests: [{ status, results }] }));
  return JSON.stringify({ suites: [{ file: "journey.spec.ts", specs }] });
}

const passed = (ms: number): Result[] => [{ status: "passed", duration: ms }];
// A skipped test has a result of its own; a few milliseconds is enough to make a sum of them look like a time.
const skipped = (): Result[] => [{ status: "skipped", duration: 4 }];

/** A fresh temporary directory, removed after the test. */
function withScratch<T>(body: (dir: string) => T): T {
  const dir = mkdtempSync(join(tmpdir(), "journey-time-597-"));
  try {
    return body(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** Runs the script on `json`; `script` and `env` override the real script path and the environment. */
function report(dir: string, json: string, options: { script?: string; env?: Record<string, string> } = {}) {
  const file = join(dir, "results.json");
  writeFileSync(file, json);
  // The runner's own job summary must not leak into the run.
  const env = { ...process.env };
  delete env.GITHUB_STEP_SUMMARY;
  Object.assign(env, options.env);
  const run = spawnSync(process.execPath, [options.script ?? SCRIPT, file], { encoding: "utf8", env });
  return { status: run.status, stdout: run.stdout, stderr: run.stderr };
}

test("two journey tests that passed give a time and no flag", () => {
  withScratch((dir) => {
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)]]));
    expect(run.status).toBe(0);
    expect(run.stdout).toContain("Time: 90.0 s of the 180 s budget (50%).");
    expect(run.stdout).not.toContain("Partial");
    expect(run.stdout).not.toContain("Expected 2");
  });
});

test("journey tests that are all skipped give unknown, not a time near 0.0 s", () => {
  withScratch((dir) => {
    const run = report(dir, resultJson([["story", "skipped", skipped()], ["features", "skipped", skipped()]]));
    expect(run.status).toBe(0);
    expect(run.stdout).toContain("Time: unknown");
    expect(run.stdout).not.toContain("0.0 s");
  });
});

test("a skipped journey test is counted as not run, not as a run of 0 s", () => {
  withScratch((dir) => {
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "skipped", skipped()]]));
    expect(run.status).toBe(0);
    expect(run.stdout).toContain("Partial: 1 test(s) did not run and 0 did not pass");
    expect(run.stdout).not.toContain("features");
  });
});

test("one journey test, or three, is flagged as a count other than two", () => {
  withScratch((dir) => {
    const one = report(dir, resultJson([["story", "expected", passed(60_000)]]));
    expect(one.stdout).toContain("Expected 2 journey.spec.ts tests and found 1");
    const three = report(
      dir,
      resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)], ["extra", "expected", passed(1_000)]]),
    );
    expect(three.stdout).toContain("Expected 2 journey.spec.ts tests and found 3");
  });
});

test("a result with no journey test says unknown and that two were expected", () => {
  withScratch((dir) => {
    const run = report(dir, JSON.stringify({ suites: [] }));
    expect(run.status).toBe(0);
    expect(run.stdout).toContain("Time: unknown");
    expect(run.stdout).toContain("Expected 2 journey.spec.ts tests and found 0");
  });
});

test("an unwritable job summary leaves exit 0 and the report on stdout", () => {
  withScratch((dir) => {
    // A directory cannot be appended to: appendFileSync throws EISDIR.
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)]]), {
      env: { GITHUB_STEP_SUMMARY: dir },
    });
    expect(run.status, run.stderr).toBe(0);
    expect(run.stdout).toContain("Time: 90.0 s");
    expect(run.stderr).toContain("could not write $GITHUB_STEP_SUMMARY");
  });
});

test("a writable job summary gets the block", () => {
  withScratch((dir) => {
    const summary = join(dir, "summary.md");
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)]]), {
      env: { GITHUB_STEP_SUMMARY: summary },
    });
    expect(run.status).toBe(0);
    expect(readFileSync(summary, "utf8")).toContain("Time: 90.0 s");
  });
});

test("the script reports when started through a symlink", () => {
  withScratch((dir) => {
    const link = join(dir, "link.mjs");
    symlinkSync(SCRIPT, link);
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)]]), { script: link });
    expect(run.status, run.stderr).toBe(0);
    expect(run.stdout).toContain("Time: 90.0 s");
  });
});

test("the script reports when its path has a space in it", () => {
  withScratch((dir) => {
    const spaced = join(dir, "with space");
    mkdirSync(spaced);
    const copy = join(spaced, "journey-time.mjs");
    copyFileSync(SCRIPT, copy);
    const run = report(dir, resultJson([["story", "expected", passed(60_000)], ["features", "expected", passed(30_000)]]), { script: copy });
    expect(run.status, run.stderr).toBe(0);
    expect(run.stdout).toContain("Time: 90.0 s");
  });
});
