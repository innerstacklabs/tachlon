// The CI tier: which set of jobs a workflow run owes. `select` in `.github/workflows/ci.yml` calls
// `scripts/ci-tier.ts`, which calls this, and publishes the answer as its `tier` output. Pure, so the
// event table in `scripts/test-ci-tier.ts` is the whole rule and a change to it shows in a diff here.
//
//   fast     a push to a branch that is not main, release or batch/*
//   full     a push to batch/*, a manual run, and anything this file cannot place (so an event or ref
//            nobody planned for runs every job, never fewer)
//   nightly  the scheduled run: full, plus the slow checks
//   deploy   a push to main or release: the deploy path

export type Tier = "fast" | "full" | "nightly" | "deploy";

/**
 * The browser specs (under the chart folder's `tests/`) that run on full and nightly, so in the batch run and the nightly
 * run, and never on fast (gs#1163, ruling 2): they measure speed, not correctness. The fast tier's browser
 * specs are the touched ones (scripts/lib/touched-specs.ts), minus these.
 */
export const BATCH_ONLY_SPECS: readonly string[] = ["measure.spec.ts"];

/** The branches that carry a deploy. */
const DEPLOY_REFS: readonly string[] = ["refs/heads/main", "refs/heads/release"];
/** The branches the merge door pushes a batch to (`batch/<repo>-<stamp>-<n>`). */
const BATCH_PREFIX = "refs/heads/batch/";
const BRANCH_PREFIX = "refs/heads/";

/** The tier for a run: `event` is `github.event_name`, `ref` is `github.ref`. */
export function tierFor(event: string, ref: string): Tier {
  switch (event) {
    case "schedule":
      return "nightly";
    case "workflow_dispatch":
      return "full";
    case "push":
      if (DEPLOY_REFS.includes(ref)) return "deploy";
      if (ref.startsWith(BATCH_PREFIX)) return "full";
      // A tag is not a branch: it is not a PR head, so it gets everything.
      return ref.startsWith(BRANCH_PREFIX) ? "fast" : "full";
    default:
      return "full";
  }
}
