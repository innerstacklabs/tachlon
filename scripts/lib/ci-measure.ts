// What one day of CI use looks like (the measurement behind daveremy/lifeos#2677). Pure functions over
// the GitHub Actions API's JSON; the CLI in `scripts/ci-measure.ts` does the reading. Times are UTC in
// the rows; the report prints MST (UTC minus 7h, Arizona has no daylight time) beside them.

export interface ApiRun {
  id: number;
  event: string;
  created_at: string;
}

export interface ApiJob {
  id: number;
  run_id: number;
  name: string;
  status: string;
  conclusion: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  runner_name: string | null;
  labels: string[];
}

/** One page of `actions/runs` or of `actions/runs/<id>/jobs`, as the API returns it. */
export interface RunsPage {
  total_count: number;
  workflow_runs: ApiRun[];
}
export interface JobsPage {
  total_count: number;
  jobs: ApiJob[];
}

/** Where the pages come from. A failed read throws; the page count is the API request count. */
export interface Source {
  runs(day: string): RunsPage[];
  jobs(runId: number): JobsPage[];
}

export type RunnerKind = "fleet" | "hosted" | "vendor";

/** One row per job attempt. Times are UTC. */
export interface Row {
  day: string;
  run_id: number;
  job_id: number;
  name: string;
  event: string;
  class: string;
  conclusion: string;
  runner_name: string | null;
  runner_kind: RunnerKind;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  /** Seconds from created to started; null for a job that never ran. */
  wait_s: number | null;
  /** Seconds a job that never ran waited before it was cancelled or failed; null for a job that ran. */
  abandoned_wait_s: number | null;
  run_s: number;
  billable_min: number;
}

/** The API's cap on a filtered runs list (`created=`): beyond it the list is cut without an error. */
export const RUNS_CAP = 1000;

const CLASS_NAMES = ["select", "rust", "wasm", "web", "hosted", "cloud", "book", "deploy"] as const;

/** `web (2)` is a matrix shard of `web`; `ci-summary` is `summary`; a name the map lacks is `other`. */
export function classOf(name: string): string {
  if (name === "ci-summary") return "summary";
  const base = name.split(" ")[0];
  return (CLASS_NAMES as readonly string[]).includes(base) ? base : "other";
}

/** Our own runners carry `tachlon-ci` (or the later `ci-select`); GitHub's carry no `self-hosted` and an OS label. */
export function runnerKind(labels: readonly string[]): RunnerKind {
  if (labels.includes("self-hosted")) {
    return labels.includes("tachlon-ci") || labels.includes("ci-select") ? "fleet" : "vendor";
  }
  return labels.some((l) => /^(ubuntu|windows|macos)-/.test(l)) ? "hosted" : "vendor";
}

const secs = (from: string, to: string): number => Math.max(0, Math.round((Date.parse(to) - Date.parse(from)) / 1000));

/**
 * What a read found. `truncated` holds every reason the read is not a whole day: a cap, a failed call,
 * a count that does not add up, an unfinished job. Empty means complete.
 */
export interface Collected {
  rows: Row[];
  skipped: number;
  /** Copies of an earlier attempt's job that a re-run listed again (see dropCarried). */
  carried: number;
  open: number;
  runs: number;
  /** API requests made (pages read), and `gh api` invocations. */
  requests: number;
  invocations: number;
  truncated: string[];
}

const sum = (xs: readonly number[]): number => xs.reduce((a, b) => a + b, 0);

/**
 * Turns one job into a row, or null for a job the measure drops. `skipped` rows are dropped. A job
 * that never ran has no runner: the API leaves `started_at` empty or sets it to `created_at`, so the
 * runner decides. Its whole wait goes in `abandoned_wait_s` and it adds no job time.
 */
export function toRow(job: ApiJob, event: string): Row | null {
  if (job.conclusion === "skipped") return null;
  const ran = job.started_at !== null && !!job.runner_name;
  const done = job.completed_at ?? job.created_at;
  const run_s = ran && job.completed_at ? secs(job.started_at as string, job.completed_at) : 0;
  return {
    day: job.created_at.slice(0, 10),
    run_id: job.run_id,
    job_id: job.id,
    name: job.name,
    event,
    class: classOf(job.name),
    conclusion: job.conclusion ?? "none",
    runner_name: ran ? job.runner_name : null,
    runner_kind: runnerKind(job.labels),
    created_at: job.created_at,
    started_at: ran ? job.started_at : null,
    completed_at: job.completed_at,
    wait_s: ran ? secs(job.created_at, job.started_at as string) : null,
    abandoned_wait_s: ran ? null : secs(job.created_at, done),
    run_s,
    billable_min: Math.ceil(run_s / 60),
  };
}

/**
 * A re-run attempt lists the jobs it did not run again as new jobs (new id, new `created_at`) that carry
 * the first attempt's runner and times. Counting both runs one execution twice (measured on a real day:
 * one job listed six times, which raised the peak concurrency to 18 on six runners). Of rows that share
 * a run, name, runner, start and end, the lowest job id is the execution and the rest are copies.
 */
