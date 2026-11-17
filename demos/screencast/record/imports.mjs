// The relative imports of a module, followed transitively. test-mac-copy.mjs uses it to check that every module the recorder
// needs reaches the Mac; record/clips.mjs uses it to hash the source a clip depends on.
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";

/** The relative specifiers a module imports, static and dynamic ("./a.mjs", "../b.mjs"). */
export function relativeSpecifiers(source) {
  return [...source.matchAll(/(?:\bfrom\s*|\bimport\s*\(?\s*|\bimport\s+)["'](\.{1,2}\/[^"']+)["']/g)].map((match) => match[1]);
}

/**
 * Every file reachable from `entry` (an absolute path) through relative imports, the entry first. `onImport(from, specifier,
 * target)` is called once for each import found, with absolute paths, before the target is followed.
 */
export function importClosure(entry, onImport = () => {}) {
  const seen = new Set();
  const visit = (file) => {
    if (seen.has(file)) return;
    seen.add(file);
    for (const specifier of relativeSpecifiers(readFileSync(file, "utf8"))) {
      const target = resolve(dirname(file), specifier);
      onImport(file, specifier, target);
      visit(target);
    }
  };
  visit(entry);
  return [...seen];
}
