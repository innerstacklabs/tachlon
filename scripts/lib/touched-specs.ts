// The browser specs a run owes the `web` job (gs#1163, PR 1 of the test-pyramid plan). `select` in
// `.github/workflows/ci.yml` calls `scripts/touched-specs.ts`, which calls this. Pure, so the table in
// `scripts/test-touched-specs.ts` is the whole rule.
//
// On the fast tier (a PR branch) a spec under the chart folder's `tests/` is touched when the diff changes it, or changes
// a `src` module whose name it shares: the module's file name, without its extension, is the spec's
// name or a hyphen-separated part of it (`find.ts` touches `find.spec.ts` and `find-live.spec.ts`,
// `open.ts` touches `save-open.spec.ts`). A `src` file that touches no spec, and any other file under
// the chart folder (a test helper, a fixture, the config, the package), runs every spec, except a Node test under `tests-node/` and the folder's
// README (`SPEC_FREE_WEB`); a file under `examples/` or `demos/print/`, and a file that stages, checks or publishes the package (`PUBLISH_PATH`), is read only by the
// `web` job's `checks` leg, so it runs that leg and no shard (`checks`); beside a file that asks for specs it adds nothing. A file under `apps/chrome/` or a crate's `src/` adds the specs that drive it (`DRIVEN_BY`, and the name rule for the chrome's
// and the app crate's modules). A file outside all of these (`scripts/`, `docs/`, the workflow, a crate's tests) adds no spec. The batch-only specs never run on fast.
// The deploy tier (a push to main) reads the push's own commits the same way (`before..HEAD`); a diff it cannot read runs every spec. Full and nightly run every spec.
//
// `hostedPlan` is the same rule for the `hosted-browser` job's Playwright step (lo#3176): on fast, a diff with a file under `apps/hosted/`
// adds the hosted specs it reaches to the `@smoke` tier, or every hosted spec when a changed file there is no spec or src module.
// A type declaration, a markdown file and a Node test under `apps/hosted/` add no spec (`SPEC_FREE_HOSTED`).
// The chart's page (`src/chart.html`) and its `src/styles/` files run every hosted spec: every hosted page loads them (gantt#1614).
//
// The chart's entry file (`src/index.ts`) imports every module and stylesheet, so a change to it runs every spec, unless its runtime code is the same before and
// after (`sameRuntime`): then only comments or types changed, and `ENTRY_SPEC` runs. Node's own type stripper reads both texts, so `select` needs no install.
import * as nodeModule from "node:module";
import { BATCH_ONLY_SPECS, type Tier } from "./ci-tier.ts";
import { WEB_ROOT } from "./web-root.ts";

export interface WebPlan {
  /** none: the `web` job is skipped. checks: it runs its `checks` leg and no shard. some: it runs `specs`. all: it runs every spec (`specs` lists them on fast, is empty otherwise). */
  web: "none" | "checks" | "some" | "all";
  /** Spec file names under `tests/`; empty means no filter. */
  specs: string[];
  /** One line for the log. */
  why: string;
}

/** The legs of the `web` matrix as a JSON array for `select`'s `shards` output: the four Playwright shards (`--shard=i/4`) and `checks`, or `checks` alone. */
export const webShards = (web: WebPlan["web"]): string => (web === "checks" ? '["checks"]' : '[1,2,3,4,"checks"]');

const SPEC_DIR = `${WEB_ROOT}/tests/`;
const SRC_DIR = `${WEB_ROOT}/src/`;
const SPEC_EXT = ".spec.ts";

/** A path's file name without its extension(s): `src/styles/toolbar.css` is `toolbar`. */
const stem = (path: string): string => path.slice(path.lastIndexOf("/") + 1).replace(/\..*$/, "");

/** Whether `spec` (a file name in `tests/`) shares its name with the module stem `module`. */
const sharesName = (spec: string, module: string): boolean => `-${stem(spec)}-`.includes(`-${module}-`);

/** Files under the chart folder that no browser spec can see change: a Node test (the `node --test` step runs it) and the package README. */
const SPEC_FREE_WEB: Array<(file: string) => boolean> = [(f) => f.startsWith(`${WEB_ROOT}/tests-node/`), (f) => f === `${WEB_ROOT}/README.md`];

