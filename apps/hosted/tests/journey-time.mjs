// Reports the journey spec's wall time against its budget (gs#495). Node 22, no dependencies, no browser. Run from apps/hosted:
//
//   node tests/journey-time.mjs test-results/results.json
//
// The argument is the Playwright JSON reporter's file. The `hosted-browser` job in .github/workflows/ci.yml runs the specs with that
// reporter and then runs this. It prints a short Markdown block and appends the same block to $GITHUB_STEP_SUMMARY when that
// is set. The time is the sum of the journey tests' durations (the story list and the features list are two tests).
//
// A result it cannot read, or one with no journey test that ran (a skipped test does not count as run), is reported as
// "unknown", never as a time: a missing file or an all-skipped run must not look like a fast run. A result with any count
// of journey tests other than two is flagged. The script exits 0 with a result path, including when the job summary cannot
// be written; it reports, it does not gate (the spec's own timeout does). No path is a usage error, exit 2.

import { appendFileSync, readFileSync, realpathSync } from "node:fs";
import { fileURLToPath } from "node:url";

const SPEC = "journey.spec.ts";
const BUDGET_S = 180;
// The story list and the features list. A count other than this means a journey test was dropped or added.
const EXPECTED_TESTS = 2;

/** Every test of a Playwright JSON suite tree that belongs to `SPEC`, as { title, status, ms }. */
function journeyTests(suites) {
  const found = [];
  const walk = (suite) => {
    for (const spec of suite.specs ?? []) {
      if (!String(spec.file ?? suite.file ?? "").endsWith(SPEC)) continue;
      for (const test of spec.tests ?? []) {
        // A skipped test still has a result (status "skipped", duration 0): only a result that is not skipped counts as a run.
        const results = (test.results ?? []).filter((result) => result.status !== "skipped");
        const ms = results.reduce((sum, result) => sum + (Number(result.duration) || 0), 0);
        found.push({ title: spec.title, status: test.status, ran: results.length > 0, ms });
      }
    }
    for (const child of suite.suites ?? []) walk(child);
  };
  for (const suite of suites ?? []) walk(suite);
  return found;
}

/** The Markdown block for the saved result at `path`. */
export function summarize(path) {
  const head = `### Journey spec time (${SPEC})`;
  let tests;
  try {
    tests = journeyTests(JSON.parse(readFileSync(path, "utf8")).suites);
  } catch (error) {
    return `${head}\n\nTime: unknown (no readable result at \`${path}\`: ${error.message}). Budget: ${BUDGET_S} s.\n`;
  }
  const ran = tests.filter((test) => test.ran);
  const total = ran.reduce((sum, test) => sum + test.ms, 0);
  const countFlag =
    tests.length === EXPECTED_TESTS ? [] : ["", `Expected ${EXPECTED_TESTS} ${SPEC} tests and found ${tests.length}, so a journey test was dropped or added and this time is not a full run.`];
  if (ran.length === 0 || total <= 0) {
    return [`${head}\n\nTime: unknown (the result has no timed ${SPEC} test). Budget: ${BUDGET_S} s.`, ...countFlag, ""].join("\n");
  }
  const seconds = total / 1000;
  const lines = [head, "", `Time: ${seconds.toFixed(1)} s of the ${BUDGET_S} s budget (${Math.round((seconds / BUDGET_S) * 100)}%).`];
  if (seconds > BUDGET_S) lines.push("", `Over the budget by ${(seconds - BUDGET_S).toFixed(1)} s.`);
  const notRun = tests.length - ran.length;
  const notPassed = ran.filter((test) => test.status !== "expected").length;
  if (notRun > 0 || notPassed > 0) {
    lines.push("", `Partial: ${notRun} test(s) did not run and ${notPassed} did not pass, so this time is not a full run.`);
  }
  lines.push(...countFlag);
  lines.push("", ...ran.map((test) => `- ${test.title}: ${(test.ms / 1000).toFixed(1)} s (${test.status})`), "");
  return lines.join("\n");
}

/** True when this file is the script node was started with: both paths are real paths, so a symlink or a space in the path does not hide it. */
function isMain() {
  if (!process.argv[1]) return false;
  try {
    return realpathSync(process.argv[1]) === realpathSync(fileURLToPath(import.meta.url));
  } catch {
    return false;
  }
}

if (isMain()) {
  const path = process.argv[2];
  if (!path) {
    console.error("usage: node tests/journey-time.mjs <playwright results.json>");
    process.exit(2);
  }
  const block = summarize(path);
  console.log(block);
  if (process.env.GITHUB_STEP_SUMMARY) {
    // The report is already on stdout; a summary file that cannot be written must not turn a report into a failed step.
    try {
      appendFileSync(process.env.GITHUB_STEP_SUMMARY, `${block}\n`);
    } catch (error) {
      console.error(`journey-time: could not write $GITHUB_STEP_SUMMARY (${error.message}); the block above is on stdout only.`);
    }
  }
}