export function dropCarried(rows: readonly Row[]): { rows: Row[]; carried: number } {
  const seen = new Set<string>();
  const kept: Row[] = [];
  for (const r of [...rows].sort((a, b) => a.job_id - b.job_id)) {
    const key = r.runner_name === null ? null : [r.run_id, r.name, r.runner_name, r.started_at, r.completed_at].join("|");
    if (key !== null && seen.has(key)) continue;
    if (key !== null) seen.add(key);
    kept.push(r);
  }
  return { rows: kept, carried: rows.length - kept.length };
}

/** Reads one UTC day's runs and every attempt of every job. Never throws: a failure becomes a `truncated` reason. */
export function collect(day: string, src: Source): Collected {
  const out: Collected = { rows: [], skipped: 0, carried: 0, open: 0, runs: 0, requests: 0, invocations: 0, truncated: [] };
  let runs: ApiRun[] = [];
  try {
    const pages = src.runs(day);
    out.invocations += 1;
    out.requests += pages.length;
    runs = pages.flatMap((p) => p.workflow_runs);
    const total = pages[0]?.total_count ?? 0;
    if (total > RUNS_CAP) out.truncated.push(`the runs list is capped: ${total} runs match, the API returns at most ${RUNS_CAP}`);
    if (runs.length !== total) out.truncated.push(`runs read ${runs.length} of ${total}`);
  } catch (e) {
    out.truncated.push(`runs read failed: ${(e as Error).message}`);
  }
  out.runs = runs.length;
  for (const run of runs) {
    try {
      const pages = src.jobs(run.id);
      out.invocations += 1;
      out.requests += pages.length;
      const jobs = pages.flatMap((p) => p.jobs);
      const total = pages[0]?.total_count ?? 0;
      if (jobs.length !== total) out.truncated.push(`run ${run.id}: jobs read ${jobs.length} of ${total}`);
      const runRows: Row[] = [];
      for (const job of jobs) {
        if (job.status !== "completed") {
          out.open += 1;
        } else {
          const row = toRow(job, run.event);
          if (row) runRows.push(row);
          else out.skipped += 1;
        }
      }
      const unique = dropCarried(runRows);
      out.rows.push(...unique.rows);
      out.carried += unique.carried;
    } catch (e) {
      out.truncated.push(`run ${run.id}: jobs read failed: ${(e as Error).message}`);
    }
  }
  if (out.open > 0) out.truncated.push(`${out.open} jobs not finished: their waits are missing from the day`);
  return out;
}

/** Nearest rank: the smallest value with at least p percent of the values at or below it. */
export function percentile(sorted: readonly number[], p: number): number | null {
  if (sorted.length === 0) return null;
  const rank = Math.max(1, Math.ceil((p * sorted.length) / 100));
  return sorted[rank - 1];
}

export interface WaitStats {
  n: number;
  p50: number;
  p90: number;
  max: number;
}

export function waitStats(waits: readonly number[]): WaitStats | null {
  if (waits.length === 0) return null;
  const s = [...waits].sort((a, b) => a - b);
  return { n: s.length, p50: percentile(s, 50) as number, p90: percentile(s, 90) as number, max: s[s.length - 1] };
}

/** The most intervals open at once, and the first time that happened. An interval is [start, end): an end frees its slot before a start at the same second takes one. */
export function peakOverlap(intervals: ReadonlyArray<readonly [string, string]>): { peak: number; at: string | null } {
  const events: Array<[number, number]> = [];
  for (const [from, to] of intervals) {
    const a = Date.parse(from);
    const b = Date.parse(to);
    if (b > a) events.push([a, 1], [b, -1]);
  }
  events.sort((x, y) => x[0] - y[0] || x[1] - y[1]);
  let open = 0;
  let peak = 0;
  let at: number | null = null;
  for (const [t, d] of events) {
    open += d;
    if (open > peak) {
      peak = open;
      at = t;
    }
  }
  return { peak, at: at === null ? null : new Date(at).toISOString().replace(".000Z", "Z") };
}

export interface Summary {
  jobs: number;
  jobSeconds: number;
  billableMin: number;
  cancelledSeconds: number;
  abandoned: { n: number; seconds: number };
  wait: WaitStats | null;
  waitByRunner: Map<string, WaitStats>;
  waitByClass: Map<string, WaitStats>;
  selectWait: WaitStats | null;
  secondsByClass: Map<string, number>;
  concurrency: { peak: number; at: string | null };
  fleetConcurrency: { peak: number; at: string | null };
  queued: { peak: number; at: string | null };
}

function groupWaits(rows: readonly Row[], key: (r: Row) => string): Map<string, WaitStats> {
  const groups = new Map<string, number[]>();
  for (const r of rows) {
    if (r.wait_s === null) continue;
    groups.set(key(r), [...(groups.get(key(r)) ?? []), r.wait_s]);
  }
  return new Map([...groups].sort(([a], [b]) => a.localeCompare(b)).map(([k, v]) => [k, waitStats(v) as WaitStats]));
}

/** The one row the per-runner table keeps for every GitHub-hosted runner. */
export const HOSTED_RUNNERS = "(GitHub-hosted)";

