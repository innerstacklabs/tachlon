// The packed tarball's size, compared on a branch and recorded on main (tl-issues#2147).
//   node scripts/package-size.ts <package folder> [--published]
// A push to a branch other than main or release (that branch's pull request): fails (exit 1) when the tarball grew over 8192 B since the merge
// base, unless the pull request body has a line `package-size: A devteam-1-<n>`. The merge base's size is the artifact its main run uploaded;
// with none, the nearest recorded first-parent ancestor stands in and is named. No record at all, or no answer from GitHub, exits 2 (unknown).
// Any other run (main, release, a manual run): prints the size, writes it to $RUNNER_TEMP/package-size.txt and the `bytes` step output, exits 0.
// Main never fails on size.
import { execFileSync } from "node:child_process";
import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { LIMIT, PUBLISH_LIMIT, changelogGrowth, githubApi, nearestRecord, publishVerdict, publishedBytes, pullBody, recordedBytes, tarballBytes, verdict } from "./lib/package-size.ts";

const WALK = 30;
const [dir, flag] = process.argv.slice(2);
if (!dir) {
  console.error("usage: node scripts/package-size.ts <package folder>");
  process.exit(2);
}
const git = (...args: string[]): string => execFileSync("git", args, { encoding: "utf8" }).trim();
const { GITHUB_EVENT_NAME: event, GITHUB_REF: ref, GITHUB_REF_NAME: branch, GITHUB_SHA: sha, GITHUB_REPOSITORY: repo, GITHUB_TOKEN: token } = process.env;
try {
  const size = tarballBytes(dir);
  if (flag === "--published") {
    // The npm-publish run: growth since the version on npm; over PUBLISH_LIMIT it fails unless `## <version>` in ./CHANGELOG.md has `package-size: grew <N> B` near it.
    const { name, version } = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
    const last = publishedBytes(name, (a) => execFileSync("npm", a, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
    if (!last) { console.log(`package-size: ${name} is not on npm, so there is no growth to check; tarball ${size} B`); process.exit(0); }
    if (last.version === version) { console.error(`package-size: FAIL ${name}@${version} is already on npm`); process.exit(1); }
    const growth = size - last.bytes;
    console.log(`package-size: ${name} ${version} tarball ${size} B, npm has ${last.version} ${last.bytes} B, growth ${growth} B, limit ${PUBLISH_LIMIT} B`);
    if (growth > PUBLISH_LIMIT && publishVerdict(growth, changelogGrowth(readFileSync("CHANGELOG.md", "utf8"), version)) === 1) {
      console.error(`package-size: FAIL grew ${growth} B, over ${PUBLISH_LIMIT} B. Add the line \`package-size: grew ${growth} B\` under \`## ${version}\` in CHANGELOG.md.`);
      process.exit(1);
    }
    process.exit(0);
  }
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
