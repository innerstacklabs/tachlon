# Examples

Two small sites to copy: `plain/` (one page and one script, no framework) and `react/` (an app on the React
component from the package's `/react` entry). Both run on Vite. Each depends on the published `tachlon` package, shows a three-task chart, shows the name of
the task you pick, and has a Stop button that calls `destroy()`.

To run one, copy its folder somewhere, then install and start Vite:

```sh
npm install
npx vite
```

To run one on a build that is not published yet, build the package (`pnpm build:lib` in the chart's folder), pack it
(`npm pack` there), and in the copy install the tarball under the name the examples import:

```sh
npm install tachlon@file:<path to the tarball>
npx vite
```

`save/` is the host save contract's example (decision 0022): a React app on the first section's handlers form, talking to `save/server.ts`, a fake host server that can delay, refuse or drop requests on a script. Its scenarios are in `save/scenarios.ts`; those listed in `PENDING` in `save/spec.ts` fail until the `save` option is built, and the run fails if one passes. It is not a site to copy yet.

`node examples/check.ts plain react save` does the tarball way in a new folder and runs each example's `spec.ts` in
Chromium (it needs the library build and Playwright's Chromium, as the `web` job has them, and the registry
for the example's own packages). It is a step of the `web` job in `.github/workflows/ci.yml`.

`node examples/check.ts --registry [<version>] plain react` does the same with the published package from npm
(the version the example pins, unless you give one), and needs no library build. Run it by hand after a release; CI does not run it.
