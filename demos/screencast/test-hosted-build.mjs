// Checks that the README video and the book's clip record the same app (gs#601), with no browser and no ffmpeg:
//   node demos/screencast/test-hosted-build.mjs
// hosted-build.mjs names the hosted build directory and the address's query. run.sh builds that directory and hands it to
// record.mjs and to the Mac, record.mjs opens the address, and clip.mjs records on the build and opens the address. The
// test reads each of them, so a change that makes one name another build or another sample fails here by name.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { DEFAULT_DIST, checkHostedDist, parseArgs } from "./clip.mjs";
import { HOSTED_DIST, HOSTED_MARKER, HOSTED_QUERY } from "./hosted-build.mjs";

const here = dirname(fileURLToPath(import.meta.url));
const repo = resolve(here, "..", "..");
const read = (name) => readFileSync(join(here, name), "utf8");
/** The lines of a file that are not comments, so a sentence about a build is not mistaken for a command that names one. */
const code = (text, comments) => text.split("\n").filter((line) => !comments.some((c) => line.trimStart().startsWith(c))).join("\n");

// The shared values are what the hosted app is: a build under apps/hosted, opened on the 50,000-task sample.
assert.equal(HOSTED_DIST, "apps/hosted/dist");
assert.equal(HOSTED_QUERY, "?sample=demo-50k");
assert.equal(HOSTED_MARKER, "samples/samples.json");
assert.match(readFileSync(join(repo, "apps/hosted/package.json"), "utf8"), /"build": "vite build"/, "pnpm build in apps/hosted writes dist there");

// clip.mjs: with no --dist it records the shared build, wherever it runs from, and it opens the shared address.
const options = parseArgs(["--step", "open-file", "--out", join(tmpdir(), "gantt-hosted-build-test")]);
assert.equal(options.dist, DEFAULT_DIST);
assert.equal(DEFAULT_DIST, join(repo, HOSTED_DIST), "the default build is the shared one in this checkout");
assert.equal(parseArgs(["--step", "open-file", "--out", "/x", "--dist", "dist"]).dist, "dist", "a copy of the build, as on the MacBook");
const clip = read("clip.mjs");
assert.match(clip, /import \{[^}]*\bHOSTED_QUERY\b[^}]*\} from "\.\/hosted-build\.mjs"/, "clip.mjs reads the address from hosted-build.mjs");
assert.match(clip, /page\.goto\(`http:\/\/127\.0\.0\.1:\$\{server\.port\}\/\$\{HOSTED_QUERY\}`\)/, "clip.mjs opens the shared address");
assert.match(clip, /await checkHostedDist\(options\.dist\);\n/, "clip.mjs checks the build before it records");
const clipCode = code(clip, ["//", "/*", "*"]);
assert.doesNotMatch(clipCode, /apps\/hosted\/dist|web\/dist|sample=/, "clip.mjs has no build or sample of its own");

// run.sh asks hosted-build.mjs for the directory (this is the command it runs) and uses only that value after the build.
const runSh = read("run.sh");
const printed = execFileSync(process.execPath, [join(here, "hosted-build.mjs"), "dist"], { encoding: "utf8" }).trim();
assert.equal(printed, HOSTED_DIST, "run.sh reads the shared directory");
assert.equal(execFileSync(process.execPath, [join(here, "hosted-build.mjs"), "query"], { encoding: "utf8" }).trim(), HOSTED_QUERY);
assert.match(runSh, /^hosted_dist="\$\(node demos\/screencast\/hosted-build\.mjs dist\)"$/m, "run.sh reads the build directory from hosted-build.mjs");
const dists = [...runSh.matchAll(/^\s+--dist (\S+)/gm)].map((m) => m[1]);
assert.equal(dists.length, 2, "run.sh hands the build to record.mjs twice (the story and the features)");
for (const dist of dists) assert.equal(dist, '"$hosted_dist"', "record.mjs gets the shared build");
assert.match(runSh, /^ {2}"\$hosted_dist\/" mbp:gantt-probe\/screencast\/dist\/$/m, "the Mac gets a copy of the shared build");
assert.doesNotMatch(code(runSh, ["#"]), /apps\/hosted\/dist|web\/dist|sample=/, "run.sh has no build or sample of its own");
assert.match(runSh, /\(cd apps\/hosted && pnpm install --frozen-lockfile && node scripts\/build-samples\.mjs && pnpm build\)/, "run.sh builds the hosted app, which writes the shared directory");

// record.mjs (the README video) opens the same address as clip.mjs.
const goto = /page\.goto\(`http:\/\/127\.0\.0\.1:\$\{server\.port\}\/(\?[^`]*)`\)/.exec(read("record.mjs"));
assert.ok(goto, "record.mjs opens an address with a query");
assert.equal(goto[1], HOSTED_QUERY, "record.mjs and clip.mjs open the same address");

// A build without the hosted app's sample list is refused by name; a hosted build passes.
const scratch = mkdtempSync(join(tmpdir(), "gantt-hosted-build-"));
try {
  const web = join(scratch, "web-dist");
  mkdirSync(web);
  writeFileSync(join(web, "index.html"), "<!doctype html>");
  await assert.rejects(checkHostedDist(web), /not the hosted build: it has no samples\/samples\.json.*clip\.mjs records apps\/hosted\/dist/s);
  const hosted = join(scratch, "hosted-dist");
  mkdirSync(join(hosted, "samples"), { recursive: true });
  writeFileSync(join(hosted, HOSTED_MARKER), "{}");
  await checkHostedDist(hosted);
} finally {
  rmSync(scratch, { recursive: true, force: true });
}
console.log("ok: run.sh, record.mjs and clip.mjs name the same hosted build and address");
