// Checks which browser specs a run owes (see scripts/lib/touched-specs.ts). Run from anywhere:
//   node scripts/test-touched-specs.ts
// Needs Node 22.18 or later (type stripping). A table of tiers and changed files against the plan the
// `web` job gets, then a table of spec sources against the no-page tests. Exits 1, one line per problem.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { DRIVEN_BY, benchPlan, grepFor, hostedPlan, noPageTests, webPlan, type WebPlan } from "./lib/touched-specs.ts";
import type { Tier } from "./lib/ci-tier.ts";
import { WEB_ROOT } from "./lib/web-root.ts";

let failed = false;
const fail = (line: string): void => {
  failed = true;
  console.error(`test-touched-specs: FAIL ${line}`);
};

const SPECS = ["find.spec.ts", "find-live.spec.ts", "save-open.spec.ts", "open.spec.ts", "measure.spec.ts", "measure-link-edit.spec.ts", "toolbar.spec.ts", "react-events.spec.ts", "react-strict-mode.spec.ts", "journey-j5.spec.ts"];
const EVERY_PR_SPEC = SPECS.filter((s) => s !== "measure.spec.ts");
const none: Pick<WebPlan, "web" | "specs"> = { web: "none", specs: [] };
const all: Pick<WebPlan, "web" | "specs"> = { web: "all", specs: EVERY_PR_SPEC };
const checks: Pick<WebPlan, "web" | "specs"> = { web: "checks", specs: [] };

const rows: Array<{ name: string; tier: Tier; changed: string[] | null; want: Pick<WebPlan, "web" | "specs"> }> = [
  { name: "a crate's tests, benches and manifest run no shard", tier: "fast", changed: ["crates/app/tests/insert.rs", "crates/engine/benches/cpm.rs", "crates/engine/Cargo.toml"], want: none },
  { name: "scripts, docs and the workflow run no shard", tier: "fast", changed: ["scripts/ci-tier.ts", "docs/PLAN.md", ".github/workflows/ci.yml"], want: none },
  { name: "a changed spec runs that spec", tier: "fast", changed: [`${WEB_ROOT}/tests/toolbar.spec.ts`], want: { web: "some", specs: ["toolbar.spec.ts"] } },
  { name: "a src module runs the specs that share its name", tier: "fast", changed: [`${WEB_ROOT}/src/find.ts`], want: { web: "some", specs: ["find.spec.ts", "find-live.spec.ts"] } },
  { name: "a name shared after a hyphen counts, a bare substring does not", tier: "fast", changed: [`${WEB_ROOT}/src/open.ts`], want: { web: "some", specs: ["save-open.spec.ts", "open.spec.ts"] } },
  { name: "a stylesheet maps by its name too", tier: "fast", changed: [`${WEB_ROOT}/src/styles/toolbar.css`, "crates/app/src/lib.rs"], want: { web: "some", specs: ["toolbar.spec.ts"] } },
  { name: "a src change that maps to no spec runs every shard", tier: "fast", changed: [`${WEB_ROOT}/src/find.ts`, `${WEB_ROOT}/src/boot.ts`], want: all },
  { name: "a shared test helper runs every shard", tier: "fast", changed: [`${WEB_ROOT}/tests/support/gantt.ts`], want: all },
  { name: "the web package runs every shard", tier: "fast", changed: [`${WEB_ROOT}/package.json`], want: all },
  { name: "a Node test adds no spec (its own step runs it)", tier: "fast", changed: [`${WEB_ROOT}/tests-node/cli.test.ts`], want: none },
  { name: "the package README adds no spec", tier: "fast", changed: [`${WEB_ROOT}/README.md`], want: none },
  { name: "a Node test beside a src module adds only that module's specs", tier: "fast", changed: [`${WEB_ROOT}/tests-node/cli.test.ts`, `${WEB_ROOT}/src/find.ts`], want: { web: "some", specs: ["find.spec.ts", "find-live.spec.ts"] } },
  { name: "a Node test beside an unmapped file still runs every shard", tier: "fast", changed: [`${WEB_ROOT}/tests-node/cli.test.ts`, `${WEB_ROOT}/package.json`], want: all },
  { name: "the React wrapper runs its host specs", tier: "fast", changed: [`${WEB_ROOT}/src/react.tsx`], want: { web: "some", specs: ["react-events.spec.ts", "react-strict-mode.spec.ts"] } },
  { name: "the consumer check runs the checks leg and no shard", tier: "fast", changed: ["scripts/lib/consumer-check.ts"], want: checks },
  { name: "the staging script runs the checks leg and no shard (the checks run on its staged copy)", tier: "fast", changed: ["scripts/lib/stage-package.ts"], want: checks },
  { name: "the pack check runs the checks leg and no shard", tier: "fast", changed: ["scripts/pack-check.ts"], want: checks },
  { name: "the publish workflow runs the checks leg and no shard (the checks guard what it publishes)", tier: "fast", changed: [".github/workflows/npm-publish.yml"], want: checks },
  { name: "an example runs the checks leg and no shard", tier: "fast", changed: ["examples/check.ts"], want: checks },
  { name: "the print demo runs the checks leg and no shard", tier: "fast", changed: ["demos/print/run.sh"], want: checks },
  { name: "an example beside a src module keeps that module's specs", tier: "fast", changed: ["examples/check.ts", `${WEB_ROOT}/src/find.ts`], want: { web: "some", specs: ["find.spec.ts", "find-live.spec.ts"] } },
  { name: "an example on deploy runs the checks leg and no shard", tier: "deploy", changed: ["examples/plain/main.js", "docs/PLAN.md"], want: checks },
  { name: "a diff that could not be read runs every shard", tier: "fast", changed: null, want: all },
  { name: "measure.spec never runs on a PR, even when changed", tier: "fast", changed: [`${WEB_ROOT}/tests/measure.spec.ts`], want: none },
  { name: "the measure module runs its PR specs only", tier: "fast", changed: [`${WEB_ROOT}/src/measure.ts`], want: { web: "some", specs: ["measure-link-edit.spec.ts"] } },
  { name: "a deleted spec runs nothing", tier: "fast", changed: [`${WEB_ROOT}/tests/gone.spec.ts`], want: none },
  { name: "full runs every spec, measure.spec included", tier: "full", changed: null, want: { web: "all", specs: [] } },
  { name: "nightly runs every spec", tier: "nightly", changed: ["crates/app/src/lib.rs"], want: { web: "all", specs: [] } },
  { name: "a print-only change on deploy runs the J5 spec", tier: "deploy", changed: ["crates/app/src/print/page.rs", "crates/app/tests/print_pages.rs"], want: { web: "some", specs: ["journey-j5.spec.ts"] } },
  { name: "a docs-only change on deploy runs no spec", tier: "deploy", changed: ["docs/PLAN.md"], want: none },
  { name: "a diff on deploy that could not be read runs every spec but measure.spec", tier: "deploy", changed: null, want: all },
];
for (const row of rows) {
  const got = webPlan(row.tier, row.changed, SPECS);
  if (got.web !== row.want.web || JSON.stringify(got.specs) !== JSON.stringify(row.want.specs)) {
    fail(`"${row.name}": got ${got.web} ${JSON.stringify(got.specs)} (${got.why}), wanted ${row.want.web} ${JSON.stringify(row.want.specs)}`);
  }
}

