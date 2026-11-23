// Fails when a package's `exports` names a file its packed tarball leaves out, or the tarball lacks the engine file, the size recorded for it or the fonts' licence (the library build writes it beside the script, which loads it by a URL relative to itself) or a chapter of the user guide (`dist-lib/guide/`: the `.md` files of packages/gantt/guide/), or `dist-lib/guide/` holds anything but a `.md` file and its LICENSE, or a packed `.d.ts` imports from the generated `wasm/` folder, or a packed `.css` has a `url()` that names no file in the tarball, or it holds a file outside dist-lib/, README.md, AGENTS.md and package.json, or the package names no `bin` or one whose target the tarball lacks or that has no `#!` first line.
//   node scripts/pack-check.ts <package folder> [--published]
// `--published` also fails a `package.json` that holds `scripts`, `devDependencies` or `packageManager`: the folder is the one `npm publish` runs on (scripts/stage-package.ts writes it), not the source folder.
import { readdirSync } from "node:fs";
import { binProblems, cssUrlsOutsidePack, declarationsNamingWasm, manifestProblems, missingExpected, missingFromPack, packedFiles, strayFiles } from "./lib/pack-check.ts";

// The user guide's chapters: the `.md` files of packages/gantt/guide/, which scripts/emit-guide.ts copies. The folder's README describes the file format and lists no chapter, so the folder is the index.
const GUIDE = readdirSync(new URL("../packages/gantt/guide", import.meta.url)).filter((n) => n.endsWith(".md")).map((n) => `dist-lib/guide/${n}`);
// dist-lib/guide/ holds Markdown and the guide's LICENSE, nothing else.
const guideStrays = (dir: string): string[] => [...packedFiles(dir)].filter((f) => f.startsWith("dist-lib/guide/") && !/^dist-lib\/guide\/([^/]+\.md|LICENSE)$/.test(f)).sort();

// Files the tarball must hold that no `exports` entry names.
const EXPECTED = ["README.md", "LICENSE", "AGENTS.md", "dist-lib/NOTICE.md", "dist-lib/OFL.txt", "dist-lib/app_bg.wasm", "dist-lib/engine-size.json", "dist-lib/schema.json", "dist-lib/describe.json", "dist-lib/fit.md", "dist-lib/guide/LICENSE", ...GUIDE];

// A publish ships the built library, the package's README, LICENSE and package.json, and nothing else (the npm publish workflow runs this check first).
const ALLOWED_PREFIXES = ["dist-lib/"];
const ALLOWED_NAMES = ["README.md", "LICENSE", "AGENTS.md", "package.json"];

const args = process.argv.slice(2);
const published = args.includes("--published");
const dir = args.find((a) => !a.startsWith("--"));
if (!dir) {
  console.error("usage: node scripts/pack-check.ts <package folder> [--published]");
  process.exit(2);
}
try {
  const missing = missingFromPack(dir);
  for (const t of missing) console.error(`pack-check: FAIL ${dir}: exports names ${t}, the tarball lacks it`);
  const lacking = missingExpected(dir, EXPECTED);
  for (const f of lacking) console.error(`pack-check: FAIL ${dir}: the tarball lacks ${f}`);
  const stray = strayFiles(dir, ALLOWED_PREFIXES, ALLOWED_NAMES);
  for (const f of stray) console.error(`pack-check: FAIL ${dir}: the tarball holds ${f}, which is not under dist-lib/ and is not README.md, LICENSE, AGENTS.md or package.json`);
  const guideStray = guideStrays(dir);
  for (const f of guideStray) console.error(`pack-check: FAIL ${dir}: the tarball holds ${f}, and dist-lib/guide/ holds only Markdown files and its LICENSE`);
  const wasm = declarationsNamingWasm(dir);
  for (const f of wasm) console.error(`pack-check: FAIL ${dir}: ${f} imports from the generated wasm folder, which the tarball lacks`);
  const bins = binProblems(dir);
  for (const b of bins) console.error(`pack-check: FAIL ${dir}: bin ${b}`);
  const urls = cssUrlsOutsidePack(dir);
  for (const u of urls) console.error(`pack-check: FAIL ${dir}: url() ${u} names no file in the tarball`);
  const manifest = published ? manifestProblems(dir) : [];
  for (const m of manifest) console.error(`pack-check: FAIL ${dir}: ${m}`);
  if (manifest.length || missing.length || lacking.length || stray.length || guideStray.length || wasm.length || bins.length || urls.length) process.exit(1);
  console.log(`pack-check: PASS ${dir}`);
} catch (e) {
  console.error(`pack-check: ${(e as Error).message}`);
  process.exit(2);
}
