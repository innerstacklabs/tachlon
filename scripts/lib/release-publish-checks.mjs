// The checks of the release-only steps in the `deploy` job of .github/workflows/ci.yml that publish the product page to public-site
// (gantt#1366), called from scripts/test-deploy-conditions.mjs (a file over its size limit cannot grow). Node 22, no dependencies, no network.
// The six steps sit between `comment the preview URL` and `list Pages projects`, so the eight main-only steps keep their place. They run on a
// push to release only while the repository variable SITE_LIVE is exactly the lowercase string true, and the claims check comes before the deploy.
import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

// `steps` is the deploy job's steps without comment lines; `check`, `evaluate` and `text` (ci.yml) are the test file's.
export function releaseChecks({ steps, check, evaluate, text }) {
  // The names scan in `select` is the one secret outside the deploy job: read once, as an env entry.
  const named = [...text.matchAll(/^(.*)\$\{\{\s*secrets\.PUBLIC_RELEASE_NAMES\s*\}\}(.*)$/gm)];
  check('the names scan reads its secret once, as an env entry', named.length === 1 && /^ {10}PUBLIC_RELEASE_NAMES: $/.test(named[0][1]) && named[0][2] === '');
  const rel = steps.slice(steps.findIndex((s) => s.startsWith('name: comment the preview URL')) + 1, steps.findIndex((s) => s.startsWith('name: list Pages projects')));
  const ifOf = (s) => /\n {8}if: \$\{\{ (.*) \}\}\n/.exec(s)?.[1];
  const ctx = (event, ref, live) => ({ github: { event_name: event, ref }, steps: { site: { outputs: live ? { live } : {} } } });
  const order = [/^name: SITE_LIVE is exactly true\n/, /^uses: actions\/setup-node@[0-9a-f]{40}/, /\n {8}run: cp -r apps\/site site-page && rm site-page\/launch\.html && cp -r package-lib site-page\/package\n?$/, /^name: the address is set and no draft text[^\n]*\n/, /\n {10}command: pages deploy site-page --project-name public-site --branch=main\n?$/, /^name: the address serves the product page\n/]
    .map((re) => rel.findIndex((s) => re.test(s)));
  check('the release steps run in order: the SITE_LIVE gate, Node, the stage, the checks of the staged copy, the deploy to public-site as the main branch, the check of the address', rel.length === 6 && order.every((v, i) => v === i));
  check('no release step runs after a failure, so a failed claims check stops the deploy; none uses pages domain or follows a redirect', !rel.some((s) => /always\(|failure\(|cancelled\(|continue-on-error|pages domain|--domain|curl -\w*L|--location/.test(s)));

  // The two run steps run for real, as GitHub runs a step (bash -eo pipefail), with stand-ins for node, curl and sleep that log their calls.
  // curl answers from $ANS, one answer per call (the last repeats): 200 serves $TITLE, 200x another title, FAIL a curl failure, any other answer is that status, with $TITLE served.
  const dir = mkdtempSync(join(tmpdir(), 'release-publish-'));
  const work = join(dir, 'work');
  mkdirSync(join(work, 'site-page'), { recursive: true });
  writeFileSync(join(work, 'site-page', 'index.html'), '<html><head><title>Page One</title></head></html>\n');
  mkdirSync(join(work, 'packages', 'gantt', 'src'), { recursive: true });
  writeFileSync(join(work, 'packages', 'gantt', 'src', 'product.ts'), 'export const PRODUCT_NAME = "Acme";\n'); // a stand-in name: only product.ts may hold the real one
  const tool = (name, body) => writeFileSync(join(dir, name), `#!/bin/sh\n${body}\n`, { mode: 0o755 });
  tool('node', 'echo "$*" >> "$DIR/node.log"; exit ${NODE_RC:-0}');
  tool('sleep', 'echo "$1" >> "$DIR/sleeps"');
  tool('curl', 'for u; do :; done; echo "$u" >> "$DIR/urls"\nn=$(cat "$DIR/n" 2>/dev/null || echo 0); n=$((n+1)); echo $n > "$DIR/n"\na=$(echo $ANS | cut -d" " -f$n); [ -n "$a" ] || a=${ANS##* }\nwhile [ $# -gt 0 ]; do [ "$1" = -o ] && out=$2; shift; done\n'
    + 'case $a in FAIL) exit 6;; 200) echo "<title>$TITLE</title>" > "$out"; printf 200;; 200x) echo "<title>Other</title>" > "$out"; printf 200;; *) echo "<title>$TITLE</title>" > "$out"; printf %s "$a";; esac');
  const lines = (f) => { try { return readFileSync(join(dir, f), 'utf8').split('\n').filter(Boolean); } catch { return []; } };
  const run = (step, env) => {
    for (const f of ['n', 'sleeps', 'urls', 'node.log', 'out']) writeFileSync(join(dir, f), f === 'n' ? '0' : '');
    const script = /\n {8}run: \|\n((?: {10}.*\n?)+)/.exec(step ?? '')?.[1].replace(/^ {10}/gm, '') ?? 'exit 99';
    const r = spawnSync('bash', ['-eo', 'pipefail', '-c', script], { cwd: work, env: { PATH: `${dir}:${process.env.PATH}`, DIR: dir, RUNNER_TEMP: dir, GITHUB_OUTPUT: join(dir, 'out'), TITLE: 'Page One', ...env }, encoding: 'utf8' });
    return { ok: r.status === 0, out: lines('out'), node: lines('node.log'), curls: Number(lines('n')[0]), sleeps: lines('sleeps'), urls: lines('urls') };
  };
  // The SITE_LIVE gate runs for real; the steps after it take its output, as GitHub hands it over.
  const live = (v) => (run(rel[0], { SITE_LIVE: v }).out.includes('live=true') ? 'true' : undefined);
  const runs = (e, r, v) => rel.slice(1).map((s) => { try { return evaluate(ifOf(s), ctx(e, r, live(v))); } catch { return null; } });
  const gateIf = (e, r) => { try { return evaluate(ifOf(rel[0]), ctx(e, r)); } catch { return null; } };
  check('the public-site steps run on a release push when SITE_LIVE is "true", and never with SITE_LIVE false or unset, on main, on a PR or on a manual run; the gate itself runs only on a release push',
    runs('push', 'refs/heads/release', 'true').every((v) => v === true) && ['false', ''].every((v) => runs('push', 'refs/heads/release', v).every((x) => x === false))
      && [['push', 'refs/heads/main'], ['pull_request', 'refs/pull/7/merge'], ['workflow_dispatch', 'refs/heads/release']].every((c) => runs(...c, 'true').every((x) => x === false) && gateIf(...c) === false)
      && gateIf('push', 'refs/heads/release') === true);
  check('SITE_LIVE of TRUE or True publishes nothing: the gate sets its output only for the exact string true', ['TRUE', 'True', ' true'].every((v) => live(v) === undefined && runs('push', 'refs/heads/release', v).every((x) => x === false)));
  const pages = 'site-page/index.html site-page/how-it-works.html site-page/features.html';
  const gate = (env) => run(rel[3], env);
  const staged = (html) => { const p = join(work, 'site-page', 'p.html'); writeFileSync(p, html); const r = gate({ SITE_URL: 'https://site.example' }); rmSync(p); return r; };
  const set = gate({ SITE_URL: 'https://site.example' }); const unset = gate({ SITE_URL: '' }); const claims = gate({ SITE_URL: 'https://site.example', NODE_RC: '1' });
  check('the claims step fails with no SITE_URL set and before it runs the claims check, runs the claims check on each page, and fails when that exits non-zero',
    set.ok && set.node.join() === `scripts/site-claims.ts ${pages}` && !unset.ok && unset.node.length === 0 && !claims.ok && claims.node.length === 1);
  check('the claims step fails on a staged page that says "Draft, not published" or "Draft, unpublished" or links to launch.html, before the claims check runs, and passes a clean page',
    ['<p>Draft, not published.</p>', '<dd>Draft, unpublished</dd>', '<a href="launch.html">x</a>', '<a href = "launch.html">x</a>', '<a href=launch.html>x</a>', "<A HREF='./launch.html'>x</A>", '<a href=\n"launch.html">x</a>', '<a href=" launch.html">x</a>', '<a href="\n./launch.html">x</a>'].every((h) => { const r = staged(h); return !r.ok && r.node.length === 0; }) && staged('<p>Ready</p>').ok);
  const url = 'https://acme.com';
  const check5 = (u) => run(rel[5], { SITE_URL: u, ANS: '200' });
  check('the address check refuses a SITE_URL that is not the apex https://acme.com (another host, a subdomain, http, a path) before any curl, and accepts it with or without a trailing slash',
    ['https://site.example', 'https://www.acme.com', 'http://acme.com', 'https://acme.com.site.example', 'https://acme.com/page'].every((u) => { const r = check5(u); return !r.ok && r.curls === 0; }) && check5(`${url}/`).ok && check5(url).ok);
  const [first, late, never] = ['200', '302 FAIL 200x 200', '302'].map((ANS) => run(rel[5], { SITE_URL: url, ANS }));
  check('the address check passes on a 200 with the deployed page\'s title, retries after a 302, a curl failure or another title (10 s apart), and after five tries fails',
    first.ok && first.curls === 1 && first.sleeps.length === 0 && first.urls.join() === url && late.ok && late.curls === 4 && late.sleeps.join() === '10,10,10'
      && !never.ok && never.curls === 5 && never.sleeps.length === 4 && !run(rel[5], { SITE_URL: url, ANS: '200x' }).ok && !run(rel[5], { SITE_URL: url, ANS: 'FAIL' }).ok);
}
