// Emits a package's type declarations (`tsconfig.lib.json`) and trims them to what a consumer can reach (tl#1089).
// tsc writes a `.d.ts` for every module the entry imports for its code: the shell's own parts, which name the generated wasm types.
// So this keeps the `.d.ts` files the entry's typings reach (`index.d.ts`, `react.d.ts` and the files they import, and so on) and deletes the rest,
// and takes the side-effect style imports out of the ones left: a `.d.ts` that says `import "./styles/x.css"` fails a consumer that
// checks side-effect imports (TS2882), and the stylesheets ship in the bundle. Run after the bundle, which empties its output folder.
//   node scripts/emit-types.ts <package folder> [output folder, default dist-lib/types]
import { execFileSync } from "node:child_process";
import { copyFileSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";

const STYLE_IMPORT = /^import "[^"]+\.css";\n/gm;
const SPECIFIER = /\bfrom\s+"(\.{1,2}\/[^"]+)\.js"/g;

// Every `.d.ts` under `dir`, as paths.
function declarations(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? declarations(join(dir, e.name)) : e.name.endsWith(".d.ts") ? [join(dir, e.name)] : []));
}

const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/emit-types.ts <package folder> [output folder]");
  process.exit(2);
}
const out = resolve(dir, process.argv[3] ?? "dist-lib/types");
execFileSync(join(dir, "node_modules/.bin/tsc"), ["-p", "tsconfig.lib.json", "--outDir", out], { cwd: dir, stdio: "inherit" });
// `angular.d.ts` comes from `ngc` (tsconfig.ng.json): the `ɵfac` and `ɵcmp` members that an Angular app's compiler reads are ones only it writes (gantt#1875).
execFileSync(join(dir, "node_modules/.bin/ngc"), ["-p", "tsconfig.ng.json"], { cwd: dir, stdio: "inherit" });
copyFileSync(join(dir, "dist-ng/angular.d.ts"), join(out, "angular.d.ts"));
const reached = new Set<string>();
const todo = ["index.d.ts", "react.d.ts", "vue.d.ts", "angular.d.ts", "headless.d.ts"].map((f) => join(out, f)); // the package's five entries
for (let file = todo.pop(); file !== undefined; file = todo.pop()) {
  if (reached.has(file)) continue;
  reached.add(file);
  const text = readFileSync(file, "utf8");
  for (const [, from] of text.matchAll(SPECIFIER)) todo.push(resolve(dirname(file), `${from}.d.ts`));
  const stripped = text.replace(STYLE_IMPORT, "");
  if (stripped !== text) writeFileSync(file, stripped);
}
for (const file of declarations(out)) if (!reached.has(file)) rmSync(file);
