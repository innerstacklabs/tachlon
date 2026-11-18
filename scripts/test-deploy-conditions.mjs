// Checks the `deploy` job in .github/workflows/ci.yml without deploying anything.
// Node 22, no dependencies, no network. Run from anywhere:
//
//   node scripts/test-deploy-conditions.mjs [path/to/ci.yml]
//
// It does two things:
//   1. Reads the job's one-line `if:` and evaluates it, with GitHub's rules for
//      `==`, `!`, `&&` and `||`, against a table of event fixtures (a push to main,
//      a draft push, a ready PR, a draft PR, a fork PR, a failed `book` job...).
//      Each row says whether the job must run. An expression this file cannot
//      evaluate (a function, an operator it does not know) is an error, not a pass.
//   2. Checks the job's shape: it needs `book`, `hosted` and `hosted-browser`, it runs on a GitHub-hosted runner,
//      every action it uses is pinned to a commit, the only secrets are the two
//      Cloudflare ones and only in the wrangler step's `with:`, and nothing in the
//      file sets continue-on-error or calls the Cloudflare Access API.
// It also checks that the `web` job is a matrix of Playwright shards whose count matches the
// `--shard=i/N` denominator, and that `ci-summary` still judges the `web` result (gs#1122).
// Exit 0 when every check holds, 1 otherwise, one line per check.

import { spawnSync } from 'node:child_process';
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { browserCases, hostedBrowserChecks } from './lib/hosted-browser-checks.mjs';
import { pnpmStepsWithoutDest } from './lib/pnpm-dest-checks.mjs';
import { wasmHandoffChecks } from './lib/wasm-handoff-checks.mjs';
import { webMatrixChecks } from './lib/web-matrix-checks.mjs';

const REPO = 'owner/repo';
const SECRET_NAMES = ['CLOUDFLARE_API_TOKEN', 'CLOUDFLARE_ACCOUNT_ID'];

// ---- the expression evaluator (the subset the deploy job's `if:` may use) ------

function tokenize(src) {
  const tokens = [];
  let i = 0;
  while (i < src.length) {
    const c = src[i];
    if (/\s/.test(c)) { i++; continue; }
    if (c === "'") {
      let j = i + 1;
      let s = '';
      for (;;) {
        if (j >= src.length) throw new Error(`unterminated string in: ${src}`);
        if (src[j] === "'" && src[j + 1] === "'") { s += "'"; j += 2; continue; }
        if (src[j] === "'") break;
        s += src[j++];
      }
      tokens.push({ t: 'lit', v: s });
      i = j + 1;
      continue;
    }
    const two = src.slice(i, i + 2);
    if (two === '==' || two === '!=' || two === '&&' || two === '||') {
      tokens.push({ t: two });
      i += 2;
      continue;
    }
    if (c === '!' || c === '(' || c === ')') { tokens.push({ t: c }); i++; continue; }
    const word = /^[A-Za-z_][A-Za-z0-9_.-]*/.exec(src.slice(i));
    if (word) {
      const w = word[0];
      i += w.length;
      if (src[i] === '(') throw new Error(`unsupported function ${w}() in: ${src}`);
      if (w === 'true') tokens.push({ t: 'lit', v: true });
      else if (w === 'false') tokens.push({ t: 'lit', v: false });
      else if (w === 'null') tokens.push({ t: 'lit', v: null });
      else tokens.push({ t: 'path', v: w });
      continue;
    }
    const num = /^[0-9]+(\.[0-9]+)?/.exec(src.slice(i));
    if (num) { tokens.push({ t: 'lit', v: Number(num[0]) }); i += num[0].length; continue; }
    throw new Error(`cannot evaluate character '${c}' in: ${src}`);
  }
  return tokens;
}

const truthy = (v) => !(v === null || v === false || v === 0 || v === '' || Number.isNaN(v));

function toNumber(v) {
  if (v === null) return 0;
  if (typeof v === 'boolean') return v ? 1 : 0;
  if (typeof v === 'number') return v;
  if (v.trim() === '') return 0;
  return Number(v);
}

// GitHub: same types compare directly (strings ignoring case); otherwise both go to numbers.
function looseEqual(a, b) {
  if (typeof a === 'string' && typeof b === 'string') return a.toLowerCase() === b.toLowerCase();
  if (a === null && b === null) return true;
  if (typeof a === typeof b) return a === b;
  return toNumber(a) === toNumber(b);
}

