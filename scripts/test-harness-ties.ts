// Checks the harness ties check (gantt#1285). Run from anywhere: node scripts/test-harness-ties.ts
// Needs Node 22.18 or later and git. It checks the two classes on fixtures, the file pick, and the command on this tree.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { harnessFiles, harnessTies } from "./lib/harness-ties.ts";

const ties = (path: string, text: string) => harnessTies([{ path, text }, { path: "demos/bench/b.ts", text: "" }]).map((t) => `${t.kind} ${t.line} ${t.what}`);
const P = "demos/bench/a.ts";

// text ties: one class each, on the line that holds it
assert.deepEqual(ties(P, "ok\n// see gs#12\n// Dave said\n// on the mini\nhomedir()\nconst a = '~/x'"),
  ["text 2 private issue", "text 3 person", "text 4 computer", "text 5 home folder", "text 6 home folder"]);
assert.deepEqual(ties(P, "// #1292 and github.com/a and #fff and /Users/ci/x and a minimum"), [], "a bare number, a github host and a fixture path are not ties");
assert.deepEqual(ties("demos/bench/versions.json", '{"display": "on the mini, gs#1"}'), [], "a .json file is data and is not read");

// outside imports: printed, whatever the way of writing them; an import inside the set, a package and node_modules are not
assert.deepEqual(ties(P, 'import { a } from "./b.ts";\nimport c from "node:fs";\nimport "../../x.ts";\nconst d = await import("../y.ts");\nexport * from "./node_modules/z.js";'),
  ["import 3 ../../x.ts", "import 4 ../y.ts"]);

// the pick: both folders, not the saved runs, not profile*
assert.deepEqual(harnessFiles(["demos/bench/a.ts", "demos/bench/results/x.json", "demos/bench/profile.ts", "demos/bench/sides/profile-mini.ts", "demos/dhtmlx-compare/p.ts", "demos/50k/m.mjs"]),
  ["demos/bench/a.ts", "demos/dhtmlx-compare/p.ts"]);

// this tree: no text tie, and the command says what it found
const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const run = spawnSync("node", [join(root, "scripts/harness-ties.ts")], { encoding: "utf8" });
assert.equal(run.status, 0, run.stdout);
assert.match(run.stdout, /^outside import: .*\n(?:.*\n)*\d+ files, 0 text ties, \d+ outside imports\n$/m);
console.log("ok");
