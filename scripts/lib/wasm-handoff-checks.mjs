// smoke, web, hosted and hosted-browser read the wasm bindings from the machine's disk, artifact as the fallback (gantt#1878). Called from test-deploy-conditions.mjs (at its size limit).
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export function wasmHandoffChecks(text, lines, jobBlock) {
  const action = readFileSync(new URL('../../.github/actions/wasm-bindings/action.yml', import.meta.url), 'utf8');
  const block = (j) => (jobBlock(lines, j) ?? []).join('\n');
  const home = mkdtempSync(join(tmpdir(), 'handoff-')); // the disk step, run for a run key: did it take the bindings from the disk?
  mkdirSync(join(home, 'run-1-1')); writeFileSync(join(home, 'run-1-1/a.wasm'), 'x');
  const read = (key) => {
    writeFileSync(join(home, 'out'), '');
    spawnSync('bash', ['-c', /run: \|\n\s+(.*)\n/.exec(action)?.[1] ?? 'exit 1'], { stdio: 'ignore', env: { ...process.env, HOME: home, GITHUB_WORKSPACE: home, WEB_ROOT: 'web', GITHUB_OUTPUT: join(home, 'out'), WASM_HANDOFF: key } });
    return readFileSync(join(home, 'out'), 'utf8').includes('hit=true') && existsSync(join(home, 'web/src/wasm/a.wasm'));
  };
  return [
    ...['smoke', 'web', 'hosted', 'hosted-browser'].map((j) => [`${j} gets the wasm bindings from the disk-or-artifact action, with no download of its own`, block(j).includes('uses: ./.github/actions/wasm-bindings') && !/name: wasm-bindings/.test(block(j))]),
    ['the action downloads the artifact when the disk step found no hit', /- if: steps\.disk\.outputs\.hit != 'true'\n\s+uses: actions\/download-artifact@v4\n\s+with: \{name: wasm-bindings,/.test(action)],
    ['the disk folder is keyed by run id and attempt, and `wasm` writes it', /WASM_HANDOFF: \S+\$\{\{ github\.run_id \}\}-\$\{\{ github\.run_attempt \}\}/.test(text) && /\$HOME\/\$WASM_HANDOFF[\s\S]*cp -r "\$WEB_ROOT\/src\/wasm"/.test(block('wasm'))],
    ['an empty run key stops the disk step and the `wasm` keep step before any copy, find or delete', /\[ -n "\$WASM_HANDOFF" \][^\n]*cp -r/.test(action) && /\[ -n "\$WASM_HANDOFF" \][^\n]*mkdir -p[^\n]*find/.test(block('wasm')) && !read('')],
    ['the disk step takes this run key\'s bindings and no other run\'s', read('run-1-1') && !read('run-1-2') && !read('run-2-1')],
  ];
}
