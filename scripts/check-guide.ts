// Checks the embeddable user guide against its rules (see scripts/lib/guide-pages.ts). Run from the repo root:
//   node scripts/check-guide.ts [repo-root]
// Needs Node 22.18 or later (type stripping). Exits 1 and prints one line per problem.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { FIELDS_EN, TEXTS_EN } from "../packages/gantt/src/texts-en.ts";
import { definedSamples } from "./lib/book-pages.ts";
import { buildModel } from "./lib/describe-model.ts";
import { checkGuide } from "./lib/guide-pages.ts";
import { WEB_ROOT } from "./lib/web-root.ts";

const root = process.argv[2] ?? ".";
const read = (file: string): string => readFileSync(join(root, file), "utf8");
/** The markdown files under a folder, by their path from the repo root. */
const markdown = (dir: string): Map<string, string> => {
  const files = readdirSync(join(root, dir), { recursive: true, encoding: "utf8" }).map((rel) => rel.split("\\").join("/"));
  return new Map(files.filter((rel) => rel.endsWith(".md")).map((rel) => [`${dir}/${rel}`, read(`${dir}/${rel}`)]));
};
const model = buildModel(join(root, WEB_ROOT));
const names = (items: { name: string }[]): string[] => items.map((item) => item.name);
const problems = checkGuide({
  files: markdown(`${WEB_ROOT}/guide`),
  pages: markdown("book/src"),
  names: {
    option: new Set(names(model.options)),
    event: new Set(names(model.events)),
    // A `call:` names a command or a method of the chart itself.
    call: new Set([...names(model.commands), ...names(model.methods)]),
  },
  labels: new Set([...Object.values(TEXTS_EN), ...Object.values(FIELDS_EN)]),
  samples: [...definedSamples(read("apps/hosted/scripts/build-samples.mjs"))],
});
for (const p of problems) console.error(`check-guide: ${p}`);
if (problems.length > 0) process.exit(1);
console.log("check-guide: ok");
