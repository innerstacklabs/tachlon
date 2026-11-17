// Prepares the open-source release, for either way Dave may choose (gantt#1954): a fresh-history snapshot, or a full-history publish.
// It reads scripts/public-release.json, writes only into a directory you name, creates no repository and pushes to no remote.
//   node scripts/public-release.ts files                list the tracked files that go in
//   node scripts/public-release.ts scan                 secret scan of the tracked tree, and a name scan when PUBLIC_RELEASE_NAMES is set (exit 1 on a finding; CI runs it)
//   node scripts/public-release.ts snapshot <dir>       copy the files that go in to a new directory (fresh history)
//   node scripts/public-release.ts scrub-list           full history: what to drop, which commits name a customer, secrets in history
// The names that must not reach the public repository come from PUBLIC_RELEASE_NAMES (comma-separated), never from a file in the tree.
// Output holds paths, commit ids, rule ids and counts: never a matched value, never a name. Exit 2 means it could not decide.
import { execFileSync, spawn } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
export type Config = { include: string[]; exclude: string[]; prune: string[]; secretShapes: Record<string, string> };
export const config: Config = JSON.parse(readFileSync(join(here, "public-release.json"), "utf8"));

const covers = (list: string[], path: string): boolean => list.some((e) => (e.endsWith("/") ? path.startsWith(e) : path === e));
/** The files that go in: on the include list and not on the exclude list (the exclude list wins). */
export const select = (paths: string[], cfg: Config = config): string[] => paths.filter((p) => covers(cfg.include, p) && !covers(cfg.exclude, p));
/** A list file whose lines start with a repository path (`path` or `path | text`), without the lines for paths that do not go in. */
export const pruneList = (text: string, goesIn: Set<string>): string =>
  text.split("\n").filter((l) => !l.trim() || l.startsWith("#") || goesIn.has(l.split(" | ")[0])).join("\n");
/** The rule ids whose pattern matches this line. */
export const shapesIn = (line: string, cfg: Config = config): string[] =>
  Object.entries(cfg.secretShapes).filter(([, re]) => new RegExp(re).test(line)).map(([id]) => id);

