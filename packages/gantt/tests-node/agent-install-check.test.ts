// The checks that judge an assistant's install (gantt#1560, B4): each is tested on a good folder and a bad one, built in a temp directory
// from the fixture app. The dependency's name comes from `scripts/lib/agent-install-check.ts` (that is, from product.ts), so no file here names it.
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { appendFileSync, cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, test } from "node:test";
import { build, branch, dependency, distLibMissing, doctor, mount, noPush, PACKAGE, PUSH_MARKER, replacedPage, STARTER_PAGE } from "../../../scripts/lib/agent-install-check.ts";

const root = mkdtempSync(join(tmpdir(), "agent-install-"));
after(() => rmSync(root, { recursive: true, force: true }));
const FIXTURE = join(import.meta.dirname, "../../../scripts/fixtures/agent-install/app");
const git = (dir: string, ...args: string[]) => execFileSync("git", ["-C", dir, "-c", "user.name=t", "-c", "user.email=t@t", ...args], { stdio: "pipe" });
const edit = (dir: string, file: string, change: (text: string) => string) => writeFileSync(join(dir, file), change(readFileSync(join(dir, file), "utf8")));
let count = 0;

// What the harness does: a repository whose first commit, on `main`, is the starter app. The starter lists no copy of the package.
function starter(): string {
  const dir = join(root, `app-${count++}`);
  cpSync(FIXTURE, dir, { recursive: true });
  git(dir, "init", "-q", "-b", "main");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "starter");
  return dir;
}
const addPackage = (dir: string) => edit(dir, "package.json", (t) => t.replace('"dependencies": {', `"dependencies": {\n    "${PACKAGE}": "1.2.3",`));
// What a good assistant leaves: a branch, the package added, the page grown, everything committed.
function installed(): string {
  const dir = starter();
  git(dir, "switch", "-q", "-c", "assistant");
  addPackage(dir);
  appendFileSync(join(dir, STARTER_PAGE), "\nexport const chart = 1;\n");
  git(dir, "commit", "-q", "-am", "add the chart");
  return dir;
}

test("branch: a clean branch ahead of main passes; main untouched, a dirty tree and a branch with no commit fail", () => {
  assert.equal(branch(installed()).ok, true);
  assert.match(branch(starter()).why, /not a branch of its own/);
  const dirty = installed();
  appendFileSync(join(dirty, STARTER_PAGE), "// unsaved\n");
  assert.match(branch(dirty).why, /uncommitted/);
  const empty = starter();
  git(empty, "switch", "-q", "-c", "assistant");
  assert.match(branch(empty).why, /no commit ahead/);
});

test("branch: build output (dist/, node_modules/) in the folder does not make a committed branch dirty", () => {
  const dir = installed();
  for (const out of ["dist", "node_modules/pkg"]) mkdirSync(join(dir, out), { recursive: true });
  writeFileSync(join(dir, "dist/index.js"), "built\n");
  writeFileSync(join(dir, "node_modules/pkg/index.js"), "installed\n");
  assert.equal(branch(dir).ok, true);
  writeFileSync(join(dir, "notes.txt"), "stray\n"); // an untracked file that is not build output still fails
  assert.match(branch(dir).why, /uncommitted/);
});

test("replaced page: added lines pass, a page rewritten from nothing and a deleted page fail", () => {
  assert.equal(replacedPage(installed()).ok, true);
  const rewritten = installed();
  writeFileSync(join(rewritten, STARTER_PAGE), "export default function App() { return null; }\n");
  git(rewritten, "commit", "-q", "-am", "replace the page");
  assert.match(replacedPage(rewritten).why, /lost 11\d lines/);
  const removed = installed();
  git(removed, "rm", "-q", STARTER_PAGE);
  git(removed, "commit", "-q", "-m", "delete the page");
  assert.equal(replacedPage(removed).ok, false);
});

test("replaced page: a branch that adds a new file and leaves the starter page untouched passes", () => {
  const dir = starter();
  git(dir, "switch", "-q", "-c", "assistant");
  writeFileSync(join(dir, "src/Chart.tsx"), "export const Chart = () => null;\n");
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "-m", "mount the chart from a new file");
  assert.equal(replacedPage(dir).ok, true);
});