function lookup(ctx, path) {
  let v = ctx;
  for (const key of path.split('.')) {
    if (v === null || typeof v !== 'object' || !(key in v)) return null;
    v = v[key];
  }
  return v === undefined ? null : v;
}

function evaluate(expr, ctx) {
  const tokens = tokenize(expr);
  let pos = 0;
  const peek = () => tokens[pos]?.t;
  const eat = (t) => {
    if (peek() !== t) throw new Error(`expected ${t} at token ${pos} in: ${expr}`);
    return tokens[pos++];
  };
  function or() {
    let left = and();
    while (peek() === '||') { pos++; const right = and(); left = truthy(left) ? left : right; }
    return left;
  }
  function and() {
    let left = eq();
    while (peek() === '&&') { pos++; const right = eq(); left = truthy(left) ? right : left; }
    return left;
  }
  function eq() {
    let left = unary();
    while (peek() === '==' || peek() === '!=') {
      const op = tokens[pos++].t;
      const right = unary();
      left = op === '==' ? looseEqual(left, right) : !looseEqual(left, right);
    }
    return left;
  }
  function unary() {
    if (peek() === '!') { pos++; return !truthy(unary()); }
    return primary();
  }
  function primary() {
    const tok = tokens[pos];
    if (!tok) throw new Error(`unexpected end of expression: ${expr}`);
    if (tok.t === '(') { pos++; const v = or(); eat(')'); return v; }
    if (tok.t === 'lit') { pos++; return tok.v; }
    if (tok.t === 'path') { pos++; return lookup(ctx, tok.v); }
    throw new Error(`unexpected token ${tok.t} in: ${expr}`);
  }
  const value = or();
  if (pos !== tokens.length) throw new Error(`trailing tokens in: ${expr}`);
  return truthy(value);
}

// ---- reading the workflow ------------------------------------------------------