const git = (root: string, ...args: string[]): string => execFileSync("git", ["-C", root, ...args], { encoding: "utf8", maxBuffer: 1 << 30 });
const tracked = (root: string): string[] => git(root, "ls-files", "-z").split("\0").filter((p) => p && existsSync(join(root, p)));
const names = (): RegExp => {
  const list = (process.env.PUBLIC_RELEASE_NAMES ?? "").split(",").map((n) => n.trim()).filter(Boolean);
  if (!list.length) { console.error("public-release: set PUBLIC_RELEASE_NAMES (comma-separated); without the names it cannot check anything"); process.exit(2); }
  return new RegExp(list.map((n) => n.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")).join("|"), "i");
};

/** Tree findings as `path:line rule`, for the files named (all tracked files when none are named). */
export function scanTree(root: string, files: string[]): string[] {
  const out: string[] = [];
  for (const f of files) {
    const text = readFileSync(join(root, f), "latin1");
    if (text.includes("\0")) continue;
    text.split("\n").forEach((line, i) => shapesIn(line).forEach((id) => out.push(`${f}:${i + 1} ${id}`)));
  }
  return out;
}

/** Name findings in the paths and lines of the files named, as `path` or `path:line`: never the name. */
export function scanNames(root: string, files: string[], naming: RegExp): string[] {
  const out: string[] = [];
  for (const f of files) {
    if (naming.test(f)) out.push(f);
    const text = readFileSync(join(root, f), "latin1");
    if (!text.includes("\0")) text.split("\n").forEach((line, i) => { if (naming.test(line)) out.push(`${f}:${i + 1}`); });
  }
  return out;
}

/** Findings in the added lines of every commit on every branch: `commit path rule`, found by streaming `git log -p`. */
export function scanHistory(root: string): Promise<string[]> {
  return new Promise((done, fail) => {
    const p = spawn("git", ["-C", root, "log", "--all", "-p", "--no-renames", "--format=%x00%h"], { stdio: ["ignore", "pipe", "inherit"] });
    const out = new Set<string>();
    let commit = "", path = "", rest = "";
    const read = (line: string): void => {
      if (line.startsWith("\0")) commit = line.slice(1);
      else if (line.startsWith("+++ ")) path = line.slice(6);
      else if (line.startsWith("+")) shapesIn(line).forEach((id) => out.add(`${commit} ${path} ${id}`));
    };
    p.stdout.setEncoding("latin1").on("data", (chunk: string) => { const lines = (rest + chunk).split("\n"); rest = lines.pop() ?? ""; lines.forEach(read); });
    p.on("error", fail).on("close", (code) => { read(rest); code === 0 ? done([...out]) : fail(new Error(`git log exited ${code}`)); });
  });
}

async function main(argv: string[]): Promise<number> {
  const root = git(process.cwd(), "rev-parse", "--show-toplevel").trim();
  const [cmd, dest] = argv;
  const goesIn = select(tracked(root));
  if (cmd === "files") { console.log(goesIn.join("\n")); return 0; }
  if (cmd === "scan") {
    const files = tracked(root);
    const checkNames = Boolean(process.env.PUBLIC_RELEASE_NAMES?.trim());
    const found = [...scanTree(root, files), ...(checkNames ? scanNames(root, files, names()).map((f) => `${f} names a customer`) : [])];
    console.log(found.length ? found.join("\n") : `public-release scan: no secret shape and ${checkNames ? "no name" : "no name check (PUBLIC_RELEASE_NAMES unset)"} in the tracked tree`);
    return found.length ? 1 : 0;
  }
  if (cmd === "snapshot") {
    if (!dest) { console.error("usage: public-release.ts snapshot <new directory>"); return 2; }
    if (existsSync(dest) && readdirSync(dest).length) { console.error(`public-release: ${dest} is not empty`); return 2; }
    const naming = names();
    const bad = goesIn.filter((f) => naming.test(f) || naming.test(readFileSync(join(root, f), "latin1")));
    const secrets = scanTree(root, goesIn);
    if (bad.length || secrets.length) {
      console.error([...bad.map((f) => `names a customer: ${f}`), ...secrets].join("\n"));
      return 1;
    }
    const set = new Set(goesIn);
    for (const f of goesIn) {
      mkdirSync(dirname(join(dest, f)), { recursive: true });
      if (config.prune.includes(f)) writeFileSync(join(dest, f), pruneList(readFileSync(join(root, f), "utf8"), set));
      else copyFileSync(join(root, f), join(dest, f));
    }
    console.log(`public-release: ${goesIn.length} files copied to ${dest}; run git init there yourself`);
    return 0;
  }
  if (cmd === "scrub-list") {
    const naming = names();
    const everPath = [...new Set(git(root, "log", "--all", "--name-only", "--format=").split("\n").filter(Boolean))];
    const drop = everPath.filter((p) => !select([p]).length);
    const namingCommits = git(root, "log", "--all", "--format=%h%x00%an%x00%ae%x00%s%x00%b%x01").split("\x01").map((c) => c.trim()).filter((c) => c && naming.test(c.replace(/\0/g, " "))).map((c) => c.split("\0")[0]);
    const inContent = git(root, "log", "--all", "-i", `-G${naming.source}`, "--format=%h").split("\n").filter(Boolean);
    const secrets = await scanHistory(root);
    console.log(`paths in history: ${everPath.length}; to drop for a full-history publish: ${drop.length}`);
    console.log(`commits whose message or author names a customer (rewrite or drop): ${namingCommits.length}: ${namingCommits.join(" ")}`);
    console.log(`commits whose diff adds or removes a line naming a customer: ${inContent.length}`);
    console.log(`secret shapes in history: ${secrets.length}${secrets.length ? "\n" + secrets.join("\n") : ""}`);
    return namingCommits.length || inContent.length || secrets.length ? 1 : 0;
  }
  console.error("usage: public-release.ts files | scan | snapshot <dir> | scrub-list");
  return 2;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) main(process.argv.slice(2)).then((c) => process.exit(c), (e) => { console.error(`public-release: ${e instanceof Error ? e.message : e}`); process.exit(2); });
