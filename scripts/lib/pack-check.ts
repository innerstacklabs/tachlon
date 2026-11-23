// Fails a package whose `exports` names a file the tarball leaves out (tl#1089). `npm pack --dry-run` lists what the tarball would hold (no pnpm needed: the tier step's runner has none).
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, posix } from "node:path";

// Every file path an `exports` value names: a string, an array, or the values of a conditions object. Patterns (`*`) and null are skipped.
export function exportTargets(exp: unknown): string[] {
  if (typeof exp === "string") return exp.includes("*") ? [] : [exp];
  if (Array.isArray(exp)) return exp.flatMap(exportTargets);
  if (exp && typeof exp === "object") return Object.values(exp).flatMap(exportTargets);
  return [];
}

// The paths (relative to `dir`) its tarball would hold. Each check below takes the list as `files`, so a caller runs `npm pack` once for all of them; one called without it reads its own.
export function packedFiles(dir: string): Set<string> {
  const [packed] = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
  return new Set<string>(packed.files.map((f: { path: string }) => f.path));
}

// The sorted export targets of `dir` that its packed tarball lacks. Throws if the package cannot be packed or names no `exports`.
export function missingFromPack(dir: string, files: Set<string> = packedFiles(dir)): string[] {
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  if (pkg.exports === undefined) throw new Error(`${dir}/package.json has no exports`);
  const has = (t: string) => files.has(t.replace(/^\.\//, ""));
  return [...new Set(exportTargets(pkg.exports))].filter((t) => !has(t)).sort();
}

// The sorted paths in `dir`'s tarball that sit under none of `prefixes` and are none of `names` (what a publish must not ship).
export function strayFiles(dir: string, prefixes: string[], names: string[], files: Set<string> = packedFiles(dir)): string[] {
  return [...files].filter((f) => !names.includes(f) && !prefixes.some((p) => f.startsWith(p))).sort();
}

// The sorted `expected` paths (relative to `dir`) that its tarball lacks.
export function missingExpected(dir: string, expected: string[], files: Set<string> = packedFiles(dir)): string[] {
  return expected.filter((f) => !files.has(f)).sort();
}

// The sorted `.d.ts` files of `dir`'s tarball that import from the generated `wasm/` folder, which the tarball does not carry (a consumer's compile would fail on it).
export function declarationsNamingWasm(dir: string, files: Set<string> = packedFiles(dir)): string[] {
  const names = /(?:from|import)\s*\(?\s*["'][^"']*\/wasm\//;
  return [...files].filter((f) => f.endsWith(".d.ts") && names.test(readFileSync(join(dir, f), "utf8"))).sort();
}

// The problems with `dir`'s `bin`, sorted: no `bin` at all, or a target the tarball lacks or that does not start with a `#!` line (npm links a bin as a command, which needs the interpreter line).
export function binProblems(dir: string, files: Set<string> = packedFiles(dir)): string[] {
  const { bin } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  const targets: string[] = typeof bin === "string" ? [bin] : bin && typeof bin === "object" ? Object.values(bin) : [];
  if (targets.length === 0) return ["package.json has no bin"];
  return targets
    .flatMap((t) => (!files.has(t.replace(/^\.\//, "")) ? [`${t}: the tarball lacks it`] : readFileSync(join(dir, t), "utf8").startsWith("#!") ? [] : [`${t}: the first line is not a #! line`]))
    .sort();
}

// The keys of a manifest that belong to the project's own build and stay out of what a publish ships.
export const PRIVATE_KEYS = ["scripts", "devDependencies", "packageManager"];

// The problems with `dir`'s `package.json` as a published manifest, sorted: one for each private build key it holds (tl#1560).
export function manifestProblems(dir: string): string[] {
  const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
  return PRIVATE_KEYS.filter((k) => pkg[k] !== undefined).map((k) => `package.json holds ${k}`).sort();
}

// The sorted `url()` references of `dir`'s packed `.css` files that name no file inside the tarball: a data URL, an absolute or remote URL, or a relative path (resolved against the stylesheet) the tarball lacks.
export function cssUrlsOutsidePack(dir: string, files: Set<string> = packedFiles(dir)): string[] {
  const bad = new Set<string>();
  for (const css of [...files].filter((f) => f.endsWith(".css"))) {
    for (const [, , ref] of readFileSync(join(dir, css), "utf8").matchAll(/url\(\s*(["']?)([^"')]*)\1\s*\)/g)) {
      if (ref.startsWith("#")) continue;
      if (/^(?:[a-z][a-z0-9+.-]*:|\/)/i.test(ref) || !files.has(posix.join(posix.dirname(css), ref.replace(/[?#].*$/, "")))) bad.add(`${css}: ${ref.slice(0, 40)}`);
    }
  }
  return [...bad].sort();
}
