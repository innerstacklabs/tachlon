// The smoke set: the Playwright tests tagged `@smoke`, one per journey that works today plus startup,
// save and teardown. Pure functions over the output of `playwright test --list --reporter=json`; the
// check in `scripts/test-smoke-set.ts` runs the command and calls these. The set is small on purpose, so
// a change to its size or to the files it covers shows in a diff to this file or to a spec's tag.
import { spawnSync } from "node:child_process";

/** The tag a smoke test carries, as the spec writes it and as `--grep` matches it. */
export const SMOKE_TAG = "@smoke";
/** The same tag as the JSON report lists it: Playwright drops the leading `@`. */
const REPORTED_TAG = "smoke";

/** What Playwright puts in the report's `errors` when the grep matches nothing. */
const NO_TESTS_MESSAGE = "Error: No tests found";

/** The set holds at least this many tests... */
export const MIN_TESTS = 10;
/** ...and at most this many. */
export const MAX_TESTS = 25;

/** The spec files (paths under the chart folder's `tests/`) that must each hold at least one smoke test, and what each covers. */
export const SMOKE_FILES: ReadonlyArray<{ file: string; covers: string }> = [
  { file: "startup.spec.ts", covers: "the page loads and the chart is ready" },
  { file: "open.spec.ts", covers: "open a file" },
  { file: "picker.spec.ts", covers: "the project chooser for a multi-project XER" },
  { file: "outline.spec.ts", covers: "collapse the WBS" },
  { file: "filters.spec.ts", covers: "critical only and longest path" },
  { file: "lookahead.spec.ts", covers: "lookahead window" },
  { file: "find.spec.ts", covers: "find by ID or name" },
  { file: "selection.spec.ts", covers: "select a row" },
  { file: "moved.spec.ts", covers: "what moved" },
  { file: "print-pages.spec.ts", covers: "print" },
  { file: "key-map.spec.ts", covers: "every key binding, add and delete activities among them" },
  { file: "table-edit.spec.ts", covers: "edit a cell" },
  { file: "links.spec.ts", covers: "add a link" },
  { file: "save-open.spec.ts", covers: "save, then open again" },
  { file: "zoom-pan.spec.ts", covers: "zoom and pan" },
  { file: "destroy.spec.ts", covers: "unmount frees everything" },
  { file: "webgl-switch.spec.ts", covers: "a WebGPU failure switches to WebGL2 in place" },
];

/** One test as the list reports it. */
export interface ListedTest {
  file: string;
  title: string;
  tags: string[];
}

interface JsonSuite {
  specs?: Array<{ title?: unknown; file?: unknown; tags?: unknown }>;
  suites?: JsonSuite[];
}

/** The tests in the JSON report that carry the smoke tag, or an error when the text is not a Playwright report. */
export function parseSmokeList(json: string): { tests: ListedTest[] } | { error: string } {
  let doc: { suites?: JsonSuite[]; errors?: unknown[] };
  try {
    doc = JSON.parse(json);
  } catch {
    return { error: "the list output is not JSON" };
  }
  if (!Array.isArray(doc.suites)) return { error: "the list output has no suites array" };
  // `--grep` with no match is the one report error that is a result: it means zero tagged tests.
  const errors = (Array.isArray(doc.errors) ? doc.errors : []).filter(
    (e) => (e as { message?: unknown } | null)?.message !== NO_TESTS_MESSAGE,
  );
  if (errors.length > 0) return { error: `Playwright reported ${errors.length} error(s) while listing` };
  const tests: ListedTest[] = [];
  const walk = (s: JsonSuite): void => {
    for (const spec of s.specs ?? []) {
      const tags = Array.isArray(spec.tags) ? spec.tags.filter((t): t is string => typeof t === "string") : [];
      if (typeof spec.file !== "string" || typeof spec.title !== "string") continue;
      if (tags.includes(REPORTED_TAG)) tests.push({ file: spec.file, title: spec.title, tags });
    }
    for (const child of s.suites ?? []) walk(child);
  };
  for (const s of doc.suites) walk(s);
  return { tests };
}

/** The problems with a set of smoke tests: a count outside the range, a listed file with none, a file outside the list. */
export function checkSmokeSet(
  tests: readonly ListedTest[],
  files: ReadonlyArray<{ file: string }> = SMOKE_FILES,
): string[] {
  const problems: string[] = [];
  if (tests.length < MIN_TESTS || tests.length > MAX_TESTS) {
    problems.push(`${tests.length} tagged tests; the set holds ${MIN_TESTS} to ${MAX_TESTS}`);
  }
  const per = new Map<string, number>();
  for (const t of tests) per.set(t.file, (per.get(t.file) ?? 0) + 1);
  for (const { file } of files) {
    if (!per.has(file)) problems.push(`${file} has no ${SMOKE_TAG} test`);
  }
  const known = new Set(files.map((f) => f.file));
  for (const file of [...per.keys()].sort()) {
    if (!known.has(file)) problems.push(`${file} has ${SMOKE_TAG} tests but is not in SMOKE_FILES`);
  }
  return problems;
}

/** Runs `playwright test --list --grep @smoke --reporter=json` in `webDir` and returns what it printed. Listing needs no build. */
export function listSmokeJson(webDir: string): { json: string } | { error: string } {
  const run = spawnSync(
    "pnpm",
    ["exec", "playwright", "test", "--list", "--grep", SMOKE_TAG, "--reporter=json"],
    { cwd: webDir, encoding: "utf8", maxBuffer: 64 * 1024 * 1024, timeout: 300_000 },
  );
  if (run.error) return { error: `could not run playwright: ${run.error.message}` };
  // With no match Playwright exits 1 but still prints the report: the count rule names that case.
  if (run.status !== 0 && !run.stdout.trimStart().startsWith("{")) {
    const tail = (run.stderr || run.stdout || "").trim().split("\n").slice(-5).join(" | ");
    return { error: `playwright exited ${run.status}: ${tail}` };
  }
  return { json: run.stdout };
}