/**
 * The specs that drive code outside the chart folder (gs#1163 T7): a file under `prefix` adds `specs`, besides the specs that share its name when it is
 * a source file of the chrome or the app crate (`find.rs` touches `find.spec.ts`). Each row is read from the specs' imports and page actions.
 * Only a crate's `src/` is listed: its tests, benches and manifest run no browser. The table is held to the real folder by `test-touched-specs.ts`.
 */
export const DRIVEN_BY: ReadonlyArray<{ prefix: string; specs: readonly string[] }> = [
  // These five import `installChrome` from `@gantt/chrome`; the developer page installs it for every other spec.
  { prefix: "apps/chrome/", specs: ["bare-chart.spec.ts", "host-commands.spec.ts", "host-open-state.spec.ts", "two-charts.spec.ts", "webgl-switch.spec.ts"] },
  { prefix: "apps/chrome/src/keys-help.ts", specs: ["key-map.spec.ts"] }, // it reads `KEYS` from the source
  { prefix: "crates/engine/src/", specs: ["insert-delete.spec.ts", "links.spec.ts", "lookahead.spec.ts", "moved.spec.ts", "schedule-route.spec.ts"] },
  { prefix: "crates/import/src/", specs: ["open.spec.ts", "open-document.spec.ts", "import-notes.spec.ts", "ledger-timeline.spec.ts"] },
  { prefix: "crates/xer/src/", specs: ["open.spec.ts", "open-document.spec.ts", "import-notes.spec.ts", "ledger-timeline.spec.ts"] },
  { prefix: "crates/timeline/src/", specs: ["bar-look.spec.ts", "zoom-pan.spec.ts", "ledger-timeline.spec.ts"] },
  { prefix: "crates/app/src/", specs: ["startup.spec.ts", "data-api.spec.ts"] },
  // The document reader and writer: the adapter specs compare `toDocument()` with fixtures, so a format change must run them (gantt#1644).
  { prefix: "crates/app/src/document/", specs: ["adapter-dhtmlx.spec.ts", "adapter-bryntum.spec.ts"] },
  // The printed pages: J5 fires `beforeprint` and reads the pages and `#print-sheet`; print-pages.spec reads `print_pages`.
  { prefix: "crates/app/src/print/", specs: ["journey-j5.spec.ts", "print-pages.spec.ts"] },
  { prefix: `${WEB_ROOT}/src/print`, specs: ["journey-j5.spec.ts"] }, // a chart module that builds the sheet J5 reads: the name rule adds print-sheet.spec
  // Chart files that share their name with no spec (gantt#1163), each read from its importers and the page actions the specs make; one with no row here runs every spec.
  { prefix: `${WEB_ROOT}/src/gantt-object.ts`, specs: ["adapter-bryntum.spec.ts", "adapter-dhtmlx.spec.ts", "bare-chart.spec.ts", "commands.spec.ts", "data-api.spec.ts", "destroy.spec.ts", "events.spec.ts", "export.spec.ts", "host-commands.spec.ts", "host-open-state.spec.ts", "host-state.spec.ts", "journey-j5.spec.ts", "mount-json.spec.ts", "open-guard.spec.ts", "print-variance.spec.ts", "set-data-rows.spec.ts", "shared-gpu.spec.ts", "toolbar-api.spec.ts", "two-charts.spec.ts", "wasm-url.spec.ts", "webgl-switch.spec.ts", "webgl-switch-tasks.spec.ts"] }, // the `Gantt` object a host calls
  { prefix: `${WEB_ROOT}/src/table-row.ts`, specs: ["baseline-columns.spec.ts", "column-order.spec.ts", "id-column.spec.ts", "outline.spec.ts", "percent-cell-scrolled.spec.ts", "progress-toggle.spec.ts", "start-mark.spec.ts", "table-layout.spec.ts", "wbs-column.spec.ts"] }, // a row's cells
  { prefix: `${WEB_ROOT}/src/styles/table-cells.css`, specs: ["cell-edit-name.spec.ts", "outline.spec.ts", "start-hint.spec.ts", "start-mark.spec.ts", "table-edit.spec.ts", "table-roles.spec.ts"] }, // the chevron, the start mark, the cell editor
  { prefix: `${WEB_ROOT}/src/table-aria.ts`, specs: ["table-roles.spec.ts"] },
  { prefix: `${WEB_ROOT}/src/column-drag.ts`, specs: ["column-order.spec.ts"] }, // the head cells' drag
  { prefix: `${WEB_ROOT}/src/filter-names.ts`, specs: ["commands.spec.ts", "host-state.spec.ts", "resource-filter.spec.ts", "webgl-switch.spec.ts"] }, // `state().filter`, `setFilter("off")`
  { prefix: `${WEB_ROOT}/src/set-filter.ts`, specs: ["commands.spec.ts", "host-commands.spec.ts", "host-state.spec.ts", "resource-filter.spec.ts", "toolbar-api.spec.ts", "webgl-switch.spec.ts"] },
  { prefix: `${WEB_ROOT}/src/view-state.ts`, specs: ["bare-chart.spec.ts", "host-commands.spec.ts", "host-open-state.spec.ts", "host-state.spec.ts", "mount-json.spec.ts", "toolbar-api.spec.ts", "wasm-url.spec.ts", "webgl-switch.spec.ts", "webgl-switch-tasks.spec.ts"] }, // `state()`, `filterInputs()`, the `change` event
  { prefix: `${WEB_ROOT}/src/row-edit.ts`, specs: ["data-date.spec.ts", "indent.spec.ts", "insert-delete.spec.ts", "link-edit.spec.ts", "links.spec.ts", "move-row.spec.ts", "notice-placement.spec.ts", "paste.spec.ts", "save-failure.spec.ts", "save-name.spec.ts", "toolbar-api.spec.ts"] }, // the row commands and the alert line
  { prefix: `${WEB_ROOT}/src/edit-result.ts`, specs: ["host-commands.spec.ts", "layout-commands.spec.ts", "toolbar-api.spec.ts", "webgl-switch.spec.ts"] }, // a refused host command's reason
  { prefix: `${WEB_ROOT}/src/open-schedule.ts`, specs: ["events.spec.ts", "host-open-state.spec.ts", "import-notes.spec.ts", "open.spec.ts", "open-document.spec.ts", "open-guard.spec.ts", "open-unsaved.spec.ts", "picker.spec.ts", "save-open.spec.ts", "unsaved-mark.spec.ts"] }, // `open`: the size cap, the project list, the unsaved guard
  { prefix: `${WEB_ROOT}/src/explain.ts`, specs: ["host-commands.spec.ts"] }, // `commands.explain`
  { prefix: `${WEB_ROOT}/src/health.ts`, specs: ["host-commands.spec.ts"] }, // `commands.health`
  { prefix: `${WEB_ROOT}/src/propose.ts`, specs: ["host-commands.spec.ts"] }, // `commands.propose`, accept and reject
  { prefix: `${WEB_ROOT}/src/what-if.ts`, specs: ["host-commands.spec.ts", "webgl-switch.spec.ts"] }, // `commands.whatIf` and `whatIfLonger`; the switch spec reads their busy answer
];

