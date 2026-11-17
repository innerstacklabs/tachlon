// Install the Bryntum Gantt trial into a scratch directory outside any git work tree, with the page
// the driver opens. Run by the reader, for the reader's own trial: nothing of Bryntum's is in this
// repository or written into a work tree of ours.
//
//   node demos/bench/sides/bryntum-setup.ts --trial <scratch-dir>
//
// `--trial` is the reader's own statement that they start a Bryntum trial under Bryntum's EULA
// (EULA_URL; its trial grant is for internal evaluation only). Without it the setup refuses and
// installs nothing. The trial is Bryntum's public npm package at a pinned version (TRIAL), installed
// as Bryntum's install guide says (aliased to `@bryntum/gantt`, exact version, its own install
// script run). Then the side's page goes beside it: `index.html` (the build's label, Bryntum's CSS as
// its examples load it) and `page.js` (`bryntum-page.js` of this folder, our code). Check the schedule
// and write both variants' data into the same directory with `bryntum-dates.ts`, serve the directory
// and drive it (`?dates` for the variant with the dates given):
//
//   node demos/bench/sides/bryntum-dates.ts <dir>/dataset.json <scratch-dir>
//   node demos/bench/drive.ts --url http://127.0.0.1:<port>/ --probe demos/bench/sides/bryntum.ts --rows <n>
//
// `--community <scratch-dir>` installs Bryntum Gantt 8's Community tier instead (`@bryntum/gantt-community`,
// MIT, public npm: no trial, so no `--trial` statement) and bundles the page with esbuild, as an app
// would ship it. Its engine has no working calendar and no constraints (adapt with `bryntum-adapt.ts --community`).
//
// Exit code: 0 installed; 1 the install failed or the directory is not usable; 64 usage or no `--trial`.

import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { assertOutsideWorkTree } from "../../dhtmlx-compare/setup-harness.ts";

export const EXIT_FAILED = 1;
export const EXIT_USAGE = 64;

/** The trial package and the version measured: `npm view @bryntum/gantt-trial dist-tags` read 2026-11-02. */
export const TRIAL = { name: "@bryntum/gantt-trial", version: "7.3.8", dateRead: "2026-11-02" } as const;
/** The name Bryntum's install guide aliases the trial to, so the page imports what a licensed app imports. */
export const ALIAS = "@bryntum/gantt";
/** Bryntum Gantt 8's Community tier (MIT) and the bundler that makes its page: `npm view @bryntum/gantt-community version` read 2026-11-13. */
export const COMMUNITY = { name: "@bryntum/gantt-community", version: "8.0.1", esbuild: "0.28.2" } as const;
export const EULA_URL = "https://bryntum.com/products/license/";

const here = dirname(fileURLToPath(import.meta.url));

/** How the table names this build: Bryntum's version and that it is the trial. */
export const buildLabel = (community = false) =>
  community ? `Bryntum ${COMMUNITY.version} Community (MIT), no working calendar, no constraints` : `Bryntum ${TRIAL.version} trial build`;

export const USAGE = "usage: node bryntum-setup.ts --trial|--community <scratch-dir>";

/** The scratch directory, or why the arguments are not usable (no `--trial` among them). */
export function parseArgs(argv: string[]): { scratch: string; community?: true } | string {
  if (argv[0] === "--community") return argv.length === 2 && !argv[1].startsWith("--") ? { scratch: resolve(argv[1]), community: true } : USAGE;
  const rest = argv.filter((a) => a !== "--trial");
  if (rest.length === argv.length) {
    return `refusing to install without --trial: the Bryntum trial is the reader's own, under Bryntum's EULA (${EULA_URL}), for internal evaluation`;
  }
  if (argv.length - rest.length > 1 || rest.length !== 1 || rest[0].startsWith("--")) return USAGE;
  return { scratch: resolve(rest[0]) };
}