// The chrome and the crates map to the real specs (T7): one row per `DRIVEN_BY` row, and one for each name rule. A table entry that names no spec fails below.
const REAL = readdirSync(fileURLToPath(new URL(`../${WEB_ROOT}/tests`, import.meta.url))).filter((f) => f.endsWith(".spec.ts")).sort();
const CHROME = ["bare-chart.spec.ts", "host-commands.spec.ts", "host-open-state.spec.ts", "two-charts.spec.ts", "webgl-switch.spec.ts"];
const FILES = ["open.spec.ts", "open-document.spec.ts", "import-notes.spec.ts", "ledger-timeline.spec.ts"];
const driven: Array<{ name: string; changed: string[]; want: string[] }> = [
  { name: "a chrome module with no spec of its name runs the specs that install the chrome", changed: ["apps/chrome/src/parts.ts"], want: CHROME },
  { name: "a chrome module also runs the specs of its name", changed: ["apps/chrome/src/keys-help.ts"], want: [...CHROME, "keys-help.spec.ts", "key-map.spec.ts"] },
  { name: "the engine runs the specs that edit and recompute", changed: ["crates/engine/src/cpm.rs"], want: ["insert-delete.spec.ts", "links.spec.ts", "lookahead.spec.ts", "moved.spec.ts", "schedule-route.spec.ts"] },
  { name: "the importer runs the specs that open a file", changed: ["crates/import/src/activity_id.rs"], want: FILES },
  { name: "the XER crate runs the specs that open a file", changed: ["crates/xer/src/lib.rs"], want: FILES },
  { name: "the timeline runs the specs that read its bars and axis", changed: ["crates/timeline/src/theme.rs"], want: ["bar-look.spec.ts", "zoom-pan.spec.ts", "ledger-timeline.spec.ts"] },
  { name: "the app crate runs the startup and data API specs, and the specs of a module's name", changed: ["crates/app/src/lookahead.rs"], want: ["startup.spec.ts", "data-api.spec.ts", "lookahead.spec.ts"] },
  { name: "a document writer change runs the adapter round-trip specs", changed: ["crates/app/src/document/write.rs"], want: ["adapter-dhtmlx.spec.ts", "adapter-bryntum.spec.ts", "startup.spec.ts", "data-api.spec.ts"] },
  { name: "the printed pages run the journey that prints and the page spec", changed: ["crates/app/src/print/legend.rs"], want: ["journey-j5.spec.ts", "print-pages.spec.ts", "legend.spec.ts", "startup.spec.ts", "data-api.spec.ts"] },
  { name: "a chart print module runs the journey that reads its sheet, and its own spec", changed: [`${WEB_ROOT}/src/print-sheet.ts`], want: ["journey-j5.spec.ts", "print-sheet.spec.ts"] },
  { name: "a chart file and a crate file add their specs together", changed: [`${WEB_ROOT}/src/range.ts`, "crates/xer/src/lib.rs"], want: [...FILES, "range.spec.ts", "copy-range.spec.ts"] },
];
for (const file of ["gantt-object.ts", "host-data.ts", "update.ts", "webgl-switch.ts"]) {
  const got = webPlan("fast", [`${WEB_ROOT}/src/${file}`], REAL);
  if (got.web !== "some" || !got.specs.includes("set-data-rows.spec.ts")) fail(`${file} is on setData's path, so it must run set-data-rows.spec.ts and not every spec`);
}
for (const row of driven) {
  const got = webPlan("fast", row.changed, REAL);
  const want = REAL.filter((s) => row.want.includes(s));
  if (got.web !== "some" || JSON.stringify(got.specs) !== JSON.stringify(want)) fail(`"${row.name}": got ${got.web} ${JSON.stringify(got.specs)} (${got.why}), wanted ${JSON.stringify(want)}`);
}
// The fifteen chart files with no spec of their name (gantt#1163): a change to any one starts some specs, never every spec.
for (const file of ["gantt-object.ts", "styles/table-cells.css", "table-row.ts", "filter-names.ts", "set-filter.ts", "row-edit.ts", "edit-result.ts", "view-state.ts", "column-drag.ts", "open-schedule.ts", "table-aria.ts", "explain.ts", "health.ts", "propose.ts", "what-if.ts"]) {
  const got = webPlan("fast", [`${WEB_ROOT}/src/${file}`], REAL);
  if (got.web !== "some") fail(`${file} is driven by no spec: got ${got.web} (${got.why})`);
}
for (const row of DRIVEN_BY) for (const spec of row.specs) if (!REAL.includes(spec)) fail(`DRIVEN_BY "${row.prefix}" names ${spec}, which is not in ${WEB_ROOT}/tests`);