export function summarize(rows: readonly Row[]): Summary {
  const ran = rows.filter((r) => r.started_at !== null && r.completed_at !== null);
  const secondsByClass = new Map<string, number>();
  for (const r of rows) secondsByClass.set(r.class, (secondsByClass.get(r.class) ?? 0) + r.run_s);
  const running = (r: Row): [string, string] => [r.started_at as string, r.completed_at as string];
  const queuedIn = (r: Row): [string, string] => [r.created_at, (r.started_at ?? r.completed_at ?? r.created_at) as string];
  const abandoned = rows.filter((r) => r.abandoned_wait_s !== null);
  return {
    jobs: rows.length,
    jobSeconds: sum(rows.map((r) => r.run_s)),
    billableMin: sum(rows.map((r) => r.billable_min)),
    cancelledSeconds: sum(rows.filter((r) => r.conclusion === "cancelled").map((r) => r.run_s)),
    abandoned: { n: abandoned.length, seconds: sum(abandoned.map((r) => r.abandoned_wait_s as number)) },
    wait: waitStats(rows.flatMap((r) => (r.wait_s === null ? [] : [r.wait_s]))),
    // GitHub names every hosted runner afresh ("GitHub Actions 1000019117"), so one row per name says nothing.
    waitByRunner: groupWaits(rows, (r) => (r.runner_kind === "hosted" ? HOSTED_RUNNERS : (r.runner_name ?? ""))),
    waitByClass: groupWaits(rows, (r) => r.class),
    selectWait: waitStats(rows.flatMap((r) => (r.class === "select" && r.wait_s !== null ? [r.wait_s] : []))),
    secondsByClass: new Map([...secondsByClass].sort(([a], [b]) => a.localeCompare(b))),
    concurrency: peakOverlap(ran.map(running)),
    fleetConcurrency: peakOverlap(ran.filter((r) => r.runner_kind === "fleet").map(running)),
    queued: peakOverlap(rows.map(queuedIn)),
  };
}

const min1 = (s: number): string => (s / 60).toFixed(1);
/** The UTC time with the MST time beside it. */
export const withMst = (iso: string | null): string =>
  iso === null ? "none" : `${iso} (${new Date(Date.parse(iso) - 7 * 3600_000).toISOString().slice(0, 19).replace("T", " ")} MST)`;

function waitLine(w: WaitStats | null): string {
  return w === null ? "n=0" : `n=${w.n} p50=${w.p50} p90=${w.p90} max=${w.max}`;
}

/** The daily outputs as text. `loadavg` is printed last: the box's load colours every wait it measured. */
export function formatReport(day: string, c: Collected, loadavg: string): string {
  const s = summarize(c.rows);
  const share = s.jobSeconds === 0 ? "n/a" : `${((100 * s.cancelledSeconds) / s.jobSeconds).toFixed(1)}%`;
  const start = Date.parse(`${day}T00:00:00Z`);
  const mst = (ms: number): string => new Date(ms - 7 * 3600_000).toISOString().slice(0, 16).replace("T", " ");
  const lines = [
    `ci-measure: day ${day} UTC, which is ${mst(start)} to ${mst(start + 86_400_000)} MST`,
    `read: ${c.runs} runs created on the day, ${c.rows.length} jobs, ${c.skipped} skipped and ${c.carried} re-run copies dropped, ${c.open} unfinished; ${c.requests} API requests in ${c.invocations} gh api calls`,
    `jobs: ${s.jobs}`,
    `job-minutes: ${min1(s.jobSeconds)}`,
    `billable-minutes: ${s.billableMin}`,
    `cancelled-minutes: ${min1(s.cancelledSeconds)} (${share} of job-minutes)`,
    `abandoned-wait: ${s.abandoned.n} jobs never ran, ${min1(s.abandoned.seconds)} min waited before they were cancelled`,
    `wait seconds, jobs that ran: ${waitLine(s.wait)}${s.wait ? ` (p90 ${min1(s.wait.p90)} min, max ${min1(s.wait.max)} min)` : ""}`,
    `select job wait seconds: ${waitLine(s.selectWait)}`,
    `peak concurrency: ${s.concurrency.peak} at ${withMst(s.concurrency.at)}; fleet runners only ${s.fleetConcurrency.peak}`,
    `peak queued: ${s.queued.peak} at ${withMst(s.queued.at)}`,
    `minutes by class: ${[...s.secondsByClass].map(([k, v]) => `${k}=${min1(v)}`).join(" ") || "none"}`,
    "wait seconds by class:",
    ...[...s.waitByClass].map(([k, w]) => `  ${k}: ${waitLine(w)}`),
    "wait seconds by runner_name:",
    ...[...s.waitByRunner].map(([k, w]) => `  ${k}: ${waitLine(w)}`),
  ];
  if (c.truncated.length > 0) {
    lines.push("TRUNCATED: this is not a complete day", ...c.truncated.map((t) => `  ${t}`));
  }
  lines.push(`loadavg: ${loadavg.trim()}`);
  return lines.join("\n");
}
