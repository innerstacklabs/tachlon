// Checks the pack check (see scripts/lib/pack-check.ts) on temporary fixture packages. Uses the real `npm pack --dry-run`, which runs offline.
//   node scripts/test-pack-check.ts
// Exits 1, one FAIL line per problem.
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { binProblems, cssUrlsOutsidePack, declarationsNamingWasm, exportTargets, manifestProblems, missingExpected, missingFromPack, strayFiles } from "./lib/pack-check.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-pack-check: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

const root = mkdtempSync(join(tmpdir(), "test-pack-check-"));
// A package whose `files` ships `shipped` and whose files on disk are `disk`.
function fixture(name: string, exp: unknown, shipped: string[], disk: string[]): string {
  const dir = join(root, name);
  for (const f of disk) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    writeFileSync(join(dir, f), "export {};\n");
  }
  writeFileSync(join(dir, "package.json"), JSON.stringify({ name, version: "1.0.0", exports: exp, files: shipped }));
  return dir;
}

try {
  // Every target present: PASS.
  check("all present", missingFromPack(fixture("ok", { ".": "./dist/index.js", "./x": { import: "./dist/x.js" } }, ["dist"], ["dist/index.js", "dist/x.js"])), []);
  // A target the `files` list leaves out, and one that does not exist on disk: both named.
  check("left out", missingFromPack(fixture("bad", { ".": "./dist/index.js", "./x": "./src/x.js" }, ["dist"], ["dist/index.js", "src/x.js"])), ["./src/x.js"]);
  check("not on disk", missingFromPack(fixture("gone", { ".": ["./a.js", "./b.js"] }, ["a.js"], ["a.js"])), ["./b.js"]);
  // A packed `.d.ts` that imports from the generated wasm folder (the tarball lacks it) is named; one that only says "wasm" in a comment is not.
  const typed = fixture("typed", { ".": "./dist/index.d.ts" }, ["dist"], ["dist/index.d.ts", "dist/api.d.ts"]);
  writeFileSync(join(typed, "dist/index.d.ts"), 'import type { App } from "./wasm/app.js";\n');
  writeFileSync(join(typed, "dist/api.d.ts"), "// the wasm module is not named here\nexport {};\n");
  check("wasm import", declarationsNamingWasm(typed), ["dist/index.d.ts"]);
  // A packed stylesheet whose url() inlines a font as a data URL, or names a font the tarball lacks, is named; one whose url() names a packed file is not.
  const css = fixture("css", { ".": "./dist/a.css" }, ["dist"], ["dist/a.css", "dist/f.woff2"]);
  writeFileSync(join(css, "dist/a.css"), '@font-face{src:url("./f.woff2")}a{b:url(#x)}\n');
  check("css url packed", cssUrlsOutsidePack(css), []);
  writeFileSync(join(css, "dist/a.css"), "@font-face{src:url(data:font/woff2;base64,AAAA)}@font-face{src:url(./gone.woff2)}\n");
  check("css url outside", cssUrlsOutsidePack(css), ["dist/a.css: ./gone.woff2", "dist/a.css: data:font/woff2;base64,AAAA"]);
  // A file the package must ship although no export names it (the engine, which the script loads by a URL): named when the tarball lacks it, silent once it is packed.
  const engine = fixture("engine", { ".": "./dist/a.js" }, ["dist"], ["dist/a.js"]);
  check("engine lacking", missingExpected(engine, ["dist/app_bg.wasm"]), ["dist/app_bg.wasm"]);
  writeFileSync(join(engine, "dist/app_bg.wasm"), "\0asm");
  check("engine packed", missingExpected(engine, ["dist/app_bg.wasm"]), []);
  // A package must name a `bin`, each target must be packed and start with a shebang (npm links it as a command): the problem is named, silent once fixed.
  const cli = fixture("bin", { ".": "./dist/a.js" }, ["dist"], ["dist/a.js", "dist/cli.js"]);
  const setBin = (bin?: unknown) => writeFileSync(join(cli, "package.json"), JSON.stringify({ name: "bin", version: "1.0.0", exports: { ".": "./dist/a.js" }, files: ["dist"], bin }));
  check("no bin", binProblems(cli), ["package.json has no bin"]);
  setBin({ x: "./dist/gone.js" });
  check("bin missing", binProblems(cli), ["./dist/gone.js: the tarball lacks it"]);
  setBin("./dist/cli.js");
  check("bin without shebang", binProblems(cli), ["./dist/cli.js: the first line is not a #! line"]);
  writeFileSync(join(cli, "dist/cli.js"), "#!/usr/bin/env node\n");
  check("bin ok", binProblems(cli), []);
  // The fonts' licence ships beside them: the real script (its EXPECTED list) fails a pack that holds every other expected file but not dist-lib/OFL.txt, and passes once the licence is packed.
  const names = readdirSync(new URL("../packages/gantt/guide", import.meta.url));
  const guide = names.map((n) => `dist-lib/guide/${n}`);
  const lic = fixture("licence", { ".": "./dist-lib/a.js" }, ["dist-lib", "README.md"], ["README.md", "LICENSE", "AGENTS.md", "dist-lib/a.js", "dist-lib/app_bg.wasm", "dist-lib/engine-size.json", "dist-lib/schema.json", "dist-lib/describe.json", "dist-lib/fit.md", "dist-lib/NOTICE.md", ...guide]);
  const setFiles = (files: string[]) => writeFileSync(join(lic, "package.json"), JSON.stringify({ name: "licence", version: "1.0.0", exports: { ".": "./dist-lib/a.js" }, files, bin: "./dist-lib/a.js" }));
  setFiles(["dist-lib", "README.md", "AGENTS.md"]);
  writeFileSync(join(lic, "dist-lib/a.js"), "#!/usr/bin/env node\n");
  const packCheck = (...flags: string[]) => spawnSync(process.execPath, [fileURLToPath(new URL("./pack-check.ts", import.meta.url)), lic, ...flags], { encoding: "utf8" });
  check("licence lacking", packCheck().stderr.includes("the tarball lacks dist-lib/OFL.txt"), true);
  writeFileSync(join(lic, "dist-lib/OFL.txt"), "SIL OPEN FONT LICENSE\n");
  check("licence packed", packCheck().status, 0);
  // `describe` reads dist-lib/describe.json and dist-lib/fit.md beside the CLI: a pack without either fails.
  for (const f of ["describe.json", "fit.md"]) {
    rmSync(join(lic, "dist-lib", f));
    check(`${f} lacking`, packCheck().stderr.includes(`the tarball lacks dist-lib/${f}`), true);
    writeFileSync(join(lic, "dist-lib", f), "x\n");
  }
  // The user guide's files ship in dist-lib/guide/: the real emit-guide.ts stops on an empty guide/, copies a full one, and the pack passes; a pack without one of the files fails naming it.
  const emit = () => spawnSync(process.execPath, [fileURLToPath(new URL("./emit-guide.ts", import.meta.url)), lic], { encoding: "utf8" });
  rmSync(join(lic, "dist-lib/guide"), { recursive: true });
  mkdirSync(join(lic, "guide"));
  check("empty guide stops the copy", emit().status, 1);
  for (const n of names) writeFileSync(join(lic, "guide", n), "x\n");
  check("emit-guide then pack passes", [emit().status, packCheck().status], [0, 0]);
  // Each chapter in turn: the list the check holds the pack to comes from the guide folder, so none can drop out of it.
  for (const n of names.filter((f) => f.endsWith(".md"))) {
    rmSync(join(lic, "dist-lib/guide", n));
    check(`guide chapter ${n} lacking`, packCheck().stderr.includes(`the tarball lacks dist-lib/guide/${n}`), true);
    emit();
  }
  // dist-lib/guide/ holds Markdown and the guide's LICENSE: a picture, or a file in a folder, fails the pack naming it.
  for (const stray of ["cover.png", "img/cover.png"]) {
    mkdirSync(dirname(join(lic, "dist-lib/guide", stray)), { recursive: true });
    writeFileSync(join(lic, "dist-lib/guide", stray), "x\n");
    const r = packCheck();
    check(`guide holds ${stray}`, [r.status, r.stderr.includes(`the tarball holds dist-lib/guide/${stray}`)], [1, true]);
    rmSync(join(lic, "dist-lib/guide", stray));
  }
  check("guide of Markdown and LICENSE only", packCheck().status, 0);
  // The guide and the prompt carry MIT-0: its own LICENSE ships beside them, and the text asks for no notice.
  rmSync(join(lic, "dist-lib/guide/LICENSE"), { force: true });
  check("the guide licence lacking", packCheck().stderr.includes("the tarball lacks dist-lib/guide/LICENSE"), true);
  const mit0 = readFileSync(new URL("../packages/gantt/guide/LICENSE", import.meta.url), "utf8");
  check("the guide licence is MIT-0", [mit0.startsWith("MIT No Attribution\n"), /copyright notice and this permission notice shall be included/.test(mit0)], [true, false]);
  emit();
  // The assistant guide ships (gantt#1560): a `files` list without AGENTS.md fails the pack check, and with it the check passes (AGENTS.md is an allowed name).
  setFiles(["dist-lib", "README.md"]);
  check("guide left out of files", packCheck().stderr.includes("the tarball lacks AGENTS.md"), true);
  setFiles(["dist-lib", "README.md", "AGENTS.md"]);
  check("guide in files", packCheck().status, 0);
  // A publish ships no build settings (tl#1560): a manifest with `scripts`, `devDependencies` and `packageManager` has all three named, one without has none; the check without `--published` leaves the source folder passing, with it fails.
  const manifest = (extra: object) => writeFileSync(join(lic, "package.json"), JSON.stringify({ name: "licence", version: "1.0.0", exports: { ".": "./dist-lib/a.js" }, files: ["dist-lib", "README.md", "AGENTS.md"], bin: "./dist-lib/a.js", ...extra }));
  manifest({ scripts: { build: "x" }, devDependencies: { y: "1" }, packageManager: "pnpm@1" });
  check("build settings named", manifestProblems(lic), ["package.json holds devDependencies", "package.json holds packageManager", "package.json holds scripts"]);
  check("source folder passes", packCheck().status, 0);
  check("published flag fails", packCheck("--published").status, 1);
  manifest({});
  check("clean manifest", manifestProblems(lic), []);
  check("published flag passes", packCheck("--published").status, 0);
  // The repo's own package ships its data schema, so a customer reads the task format from node_modules: an `exports` entry names it (the library build copies it into dist-lib/, and the pack check above fails when it is missing).
  const real = JSON.parse(readFileSync(new URL("../packages/gantt/package.json", import.meta.url), "utf8"));
  check("ships the schema", real.exports["./schema.json"], "./dist-lib/schema.json");
  // The React wrapper ships in the same package (gantt#1096): the package's `./react` export names its script and declarations, which the pack check above holds the tarball to, and `react` is an optional peer, so an install with no React is clean.
  check(
    "ships ./react with its declarations",
    { react: real.exports["./react"], peer: real.peerDependencies?.react, optional: real.peerDependenciesMeta?.react?.optional },
    { react: { types: "./dist-lib/types/react.d.ts", default: "./dist-lib/react.js" }, peer: ">=18", optional: true },
  );
  // The Vue wrapper ships the same way (gantt#1875): a `./vue` export that names its script and declarations, and `vue` an optional peer.
  const vue = { export: real.exports["./vue"], peer: real.peerDependencies?.vue, optional: real.peerDependenciesMeta?.vue?.optional };
  check("ships ./vue with its declarations", vue, { export: { types: "./dist-lib/types/vue.d.ts", default: "./dist-lib/vue.js" }, peer: ">=3", optional: true });
  // The Angular wrapper too (gantt#1875): a `./angular` export that names its script and declarations, and `@angular/core` an optional peer.
  const angular = { export: real.exports["./angular"], peer: real.peerDependencies?.["@angular/core"], optional: real.peerDependenciesMeta?.["@angular/core"]?.optional };
  check("ships ./angular with its declarations", angular, { export: { types: "./dist-lib/types/angular.d.ts", default: "./dist-lib/angular.js" }, peer: ">=17", optional: true });
  // The repo's own package lists the guide in `files` and the file exists.
  check("files lists AGENTS.md", real.files.includes("AGENTS.md"), true);
  check("AGENTS.md exists", readFileSync(new URL("../packages/gantt/AGENTS.md", import.meta.url), "utf8").length > 0, true);
  // The tarball carries the license: `files` lists LICENSE, and the package's copy is the repository's LICENSE, byte for byte.
  check("files lists LICENSE", real.files.includes("LICENSE"), true);
  check("the package LICENSE is the repository LICENSE", readFileSync(new URL("../packages/gantt/LICENSE", import.meta.url), "utf8") === readFileSync(new URL("../LICENSE", import.meta.url), "utf8"), true);
  // The package's README (npm packs one outside `files`) names every `exports` entry as a reader imports it, and `destroy`, so a reader of node_modules finds how to mount, style, read the schema and stop the chart.
  const readme = (() => {
    try {
      return readFileSync(new URL("../packages/gantt/README.md", import.meta.url), "utf8");
    } catch {
      return "";
    }
  })();
  const named = Object.keys(real.exports).map((k) => (k === "." ? `"${real.name}"` : `${real.name}/${k.slice(2)}`));
  check("README names every export and destroy", [...named, "destroy"].filter((n) => !readme.includes(n)), []);
  // A publish may ship only the built folder, README.md and package.json: a stray source file is named, the allowed files are not.
  const stray = fixture("stray", { ".": "./dist/a.js" }, ["dist", "src"], ["dist/a.js", "src/secret.ts"]);
  writeFileSync(join(stray, "README.md"), "# x\n");
  check("stray file named", strayFiles(stray, ["dist/"], ["README.md", "package.json"]), ["src/secret.ts"]);
  check("allowed files only", strayFiles(engine, ["dist/"], ["README.md", "package.json"]), []);
  // Patterns and null are not file names.
  check("targets", exportTargets({ ".": { import: "./i.js", require: null }, "./f/*": "./f/*.js" }), ["./i.js"]);
  // No exports at all is an error, not a pass.
  let threw = false;
  try {
    missingFromPack(fixture("none", undefined, ["a.js"], ["a.js"]));
  } catch {
    threw = true;
  }
  check("no exports throws", threw, true);
} finally {
  rmSync(root, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("test-pack-check: ok");
