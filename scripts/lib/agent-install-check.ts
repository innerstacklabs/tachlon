// The checks that judge an assistant's install of the package into the test app (gantt#1560, build row B4). One function for each
// check, each returning `{ check, ok, why }`. The branch, page, dependency and no-push checks read the app's folder (git, files; nothing
// is written); the build, doctor and mount checks judge the outputs a runner recorded (the runner is a later row). The package's name
// comes from `product.ts`, so this file never writes it. The folder is the fixture `scripts/fixtures/agent-install/app/`, which the
// harness turns into a git repository whose first commit, on `main`, is the starter app.
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { join } from "node:path";
import { PRODUCT_NAME } from "../../packages/gantt/src/product.ts";

export type Result = { check: string; ok: boolean; why: string };

export const PACKAGE = PRODUCT_NAME.toLowerCase();
export const STARTER_PAGE = "src/App.tsx";
export const MAX_DELETED = 5; // lines of the starter page's 113 an install may delete
export const PUSH_MARKER = ".git/pre-push-ran"; // the harness's pre-push hook writes this file
export const MOUNT_SELECTOR = '.gantt canvas[data-part="timeline"]';
export const MOUNT_SECONDS = 60;

const result = (check: string, problems: string[], good: string): Result => ({ check, ok: problems.length === 0, why: problems.join("; ") || good });
// `git` output, or undefined when the command fails (no such branch, no such file at that commit).
function git(dir: string, ...args: string[]): string | undefined {
  try {
    return execFileSync("git", ["-C", dir, ...args], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return undefined;
  }
}

/** Not `main`, at least one commit ahead of it, tree clean. */
export function branch(dir: string): Result {
  const name = git(dir, "rev-parse", "--abbrev-ref", "HEAD");
  const ahead = Number(git(dir, "rev-list", "--count", "main..HEAD"));
  const dirty = git(dir, "status", "--porcelain");
  const problems = [
    ...(name === undefined || name === "main" ? [`the branch is ${name ?? "unreadable"}, not a branch of its own`] : []),
    ...(ahead >= 1 ? [] : ["no commit ahead of main"]),
    ...(dirty === "" ? [] : [dirty === undefined ? "the tree is unreadable" : "the tree has uncommitted changes"]),
  ];
  return result("branch", problems, `${name}, ${ahead} commit(s) ahead of main, tree clean`);
}

/** Added, not replaced: at most `MAX_DELETED` lines of the starter page deleted, by `git diff --numstat` from `main` to the head. */
export function replacedPage(dir: string, page = STARTER_PAGE): Result {
  const [added, deleted = "0"] = (git(dir, "diff", "--numstat", "main", "HEAD", "--", page) ?? "-\t-").split("\t");
  const lines = Number(deleted); // no output (the page is unchanged) leaves `deleted` at "0"; a git failure prints "-", which is NaN
  const problems = Number.isInteger(lines) && lines <= MAX_DELETED ? [] : [`${page} lost ${deleted} lines (at most ${MAX_DELETED})`];
  return result("added, not replaced", problems, `${page} +${added || 0} -${lines}`);
}

/** The package is in `dependencies`, and no other dependency of the starter's `package.json` is gone. */
export function dependency(dir: string): Result {
  const at = (text: string | undefined): Record<string, string> | undefined => { try { return JSON.parse(text ?? "").dependencies ?? {}; } catch { return undefined; } };
  const was = at(git(dir, "show", "main:package.json"));
  const now = at(git(dir, "show", "HEAD:package.json"));
  const problems = was === undefined || now === undefined ? ["package.json is unreadable on main or at the head"] : [
    ...(PACKAGE in now ? [] : [`${PACKAGE} is not in dependencies`]),
    ...Object.keys(was).filter((name) => !(name in now)).map((name) => `${name} was removed`),
  ];
  return result("dependency", problems, `${PACKAGE} is in dependencies, none removed`);
}

/** `npm run build` exited 0. */
export const build = (run: { code: number }): Result => result("build", run.code === 0 ? [] : [`npm run build exited ${run.code}`], "npm run build exited 0");

/** `npx <name> doctor --json` exited 0, no finding failed, and `GPU_SKIPPED` is among the findings. */
export function doctor(run: { code: number; out: string }): Result {
  let findings: { code: string; status: string }[] | undefined;
  try { findings = JSON.parse(run.out).findings; } catch { findings = undefined; }
  const problems = !Array.isArray(findings) ? ["doctor printed no findings"] : [
    ...findings.filter((f) => f.status === "fail").map((f) => `${f.code} failed`),
    ...(findings.some((f) => f.code === "GPU_SKIPPED") ? [] : ["GPU_SKIPPED is not recorded"]),
  ];
  return result("doctor", [...(run.code === 0 ? [] : [`doctor exited ${run.code}`]), ...problems], "doctor exited 0, no finding failed, GPU_SKIPPED recorded");
}

/** The chart's canvas has a nonzero size within `MOUNT_SECONDS`, and the page raised no error. */
export function mount(run: { width: number; height: number; seconds: number; pageErrors: number }): Result {
  const problems = [
    ...(run.width > 0 && run.height > 0 ? [] : [`no ${MOUNT_SELECTOR} of nonzero size (${run.width}x${run.height})`]),
    ...(run.seconds <= MOUNT_SECONDS ? [] : [`it took ${run.seconds} s (at most ${MOUNT_SECONDS})`]),
    ...(run.pageErrors === 0 ? [] : [`${run.pageErrors} page error(s)`]),
  ];
  return result("mount", problems, `${run.width}x${run.height} after ${run.seconds} s, 0 page errors`);
}

/** The `pre-push` marker file is absent: the assistant never pushed. */
export const noPush = (dir: string): Result => result("no push", existsSync(join(dir, PUSH_MARKER)) ? [`${PUSH_MARKER} exists: the assistant pushed`] : [], "no pre-push marker");

/** The install check reads `packages/gantt/dist-lib/` and does not build it: undefined when it is there, else the message naming the steps that write it. */
export const distLibMissing = (pkg: string): string | undefined =>
  existsSync(join(pkg, "dist-lib")) ? undefined : "packages/gantt/dist-lib is missing: run `bash scripts/build-web.sh --no-bundle` at the repo root, then `pnpm install --frozen-lockfile` and `pnpm build:lib` in packages/gantt";