/** The specs (of `specs`) that `file` drives besides the ones that share its name: `file` is outside the chart folder, or a chart module that a `DRIVEN_BY` row names. */
const drivenSpecs = (file: string, specs: string[]): string[] => {
  const named = file.startsWith("apps/chrome/src/") || file.startsWith("crates/app/src/") ? specs.filter((s) => sharesName(s, stem(file))) : [];
  const listed = DRIVEN_BY.filter((row) => file.startsWith(row.prefix)).flatMap((row) => row.specs);
  return specs.filter((s) => named.includes(s) || listed.includes(s));
};

/** The files that stage, check or publish the package. The `web` job's `checks` leg runs `pack-check` and `consumer-check` on a staged copy, so a change to one runs that job. */
const PUBLISH_PATH: readonly string[] = [
  "scripts/lib/pack-check.ts", "scripts/pack-check.ts", "scripts/test-pack-check.ts", "scripts/lib/stage-package.ts", "scripts/stage-package.ts",
  "scripts/test-stage-package.ts", "scripts/lib/consumer-check.ts", "scripts/consumer-check.ts", "scripts/test-consumer-check.ts", ".github/workflows/npm-publish.yml",
];

const HOSTED_ROOT = "apps/hosted";

export interface HostedPlan {
  /** none: only the `@smoke` tier runs, as before. some: it also runs `specs`. all: the whole hosted suite runs. */
  hosted: "none" | "some" | "all";
  /** Spec file names under `apps/hosted/tests/`, for some. */
  specs: string[];
  /** One line for the log. */
  why: string;
}