function jobBlock(lines, name) {
  const start = lines.findIndex((l) => l === `  ${name}:`);
  if (start < 0) return null;
  let end = lines.length;
  for (let i = start + 1; i < lines.length; i++) {
    if (/^ {0,2}\S/.test(lines[i]) && !/^ {0,2}#/.test(lines[i])) { end = i; break; }
  }
  return lines.slice(start, end);
}

function ifExpression(block) {
  const line = block.find((l) => /^ {4}if:/.test(l));
  if (!line) return null;
  const m = /^ {4}if:\s*\$\{\{\s*(.*?)\s*\}\}\s*$/.exec(line);
  if (!m) throw new Error(`the job's if: must be one line of the form "if: \${{ ... }}": ${line}`);
  return m[1];
}

// ---- the fixtures --------------------------------------------------------------

function ctx({ event, eventName = event, ref = 'refs/heads/main', draft, headRepo = REPO, book = 'success', hosted = 'success', hostedBrowser = 'success', siteUrl = 'https://example.com' }) {
  const github = { event_name: eventName, ref, repository: REPO, event: {} };
  if (event === 'pull_request') {
    github.ref = 'refs/pull/7/merge';
    github.event = {
      pull_request: { draft, head: { repo: headRepo === null ? null : { full_name: headRepo } } },
    };
  }
  return { github, vars: { SITE_URL: siteUrl }, needs: { book: { result: book }, hosted: { result: hosted }, 'hosted-browser': { result: hostedBrowser } } };
}

const CASES = [
  // [name, context, does the job run]
  ['push to main, book passed', ctx({ event: 'push' }), true],
  ['push to main, book failed', ctx({ event: 'push', book: 'failure' }), false],
  ['push to main, repository without SITE_URL', ctx({ event: 'push', siteUrl: '' }), false],
  ['push to main, book cancelled', ctx({ event: 'push', book: 'cancelled' }), false],
  ['push to main, book skipped', ctx({ event: 'push', book: 'skipped' }), false],
  ['push to main, hosted failed', ctx({ event: 'push', hosted: 'failure' }), false],
  ['push to main, hosted cancelled', ctx({ event: 'push', hosted: 'cancelled' }), false],
  ['push to main, hosted skipped', ctx({ event: 'push', hosted: 'skipped' }), false],
  ['draft push to a branch', ctx({ event: 'push', ref: 'refs/heads/feat/458-book-deploy' }), false],
  ['push to a branch named like main', ctx({ event: 'push', ref: 'refs/heads/main-old' }), false],
  ['push to release, book passed', ctx({ event: 'push', ref: 'refs/heads/release' }), true],
  ['push to release, book failed', ctx({ event: 'push', ref: 'refs/heads/release', book: 'failure' }), false],
  ['push to a branch named like release', ctx({ event: 'push', ref: 'refs/heads/release-old' }), false],
  ['push of a tag', ctx({ event: 'push', ref: 'refs/tags/main' }), false],
  ['manual run on main', ctx({ event: 'workflow_dispatch' }), false],
  ['draft PR from a branch named release', ctx({ event: 'pull_request', draft: true }), false],
  ['ready PR from this repo, book passed', ctx({ event: 'pull_request', draft: false }), true],
  ['ready PR from this repo, book failed', ctx({ event: 'pull_request', draft: false, book: 'failure' }), false],
  ['ready PR from this repo, book skipped', ctx({ event: 'pull_request', draft: false, book: 'skipped' }), false],
  ['ready PR from this repo, hosted failed', ctx({ event: 'pull_request', draft: false, hosted: 'failure' }), false],
  ['ready PR from this repo, hosted skipped', ctx({ event: 'pull_request', draft: false, hosted: 'skipped' }), false],
  ['draft PR from this repo', ctx({ event: 'pull_request', draft: true }), false],
  ['ready PR from a fork', ctx({ event: 'pull_request', draft: false, headRepo: 'someone/gantt' }), false],
  ['draft PR from a fork', ctx({ event: 'pull_request', draft: true, headRepo: 'someone/gantt' }), false],
  ['ready PR whose fork was deleted', ctx({ event: 'pull_request', draft: false, headRepo: null }), false],
  ['pull_request_target', ctx({ event: 'pull_request', eventName: 'pull_request_target', draft: false }), false],
  ...browserCases(ctx),
];

// ---- the checks ----------------------------------------------------------------

const path = process.argv[2] ?? fileURLToPath(new URL('../.github/workflows/ci.yml', import.meta.url));
const text = readFileSync(path, 'utf8');
const lines = text.split('\n');
let failed = 0;
const check = (name, ok, detail = '') => {
  if (!ok) failed++;
  console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${!ok && detail ? ` (${detail})` : ''}`);
};

for (const job of ['rust', 'wasm', 'web', 'book']) {
  check(`job ${job} is still there`, jobBlock(lines, job) !== null);
}

// The `web` job is a matrix of Playwright shards (gs#1122). Every spec file must land in exactly
// one shard, and `ci-summary` must still fail when any shard fails or is skipped.
const web = jobBlock(lines, 'web') ?? [];
const webBody = web.join('\n');
check('web does not stop the other shards when one fails', /^ {6}fail-fast:\s*false\s*$/m.test(webBody));
check('web gives each shard its own report artifact name', /name: playwright-report-\$\{\{ matrix\.shard \}\}\s*$/m.test(webBody));
check('web sets pnpm up in a directory per runner', /uses: pnpm\/action-setup@v\d+\n\s+with:\n\s+package_json_file: \$\{\{ env\.WEB_ROOT \}\}\/package\.json\n\s+dest: ~\/setup-pnpm-\$\{\{ runner\.name \}\}\s*\n/.test(webBody));
check('every pnpm/action-setup step in ci.yml sets a directory per runner', pnpmStepsWithoutDest(text).length === 0, `jobs without: ${pnpmStepsWithoutDest(text).join(',')}`);
const summary = jobBlock(lines, 'ci-summary') ?? [];
const summaryBody = summary.join('\n');
check('ci-summary needs web and reads its aggregate result', /^ {4}needs:\s*\[[^\]]*\bweb\b[^\]]*\]/m.test(summaryBody) && /WEB: \$\{\{ needs\.web\.result \}\}/.test(summaryBody));
check('ci-summary wants web success whenever the code jobs run', /"web:\$WEB"/.test(summaryBody) && /\[ "\$\{pair#\*:\}" = "\$w" \] \|\| fail/.test(summaryBody));
[...hostedBrowserChecks(lines, jobBlock, summaryBody), ...wasmHandoffChecks(text, lines, jobBlock), ...await webMatrixChecks(text, webBody, summaryBody)].forEach(([name, ok, detail]) => check(name, ok, detail)); // gantt#1163: hosted-browser is waited on wherever hosted is
check('hosted runs the package size check right after it builds the package', /pnpm build:lib\n\s+working-directory: packages\/gantt\n(?: *#.*\n)*\s+- name: package size\n\s+id: size\n\s+working-directory: packages\/gantt\n\s+env:\n\s+GITHUB_TOKEN: .*\n\s+run: \|\n(?:\s+.*\n)*?\s+node "\$GITHUB_WORKSPACE\/scripts\/package-size\.ts" \. \|\| rc=\$\?\n/.test((jobBlock(lines, 'hosted') ?? []).join('\n') + '\n'));

const deploy = jobBlock(lines, 'deploy');
check('job deploy exists', deploy !== null);
if (deploy === null) process.exit(1);

let expr = null;
try {
  expr = ifExpression(deploy);
} catch (e) {
  check('deploy has a one-line if:', false, e.message);
}
check('deploy has an if:', expr !== null);

if (expr !== null) {
  for (const [name, c, want] of CASES) {
    let got;
    try {
      got = evaluate(expr, c);
    } catch (e) {
      check(`if: ${name}`, false, e.message);
      continue;
    }
    check(`if: ${name} -> ${want ? 'runs' : 'skipped'}`, got === want, `got ${got ? 'runs' : 'skipped'}`);
  }
}

const body = deploy.join('\n');
check('deploy downloads the book job\'s artifact into site/book/ (no mdbook of its own) and the hosted-dist artifact into the site root',
  !/mdbook/.test(body.replace(/#.*$/gm, '')) && /- uses: actions\/download-artifact@[0-9a-f]{40} # v\S+\n\s+with:\n\s+name: book\n\s+path: site\/book\n(?:\s*#.*\n)*\s+- uses: actions\/download-artifact@[0-9a-f]{40} # v\S+\n\s+with:\n\s+name: hosted-dist\n\s+path: site\/\n/.test(body));
check('deploy does not rebuild the wasm', !/build-web\.sh|wasm-pack|cargo /.test(body));
check('deploy runs on ubuntu-latest, never a hub runner', /^ {4}runs-on:\s*ubuntu-latest\s*$/m.test(body));
check('deploy has a timeout', /^ {4}timeout-minutes:\s*\d+\s*$/m.test(body));

const uses = [...body.matchAll(/^\s*(?:- )?uses:\s*(\S+)/gm)].map((m) => m[1]);
check('deploy uses the wrangler action', uses.some((u) => u.startsWith('cloudflare/wrangler-action@')));
for (const u of uses) {
  check(`deploy action pinned to a commit: ${u.split('@')[0]}`, /@[0-9a-f]{40}$/.test(u), u);
}
check(
  'deploy publishes site/ to the gantt-book project',
  /pages deploy site --project-name gantt-book /.test(body),
);
check(
  'deploy redirects the old /app/ links to the root',
  /printf '%s\\n' '\/app \/ 301' '\/app\/\* \/:splat 301' > site\/_redirects/.test(body),
);

// Run the `deploy branch` step's script for real and read the branch it writes.
const stepMatch = /- name: deploy branch\n(?:.*\n)*?\s+run: \|\n((?: {10}.*\n)+)/.exec(body);
check('deploy has a deploy branch step', stepMatch !== null);
if (stepMatch !== null) {
  const script = stepMatch[1].replace(/^ {10}/gm, '');
  const dir = mkdtempSync(join(tmpdir(), 'deploy-branch-'));
  const target = (env) => {
    const out = join(dir, 'out');
    writeFileSync(out, '');
    const r = spawnSync('bash', ['-c', script], { env: { PATH: process.env.PATH, GITHUB_OUTPUT: out, ...env }, encoding: 'utf8' });
    return { status: r.status, branch: /^branch=(.*)$/m.exec(readFileSync(out, 'utf8'))?.[1] ?? null };
  };
  const push = (name) => target({ EVENT: 'push', REF_NAME: name, HEAD_REF: '' });
  const pr = (name) => target({ EVENT: 'pull_request', REF_NAME: `${name}`, HEAD_REF: name });
  const sameBranch = (r, want) => r.status === 0 && r.branch === want;
  check('deploy branch: a push to release deploys with branch release', sameBranch(push('release'), 'release'));
  check('deploy branch: a push to main deploys with branch main', sameBranch(push('main'), 'main'));
  check('deploy branch: a PR from a branch named release is refused', pr('release').status !== 0);
  check('deploy branch: a PR from a branch named main is refused', pr('main').status !== 0);
  check('deploy branch: a PR keeps its own branch name', sameBranch(pr('feat/x'), 'feat/x'));
}

// The product page (apps/site) goes to its own project, gantt-product-page, on a push to main only, and only after
// the gate that finds its pages.dev address behind Access. launch.html is never published.
const steps = body.replace(/^ *#.*\n/gm, '').split(/\n {6}- /).slice(1);
const siteSteps = steps.slice(Math.max(0, steps.findIndex((s) => s.startsWith('name: list Pages projects'))));
const mainOnly = /\n {8}if: \$\{\{ github\.event_name == 'push' && github\.ref == 'refs\/heads\/main'( && [^\n]*)? \}\}\n/;
check('every step of the product page runs on a push to main only, never on a PR or release', siteSteps.length === 8 && siteSteps.every((s) => mainOnly.test(s)), `${siteSteps.length} steps`);
const siteDeploy = siteSteps.findIndex((s) => /command: pages deploy site-page --project-name gantt-product-page --branch=main\n?$/.test(s));
const siteGate = siteSteps.findIndex((s) => s.startsWith("name: the project's pages.dev addresses are behind Access\n"));
const siteDomains = siteSteps.findIndex((s) => s.startsWith('name: the project has no custom domain\n'));
check('the product page deploys to gantt-product-page as the main branch, only after the no-custom-domain check and the Access gate', siteGate >= 0 && siteDomains >= 0 && siteDeploy > siteGate && siteDeploy > siteDomains);
// The two gates are run for real, as GitHub runs a step (bash -eo pipefail), so a gate that fails open goes red.
const runStep = (i, env, path = process.env.PATH) => {
  const m = /\n {8}run: \|\n((?: {10}.*\n?)+)/.exec(siteSteps[i] ?? '');
  return m === null ? -1 : spawnSync('bash', ['-eo', 'pipefail', '-c', m[1].replace(/^ {10}/gm, '')], { env: { PATH: path, ...env }, encoding: 'utf8' }).status;
};
const bin = mkdtempSync(join(tmpdir(), 'fake-curl-'));
// The stand-in curl answers by host. With COVERED set it acts as Access with those destinations (shell patterns): a covered host gets
// the login redirect, any other 200. Without it, the answer is A_MAIN for main.*, A_BARE for the bare name, A_RANDOM for any other host.
writeFileSync(join(bin, 'curl'), '#!/bin/sh\nset -f\nfor u; do :; done\nh=${u#*://}\necho "$h" >> "$LOG"\nif [ -n "$COVERED" ]; then a="200 "; for g in $COVERED; do case "$h" in $g) a="302 https://team.cloudflareaccess.com/cdn-cgi/x";; esac; done\nelse case "$h" in main.*) a="$A_MAIN";; gantt-product-page.pages.dev) a="$A_BARE";; *) a="$A_RANDOM";; esac; fi\n[ "$a" = FAIL ] && exit 6\ncase "$a" in TLS) exit 35;; X[0-9]*) exit ${a#X};; esac\nprintf "%s" "$a"\n', { mode: 0o755 });
const logFile = join(bin, 'hosts'); const probe = (step, bare, main, random, extra = {}) => runStep(step, { LOG: logFile, A_BARE: bare, A_MAIN: main, A_RANDOM: random, ...extra }, `${bin}:${process.env.PATH}`);
const gate = (...c) => probe(siteGate, ...c); const login = '302 https://team.cloudflareaccess.com/cdn-cgi/access/login/x'; const TLS_EXITS = [35, 51, 60, 83, 91];
// A TLS handshake failure (curl exit 35, 51, 60, 83, 91) serves nothing, so the gate passes that host; a local-setup exit (77, 58...) or a timeout (28) measured nothing. After the deploy only a 302 to Access passes, and a failure is followed by the delete.
check('the Access gate passes a TLS handshake failure on any host, and still fails a 200, a 302 elsewhere, another status, a local-setup curl exit and a timeout', [['TLS', 'TLS', 'TLS'], [login, 'TLS', 'TLS'], ['TLS', login, 'TLS'], ...TLS_EXITS.map((n) => [login, `X${n}`, login])].every((c) => gate(...c) === 0)
  && [['TLS', 'TLS', '200 '], ['200 ', 'TLS', 'TLS'], [login, 'TLS', '302 https://evil.example/'], ['TLS', '404 ', login], ['TLS', 'FAIL', 'TLS'], ...Array.from({ length: 99 }, (_, i) => i + 1).filter((n) => !TLS_EXITS.includes(n)).map((n) => [login, login, `X${n}`])].every((c) => gate(...c) !== 0));
const sitePost = siteSteps.findIndex((s) => s.startsWith('name: the deployed product page is still behind Access\n')), del = siteSteps.at(-1);
check('the check after the deploy passes only when all three addresses answer 302 to Access; a 200, any other answer, a TLS failure or a timeout fails it', probe(sitePost, login, login, login) === 0
  && [['200 ', login, login], [login, '200 ', login], [login, login, '200 '], [login, '404 ', login], ['302 https://evil.example/', login, login], ['TLS', login, login], [login, 'TLS', login], [login, login, 'TLS'], [login, login, 'FAIL'], [login, 'X28', login]].every((c) => probe(sitePost, ...c) !== 0));
const delRuns = (event, ref, failed, outcome) => { try { return evaluate(/\n {8}if: \$\{\{ (.*) \}\}\n/.exec(del)[1].replace('failure()', 'FAILED'), { github: { event_name: event, ref }, FAILED: failed, steps: { postcheck: { outcome } } }); } catch { return null; } };
check('the check runs right after the deploy, and only its failure on a push to main runs the wrangler step that deletes that deployment by its id', sitePost === siteDeploy + 1 && /\n {8}id: product\n/.test(siteSteps[siteDeploy]) && /\n {8}id: postcheck\n/.test(siteSteps[sitePost])
  && del.startsWith('name: delete the deployment that is not behind Access\n') && /uses: (\S+)/.exec(del)?.[1] === /uses: (\S+)/.exec(siteSteps[siteDeploy])[1] && /\n {10}command: pages deployment delete \$\{\{ steps\.product\.outputs\.pages-deployment-id \}\} --project-name gantt-product-page --force\s*$/.test(del)
  && delRuns('push', 'refs/heads/main', true, 'failure') === true && [['push', 'refs/heads/main', true, 'success'], ['push', 'refs/heads/main', false, 'failure'], ['push', 'refs/heads/release', true, 'failure'], ['pull_request', 'refs/heads/main', true, 'failure']].every((c) => delRuns(...c) === false));
check('every probe of both product-page checks has a time limit, so the delete is reached', [siteGate, sitePost].every((i) => /\$\(curl -sS --max-time (?:[1-9]|[1-5]\d|60) /.test(siteSteps[i]) && !/\$\(curl (?!-sS --max-time (?:[1-9]|[1-5]\d|60) )/.test(siteSteps[i])));
check('the Access gate passes only when the bare, the main preview and a random-label address all answer 302 to a cloudflareaccess.com login',
  gate(login, login, login) === 0 && [['200 ', login, login], [login, '200 ', login], [login, login, '200 '], [login, '302 ', login], [login, login, 'FAIL'], ['302 https://evil.example/', login, login],
    [login, '302 https://evil.example/x.cloudflareaccess.com/', login], [login, login, '302 https://team.cloudflareaccess.com.evil.example/x']].every((c) => gate(...c) !== 0));
// The random label has no fixed prefix or suffix, so no partial wildcard destination (probe-*, a-*, *-x) can cover it: only a full wildcard does.
const bare = 'gantt-product-page.pages.dev';
const covered = (...c) => gate('', '', '', { COVERED: [bare, `main.${bare}`, ...c].join(' ') });
writeFileSync(logFile, '');
covered(`*.${bare}`);
covered(`*.${bare}`);
const labels = readFileSync(logFile, 'utf8').split('\n').filter((h) => h && h !== bare && h !== `main.${bare}`);
check('the Access gate probes a fresh, fully random label each run (12 hex characters), and only a full wildcard destination passes it',
  covered(`*.${bare}`) === 0 && covered(`probe-*.${bare}`) !== 0 && covered(`[g-z]*.${bare}`) !== 0 && covered() !== 0
    && labels.length === 2 && labels[0] !== labels[1] && labels.every((l) => new RegExp(`^[0-9a-f]{12}\\.${bare.replaceAll('.', '\\.')}$`).test(l)));
const proj = (d, name = 'gantt-product-page') => JSON.stringify([{ 'Project Name': name, 'Project Domains': d }], null, 2);
const domains = (list) => runStep(siteDomains, { PROJECTS: list });
check('the custom-domain check passes a clean or absent project and fails a custom domain, another name, a bad list and no list',
  [proj('gantt-product-page.pages.dev'), '[]', proj('public-site-ckh.pages.dev, example.com', 'public-site')].every((l) => domains(l) === 0)
    && [proj('gantt-product-page.pages.dev, example.com'), proj('gantt-product-page-x1.pages.dev'), 'not json', ''].every((l) => domains(l) !== 0));
check('the product page is staged without launch.html, with the package beside it as package/', siteSteps.some((s) => /cp -r apps\/site site-page && rm site-page\/launch\.html && cp -r package-lib site-page\/package$/.test(s)));
check('the gantt-product-page project is created only when the project list lacks it, and no custom domain is attached',
  siteSteps.some((s) => /!contains\(steps\.projects\.outputs\.command-output, '"gantt-product-page",'\)[\s\S]*command: pages project create gantt-product-page --production-branch=release$/.test(s))
    && !/pages domain|--domain/.test(siteSteps.join('\n')));
(await import('./lib/release-publish-checks.mjs')).releaseChecks({ steps, check, evaluate, text }); // the release-only public-site steps, and the names scan's secret
check('deploy permissions are contents: read and pull-requests: write only',
  /^ {4}permissions:\n {6}contents: read\n {6}pull-requests: write\n(?! {6})/m.test(body));

const secretRefs = [...text.matchAll(/^(.*)\$\{\{\s*secrets\.([A-Za-z0-9_]+)\s*\}\}(.*)$/gm)].filter((m) => m[2] !== 'PUBLIC_RELEASE_NAMES'); // its own checks: lib/release-publish-checks.mjs
check('the file references both Cloudflare secrets', SECRET_NAMES.every((n) => secretRefs.some((m) => m[2] === n)));
check('no other secret is referenced', secretRefs.every((m) => SECRET_NAMES.includes(m[2])), secretRefs.map((m) => m[2]).join(','));
check('secrets appear only as the wrangler step\'s apiToken and accountId inputs',
  secretRefs.every((m) => /^\s+(apiToken|accountId): $/.test(m[1]) && m[3] === ''),
  secretRefs.map((m) => m[0].trim()).join(' | '));
check('secrets are referenced only inside the deploy job', !/secrets\./.test(text.replace(body, '').replace(/^ {10}PUBLIC_RELEASE_NAMES: .*$/m, '')));
check('no continue-on-error anywhere', !/continue-on-error/.test(text));
check('nothing calls the Cloudflare Access API', !/access\/(apps|policies|groups)|api\.cloudflare\.com/.test(text));

// Decision 0007 may only name jobs and Pages projects that this file contains. It reads the
// `## Decision` and `## How it moves` sections; a backticked job id is one written as
// "`id` job" or "job (`id`)", a project is any backticked `gantt-*`.
const record = readFileSync(fileURLToPath(new URL('../docs/decisions/0007-hosted-app-separate-project.md', import.meta.url)), 'utf8');
const section = (heading) => new RegExp(`^## ${heading}\\n([\\s\\S]*?)(?=^## |(?![\\s\\S]))`, 'm').exec(record)?.[1] ?? '';
const claims = section('Decision') + section('How it moves');
const jobIds = [...claims.matchAll(/`([a-z][a-z0-9_-]*)` job|job \(`([a-z][a-z0-9_-]*)`\)/g)].map((m) => m[1] ?? m[2]);
const projects = [...claims.matchAll(/`(gantt-[a-z0-9-]+)`/g)].map((m) => m[1]);
const missingJobs = jobIds.filter((j) => jobBlock(lines, j) === null);
const missingProjects = projects.filter((p) => !text.includes(p));
check('decision 0007 names only jobs that ci.yml has', jobIds.length > 0 && missingJobs.length === 0, `missing: ${missingJobs.join(',')}`);
check('decision 0007 names only projects that ci.yml has', projects.length > 0 && missingProjects.length === 0, `missing: ${missingProjects.join(',')}`);

process.exit(failed === 0 ? 0 : 1);