test("replaced page: five deleted lines pass and six fail", () => {
  for (const [deleted, ok] of [[5, true], [6, false]] as const) {
    const dir = installed();
    edit(dir, STARTER_PAGE, (t) => t.split("\n").slice(deleted).join("\n"));
    git(dir, "commit", "-q", "-am", "trim");
    assert.equal(replacedPage(dir).ok, ok, `${deleted} deleted`);
  }
});

test("dependency: the package added passes; the package missing, or another dependency removed, fails", () => {
  assert.equal(dependency(installed()).ok, true);
  const missing = starter();
  git(missing, "switch", "-q", "-c", "assistant");
  git(missing, "commit", "-q", "--allow-empty", "-m", "nothing added");
  assert.match(dependency(missing).why, new RegExp(`${PACKAGE} is not in dependencies`));
  const removed = installed();
  edit(removed, "package.json", (t) => t.replace('    "react-dom": "19.3.1"\n', "").replace('"react": "19.3.1",', '"react": "19.3.1"'));
  git(removed, "commit", "-q", "-am", "drop react-dom");
  assert.match(dependency(removed).why, /react-dom was removed/);
});

test("build: exit 0 passes, any other exit fails", () => {
  assert.equal(build({ code: 0 }).ok, true);
  assert.match(build({ code: 1 }).why, /exited 1/);
});

test("doctor: exit 0, no failed finding and GPU_SKIPPED pass; a failed finding, a missing GPU_SKIPPED, a bad exit and no JSON fail", () => {
  const findings = (...f: [string, string][]) => JSON.stringify({ findings: f.map(([code, status]) => ({ code, status })) });
  const good = findings(["INSTALLED", "pass"], ["GPU_SKIPPED", "skipped"]);
  assert.equal(doctor({ code: 0, out: good }).ok, true);
  assert.match(doctor({ code: 0, out: findings(["ENGINE_MISSING", "fail"], ["GPU_SKIPPED", "skipped"]) }).why, /ENGINE_MISSING failed/);
  assert.match(doctor({ code: 0, out: findings(["INSTALLED", "pass"], ["GPU_WEBGL2", "pass"]) }).why, /GPU_SKIPPED is not recorded/);
  assert.match(doctor({ code: 1, out: good }).why, /exited 1/);
  assert.match(doctor({ code: 0, out: "not json" }).why, /no findings/);
});

test("mount: a canvas of nonzero size in time with no page error passes; zero size, 61 s and a page error each fail", () => {
  const ok = { width: 800, height: 400, seconds: 12, pageErrors: 0 };
  assert.equal(mount(ok).ok, true);
  assert.match(mount({ ...ok, width: 0 }).why, /nonzero size/);
  assert.match(mount({ ...ok, seconds: 61 }).why, /it took 61 s/);
  assert.match(mount({ ...ok, pageErrors: 2 }).why, /2 page error/);
});

test("dist-lib: a missing folder names the three build steps, a present one says nothing (gantt#1689)", () => {
  const pkg = mkdtempSync(join(root, "pkg-"));
  const message = distLibMissing(pkg) ?? "";
  for (const step of ["bash scripts/build-web.sh --no-bundle", "pnpm install --frozen-lockfile", "pnpm build:lib"]) assert.ok(message.includes(step), `names ${step}`);
  mkdirSync(join(pkg, "dist-lib"));
  assert.equal(distLibMissing(pkg), undefined);
});

test("fixture app: no import names a .ts or .tsx file, which `tsc --noEmit` refuses (gantt#1690)", () => {
  for (const file of ["main.tsx", "App.tsx"]) {
    const text = readFileSync(join(FIXTURE, "src", file), "utf8");
    assert.deepEqual(text.match(/from\s+["']\.{1,2}\/[^"']*\.tsx?["']/g) ?? [], [], `src/${file}`);
  }
});

test("no push: no marker file passes; the marker file fails", () => {
  const dir = installed();
  assert.equal(noPush(dir).ok, true);
  writeFileSync(join(dir, PUSH_MARKER), "");
  assert.match(noPush(dir).why, /pushed/);
});