/**
 * Files under `apps/hosted/` that no browser spec can see change: a type declaration (the hosted `pnpm typecheck` step covers it),
 * markdown (no spec reads it), and a Node test (the `node --test` step runs it on every tier, and no spec imports it). They add no spec.
 */
const SPEC_FREE_HOSTED: Array<(file: string) => boolean> = [
  (f) => f.endsWith(".d.ts"),
  (f) => f.endsWith(".md"),
  (f) => f.startsWith(`${HOSTED_ROOT}/tests-node/`),
];

/** What the `hosted-browser` job adds to `@smoke` on a run. `specs` is every `*.spec.ts` file name in `apps/hosted/tests/`. */
export function hostedPlan(tier: Tier, changed: string[] | null, specs: string[]): HostedPlan {
  if (tier !== "fast") return { hosted: "none", specs: [], why: `the ${tier} tier runs every hosted spec already` };
  if (changed === null) return { hosted: "all", specs: [], why: "the diff against main could not be read" };
  const page = changed.find((f) => f === `${WEB_ROOT}/src/chart.html` || f.startsWith(`${WEB_ROOT}/src/styles/`));
  if (page !== undefined) return { hosted: "all", specs: [], why: `${page} is the chart's page or styles, which every hosted spec loads` };
  const touched = new Set<string>();
  for (const file of changed.filter((f) => f.startsWith(`${HOSTED_ROOT}/`))) {
    const name = file.slice(`${HOSTED_ROOT}/tests/`.length);
    if (SPEC_FREE_HOSTED.some((free) => free(file))) continue;
    if (file.startsWith(`${HOSTED_ROOT}/tests/`) && file.endsWith(SPEC_EXT) && !name.includes("/")) {
      if (specs.includes(name)) touched.add(name);
    } else if (file.startsWith(`${HOSTED_ROOT}/src/`)) {
      const hits = specs.filter((s) => sharesName(s, stem(file)));
      if (hits.length === 0) return { hosted: "all", specs: [], why: `${file} shares its name with no hosted spec` };
      for (const hit of hits) touched.add(hit);
    } else {
      return { hosted: "all", specs: [], why: `${file} is under ${HOSTED_ROOT}/ and is no spec or src module` };
    }
  }
  if (touched.size === 0) return { hosted: "none", specs: [], why: "the diff touches no hosted spec" };
  return { hosted: "some", specs: specs.filter((s) => touched.has(s)), why: `the diff reaches ${touched.size} hosted spec(s)` };
}

export interface BenchPlan {
  /** Whether the `hosted` job runs `demos/bench/drive.test.ts`. */
  bench: boolean;
  /** One line for the log. */
  why: string;
}

/** Whether a run owes the benchmark driver test: batch, nightly and deploy do, a PR push (fast) never does; a regression shows on the batch run, and is fixed forward if it only shows on deploy or nightly (gantt#1163). */
export function benchPlan(tier: Tier): BenchPlan {
  return tier === "fast" ? { bench: false, why: "the fast tier leaves the benchmark to batch, nightly and deploy" } : { bench: true, why: `the ${tier} tier runs it` };
}

const ENTRY = `${SRC_DIR}index.ts`;
/** The one spec a comment or type change to the entry file starts: it boots the chart through that file. */
const ENTRY_SPEC = "startup.spec.ts";

/** The entry file's text on the base and on the head; null when a side could not be read. */
export interface EntryTexts {
  base: string | null;
  head: string | null;
}

/** A TypeScript text without its types and comments; throws on a syntax error, or on a Node without the stripper. */
export const stripTypes = (text: string): string => {
  if (typeof nodeModule.stripTypeScriptTypes !== "function") throw new Error("this Node has no stripTypeScriptTypes");
  return nodeModule.stripTypeScriptTypes(text, { mode: "transform" });
};