// The entry file (src/index.ts) imports everything: a change that leaves its runtime code the same starts one spec, any other change starts every spec.
const ENTRY = `${WEB_ROOT}/src/index.ts`;
const ENTRY_SPECS = [...SPECS, "startup.spec.ts"];
const entryText = 'import type { A } from "./a.js";\nimport "./styles/chrome.css";\nexport const x: number = 1;\n';
const entryRows: Array<{ name: string; head: string; web: string; strip?: (text: string) => string }> = [
  { name: "a comment-only change", head: `// a note\n${entryText.replace("= 1;", "= 1; /* one */")}`, web: "some" },
  { name: "a changed type note", head: entryText.replace("number", "bigint").replace('import type { A } from "./a.js"', 'import type { B } from "./b.js"'), web: "some" },
  { name: "a changed import", head: entryText.replace("chrome.css", "panels.css"), web: "all" },
  { name: "a commented-out import", head: entryText.replace('import "./styles/chrome.css";', '// import "./styles/chrome.css";'), web: "all" },
  { name: "a changed value", head: entryText.replace("= 1", "= 2"), web: "all" },
  { name: "a head with a syntax error", head: `${entryText}export const = ;\n`, web: "all" },
  { name: "a transpiler that cannot load", head: `// a note\n${entryText}`, web: "all", strip: () => { throw new Error("no transpiler"); } },
];
for (const row of entryRows) {
  const got = webPlan("fast", [ENTRY], ENTRY_SPECS, { base: entryText, head: row.head }, row.strip);
  if (got.web !== row.web || (row.web === "some" && JSON.stringify(got.specs) !== JSON.stringify(["startup.spec.ts"]))) fail(`entry: ${row.name}: got ${got.web} ${JSON.stringify(got.specs)} (${got.why})`);
}
if (webPlan("fast", [ENTRY], ENTRY_SPECS, { base: null, head: entryText }).web !== "all") fail("entry: a base text that could not be read runs every spec");

