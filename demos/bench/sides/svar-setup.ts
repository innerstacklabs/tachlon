// Install SVAR Gantt (MIT) into a scratch directory outside any git work tree, with the page the
// driver opens. Nothing of SVAR's is in this repository or written into a work tree of ours.
//
//   node demos/bench/sides/svar-setup.ts <scratch-dir>
//
// It installs SVAR's React Gantt from the public npm registry at a pinned version (PACKAGES), with
// React at the version SVAR's own benchmark locks (https://github.com/svar-widgets/gantt-performance,
// `svar-react/package-lock.json`), and esbuild to build the page. Then the side's page goes beside
// it: `index.html` (the build's label, SVAR's stylesheet `all.css` as its readme imports it, and its
// icon font's stylesheet) and `page.js`, our `svar-page.js` bundled with React and SVAR as a
// production build (minified, with `process.env.NODE_ENV` "production", as a Vite build of SVAR's
// benchmark makes it). SVAR's stylesheet loads its text fonts from SVAR's CDN, and its theme its icon
// font: setup fetches each of those files from the CDN once, into `fonts/` here (each checked
// against its SHA-256 in FONT_SHA256, and refused if its path leaves the directory), and writes the
// stylesheet as `svar.css` with those URLs made local, so no request of a measured run leaves the box
// (the page turns the theme's own CDN links off). Write each cell's files with `svar-adapt.ts`, serve
// the directory and drive it:
//
//   node demos/bench/sides/svar-adapt.ts <dir>/dataset.json <scratch-dir>
//   node demos/bench/drive.ts --url 'http://127.0.0.1:<port>/?cell=<cell>' --probe demos/bench/sides/svar.ts --rows <n>
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

/** What is installed, exact versions: `npm view <name> dist-tags` read 2026-11-03 (React: SVAR's benchmark's lock). */
export const PACKAGES = {
  "@svar-ui/react-gantt": "2.8.2",
  react: "18.3.1",
  "react-dom": "18.3.1",
  esbuild: "0.28.2",
} as const;
export const GANTT = "@svar-ui/react-gantt";

const here = dirname(fileURLToPath(import.meta.url));

/**
 * How the table names this build. The MIT edition does not schedule, so a drag moves one task and
 * nothing after it, where ours and Bryntum's reschedule its successors: the label says so wherever
 * the table prints SVAR's numbers.
 */
export const buildLabel = () => `SVAR React Gantt ${PACKAGES[GANTT]} (MIT, no scheduling)`;

/** Where SVAR's stylesheet and theme load their fonts from, and the local directory that replaces it. */
export const CDN = "https://cdn.svar.dev/";
/** The icon font's stylesheet, which SVAR's theme links from the CDN. */
export const ICONS_CSS = "fonts/wxi/wx-icons.css";