/** Whether both texts hold the same runtime code. A missing text or a throwing `strip` says no, so a doubt runs every spec. */
export function sameRuntime(entry: EntryTexts | null, strip: (text: string) => string = stripTypes): boolean {
  if (entry === null || entry.base === null || entry.head === null) return false;
  try {
    return strip(entry.base) === strip(entry.head);
  } catch {
    return false;
  }
}

/**
 * The plan for a run. `changed` is the diff's file list against main, or on deploy the push's own (null when it could not be read);
 * `specs` is every `*.spec.ts` file name in `tests/`. `entry` is the entry file's text before and after, which `strip` reads (a test passes its own).
 */
export function webPlan(tier: Tier, changed: string[] | null, specs: string[], entry: EntryTexts | null = null, strip?: (text: string) => string): WebPlan {
  if (tier !== "fast" && tier !== "deploy") return { web: "all", specs: [], why: `the ${tier} tier runs every spec` };
  const prSpecs = specs.filter((s) => !BATCH_ONLY_SPECS.includes(s));
  const all = (why: string): WebPlan => ({ web: "all", specs: prSpecs, why });
  if (changed === null) return all("the diff against main could not be read");
  const touched = new Set<string>();
  let checksFile: string | null = null;
  for (const file of changed) {
    if (SPEC_FREE_WEB.some((free) => free(file))) continue;
    if (file === ENTRY && specs.includes(ENTRY_SPEC) && sameRuntime(entry, strip)) {
      touched.add(ENTRY_SPEC);
    } else if (file.startsWith(SPEC_DIR) && file.endsWith(SPEC_EXT) && !file.slice(SPEC_DIR.length).includes("/")) {
      const name = file.slice(SPEC_DIR.length);
      if (specs.includes(name)) touched.add(name);
    } else if (file.startsWith(SRC_DIR)) {
      const hits = [...specs.filter((s) => sharesName(s, stem(file))), ...drivenSpecs(file, specs)];
      if (hits.length === 0) return all(`${file} shares its name with no spec`);
      for (const hit of hits) touched.add(hit);
    } else if (file.startsWith(`${WEB_ROOT}/`)) {
      return all(`${file} is under ${WEB_ROOT}/ and is no spec or src module`);
    } else if (file.startsWith("examples/") || file.startsWith("demos/print/") || PUBLISH_PATH.includes(file)) {
      checksFile ??= file;
    } else {
      for (const hit of drivenSpecs(file, specs)) touched.add(hit);
    }
  }
  const run = prSpecs.filter((s) => touched.has(s));
  if (run.length === 0 && checksFile !== null) return { web: "checks", specs: [], why: `${checksFile} is read by the checks leg only (an example, the print demo, or the package's stage, check or publish files)` };
  if (run.length === 0) return { web: "none", specs: [], why: "the diff touches no browser spec that runs on a PR" };
  return { web: "some", specs: run, why: `the diff touches ${run.length} spec(s)` };
}

/** A test at the top of a file whose callback takes no fixture: `test("title", () =>` or `test('title', { tag }, async () =>`. */
const NO_FIXTURE_TEST = /^test(?:\.(?:skip|only|fixme))?\(\s*(["'])(.*?)\1\s*,[^\n]*?(?:async\s*)?\(\s*\)\s*=>/gm;
/** A hook at the top of a file that takes a fixture, so opens a page for every test in it. */
const PAGE_HOOK = /^test\.(?:beforeAll|beforeEach)\(\s*(?:async\s*)?\(\s*\{/m;

/**
 * The titles of the tests in a spec's source that never open a page (the plan's class `d`): top-level
 * tests whose callback takes no fixture, in a file with no top-level hook that takes one. A test inside a
 * `describe` is left out, since the describe may share a page from its own hook.
 */
export function noPageTests(source: string): string[] {
  if (PAGE_HOOK.test(source)) return [];
  return [...source.matchAll(NO_FIXTURE_TEST)].map((m) => m[2]);
}

/** A `--grep` pattern that matches exactly these titles' text. */
export const grepFor = (titles: string[]): string => titles.map((t) => t.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|");
