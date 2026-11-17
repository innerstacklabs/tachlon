// The checks that `hosted-browser` (gantt#1163) is waited on wherever `hosted` is, in .github/workflows/ci.yml. Called from
// scripts/test-deploy-conditions.mjs, which is at its size limit and cannot grow. Node 22, no dependencies, no network.
//
// The Playwright specs of the hosted app are their own job, `hosted-browser`, so they run beside the benchmark test in `hosted`.
// Every job that waits on `hosted` also waits on `hosted-browser`: nothing deploys or reports green before both pass.

/** The job ids in a job block's `needs:`, written as `[a, b]` or as a list of `- a` lines. */
export function needsOf(block) {
  const body = block.join('\n');
  const line = /^ {4}needs:\s*(.*)$/m.exec(body);
  if (line === null) return [];
  if (line[1].trim() === '') return [...body.matchAll(/^ {6}- (\S+)\s*$/gm)].map((m) => m[1]);
  return line[1].replace(/[[\]]/g, '').split(',').map((s) => s.trim()).filter(Boolean);
}

/** Rows for the `deploy` `if:` table: a failed or skipped `hosted-browser` stops it. `ctx` is the caller's fixture builder. */
export function browserCases(ctx) {
  return [
    ['push to main, hosted-browser failed', ctx({ event: 'push', hostedBrowser: 'failure' }), false],
    ['push to main, hosted-browser skipped', ctx({ event: 'push', hostedBrowser: 'skipped' }), false],
    ['ready PR from this repo, hosted-browser failed', ctx({ event: 'pull_request', draft: false, hostedBrowser: 'failure' }), false],
  ];
}

/** `[name, ok, detail]` rows. `jobBlock(lines, id)` is the caller's reader; `summaryBody` is the `ci-summary` job's text. */
export function hostedBrowserChecks(lines, jobBlock, summaryBody) {
  const jobs = lines.filter((l) => /^ {2}[a-z][\w-]*:$/.test(l)).map((l) => l.trim().slice(0, -1))
    .filter((id) => jobBlock(lines, id).some((l) => /^ {4}(needs|runs-on):/.test(l)));
  const after = jobs.filter((id) => needsOf(jobBlock(lines, id)).includes('hosted'));
  const browser = jobBlock(lines, 'hosted-browser');
  const deployNeeds = needsOf(jobBlock(lines, 'deploy') ?? []);
  return [
    ['job hosted-browser exists', browser !== null, ''],
    ['hosted-browser needs what hosted needs', browser !== null && needsOf(browser).join() === needsOf(jobBlock(lines, 'hosted') ?? []).join(), ''],
    ['the jobs after hosted are found (deploy, ci-summary)', after.includes('deploy') && after.includes('ci-summary'), after.join(',')],
    ...after.map((id) => [`${id} needs hosted-browser too`, needsOf(jobBlock(lines, id)).includes('hosted-browser'), '']),
    ['deploy needs book, hosted and hosted-browser', deployNeeds.length === 3 && ['book', 'hosted', 'hosted-browser'].every((id) => deployNeeds.includes(id)), deployNeeds.join(',')],
    ['ci-summary judges the hosted-browser result', /HOSTED_BROWSER: \$\{\{ needs\.hosted-browser\.result \}\}/.test(summaryBody) && /"hosted-browser:\$HOSTED_BROWSER"/.test(summaryBody), ''],
  ];
}
