// The `web` job's matrix legs come from `select`'s `shards` output (`webShards` in scripts/lib/touched-specs.ts): the numbered Playwright
// shards and `checks`, or `checks` alone. These checks hold the matrix to the `--shard=i/N` denominator and `ci-summary` to the `none`-only skip.
// Called from test-deploy-conditions.mjs (at its size limit). Returns [name, ok, detail] rows.
export async function webMatrixChecks(text, webBody, summaryBody) {
  const { webShards } = await import('./touched-specs.ts');
  const shards = JSON.parse(webShards('some')).filter((s) => s !== 'checks').map(String);
  const shardArg = /playwright test --shard=\$\{\{ matrix\.shard \}\}\/(\d+)\s*$/m.exec(webBody);
  const webSkips = summaryBody.split('\n').filter((l) => /WEB_SET/.test(l) && /w=skipped/.test(l));
  return [
    ['web reads its matrix from select\'s shards output', /^ {8}shard:\s*\$\{\{ fromJSON\(needs\.select\.outputs\.shards\) \}\}\s*$/m.test(webBody) && /^ {6}shards: \$\{\{ steps\.specs\.outputs\.shards \}\}\s*$/m.test(text)],
    ['web is a matrix of shards numbered 1 to N, plus checks', shards.length > 1 && shards.every((s, i) => s === String(i + 1)) && JSON.parse(webShards('all')).at(-1) === 'checks', shards.join(',')],
    ['web runs only the checks leg when select says checks', webShards('checks') === '["checks"]', webShards('checks')],
    ['web runs playwright with --shard=<matrix.shard>/<number of shards>', shardArg !== null && Number(shardArg[1]) === shards.length, shardArg?.[0] ?? 'no --shard'],
    ['ci-summary skips web for none only, and words the PASS line for checks',
      webSkips.length === 1 && /"\$WEB_SET" = none \]/.test(webSkips[0]) && !/checks/.test(webSkips[0]) && /\[ "\$WEB_SET" != checks \] \|\| \{ web_ran="[^"]*"; touched=; \}/.test(summaryBody) && summaryBody.includes('web ran ${web_ran}${touched}.'), webSkips.join(' | ')],
  ];
}