/** The scratch package: the trial under its alias, exact version. */
export function packageJson(community = false): string {
  const dependencies = community ? { [COMMUNITY.name]: COMMUNITY.version, esbuild: COMMUNITY.esbuild } : { [ALIAS]: `npm:${TRIAL.name}@${TRIAL.version}` };
  const pkg = { name: "bench-side-bryntum", private: true, type: "module", dependencies };
  return `${JSON.stringify(pkg, null, 2)}\n`;
}

/** The page: the build it runs (for the side's `build` hook), Bryntum's icon font, its structural CSS and one theme, as its examples load them; then our page script. */
export function indexHtml(build: string, community = false): string {
  const dir = (f: string) => `node_modules/${f.startsWith("fontawesome") && community ? "@bryntum/core-community" : community ? COMMUNITY.name : ALIAS}/${f}`;
  const css = ["fontawesome/css/fontawesome.css", "fontawesome/css/solid.css", "gantt.css", "svalbard-light.css"]
    .map((f) => `<link rel="stylesheet" href="${dir(f)}">`)
    .join("\n");
  // The page imports `@bryntum/gantt`: the trial's module file through an import map, the Community tier through the bundle.
  const importMap = community ? "" : `<script type="importmap">{"imports": {"${ALIAS}": "./node_modules/${ALIAS}/gantt.module.js"}}</script>\n`;
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="bench-build" content="${build}">
<title>bench side</title>
${css}
<style>html, body { margin: 0; height: 100%; } #container { position: absolute; inset: 0; }</style>
</head>
<body>
<div id="container"></div>
${importMap}<script type="module" src="page.js"></script>
</body>
</html>
`;
}

/** Install into `scratch`. Throws with the reason when it is inside a work tree, not empty, or the install fails. */
export function setup(scratch: string, community = false): void {
  assertOutsideWorkTree(scratch, "Bryntum code");
  if (existsSync(scratch) && readdirSync(scratch).length > 0) throw new Error(`${scratch} is not empty; use a new directory`);
  mkdirSync(scratch, { recursive: true });
  writeFileSync(join(scratch, "package.json"), packageJson(community));
  console.log(`$ npm install --save-exact --no-audit --no-fund  (in ${scratch})`);
  execFileSync("npm", ["install", "--save-exact", "--no-audit", "--no-fund"], { cwd: scratch, stdio: "inherit" });
  const want = community ? COMMUNITY : TRIAL;
  const installed = JSON.parse(readFileSync(join(scratch, "node_modules", community ? COMMUNITY.name : ALIAS, "package.json"), "utf8"));
  if (installed.name !== want.name || installed.version !== want.version) {
    throw new Error(`installed ${installed.name}@${installed.version}, not ${want.name}@${want.version}`);
  }
  if (community && installed.license !== "MIT") throw new Error(`installed ${installed.name} is licensed ${installed.license}, not MIT`);
  writeFileSync(join(scratch, "index.html"), indexHtml(buildLabel(community), community));
  if (!community) copyFileSync(join(here, "bryntum-page.js"), join(scratch, "page.js"));
  else {
    // The minified bundle of our page and the Community tier's modules, one file, as an app would ship it.
    const args = ["esbuild", join(here, "bryntum-page.js"), "--bundle", "--minify", "--format=esm", `--alias:${ALIAS}=${COMMUNITY.name}`, `--outfile=${join(scratch, "page.js")}`, "--log-level=warning"];
    execFileSync("npx", ["--no-install", ...args], { cwd: scratch, stdio: "inherit" });
  }
  console.log(`\n${want.name}@${want.version} ready at ${scratch}. It holds Bryntum code: do not copy it into a repo.`);
}

function main(argv: string[]): number {
  const args = parseArgs(argv);
  if (typeof args === "string") {
    console.error(args === USAGE ? USAGE : `${args}\n${USAGE}`);
    return EXIT_USAGE;
  }
  try {
    setup(args.scratch, args.community === true);
    return 0;
  } catch (e) {
    console.error(`bryntum-setup: ${e instanceof Error ? e.message : String(e)}`);
    return EXIT_FAILED;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = main(process.argv.slice(2));
