// `smoke` runs on fast only; on full and nightly the `web (checks)` leg runs `node --test tests-node` in its place (tl1163sk). Called from test-deploy-conditions.mjs (at its size limit).
export function smokeTierChecks(lines, jobBlock, ifExpression, evaluate) {
  const stepIf = /- name: node --test \(tests-node\)\n\s+if: \$\{\{ (.*) \}\}\n/.exec((jobBlock(lines, 'web') ?? []).join('\n'))?.[1];
  const smokeIf = ifExpression(jobBlock(lines, 'smoke') ?? []);
  const where = (tier) => {
    const ctx = { github: { event_name: 'push', event: {} }, matrix: { shard: 'checks' }, needs: { select: { outputs: { tier, code: 'true' } } } };
    return [smokeIf && evaluate(smokeIf, ctx) ? 'smoke' : '', stepIf && evaluate(stepIf, ctx) ? 'checks' : ''].join('');
  };
  const got = ['fast', 'full', 'nightly', 'deploy'].map(where).join(',');
  return [['the tests-node tests run once per tier: in smoke on fast, in the web checks leg on full and nightly, nowhere on deploy', got === 'smoke,checks,checks,', got]];
}
