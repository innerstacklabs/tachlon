// The packed tarball's growth check (tl#1091, tl-issues#2147). `npm pack --dry-run` reports the tarball's size without writing it.
import { execFileSync } from "node:child_process";

// What one pull request may add to the tarball without an A line, in bytes.
export const LIMIT = 8192;

// The bytes of the tarball `npm pack` would write for the package in `dir`. Throws when the pack report has no size.
export function tarballBytes(dir: string): number {
  const [packed] = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: dir, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
  if (typeof packed?.size !== "number") throw new Error(`npm pack --dry-run --json in ${dir} reported no "size" number`);
  return packed.size;
}

// True when the pull request body has a line `package-size: A devteam-1-<n>`, the product owner's decision for a row that grows the package past the limit.
export function hasAllowance(body: string | null | undefined): boolean {
  return /^package-size: A devteam-1-\d+[ \t]*$/m.test(body ?? "");
}

// Exit code for a pull request that grows the tarball by `size - baseBytes`: 1 over the limit without an A line, else 0.
export function verdict(size: number, baseBytes: number, body: string | null | undefined): 0 | 1 {
  return size - baseBytes > LIMIT && !hasAllowance(body) ? 1 : 0;
}

// What a publish run may add to the tarball over the version on npm without a changelog line, and how far that line may be from the measured growth, in bytes.
export const PUBLISH_LIMIT = 65536;
export const TOLERANCE = 4096;

// The version `latest` names on npm and the size of its tarball, or null when the package is not on npm (E404, a first publish). `run` calls npm and throws on a non-zero exit; any other failure propagates.
export function publishedBytes(name: string, run: (args: string[]) => string): { version: string; bytes: number } | null {
  let version: string;
  try { version = run(["view", name, "dist-tags.latest"]).trim(); } catch (e) { if (/E404/.test(String(e))) return null; throw e; }
  const [packed] = JSON.parse(run(["pack", `${name}@${version}`, "--dry-run", "--json", "--ignore-scripts"]));
  if (typeof packed?.size !== "number") throw new Error(`npm pack ${name}@${version} --dry-run --json reported no "size" number`);
  return { version, bytes: packed.size };
}

// The N of the line `package-size: grew <N> B` under the heading `## <version>` of the changelog `text`, or null when that section has none. A line under another heading does not count.
export function changelogGrowth(text: string, version: string): number | null {
  let inside = false;
  for (const line of text.split("\n")) {
    if (line.startsWith("## ")) inside = line.trimEnd() === `## ${version}`;
    const m = inside && /^package-size: grew (\d+) B[ \t]*$/.exec(line);
    if (m) return Number(m[1]);
  }
  return null;
}

// Exit code for a publish that grew the tarball by `growth`: 1 over PUBLISH_LIMIT unless the changelog names a growth within TOLERANCE of it, else 0.
export function publishVerdict(growth: number, named: number | null): 0 | 1 {
  return growth > PUBLISH_LIMIT && !(named !== null && Math.abs(named - growth) <= TOLERANCE) ? 1 : 0;
}

export type Recorded = { sha: string; bytes: number };

// The first of `shas` (newest first) that `lookup` has a size for, or null when none has. A lookup that throws is not an answer, so it propagates.
export async function nearestRecord(shas: string[], lookup: (sha: string) => Promise<number | null>): Promise<Recorded | null> {
  for (const sha of shas) {
    const bytes = await lookup(sha);
    if (bytes !== null) return { sha, bytes };
  }
  return null;
}

// A GET against the repository's REST API; read-only (`actions: read`, `pull-requests: read`), so the workflow's token needs no write scope.
export function githubApi(repo: string, token: string): (path: string) => Promise<any> {
  return async (path) => {
    const res = await fetch(`${process.env.GITHUB_API_URL ?? "https://api.github.com"}/repos/${repo}${path}`, { headers: { authorization: `Bearer ${token}`, accept: "application/vnd.github+json" } });
    if (!res.ok) throw new Error(`GitHub ${path} answered ${res.status}`);
    return res.json();
  };
}

// Main's run uploads an artifact `package-size-<bytes>`; this finds it through the push runs of `sha`.
export function recordedBytes(api: (path: string) => Promise<any>): (sha: string) => Promise<number | null> {
  return async (sha) => {
    for (const run of (await api(`/actions/runs?head_sha=${sha}&event=push&per_page=10`)).workflow_runs) {
      for (const a of (await api(`/actions/runs/${run.id}/artifacts`)).artifacts) {
        const m = /^package-size-(\d+)$/.exec(a.name);
        if (m && !a.expired) return Number(m[1]);
      }
    }
    return null;
  };
}

// The body of the open pull request from `branch` (a push run's payload does not carry it), or null when the branch has none yet.
export async function pullBody(api: (path: string) => Promise<any>, repo: string, branch: string): Promise<string | null> {
  const [pull] = await api(`/pulls?head=${encodeURIComponent(`${repo.split("/")[0]}:${branch}`)}&state=open`);
  return pull?.body ?? null;
}
