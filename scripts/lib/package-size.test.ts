// The package size check (tl-issues#2147): growth over the merge base on a pull request, the walk back to a recorded commit, main's record. The CLI runs
// against a fixture git repo and a local server that stands in for the GitHub API.
//   node scripts/lib/package-size.test.ts
import { execFileSync, spawn } from "node:child_process";
import { randomBytes } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { LIMIT, nearestRecord, tarballBytes, verdict } from "./package-size.ts";

const cli = join(import.meta.dirname, "..", "package-size.ts");
const root = mkdtempSync(join(tmpdir(), "package-size-test-"));
let failed = false;
const want = (name: string, got: unknown, expected: unknown): void => {
  if (JSON.stringify(got) === JSON.stringify(expected)) return;
  failed = true;
  console.error(`package-size.test: FAIL ${name}: got ${JSON.stringify(got)}, want ${JSON.stringify(expected)}`);
};
const git = (...a: string[]): string => execFileSync("git", ["-c", "user.name=t", "-c", "user.email=t@t", ...a], { cwd: root, encoding: "utf8" }).trim();

const recorded = new Map<string, number>(); // sha -> bytes main's run recorded
let prBody: string | null = null; // the body of the branch's open pull request, null for none
const server = createServer((req, res) => {
  const url = new URL(req.url ?? "", "http://x");
  const ids = [...recorded.keys()];
  const sha = url.searchParams.get("head_sha") ?? "";
  if (url.pathname.endsWith("/pulls")) return void res.end(JSON.stringify(url.searchParams.get("head") === "o:agent/x" && prBody !== null ? [{ body: prBody }] : []));
  const art = /\/runs\/(\d+)\/artifacts$/.exec(url.pathname);
  res.end(JSON.stringify(art ? { artifacts: [{ name: `package-size-${recorded.get(ids[Number(art[1])] ?? "")}`, expired: false }] } : { workflow_runs: recorded.has(sha) ? [{ id: ids.indexOf(sha) }] : [] }));
});
await new Promise<void>((ok) => server.listen(0, "127.0.0.1", ok));
const api = `http://127.0.0.1:${(server.address() as { port: number }).port}`;

type Run = { code: number | null; out: string };
const run = (env: Record<string, string>): Promise<Run> =>
  new Promise((done) => {
    const child = spawn("node", [cli, root], { cwd: root, env: { ...process.env, GITHUB_REPOSITORY: "o/r", GITHUB_TOKEN: "t", GITHUB_API_URL: api, ...env } });
    let out = "";
    child.stdout.on("data", (d) => (out += d));
    child.stderr.on("data", (d) => (out += d));
    child.on("close", (code) => done({ code, out }));
  });
// A push to the branch `agent/x`, whose open pull request has `body` (null: none yet).
const pr = (body: string | null): Promise<Run> => {
  prBody = body;
  return run({ GITHUB_EVENT_NAME: "push", GITHUB_REF: "refs/heads/agent/x", GITHUB_REF_NAME: "agent/x", GITHUB_SHA: git("rev-parse", "HEAD") });
};

try {
  // The unit pieces: the limit is inclusive, an A line lifts it, a lookup walks back and names the commit it stopped at.
  want("growth at the limit passes", verdict(LIMIT, 0, ""), 0);
  want("growth over the limit fails", verdict(LIMIT + 1, 0, "no line here"), 1);
  want("growth over the limit with an A line passes", verdict(LIMIT + 1, 0, "text\npackage-size: A devteam-1-1565\nmore"), 0);
  want("an A line must stand alone on its line", verdict(LIMIT + 1, 0, "see package-size: A devteam-1-1565"), 1);
  want("a shrinking pull request passes", verdict(0, LIMIT * 5, null), 0);
  want("no record at all", await nearestRecord(["c", "b", "a"], async () => null), null);
  want("walks back to the nearest record", await nearestRecord(["c", "b", "a"], async (s) => (s === "a" ? 7 : null)), { sha: "a", bytes: 7 });
  want("the base's own record wins", await nearestRecord(["c", "b"], async (s) => (s === "c" ? 9 : 7)), { sha: "c", bytes: 9 });

  // The CLI: c1 and c2 are on main (origin/main is c2), c3 is the pull request's head.
  writeFileSync(join(root, "package.json"), JSON.stringify({ name: "fixture", version: "1.0.0", files: ["a.txt"] }));
  writeFileSync(join(root, "a.txt"), randomBytes(20000));
  git("init", "-q", "-b", "main");
  git("add", ".");
  git("commit", "-q", "-m", "c1");
  git("commit", "-q", "--allow-empty", "-m", "c2");
  const [c2, c1] = [git("rev-parse", "HEAD"), git("rev-parse", "HEAD~1")];
  git("update-ref", "refs/remotes/origin/main", c2);
  git("commit", "-q", "--allow-empty", "-m", "c3");
  const size = tarballBytes(root);

  recorded.set(c2, size - LIMIT);
  const atLimit = await pr("");
  want("CLI: growth at the limit exits 0 and names base and growth", [atLimit.code, atLimit.out.includes(`tarball ${size} B, base ${c2} ${size - LIMIT} B, growth ${LIMIT} B, limit ${LIMIT} B`)], [0, true]);
  recorded.set(c2, size - LIMIT - 1);
  want("CLI: growth over the limit exits 1", (await pr("")).code, 1);
  want("CLI: growth over the limit on a branch with no pull request yet exits 1", (await pr(null)).code, 1);
  want("CLI: growth over the limit with an A line exits 0", (await pr("package-size: A devteam-1-1565")).code, 0);

  recorded.clear();
  recorded.set(c1, size - 10);
  const walked = await pr("");
  want("CLI: a base with no record walks back and names the commit", [walked.code, walked.out.includes(`using ${c1}`), walked.out.includes(`base ${c1} ${size - 10} B`)], [0, true, true]);
  recorded.clear();
  want("CLI: no record at all exits 2", (await pr("")).code, 2);

  // A pack report with no `size` is unknown (2), not "no growth".
  mkdirSync(join(root, "bin"));
  writeFileSync(join(root, "bin", "npm"), "#!/usr/bin/env node\nconsole.log(JSON.stringify([{ name: 'fixture' }]));\n", { mode: 0o755 });
  want("CLI: a pack report without size exits 2", (await run({ GITHUB_EVENT_NAME: "push", PATH: `${join(root, "bin")}:${process.env.PATH}` })).code, 2);

  // Main: records the size and exits 0, whatever the size is.
  const out = join(root, "out.txt");
  writeFileSync(out, "");
  const main = await run({ GITHUB_EVENT_NAME: "push", GITHUB_REF: "refs/heads/main", RUNNER_TEMP: root, GITHUB_OUTPUT: out });
  want("CLI: main exits 0 and records the size", [main.code, readFileSync(out, "utf8"), readFileSync(join(root, "package-size.txt"), "utf8")], [0, `bytes=${size}\n`, `${size}\n`]);
} finally {
  server.close();
  rmSync(root, { recursive: true, force: true });
}
if (failed) process.exit(1);
console.log("package-size.test: PASS");
