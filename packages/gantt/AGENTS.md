# Add tachlon to an app

This guide is for a coding assistant that adds the tachlon Gantt chart to an existing web app. Follow the steps in order. Do not skip one, and do not do anything on the Never list.

## Do

### 1. Look, then install on a branch

Read the app's `package.json` to find the framework (Vite or Next.js) and the package manager (a `pnpm-lock.yaml`, `yarn.lock` or `package-lock.json` beside it).

Run the plan first. It writes nothing. Then switch to a branch, before your first edit, and install:

```sh
npx tachlon init --dry-run
git switch -c add-tachlon
npx tachlon init
```

If the app has no `.git`, run `git init` first, then switch; a branch cannot start in a folder that is not a git repository.

`init` supports Vite and Next.js. For any other bundler, run `npx tachlon init --plain`, which writes a chart module that needs no framework.

### 2. Render the chart in a page

`init` writes one of two shapes: the React component `Chart`, or, with `--plain` or in an app with no framework, the function `mountChart`. Import what `init` wrote into one existing page and render it. Edit that page; do not replace it.
`Chart` brings its own parent element, 300 px tall. `mountChart(element)` draws into an element you give it, and that parent needs its own width and height, or the chart is squeezed to nothing; a page that centers its content can do that to either shape.
Call `mountChart` once the page has that element, and keep what it returns: calling the returned function stops the chart.

If no page fits, add a new page file instead of changing an existing one.

### 3. Run doctor

```sh
npx tachlon doctor --json
```

It must exit 0 and report no finding with status `fail`. Fix each failure by following its `fix` text, then run doctor again.

Doctor asks a browser on this machine whether the chart gets WebGPU or WebGL2. If it reports `GPU_SKIPPED`, no browser answered. Say so in your summary. A skipped GPU check is not a pass.

### 4. Build and check

Run the project's build script. If it has a typecheck or lint script, run that too. Fix what your change broke. Leave unrelated failures alone and name them in your summary.

### 5. Commit

Run `git add` on the files you changed, by name. Never run `git add .` or `git add -A`. Commit on the `add-tachlon` branch.

### 6. Summarise

Finish with a short summary that lists:

- the files you changed;
- the doctor result, finding by finding if any failed;
- whether the GPU was checked.

## Never

- Replace or delete a page or any of its content. Add to a page; do not rewrite it.
- Hand-edit a lockfile. The package manager writes it.
- Change a Content-Security-Policy, unless doctor reports `CSP_WASM_EVAL` or `CSP_CONNECT_SRC`. For `CSP_WASM_EVAL` add `'wasm-unsafe-eval'` to the directive it names (`script-src`, or `default-src` when the policy has no `script-src`), and nothing else; for `CSP_CONNECT_SRC` add the engine's origin, or `'self'`, to the `connect-src` directive it names (or `default-src` when the policy has no `connect-src`, as for `script-src`).
- Push, open a pull request or change a remote, unless the person asks.
- Install anything globally.
- Leave a dev server running. Stop any server you started before you finish.
