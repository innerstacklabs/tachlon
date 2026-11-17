// Lists the media whose source changed since it was recorded (gantt#1562). Run from anywhere:
//   node scripts/media-stale.ts [--manifest <file>] [--repo <dir>]
// media/manifest.json has one row per media file: `commit` (the commit it was recorded at), `record` (the command
// that re-records it), `depends` (globs of what it shows). A row is STALE when `git diff --name-only <commit>..HEAD`
// names a path matching a glob, ok otherwise. Exits 1 when any row is STALE, 2 when a row or commit cannot be read
// (unknown is not ok). The re-record commit updates the row's `commit`. Release checklist: run it before tagging.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

export type Row = { file: string; commit: string; record: string; depends: string[] };

/** `**` matches any path, `*` matches within one folder, `?` one character; the rest is literal. */
export const globRe = (glob: string): RegExp =>
  new RegExp(`^${glob.replace(/\*\*\/|\*\*|\*|\?|[.+^${}()|[\]\\]/g, (m) => ({ "**/": "(?:.*/)?", "**": ".*", "*": "[^/]*", "?": "[^/]" })[m] ?? `\\${m}`)}$`);

/** The paths of `changed` that a row's globs match. */
export const staleBy = (row: Row, changed: string[]): string[] => {
  const res = row.depends.map(globRe);
  return changed.filter((p) => res.some((re) => re.test(p)));
};

export const readRows = (text: string): Row[] => {
  const rows = JSON.parse(text) as Row[];
  for (const r of rows) {
    const ok = [r.file, r.commit, r.record].every((s) => typeof s === "string" && s) && Array.isArray(r.depends) && r.depends.length > 0;
    if (!ok) throw new Error(`manifest row needs file, commit, record and depends: ${JSON.stringify(r)}`);
  }
  return rows;
};

const main = (): number => {
  const arg = (name: string, fallback: string): string => (process.argv.includes(name) ? process.argv[process.argv.indexOf(name) + 1] : undefined) ?? fallback;
  const repo = arg("--repo", join(dirname(fileURLToPath(import.meta.url)), ".."));
  const manifest = arg("--manifest", join(repo, "media/manifest.json"));
  let stale = 0;
  try {
    for (const row of readRows(readFileSync(manifest, "utf8"))) {
      const changed = execFileSync("git", ["-C", repo, "diff", "--name-only", `${row.commit}..HEAD`], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }).split("\n").filter(Boolean);
      const hit = staleBy(row, changed);
      if (hit.length === 0) console.log(`ok    ${row.file}`);
      else {
        stale++;
        console.log(`STALE ${row.file} (${hit.length} changed since ${row.commit}, first ${hit[0]}); re-record: ${row.record}`);
      }
    }
  } catch (err) {
    console.error(`media-stale: ${(err as Error).message}`);
    return 2;
  }
  return stale > 0 ? 1 : 0;
};

if (import.meta.main) process.exit(main());