const HOSTED_SPECS = ["samples.spec.ts", "first-screen.spec.ts", "journey.spec.ts"];
const hostedRows: Array<{ name: string; tier: Tier; changed: string[] | null; want: [string, string[]] }> = [
  { name: "no hosted file runs @smoke only", tier: "fast", changed: ["packages/gantt/src/find.ts", "docs/PLAN.md"], want: ["none", []] },
  { name: "a hosted src module runs the specs that name it", tier: "fast", changed: ["apps/hosted/src/samples.ts"], want: ["some", ["samples.spec.ts"]] },
  { name: "a changed hosted spec runs itself", tier: "fast", changed: ["apps/hosted/tests/journey.spec.ts", "docs/PLAN.md"], want: ["some", ["journey.spec.ts"]] },
  { name: "a hosted config, fixture or unmatched src runs the whole suite", tier: "fast", changed: ["apps/hosted/vite.config.ts"], want: ["all", []] },
  { name: "a hosted type declaration adds no spec (the typecheck covers it)", tier: "fast", changed: ["apps/hosted/tests/window-gantt.d.ts"], want: ["none", []] },
  { name: "a hosted markdown file adds no spec", tier: "fast", changed: ["apps/hosted/README.md"], want: ["none", []] },
  { name: "a hosted Node test adds no spec (its own step runs it)", tier: "fast", changed: ["apps/hosted/tests-node/samples.test.ts"], want: ["none", []] },
  { name: "the chart's page runs the whole suite", tier: "fast", changed: [`${WEB_ROOT}/src/chart.html`], want: ["all", []] },
  { name: "a chart stylesheet runs the whole suite", tier: "fast", changed: [`${WEB_ROOT}/src/styles/columns.css`], want: ["all", []] },
  { name: "a chart module with no hosted file stays on @smoke only", tier: "fast", changed: [`${WEB_ROOT}/src/table.ts`], want: ["none", []] },
  { name: "a diff that could not be read runs the whole suite", tier: "fast", changed: null, want: ["all", []] },
  { name: "another tier adds nothing", tier: "full", changed: ["apps/hosted/vite.config.ts"], want: ["none", []] },
];
for (const row of hostedRows) {
  const got = hostedPlan(row.tier, row.changed, HOSTED_SPECS);
  if (got.hosted !== row.want[0] || JSON.stringify(got.specs) !== JSON.stringify(row.want[1])) fail(`"${row.name}": got ${got.hosted} ${JSON.stringify(got.specs)} (${got.why})`);
}

// The benchmark driver test (`hosted` job) never runs on a PR push, whatever the diff reaches; batch, nightly and deploy run it.
const benchRows: Array<{ name: string; tier: Tier; want: boolean }> = [
  { name: "a PR push does not run it", tier: "fast", want: false },
  { name: "batch runs it", tier: "full", want: true },
  { name: "nightly runs it", tier: "nightly", want: true },
  { name: "deploy runs it", tier: "deploy", want: true },
];
for (const row of benchRows) {
  const got = benchPlan(row.tier);
  if (got.bench !== row.want) fail(`"${row.name}": got bench=${got.bench} (${got.why}), wanted ${row.want}`);
}

const odd = "a+b[1].spec.ts";
const oddGrep = new RegExp(grepFor(hostedPlan("fast", [`apps/hosted/tests/${odd}`], [odd, "ab1.spec.ts"]).specs));
if (!oddGrep.test(odd) || oddGrep.test("ab1.spec.ts")) fail(`a spec name with + and [ is not matched literally by its grep: ${oddGrep}`);

const sources: Array<{ name: string; source: string; want: string[] }> = [
  {
    name: "top-level tests with no fixture, quoted either way, with or without a tag",
    source: ['test("reads (a) file", () => {});', "test('parses', async () => {});", 'test("tagged", { tag: "@smoke" }, () => {});'].join("\n"),
    want: ["reads (a) file", "parses", "tagged"],
  },
  {
    name: "a test that takes a fixture or sits in a describe opens a page",
    source: ['test("g", async ({ g }) => {});', 'test.describe("d", () => {', '  test("shared page", async () => {});', "});"].join("\n"),
    want: [],
  },
  {
    name: "a top-level hook with a fixture opens a page for every test",
    source: ['test.beforeEach(async ({ page }) => {});', 'test("plain", () => {});'].join("\n"),
    want: [],
  },
];
for (const row of sources) {
  const got = noPageTests(row.source);
  if (JSON.stringify(got) !== JSON.stringify(row.want)) fail(`"${row.name}": got ${JSON.stringify(got)}, wanted ${JSON.stringify(row.want)}`);
}

