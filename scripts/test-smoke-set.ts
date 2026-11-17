// Checks the smoke set (see scripts/lib/smoke-set.ts). Run from anywhere:
//   node scripts/test-smoke-set.ts
// Needs Node 22.18 or later (type stripping) and `pnpm install` done in the chart folder. It does two things:
//   1. Runs the count and file rules over fixtures, so a rule that stops failing shows here.
//   2. Lists the tests tagged @smoke with Playwright (`--list` needs no build), prints them, and
//      applies the rules: 10 to 25 tests, and at least one in each file of the set.
// Exits 1 and prints one line per problem.
import { dirname, join } from "node:path";
import { WEB_ROOT } from "./lib/web-root.ts";
import { fileURLToPath } from "node:url";
import {
  MAX_TESTS,
  MIN_TESTS,
  SMOKE_FILES,
  SMOKE_TAG,
  checkSmokeSet,
  listSmokeJson,
  parseSmokeList,
  type ListedTest,
} from "./lib/smoke-set.ts";

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-smoke-set: FAIL ${line}`);
};

// ---- 1. the rules, over fixtures -------------------------------------------------

const sample = (files: readonly string[], extra = 0): ListedTest[] =>
  [...files, ...Array<string>(extra).fill(files[0] ?? "x.spec.ts")].map((file, i) => ({
    file,
    title: `t${i}`,
    tags: ["smoke"],
  }));
const all = SMOKE_FILES.map((f) => f.file);
const rows: Array<{ name: string; tests: ListedTest[]; expect: string | null }> = [
  { name: "every file once passes", tests: sample(all), expect: null },
  { name: "no tests fails on the count", tests: [], expect: `0 tagged tests; the set holds ${MIN_TESTS} to ${MAX_TESTS}` },
  { name: "nine tests fails on the count", tests: sample(all.slice(0, MIN_TESTS - 1)), expect: "9 tagged tests" },
  { name: "ten tests keeps the count", tests: sample(all.slice(0, MIN_TESTS)), expect: "has no @smoke test" },
  { name: "twenty-six tests fails on the count", tests: sample(all, MAX_TESTS + 1 - all.length), expect: "26 tagged tests" },
  { name: "a missing file is named", tests: sample(all.slice(1)), expect: `${all[0]} has no @smoke test` },
  { name: "a file outside the set is named", tests: sample([...all, "other.spec.ts"]), expect: "other.spec.ts has @smoke tests but is not in SMOKE_FILES" },
];
for (const row of rows) {
  const problems = checkSmokeSet(row.tests);
  const ok = row.expect === null ? problems.length === 0 : problems.some((p) => p.includes(row.expect as string));
  if (!ok) fail(`fixture "${row.name}": expected ${row.expect ?? "no problem"}, got [${problems.join("; ")}]`);
}

const report = JSON.stringify({
  suites: [
    {
      file: "a.spec.ts",
      specs: [{ title: "outer", file: "a.spec.ts", tags: ["smoke"] }],
      suites: [{ specs: [{ title: "inner", file: "a.spec.ts", tags: ["smoke"] }, { title: "plain", file: "a.spec.ts", tags: [] }] }],
    },
  ],
  errors: [],
});
const parsed = parseSmokeList(report);
if ("error" in parsed || parsed.tests.map((t) => t.title).join(",") !== "outer,inner") {
  fail(`fixture "a nested suite is walked and an untagged test skipped": got ${JSON.stringify(parsed)}`);
}
if (!("error" in parseSmokeList("not json"))) fail('fixture "text that is not JSON is an error"');
if (!("error" in parseSmokeList(JSON.stringify({ suites: [], errors: [{ message: "Error: boom" }] })))) {
  fail('fixture "a listing error is an error"');
}
const none = parseSmokeList(JSON.stringify({ suites: [], errors: [{ message: "Error: No tests found" }] }));
if ("error" in none || none.tests.length !== 0) fail(`fixture "no match is zero tests": got ${JSON.stringify(none)}`);

// ---- 2. the live set --------------------------------------------------------------

const webDir = join(dirname(fileURLToPath(import.meta.url)), "..", WEB_ROOT);
const listed = listSmokeJson(webDir);
if ("error" in listed) {
  fail(listed.error);
} else {
  const live = parseSmokeList(listed.json);
  if ("error" in live) {
    fail(live.error);
  } else {
    console.log(`test-smoke-set: ${live.tests.length} tests tagged ${SMOKE_TAG}`);
    for (const t of live.tests) console.log(`  ${t.file}: ${t.title}`);
    for (const p of checkSmokeSet(live.tests)) fail(p);
  }
}

if (failed) process.exit(1);
console.log("test-smoke-set: ok");