/** SVAR's stylesheet with every CDN font made a local path, and those paths, each once, in order. */
export function localCss(css: string): { css: string; files: string[] } {
  const files = [...new Set([...css.matchAll(/https:\/\/cdn\.svar\.dev\/([^)"'\s?#]+)/g)].map((m) => m[1]))];
  return { css: css.replaceAll(CDN, ""), files };
}

/** The font files the icon stylesheet names (`url("./wx-icons.woff?…")`), as paths beside it. */
export function iconFiles(css: string): string[] {
  const dir = ICONS_CSS.slice(0, ICONS_CSS.lastIndexOf("/") + 1);
  return [...new Set([...css.matchAll(/url\("?\.\/([^)"?#]+)/g)].map((m) => dir + m[1]))];
}

export const USAGE = "usage: node svar-setup.ts <scratch-dir>";

/** The scratch directory, or the usage when the arguments are not one directory. */
export function parseArgs(argv: string[]): { scratch: string } | string {
  if (argv.length !== 1 || argv[0].startsWith("--")) return USAGE;
  return { scratch: resolve(argv[0]) };
}

/** The scratch package: every package at its exact version. */
export function packageJson(): string {
  const pkg = { name: "bench-side-svar", private: true, type: "module", dependencies: { ...PACKAGES } };
  return `${JSON.stringify(pkg, null, 2)}\n`;
}

/** The page: the build it runs (for the side's `build` hook), SVAR's stylesheet, then our bundled page script. */
export function indexHtml(build: string): string {
  return `<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="bench-build" content="${build}">
<title>bench side</title>
<link rel="stylesheet" href="svar.css">
<link rel="stylesheet" href="${ICONS_CSS}">
<style>html, body, #root { margin: 0; height: 100%; } .wx-theme { height: 100%; }</style>
</head>
<body>
<div id="root"></div>
<script type="module" src="page.js"></script>
</body>
</html>
`;
}

/** esbuild's arguments for the production bundle of `page.src.js` into `page.js`. */
export const BUNDLE_ARGS = ["page.src.js", "--bundle", "--minify", "--format=esm", "--define:process.env.NODE_ENV=\"production\"", "--outfile=page.js"];

/**
 * The SHA-256 of each font file setup fetches from SVAR's CDN, read 2026-11-03 for 2.8.2: its URLs
 * carry no version, so a file with other bytes is refused rather than laid out under the same label.
 */
export const FONT_SHA256: Record<string, string> = {
  "fonts/open-sans/500.woff": "2350f5f527285184875644df88c6a7b6d6b4a353871462d81daeb29e9fad405c",
  "fonts/open-sans/500.woff2": "282dba9108484c7b13ad1728fa61f94a84516a96f391b0cd306e3c8468146e20",
  "fonts/open-sans/600.woff": "4b2b931b9a7e37034b1fe13bd94809420b5e4d16650b4021dfe9110dc1fe87fb",
  "fonts/open-sans/600.woff2": "922297c4e207dae08affdbc21f34329d67f4ed482f5bdecb2369381a3b5eab66",
  "fonts/open-sans/700.woff": "d195af8437cd075c6123f2fc577fcdf511a2e3fdfb1dea664e76f34f7435ce9e",
  "fonts/open-sans/700.woff2": "ed01287cc7c91ca39383bc12e449fd8042e8a119fd6864639a64566cd1328d59",
  "fonts/open-sans/regular.woff": "d80d0b81e30c24d86e1d11053982b67b639ebf6acd11b9fce5aaa14aaf38cc97",
  "fonts/open-sans/regular.woff2": "58a886cc7d1d22c51bf05e8426072841a6d7894cdbda30379273a09026346f21",
  "fonts/roboto/500.woff": "d6dbc44a1a982409179b0192e186eb43b0fb3be6e199c149684a767531bd996c",
  "fonts/roboto/500.woff2": "f684d0da0d6fe90b95711213a5bc9431580b0ebecb864f8f8de30ad70eb64559",
  "fonts/roboto/regular.woff": "16b397dac92975cc17209d59eca3cdc1bf585b87df8819f92a266c8cfd2c1675",
  "fonts/roboto/regular.woff2": "8c445c283da9a171eb4841f32cb343627ef480ed8c80c6dca83ef8287687e62f",
  "fonts/wxi/wx-icons.css": "8e9024d5a76e7117ed92a2724d8a2f0daed7547db8c12b4dd93e2e009d21e361",
  "fonts/wxi/wx-icons.woff": "61bba7d0c179a28da14c97199cb8550648b03d4ef425cc6bdb770fa5280e666a",
  "fonts/wxi/wx-icons.woff2": "1caf83b1a4f747d44613c77d0e5ac4c67722280352805fae6f603fb1486d1349",
};

/** What setup fetches with (`fetch`; a test passes a fake CDN). */
export type Fetcher = (url: string) => Promise<{ ok: boolean; status: number; arrayBuffer(): Promise<ArrayBuffer> }>;

/**
 * Fetch `path` from SVAR's CDN into `scratch`; its bytes. The path comes from a stylesheet fetched
 * from the CDN, so one that resolves outside `scratch` is refused before anything is fetched, and
 * bytes whose SHA-256 is not the one pinned for the path (or a path with no pin) are refused before
 * anything is written.
 */
async function fetchFont(scratch: string, path: string, get: Fetcher, pins: Record<string, string>): Promise<Buffer> {
  const root = resolve(scratch);
  const target = resolve(root, path);
  if (!target.startsWith(root + sep)) throw new Error(`refusing to write ${path}: outside the scratch directory ${root}`);
  const want = pins[path];
  if (want === undefined) throw new Error(`${path} is not a pinned font file`);
  const res = await get(CDN + path);
  if (!res.ok) throw new Error(`${CDN}${path}: HTTP ${res.status}`);
  const bytes = Buffer.from(await res.arrayBuffer());
  const got = createHash("sha256").update(bytes).digest("hex");
  if (got !== want) throw new Error(`${CDN}${path} has SHA-256 ${got}, not the pinned ${want}`);
  mkdirSync(dirname(target), { recursive: true });
  writeFileSync(target, bytes);
  return bytes;
}

/** SVAR's fonts and icons into `scratch/fonts/`, each checked against its pin, and its stylesheet as `svar.css` with them local. */
export async function localFonts(scratch: string, get: Fetcher = fetch, pins: Record<string, string> = FONT_SHA256): Promise<void> {
  const { css, files } = localCss(readFileSync(join(scratch, "node_modules", GANTT, "dist-full", "index.css"), "utf8"));
  const icons = (await fetchFont(scratch, ICONS_CSS, get, pins)).toString("utf8");
  for (const f of [...files, ...iconFiles(icons)]) await fetchFont(scratch, f, get, pins);
  writeFileSync(join(scratch, "svar.css"), css);
  for (const [name, text] of [["svar.css", css], [ICONS_CSS, icons]]) {
    if (/https?:\/\//.test(text.replace(/http:\/\/www\.w3\.org\/[^)"'\s]*/g, ""))) throw new Error(`${name} still names a remote URL`);
  }
  console.log(`${files.length + iconFiles(icons).length + 1} font files from ${CDN} into ${join(scratch, "fonts")}, each its pinned SHA-256`);
}

/** Install into `scratch`. Throws with the reason when it is inside a work tree, not empty, or the install fails. */
export async function setup(scratch: string): Promise<void> {
  assertOutsideWorkTree(scratch, "SVAR code");
  if (existsSync(scratch) && readdirSync(scratch).length > 0) throw new Error(`${scratch} is not empty; use a new directory`);
  mkdirSync(scratch, { recursive: true });
  writeFileSync(join(scratch, "package.json"), packageJson());
  console.log(`$ npm install --save-exact --no-audit --no-fund  (in ${scratch})`);
  execFileSync("npm", ["install", "--save-exact", "--no-audit", "--no-fund"], { cwd: scratch, stdio: "inherit" });
  for (const [name, version] of Object.entries(PACKAGES)) {
    const got = JSON.parse(readFileSync(join(scratch, "node_modules", name, "package.json"), "utf8")).version;
    if (got !== version) throw new Error(`installed ${name}@${got}, not ${version}`);
  }
  await localFonts(scratch);
  copyFileSync(join(here, "svar-page.js"), join(scratch, "page.src.js"));
  execFileSync(join(scratch, "node_modules", ".bin", "esbuild"), BUNDLE_ARGS, { cwd: scratch, stdio: "inherit" });
  writeFileSync(join(scratch, "index.html"), indexHtml(buildLabel()));
  console.log(`\n${GANTT}@${PACKAGES[GANTT]} ready at ${scratch}. It holds SVAR's code: do not copy it into a repo.`);
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
    console.error(`svar-setup: ${e instanceof Error ? e.message : String(e)}`);
    return EXIT_FAILED;
  }
}

if (process.argv[1] === fileURLToPath(import.meta.url)) process.exitCode = await main(process.argv.slice(2));
