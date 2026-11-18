// Checks the media manifest and its tools (gantt#1562). Run from anywhere: node scripts/test-media-stale.ts
// Needs Node 22.18 or later and git. It checks:
//   1. the glob matcher and `staleBy`;
//   2. `media-stale.ts` in a scratch repo: ok, STALE (exit 1), and an unknown commit (exit 2, not ok);
//   3. media/manifest.json: a row for every media file, each commit resolves, no duplicate rows;
//   4. no narration line or caption names the version ("0." and a digit, or "version"); a pinned CDN URL
//      (name@x.y.z) is not a version mention, and nothing here unpins one.
import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { globRe, readRows, staleBy } from "./media-stale.ts";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const git = (cwd: string, ...args: string[]): string => execFileSync("git", ["-C", cwd, ...args], { encoding: "utf8" }).trim();

// 1. globs
const row = { file: "a.webp", commit: "x", record: "r", depends: ["src/cli/init*", "pkg/**", "docs/**/*.md"] };
assert.deepEqual(staleBy(row, ["src/cli/init.ts", "src/cli/initial/a.ts", "src/cli/other.ts", "pkg/a/b.ts", "docs/a/b/c.md", "docs/c.txt", "x/pkg/a.ts"]),
  ["src/cli/init.ts", "pkg/a/b.ts", "docs/a/b/c.md"], "* stays in one folder, ** crosses folders, a glob is anchored");
assert.ok(globRe("a.b").test("a.b") && !globRe("a.b").test("axb"), "a dot is literal");
assert.throws(() => readRows('[{"file":"a","commit":"c","record":"r","depends":[]}]'), /needs file/, "a row with no depends is refused");

// 2. the script, in a scratch repo
const repo = mkdtempSync(join(tmpdir(), "media-stale-"));
git(repo, "init", "-q");
git(repo, "config", "user.email", "t@example.com");
git(repo, "config", "user.name", "t");
const commit = (path: string, text: string): string => {
  mkdirSync(dirname(join(repo, path)), { recursive: true });
  writeFileSync(join(repo, path), text);
  git(repo, "add", "-A");
  git(repo, "commit", "-q", "-m", path);
  return git(repo, "rev-parse", "HEAD");
};
const recorded = commit("src/a.ts", "1");
const manifest = join(repo, "m.json");
const run = (commitId: string) => {
  writeFileSync(manifest, JSON.stringify([{ file: "a.webp", commit: commitId, record: "bash rec.sh", depends: ["src/**"] }]));
  const r = spawnSync("node", [join(root, "scripts/media-stale.ts"), "--repo", repo, "--manifest", manifest], { encoding: "utf8" });
  return { code: r.status, out: r.stdout };
};
commit("docs/x.md", "1");
assert.deepEqual(run(recorded), { code: 0, out: "ok    a.webp\n" }, "a change outside the globs is ok");
commit("src/b.ts", "1");
const stale = run(recorded);
assert.equal(stale.code, 1, "a change inside the globs exits 1");
assert.match(stale.out, /^STALE a\.webp \(1 changed since \w+, first src\/b\.ts\); re-record: bash rec\.sh\n$/);
assert.equal(run(git(repo, "rev-parse", "HEAD")).code, 0, "the re-record commit updates the row");
assert.equal(run("0".repeat(40)).code, 2, "an unknown commit is unknown, not ok");

// 3. the real manifest
const rows = readRows(readFileSync(join(root, "media/manifest.json"), "utf8"));
const tracked = git(root, "ls-files", "book/src/media", "book/theme", "docs/media").split("\n").filter((f) => /\.(webp|mp4|gif|png|svg)$/.test(f) && !f.startsWith("docs/media/brand/"));
assert.deepEqual(rows.map((r) => r.file).sort(), tracked.sort(), "one row for each media file, and none for a file that is gone");
for (const r of rows) git(root, "cat-file", "-e", `${r.commit}^{commit}`);

// 4. no version in the words
const VERSION = /(?<![\d.])0\.\d|\bversions?\b/i;
const pinned = (s: string): string => s.replace(/\S*@\d+\.\d+\.\d+\S*/g, "");
const mentionsVersion = (s: string): boolean => VERSION.test(pinned(s));
assert.ok(mentionsVersion("Now at 0.2 with new bars") && mentionsVersion("The new version is out") && !mentionsVersion("Zoom to 50.5 percent"), "the check sees a version");
assert.ok(!mentionsVersion("Add the link tag https://cdn.jsdelivr.net/npm/pkg@0.1.0/dist-lib/gantt.css"), "a pinned CDN URL is not a version mention");
const strings = (v: unknown): string[] => (typeof v === "string" ? [v] : v && typeof v === "object" ? Object.values(v).flatMap(strings) : []);
const lines: string[] = [];
for (const f of git(root, "ls-files", "demos/screencast", ":(glob)**/spec.json").split("\n").filter(Boolean)) {
  const text = readFileSync(join(root, f), "utf8");
  if (f.endsWith("spec.json")) lines.push(...strings(JSON.parse(text)));
  else if (/\.mjs$/.test(f) && !/\/test-/.test(f)) for (const m of text.matchAll(/\b(?:caption|say):\s*(["'`])((?:\\.|(?!\1).)*)\1/g)) lines.push(m[2]);
}
assert.ok(lines.length > 10, `the scan finds the captions (${lines.length})`);
assert.deepEqual(lines.filter(mentionsVersion), [], "no caption or narration line names the version");
console.log("test-media-stale: ok");
