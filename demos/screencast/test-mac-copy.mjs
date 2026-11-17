// Checks that every module record.mjs needs reaches the Mac, with no Mac: node demos/screencast/test-mac-copy.mjs
import assert from "node:assert/strict";
import { readdirSync, readFileSync } from "node:fs";
import { dirname, join, relative } from "node:path";
import { fileURLToPath } from "node:url";
import { importClosure } from "./record/imports.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const runSh = readFileSync(join(here, "run.sh"), "utf8");
assert.match(runSh, /--exclude-from=demos\/screencast\/mac-copy-excludes\.txt/, "run.sh must copy with the exclude file");
assert.match(runSh, /demos\/screencast\/ mbp:gantt-probe\/screencast\//, "run.sh must copy the directory to the same remote path");

// rsync exclude rules: "dir/" matches a directory name anywhere, other globs match a file name.
const rules = readFileSync(join(here, "mac-copy-excludes.txt"), "utf8").split("\n").map((l) => l.trim()).filter((l) => l && !l.startsWith("#"));
const globToRegExp = (g) => new RegExp(`^${g.replace(/[.+^${}()|[\]\\]/g, "\\$&").replace(/\*/g, "[^/]*").replace(/\?/g, "[^/]")}$`);
const excluded = (rel) => {
  const parts = rel.split("/");
  return rules.some((r) => r.endsWith("/")
    ? parts.slice(0, -1).some((p) => globToRegExp(r.slice(0, -1)).test(p))
    : globToRegExp(r).test(parts[parts.length - 1]));
};

const walk = (dir) => readdirSync(dir, { withFileTypes: true }).flatMap((e) =>
  e.isDirectory() ? walk(join(dir, e.name)) : [relative(here, join(dir, e.name))]);
const copied = new Set(walk(here).filter((f) => !excluded(f)));

// Walk the relative imports of record.mjs, static and dynamic, through the files they pull in.
const seen = new Set(importClosure(join(here, "record.mjs"), (from, spec, target) => {
  const rel = relative(here, target);
  assert.ok(copied.has(rel), `${relative(here, from)} imports ${spec}, but ${rel} is not copied to the Mac`);
}).map((file) => relative(here, file)));

assert.ok(seen.has("record/browser.mjs") && seen.has("record/media.mjs"), "the import walk must reach record/*.mjs");
// Every test-*.mjs in this directory, read from disk so a narrower exclude than test-*.mjs fails.
const tests = readdirSync(here).filter((f) => /^test-.*\.mjs$/.test(f));
assert.ok(tests.includes("test-mac-copy.mjs"), "the test list must be read from this directory");
for (const f of [...tests, "run.sh"]) assert.ok(!copied.has(f), `${f} must stay off the Mac`);
assert.ok(![...copied].some((f) => f.startsWith("upload/")), "the hub-only upload/ directory must stay off the Mac");
console.log(`ok: ${seen.size} modules reachable from record.mjs, all copied`);
