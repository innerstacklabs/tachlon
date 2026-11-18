// The packed tarball's size, compared on a branch and recorded on main (tl-issues#2147).
//   node scripts/package-size.ts <package folder>
// A push to a branch other than main or release (that branch's pull request): fails (exit 1) when the tarball grew over 8192 B since the merge
// base, unless the pull request body has a line `package-size: A devteam-1-<n>`. The merge base's size is the artifact its main run uploaded;
// with none, the nearest recorded first-parent ancestor stands in and is named. No record at all, or no answer from GitHub, exits 2 (unknown).
// Any other run (main, release, a manual run): prints the size, writes it to $RUNNER_TEMP/package-size.txt and the `bytes` step output, exits 0.
// Main never fails on size.
import { execFileSync } from "node:child_process";
import { appendFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { LIMIT, githubApi, nearestRecord, pullBody, recordedBytes, tarballBytes, verdict } from "./lib/package-size.ts";

const WALK = 30;
const dir = process.argv[2];
if (!dir) {
  console.error("usage: node scripts/package-size.ts <package folder>");
  process.exit(2);
}
const git = (...args: string[]): string => execFileSync("git", args, { encoding: "utf8" }).trim();
const { GITHUB_EVENT_NAME: event, GITHUB_REF: ref, GITHUB_REF_NAME: branch, GITHUB_SHA: sha, GITHUB_REPOSITORY: repo, GITHUB_TOKEN: token } = process.env;
try {
  const size = tarballBytes(dir);
  if (event !== "push" || ref === "refs/heads/main" || ref === "refs/heads/release") {
    console.log(`package-size: ${dir} tarball ${size} B`);
    if (process.env.RUNNER_TEMP) writeFileSync(join(process.env.RUNNER_TEMP, "package-size.txt"), `${size}\n`);
    if (process.env.GITHUB_OUTPUT) appendFileSync(process.env.GITHUB_OUTPUT, `bytes=${size}\n`);
    process.exit(0);
  }
  if (!repo || !token || !branch || !sha) throw new Error("a branch run needs GITHUB_REPOSITORY, GITHUB_TOKEN, GITHUB_REF_NAME and GITHUB_SHA");
  const api = githubApi(repo, token);
  if (git("rev-parse", "--is-shallow-repository") === "true") git("fetch", "--no-tags", `--depth=${WALK * 7}`, "origin", "+refs/heads/main:refs/remotes/origin/main", sha);
  const base = git("merge-base", "origin/main", "HEAD");
  const found = await nearestRecord(git("rev-list", "--first-parent", "-n", String(WALK), base).split("\n"), recordedBytes(api));
  if (!found) throw new Error(`no recorded size for merge base ${base} or its ${WALK - 1} first-parent ancestors`);
  if (found.sha !== base) console.log(`package-size: merge base ${base} has no record; using ${found.sha}, the nearest one on main`);
  const growth = size - found.bytes;
  console.log(`package-size: ${dir} tarball ${size} B, base ${found.sha} ${found.bytes} B, growth ${growth} B, limit ${LIMIT} B`);
  if (growth > LIMIT && verdict(size, found.bytes, await pullBody(api, repo, branch)) === 1) {
    console.error(`package-size: FAIL ${dir}: grew ${growth} B, over ${LIMIT} B. Shrink the change, or add the line \`package-size: A devteam-1-<n>\` to the pull request body and re-run this job.`);
    process.exit(1);
  }
} catch (e) {
  console.error(`package-size: ${(e as Error).message}`);
  process.exit(2);
}
