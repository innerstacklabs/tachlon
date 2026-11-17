// Install DHTMLX Gantt's community edition (MIT) into a scratch directory outside any git work tree,
// with the page the driver opens. Nothing of DHTMLX's is in this repository or written into a work
// tree of ours.
//
//   node demos/bench/sides/dhtmlx-setup.ts <scratch-dir>
//
// It installs `dhtmlx-gantt` from the public npm registry at a pinned version (PACKAGES), as its
// readme's "Quick start" says (`npm install dhtmlx-gantt`), and loads it as that readme's plain
// `<script>` tags do: `codebase/dhtmlxgantt.js` (the published, minified build, which sets the
// global `gantt`) and `codebase/dhtmlxgantt.css`. That stylesheet loads its Inter font from Google
// Fonts (`fonts.gstatic.com`): setup fetches each of those files once into `fonts/` here (each
// checked against its SHA-256 in FONT_SHA256, and refused if its path leaves the directory) and
// writes the stylesheet as `dhtmlx.css` with those URLs made local, so no request of a measured run
// leaves the box. Then the side's page goes beside it: `index.html` (the build's label, `dhtmlx.css`,
// DHTMLX's script, then ours) and `page.js` (`dhtmlx-page.js`, our code). Write each cell's files
// with `dhtmlx-adapt.ts`, serve the directory and drive it:
//
//   node demos/bench/sides/dhtmlx-adapt.ts <dir>/dataset.json <scratch-dir>/cells/<cell>
//   node demos/bench/drive.ts --url 'http://127.0.0.1:<port>/?cell=<cell>' --probe demos/bench/sides/dhtmlx.ts --rows <n>
//
// Exit code: 0 installed; 1 the install failed or the directory is not usable; 64 usage.

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { assertOutsideWorkTree } from "../../dhtmlx-compare/setup-harness.ts";

export const EXIT_FAILED = 1;
export const EXIT_USAGE = 64;

/** What is installed, exact version: `npm view dhtmlx-gantt dist-tags` read 2026-11-03 (`latest`, its license MIT). */
export const PACKAGES = { "dhtmlx-gantt": "10.0.3" } as const;
export const GANTT = "dhtmlx-gantt";

const here = dirname(fileURLToPath(import.meta.url));

/**
 * How the table names this build. The community edition does not schedule (a drag moves one task
 * and nothing after it, where ours and Bryntum's reschedule its successors) and has no working-time
 * calendars (it is handed both dates of every task): the label says so wherever the table prints
 * DHTMLX's numbers.
 */
export const buildLabel = () => `DHTMLX Gantt ${PACKAGES[GANTT]} (MIT, no scheduling, no working-time calendars)`;

/** Where DHTMLX's stylesheet loads its font from, and the local directory that replaces it. */
export const FONT_HOST = "https://fonts.gstatic.com/s/";
export const FONT_DIR = "fonts/";

