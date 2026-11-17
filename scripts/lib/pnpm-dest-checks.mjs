// The check that every `pnpm/action-setup` step in .github/workflows/ci.yml sets a `dest` of its own per runner. Called from
// scripts/test-deploy-conditions.mjs, which is at its size limit. Node 22, no dependencies, no network.
//
// The self-hosted runners on one machine share a home directory, and the action's default `dest` (~/setup-pnpm) is deleted and rewritten
// by every run, so a neighbouring runner's job lost pnpm mid-run. npm-publish.yml is not checked: it runs on GitHub-hosted runners.

const PER_RUNNER = /dest:\s*["']?~\/setup-pnpm-\$\{\{ runner\.name \}\}["']?\s*(?:[,}]|$)/m;

/** The job of each `pnpm/action-setup` step in ci.yml text that has no `dest: ~/setup-pnpm-${{ runner.name }}`. */
export function pnpmStepsWithoutDest(text) {
  const lines = text.split('\n');
  const bad = [];
  let job = '';
  lines.forEach((line, i) => {
    job = /^ {2}([A-Za-z0-9_-]+):\s*$/.exec(line)?.[1] ?? job;
    if (!/^\s*(?:- )?uses:\s*pnpm\/action-setup@/.test(line)) return;
    let end = i + 1;
    while (end < lines.length && !/^ {0,5}\S|^ {6}- /.test(lines[end])) end++;
    if (!PER_RUNNER.test(lines.slice(i, end).join('\n'))) bad.push(job);
  });
  return bad;
}
