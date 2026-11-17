// Lists what still ties the benchmark harness to this repo (gantt#1285): `node scripts/harness-ties.ts`, from anywhere.
// A text tie fails (exit 1); an outside import is printed and passes. Needs git and Node 22.18 or later.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { harnessFiles, harnessTies } from "./lib/harness-ties.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const listed = execFileSync("git", ["-C", root, "ls-files", "-z", "demos/bench", "demos/dhtmlx-compare"], { encoding: "utf8" }).split("\0").filter(Boolean);
const files = harnessFiles(listed).map((path) => ({ path, text: readFileSync(join(root, path), "utf8") }));
const ties = harnessTies(files);
for (const t of ties) console.log(`${t.kind === "text" ? "text tie" : "outside import"}: ${t.path}:${t.line}: ${t.what}`);
const text = ties.filter((t) => t.kind === "text").length;
console.log(`${files.length} files, ${text} text ties, ${ties.length - text} outside imports`);
process.exit(text > 0 ? 1 : 0);