/** DHTMLX's stylesheet with every Google Fonts URL made a local path, and those paths, each once, in order. */
export function localCss(css: string): { css: string; files: string[] } {
  const files = [...new Set([...css.matchAll(/https:\/\/fonts\.gstatic\.com\/s\/([^)"'\s?#]+)/g)].map((m) => FONT_DIR + m[1]))];
  return { css: css.replaceAll(FONT_HOST, FONT_DIR), files };
}

export const USAGE = "usage: node dhtmlx-setup.ts <scratch-dir>";

/** The scratch directory, or the usage when the arguments are not one directory. */
export function parseArgs(argv: string[]): { scratch: string } | string {
  if (argv.length !== 1 || argv[0].startsWith("--")) return USAGE;
  return { scratch: resolve(argv[0]) };
}

/** The scratch package: every package at its exact version. */
export function packageJson(): string {
  const pkg = { name: "bench-side-dhtmlx", private: true, dependencies: { ...PACKAGES } };
  return `${JSON.stringify(pkg, null, 2)}\n`;
}

/** The library's script, as the readme's `<script>` tag loads it, from the installed package. */
export const LIBRARY = `node_modules/${GANTT}/codebase/dhtmlxgantt.js`;

/** The page: the build it runs (for the side's `build` hook), the stylesheet, the container, DHTMLX's script, then ours. */
export function indexHtml(build: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="bench-build" content="${build}">
<title>bench side</title>
<link rel="stylesheet" href="dhtmlx.css">
<style>html, body, #gantt_here { margin: 0; width: 100%; height: 100%; }</style>
</head>
<body>
<div id="gantt_here"></div>
<script src="${LIBRARY}"></script>
<script src="page.js"></script>
</body>
</html>
`;
}

/**
 * The SHA-256 of each font file setup fetches from Google Fonts for 10.0.3's stylesheet (Inter
 * v20, weights 300 to 800), read 2026-11-03: a file with other bytes is refused rather than laid out
 * under the same label.
 */
export const FONT_SHA256: Record<string, string> = {
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuDyYMZg.ttf": "eec66af7f2337bd34fe6e801cf92ededcb57a20c0d7bc40a61d4eefcbe3dd40c",
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuFuYMZg.ttf": "b37284b5701b6b168dfc770aa1a4ac492106422fd3ba76bc7641e37434e8019c",
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuGKYMZg.ttf": "e7a1aaf7eda9f2fad4131725fa556265ec75ca7b2d756260173a040363e8d4f7",
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuI6fMZg.ttf": "8c883f63b2c4157d997319f2c8bc6995ed4357ef371940d31ca159004a4aae63",
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuLyfMZg.ttf": "1b08e7fc267a5c7e1d614100f604b83e7e8a0be241f0f288faa2b3ac93a683ba",
  "fonts/inter/v20/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuOKfMZg.ttf": "d0f4bc7faca468376e3db9b5e57afcdc2192134c9ac82a9511f32767b56853a4",
};

/** What setup fetches with (`fetch`; a test passes a fake host). */
export type Fetcher = (url: string) => Promise<{ ok: boolean; status: number; arrayBuffer(): Promise<ArrayBuffer> }>;

/**
 * Fetch the font at local `path` into `scratch`. A path that resolves outside `scratch` is refused
 * before anything is fetched, and bytes whose SHA-256 is not the one pinned for the path (or a path
 * with no pin) are refused before anything is written.
 */
async function fetchFont(scratch: string, path: string, get: Fetcher, pins: Record<string, string>): Promise<void> {
  const root = resolve(scratch);
  const target = resolve(root, path);
  if (!target.startsWith(root + sep)) throw new Error(`refusing to write ${path}: outside the scratch directory ${root}`);
  const want = pins[path];
  if (want === undefined) throw new Error(`${path} is not a pinned font file`);
  const url = FONT_HOST + path.slice(FONT_DIR.length);
  const res = await get(url);
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const got = createHash("sha256").update(bytes).digest("hex");
  if (got !== want) throw new Error(`${url} has SHA-256 ${got}, not the pinned ${want}`);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, bytes);
}

/** DHTMLX's fonts into `scratch/fonts/`, each checked against its pin, and its stylesheet as `dhtmlx.css` with them local. */
export async function localFonts(scratch: string, get: Fetcher = fetch, pins: Record<string, string> = FONT_SHA256): Promise<void> {
  const { css, files } = localCss(readFileSync(join(scratch, "node_modules", GANTT, "codebase", "dhtmlxgantt.css"), "utf8"));
  for (const f of files) await fetchFont(scratch, f, get, pins);
  if (/https?:\/\//.test(css.replace(/http:\/\/www\.w3\.org\/[^)"'\s]*/g, ""))) throw new Error("dhtmlx.css still names a remote URL");
  writeFileSync(join(scratch, "dhtmlx.css"), css);
  console.log(`${files.length} font files from ${FONT_HOST} into ${join(scratch, FONT_DIR)}, each its pinned SHA-256`);
}

/** Install into `scratch`. Throws with the reason when it is inside a work tree, not empty, or the install fails. */
export async function setup(scratch: string): Promise<void> {
  assertOutsideWorkTree(scratch, "DHTMLX code");
  if (existsSync(scratch) && readdirSync(scratch).length > 0) throw new Error(`${scratch} is not empty; use a new directory`);
  mkdirSync(scratch, { recursive: true });
  writeFileSync(join(scratch, "package.json"), packageJson());
  console.log(`$ npm install --save-exact --no-audit --no-fund  (in ${scratch})`);
  execFileSync("npm", ["install", "--save-exact", "--no-audit", "--no-fund"], { cwd: scratch, stdio: "inherit" });
  for (const [name, version] of Object.entries(PACKAGES)) {
    const pkg = JSON.parse(readFileSync(join(scratch, "node_modules", name, "package.json"), "utf8"));
    if (pkg.version !== version) throw new Error(`installed ${name}@${pkg.version}, not ${version}`);
    if (pkg.license !== "MIT") throw new Error(`installed ${name} is licensed ${pkg.license}, not MIT`);
  }
  await localFonts(scratch);
  copyFileSync(join(here, "dhtmlx-page.js"), join(scratch, "page.js"));
  writeFileSync(join(scratch, "index.html"), indexHtml(buildLabel()));
  console.log(`\n${GANTT}@${PACKAGES[GANTT]} ready at ${scratch}. It holds DHTMLX's code: do not copy it into a repo.`);
}

async function main(argv: string[]): Promise<number> {
  const args = parseArgs(argv);
  if (typeof args === "string") {
    console.error(args);
    return EXIT_USAGE;
  }
  try {
    await setup(args.scratch);
    return 0;
  } catch (e) {
    console.error(`dhtmlx-setup: ${e instanceof Error ? e.message : String(e)}`);
    return EXIT_FAILED;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
