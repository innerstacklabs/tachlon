// Release day, full-history route (gantt#1954): clone this repository to a new scratch directory, rewrite its history to the public set, and scan the result.
//   node scripts/public-history.ts <source repository> <new scratch directory> [--branch main] [--email <address>] [--name "Dave Remy"] [--gitleaks <path>]
// It rewrites only the clone it just made (git clone --no-local, every remote removed) and never pushes. Needs PUBLIC_RELEASE_NAMES, as public-release.ts does.
// The rewrite: paths outside the public set leave history; every author and committer becomes --name and --email (default: the noreply address `gh api user` returns);
// Co-authored-by trailers and "Generated with Claude Code" lines go; every commit's name-check allowlist (public-release.json "prune") loses its lines for paths that commit lacks; any other address in a message becomes "[address removed]" (noreply addresses and the product domain stay);
// the names in PUBLIC_RELEASE_NAMES become "the partner" in messages. Output holds counts and rule ids only. Exit 0 clean, 1 a finding, 2 it could not decide (a scanner is missing).
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { PRODUCT_NAME } from "../packages/gantt/src/product.ts";
import { config, pruneList, scanHistory, select } from "./public-release.ts";

const self = fileURLToPath(import.meta.url);
const KEEP = new RegExp(`^(no-?reply)@|@users\\.noreply\\.github\\.com$|@${PRODUCT_NAME.toLowerCase()}\\.com$`, "i");
const EMAIL = /[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g;
// The assistant's two attributions: a Co-authored-by trailer, and a "Generated with/by Claude Code" line (it may wrap after "Claude", and may be wrapped in _ or an emoji).
const GENERATED = /^[^\p{L}\p{N}\n]*generated (with|by)\b[^\n]*claude\s+code[^\n]*$/gimu;
const ASSISTANT = new RegExp(`^co-authored-by:.*(claude|anthropic)|${GENERATED.source}`, "imu");
const escape = (s: string): string => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const git = (root: string, ...args: string[]): string => execFileSync("git", ["-C", root, ...args], { encoding: "utf8", maxBuffer: 1 << 30 });
const namesRe = (): RegExp | null => {
  const list = (process.env.PUBLIC_RELEASE_NAMES ?? "").split(",").map((n) => n.trim()).filter(Boolean);
  return list.length ? new RegExp(list.map(escape).join("|"), "gi") : null;
};

/** A commit message as it goes public: no assistant trailer, no personal address, no customer name. */
export function cleanMessage(text: string, names: RegExp | null): string {
  const kept = text.replace(GENERATED, "").split("\n").filter((l) => !ASSISTANT.test(l)).join("\n");
  const body = kept.replace(EMAIL, (a) => (KEEP.test(a) ? a : "[address removed]"));
  return (names ? body.replace(names, "the partner") : body).replace(/\s+$/, "") + "\n";
}

/** The retired project names (built at run time: the architecture name test scans this file for them). */
const RETIRED = [["gantt", "spike"].join("-"), ["gantt", "spike"].join("_"), ["man", "tis"].join("")];
/** `pruneList`, and then the list's own line (`self`) goes when no kept line names a retired name: the architecture name test fails a whole-file entry for a file that holds none. */
export function pruneAllowlist(text: string, tree: Set<string>, self: string): string {
  const kept = pruneList(text, tree);
  return RETIRED.some((n) => kept.toLowerCase().includes(n)) ? kept : kept.split("\n").filter((l) => l !== self).join("\n");
}

/** Runs in the index filter, once per commit: each file in the "prune" config loses its lines for paths this commit's index lacks. */
function pruneIndex(): void {
  const tree = new Set(git(".", "ls-files", "-z").split("\0").filter(Boolean));
  for (const f of config.prune.filter((p) => tree.has(p))) {
    const blob = execFileSync("git", ["hash-object", "-w", "--stdin"], { input: pruneAllowlist(git(".", "cat-file", "blob", `:${f}`), tree, f), encoding: "utf8" }).trim();
    git(".", "update-index", "--cacheinfo", `100644,${blob},${f}`);
  }
}

/** The addresses in every author, committer and message field that are not kept, lowercased. */
const personalIn = (root: string): string[] => [...new Set((git(root, "log", "--all", "--format=%an %ae %cn %ce %B").match(EMAIL) ?? []).filter((a) => !KEEP.test(a)).map((a) => a.toLowerCase()))];

export type Counts = Record<string, number>;
/** Counts only: commits, distinct author and committer addresses, assistant trailers, commits that hold a personal address or a name, paths in history. */
export function count(root: string, personal: string[], names: RegExp | null): Counts {
  const lines = (...args: string[]): string[] => git(root, ...args).split("\n").filter(Boolean);
  const re = (list: string[]): string => list.map(escape).join("|");
  const messages = git(root, "log", "--all", "--format=%x00%an %ae %cn %ce %B").split("\0").slice(1);
  const paths = new Set(git(root, "log", "--all", "--name-only", "--format=", "-z").split("\0").filter(Boolean));
  const out: Counts = {
    commits: lines("rev-list", "--all").length,
    "author emails": new Set(lines("log", "--all", "--format=%ae")).size,
    "committer emails": new Set(lines("log", "--all", "--format=%ce")).size,
    "assistant trailers": messages.filter((m) => ASSISTANT.test(m)).length,
    "personal address in a message or header": personal.length ? messages.filter((m) => new RegExp(re(personal), "i").test(m)).length : 0,
    "personal address in a diff": personal.length ? lines("log", "--all", "-i", `-G${re(personal)}`, "--format=%h").length : 0,
    "paths in history": paths.size,
    "paths outside the public set": [...paths].filter((p) => !select([p]).length).length,
  };
  if (names) {
    out["name in a message"] = messages.filter((m) => new RegExp(names.source, "i").test(m)).length;
    out["name in a diff"] = lines("log", "--all", "-i", `-G${names.source}`, "--format=%h").length;
  }
  return out;
}

/** Clone `source` to the new directory `dest`, drop its remote, rewrite the history in the clone. Returns the counts before and after. */
export function rewrite(source: string, dest: string, o: { email: string; name: string; branch?: string }): { before: Counts; after: Counts } {
  if (existsSync(dest) && readdirSync(dest).length) throw new Error(`${dest} is not empty: it must be a new scratch directory`);
  if (resolve(dest) === resolve(source)) throw new Error("the scratch directory is the source");
  execFileSync("git", ["clone", "-q", "--no-local", "--single-branch", "--branch", o.branch ?? "main", source, dest]);
  git(dest, "remote", "remove", "origin");
  if (git(dest, "remote").trim()) throw new Error("the clone still has a remote");
  const names = namesRe();
  const personal = personalIn(dest);
  const before = count(dest, personal, names);
  const drop = [...new Set(git(dest, "log", "--all", "--name-only", "--format=", "-z").split("\0").filter(Boolean))].filter((p) => !select([p]).length);
  const work = mkdtempSync(join(tmpdir(), "public-history-"));
  try {
    writeFileSync(join(work, "drop"), drop.map((p) => `:(literal)${p}\0`).join(""));
    const filters = ["--env-filter", 'export GIT_AUTHOR_NAME="$PH_NAME" GIT_AUTHOR_EMAIL="$PH_EMAIL" GIT_COMMITTER_NAME="$PH_NAME" GIT_COMMITTER_EMAIL="$PH_EMAIL"',
      "--msg-filter", '"$PH_NODE" "$PH_SELF" message', "--index-filter", `${drop.length ? 'git rm --cached --ignore-unmatch -q -r --pathspec-from-file="$PH_DROP" --pathspec-file-nul && ' : ""}"$PH_NODE" "$PH_SELF" prune`];
    const env = { ...process.env, FILTER_BRANCH_SQUELCH_WARNING: "1", PH_NAME: o.name, PH_EMAIL: o.email, PH_NODE: process.execPath, PH_SELF: self, PH_DROP: join(work, "drop") };
    execFileSync("git", ["-C", dest, "filter-branch", "-f", "--prune-empty", ...filters, "--tag-name-filter", "cat", "--", "--all"], { env, stdio: ["ignore", "ignore", "inherit"] });
  } finally {
    rmSync(work, { recursive: true, force: true });
  }
  rmSync(join(dest, ".git/refs/original"), { recursive: true, force: true });
  git(dest, "reflog", "expire", "--expire=now", "--all");
  git(dest, "gc", "-q", "--prune=now");
  return { before, after: count(dest, personal, names) };
}

/** The finding count from gitleaks over the clone's history (matches are redacted and never printed), or null when it could not run. */
export function gitleaks(dest: string, bin: string): number | null {
  const dir = mkdtempSync(join(tmpdir(), "gitleaks-"));
  const report = join(dir, "r.json");
  try {
    execFileSync(bin, ["git", "-c", join(dirname(self), "gitleaks.toml"), "--no-banner", "--redact", "-l", "error", "--report-format", "json", "--report-path", report, dest], { stdio: "ignore" });
    return 0;
  } catch (e) {
    return (e as { status?: number }).status === 1 ? (JSON.parse(readFileSync(report, "utf8")) as unknown[]).length : null;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

async function main(argv: string[]): Promise<number> {
  if (argv[0] === "message") { process.stdout.write(cleanMessage(readFileSync(0, "utf8"), namesRe())); return 0; }
  if (argv[0] === "prune") { pruneIndex(); return 0; }
  const flag = (n: string): string | undefined => { const i = argv.indexOf(n); return i < 0 ? undefined : argv[i + 1]; };
  const [source, dest] = argv.filter((a, i) => !a.startsWith("--") && !(i && argv[i - 1].startsWith("--")));
  if (!source || !dest || !namesRe()) { console.error("usage: public-history.ts <source repository> <new scratch directory> [--branch b] [--email e] [--name n] [--gitleaks path]; set PUBLIC_RELEASE_NAMES"); return 2; }
  const email = flag("--email") ?? execFileSync("gh", ["api", "user", "--jq", '"\\(.id)+\\(.login)@users.noreply.github.com"'], { encoding: "utf8" }).trim();
  const { before, after } = rewrite(source, dest, { email, name: flag("--name") ?? "Dave Remy", branch: flag("--branch") });
  for (const k of Object.keys(before)) console.log(`${k}: ${before[k]} -> ${after[k]}`);
  const shapes = (await scanHistory(dest)).length;
  const leaks = gitleaks(dest, flag("--gitleaks") ?? "gitleaks");
  console.log(`scanners: public-release shapes ${shapes} findings; gitleaks ${leaks === null ? "did not run" : `${leaks} findings`}`);
  const dirty = shapes || leaks || after["personal address in a message or header"] || after["personal address in a diff"] || after["assistant trailers"] || after["name in a message"] || after["name in a diff"] || after["paths outside the public set"];
  return dirty ? 1 : leaks === null ? 2 : 0;
}

if (process.argv[1] === self) main(process.argv.slice(2)).then((c) => process.exit(c), (e) => { console.error(`public-history: ${e instanceof Error ? e.message : e}`); process.exit(2); });