// A code file renamed into docs/ lists only its new path unless the diff keeps both: the spec it feeds must still run.
const repo = mkdtempSync(join(tmpdir(), "touched-specs-"));
const git = (...args: string[]) => spawnSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...args], { cwd: repo, encoding: "utf8" });
for (const dir of [`${WEB_ROOT}/src`, `${WEB_ROOT}/tests`, "apps/hosted/tests", "docs"]) mkdirSync(join(repo, dir), { recursive: true });
for (const file of [`${WEB_ROOT}/src/find.ts`, `${WEB_ROOT}/tests/find.spec.ts`, `${WEB_ROOT}/tests/journey-j5.spec.ts`, "apps/hosted/tests/a.spec.ts"]) writeFileSync(join(repo, file), "export {};\n");
git("init", "-q");
git("add", ".");
git("commit", "-qm", "base");
git("update-ref", "refs/remotes/origin/main", "HEAD");
git("mv", `${WEB_ROOT}/src/find.ts`, "docs/find.md");
git("commit", "-qam", "rename into docs");
const renamed = spawnSync("node", [fileURLToPath(new URL("touched-specs.ts", import.meta.url)), "fast"], { cwd: repo, encoding: "utf8" });
if (!renamed.stdout.includes("web=some")) fail(`a code file renamed into docs/ runs no spec: ${renamed.stdout.trim()} ${renamed.stderr.trim()}`);
if (!renamed.stdout.includes('shards=[1,2,3,4,"checks"]\n')) fail(`the script prints no shards= line with the four shards for a web=some plan: ${renamed.stdout.trim()}`);

// On deploy the diff against main is empty; the push's own commits (PUSH_BEFORE..HEAD) say what to run.
const deployOut = (before: string): string => spawnSync("node", [fileURLToPath(new URL("touched-specs.ts", import.meta.url)), "deploy"], { cwd: repo, encoding: "utf8", env: { ...process.env, PUSH_BEFORE: before } }).stdout;
const pushBase = git("rev-parse", "HEAD").stdout.trim();
mkdirSync(join(repo, "crates/app/src/print"), { recursive: true });
writeFileSync(join(repo, "crates/app/src/print/page.rs"), "// print\n");
git("add", ".");
git("commit", "-qm", "print");
const printPush = deployOut(pushBase);
if (!printPush.includes("web=some") || !printPush.includes("specs=tests/journey-j5.spec.ts\n")) fail(`a push that changes only print code does not run the J5 spec: ${printPush.trim()}`);
const printSha = git("rev-parse", "HEAD").stdout.trim();
writeFileSync(join(repo, "docs/note.md"), "note\n");
git("add", ".");
git("commit", "-qm", "docs");
if (!deployOut(printSha).includes("web=none")) fail("a push that changes only docs runs a spec");
for (const before of ["", "0".repeat(40), "1".repeat(40)]) if (!deployOut(before).includes("web=all")) fail(`a push with no readable before (${before || "unset"}) does not run every spec`);
// The script reads the entry file's base text from git: a comment-only commit runs one spec, a changed import runs the rest.
writeFileSync(join(repo, "packages/gantt/tests/startup.spec.ts"), "export {};\n");
writeFileSync(join(repo, ENTRY), entryText);
git("add", ".");
git("commit", "-qm", "entry");
const entrySha = git("rev-parse", "HEAD").stdout.trim();
writeFileSync(join(repo, ENTRY), `// a note\n${entryText}`);
git("commit", "-qam", "comment");
if (!deployOut(entrySha).includes("specs=tests/startup.spec.ts\n")) fail("a pushed comment-only change to the entry file does not run one spec");
writeFileSync(join(repo, ENTRY), entryText.replace("chrome.css", "panels.css"));
git("commit", "-qam", "import");
if (!deployOut(entrySha).includes("web=all")) fail("a pushed import change to the entry file does not run every spec");
rmSync(repo, { recursive: true });

if (failed) process.exit(1);
console.log(`test-touched-specs: ok (${rows.length} plan rows, ${benchRows.length} bench rows, ${driven.length} driven rows, ${sources.length} source rows, 1 rename row)`);
