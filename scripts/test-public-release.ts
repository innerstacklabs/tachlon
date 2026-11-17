// Checks the open-source release script (scripts/public-release.ts): the include list on one fixture and on the real tree, and the secret scan.
//   node scripts/test-public-release.ts
// No network. The secret-shaped values are built from parts, so this file holds none. Exits 1, one FAIL line per problem.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, posix } from "node:path";
import { pruneList, scanHistory, scanNames, scanTree, select, shapesIn } from "./public-release.ts";

let failed = false;
const check = (name: string, got: unknown, want: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(want)) return;
  failed = true;
  console.error(`test-public-release: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(want)}`);
};

// One fixture: the product, the agent and review rules and the CI go in (the repo is worked in directly, gantt#2089);
// the other internal files and the hosted-service code stay out.
const fixture = [
  "README.md", "LICENSE", "crates/xer/src/lib.rs", "packages/gantt/src/index.ts", ".github/workflows/pr.yml", "scripts/build-web.sh",
  "CLAUDE.md", "REVIEW.md", "CLA.md", ".claude/skills/run/SKILL.md", ".github/workflows/ci.yml", ".github/workflows/npm-publish.yml",
  "apps/cloud/src/store.ts", "docs/PRODUCT.md", "docs/PLAN.md", "docs/design/a.md", "docs/decisions/0003-product-direction.md", "docs/decisions/0013-x.md", "docs/research/a.md", "demos/screencast/upload/hub-page.mjs", "docs/ACCESS.md",
  "scripts/test-demo-access.ts", "docs/ARCHITECTURE.md",
];
check("the include list on the fixture", select(fixture), [
  "README.md", "LICENSE", "crates/xer/src/lib.rs", "packages/gantt/src/index.ts", ".github/workflows/pr.yml", "scripts/build-web.sh",
  "CLAUDE.md", "REVIEW.md", ".github/workflows/ci.yml", ".github/workflows/npm-publish.yml", "docs/decisions/0013-x.md", "docs/ARCHITECTURE.md",
]);
check("an excluded file wins over an included directory", select(["scripts/lib/demo-access.ts", "scripts/lib/web-root.ts"]), ["scripts/lib/web-root.ts"]);

check("the allowlist loses the lines for paths that stay out", pruneList("# c\na.ts | x # why\nb.ts\nc.ts | y", new Set(["a.ts", "b.ts"])), "# c\na.ts | x # why\nb.ts");

// The real tree: none of the files the plan keeps out is selected.
const root = execFileSync("git", ["rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim();
const real = execFileSync("git", ["-C", root, "ls-files"], { encoding: "utf8" }).split("\n").filter(Boolean);
const out = ["CLA.md", ".claude/", "docs/PRODUCT.md", "docs/PLAN.md", "docs/design/", "docs/decisions/0003-product-direction.md", "docs/research/2026-10-market.md", "docs/research/dhtmlx-gantt.md", "demos/screencast/upload/", "apps/cloud/"];
check("the real tree keeps the internal files out", select(real).filter((p) => out.some((o) => (o.endsWith("/") ? p.startsWith(o) : p === o))), []);
check("the real tree keeps the product in", ["README.md", "crates/engine/Cargo.toml", "packages/gantt/package.json"].filter((p) => !select(real).includes(p)), []);
check("the real tree keeps the CI and the scripts it runs", [".github/workflows/ci.yml", ".github/actions/wasm-bindings/action.yml", "scripts/touched-specs.ts", "scripts/ci-tier.ts", "CLAUDE.md", "REVIEW.md"].filter((p) => !select(real).includes(p)), []);
const selected = new Set(select(real));
const dropped = [...selected].filter((p) => p.endsWith(".md")).flatMap((from) =>
  [...readFileSync(join(root, from), "utf8").matchAll(/\]\(((?!\/)[^)#\s:]+)/g)].map((m) => [from, posix.join(posix.dirname(from), m[1])]))
  .filter(([, to]) => real.includes(to) && !selected.has(to)).map(([from, to]) => `${from} -> ${to}`);
check("no selected Markdown page links to a tracked file the public copy leaves out", dropped, []);

// The secret scan finds a shape in the tree and in history only, and says nothing else.
const key = "AKIA" + "ABCDEFGHIJKLMNOP";
check("a shape is found", shapesIn(`token = ${key}`), ["aws-access-key"]);
check("plain text is clean", shapesIn("https://example.com/sk-the-expert-says-so and a url https://example.com:8080/x"), []);
const repo = mkdtempSync(join(tmpdir(), "public-release-"));
const git = (...args: string[]) => execFileSync("git", ["-C", repo, "-c", "user.name=t", "-c", "user.email=t@example.com", ...args], { encoding: "utf8" });
try {
  git("init", "-q");
  mkdirSync(join(repo, "a"));
  writeFileSync(join(repo, "a/old.txt"), `key=${key}\n`);
  writeFileSync(join(repo, "a/note.txt"), "a zebra\nplain\n");
  check("a name is found by path and line, and the name is not printed", scanNames(repo, ["a/note.txt", "a/old.txt"], /zebra/i), ["a/note.txt:1"]);
  git("add", "-A"); git("commit", "-q", "-m", "one");
  writeFileSync(join(repo, "a/old.txt"), "key removed\n");
  git("commit", "-q", "-am", "two");
  check("the tree is clean once the line is gone", scanTree(repo, ["a/old.txt"]), []);
  const found = await scanHistory(repo);
  check("history still holds it, as commit, path and rule only", found.map((f) => f.split(" ").slice(1).join(" ")), ["a/old.txt aws-access-key"]);
  check("no value is printed", found.some((f) => f.includes(key)), false);
} finally {
  rmSync(repo, { recursive: true, force: true });
}

process.exit(failed ? 1 : 0);
