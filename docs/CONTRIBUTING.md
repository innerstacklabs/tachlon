# For contributors

Where the project came from, where it is going, the gates, the book, the screencast rule, and the license. The architecture is [ARCHITECTURE.md](ARCHITECTURE.md). To run the chart from a checkout, read [DEMOS.md](DEMOS.md), "Running it"; frame times are in [MEASURING.md](MEASURING.md). To put the chart in your own product, read the book's [Install it in your app](../book/src/install.md).

## Where it came from

It evolved from a hobby spike (Dave, 2026-10-20) that asked: can a Rust scheduling engine and a Rust GPU timeline renderer (wgpu, viewport-rendered into a canvas), in one WebAssembly module behind a thin TypeScript shell, handle a large schedule (50,000 tasks) with smooth zoom and drag in the browser?

The first plan is not kept in this repository; the decision records in [docs/decisions/](decisions/) hold the choices that came from it.

The architecture is [docs/ARCHITECTURE.md](ARCHITECTURE.md).

The WASM API is [docs/WASM_API.md](WASM_API.md).

## Where this is going

It is a Gantt and scheduling component for developers, in the category of Bryntum, DHTMLX and Syncfusion: fully featured, blazingly fast, and open source with no strings ([decision 0021](decisions/0021-gantt-component-for-developers.md)). It is built for developers who put a Gantt chart in their own product. Opening P6 XER and MS Project files is one of its features, not its audience. The measured speed claim is in [docs/MEASURING.md](MEASURING.md), the comparison with other charts in [docs/research/2026-11-13-gantt-comparison.md](research/2026-11-13-gantt-comparison.md), and each user journey has a chapter in the [book](../book/src/index.md).

## Gates

The gates to run before a push are the steps of `.github/workflows/pr.yml`; a pull request runs the same steps. For what each part of the code must keep, read [ARCHITECTURE.md](ARCHITECTURE.md), "What each part keeps". From the repo root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
cargo deny check advisories
```

For the web package, run `bash scripts/build-web.sh --no-bundle` at the root, then `pnpm build && pnpm typecheck` in `packages/gantt/`. [CLAUDE.md](../CLAUDE.md) lists the same gates, including the wasm build, and [MEASURING.md](MEASURING.md), "Tests and benches", holds the tests.

Check 13 (`the_old_name_stays_out_of_the_repository` in `crates/app/tests/architecture/name.rs`) also keeps the product's name in a few files. A tracked file, or a tracked path, may name the product, in any case, only if it is in `BRAND_FILES` (the README, `apps/site/`, `book/`, `docs/PRODUCT.md`, `docs/decisions/` and five files of `packages/gantt/`) or is under `examples/`; every other line that names it is one failure. That includes scripts, tests, fixtures, the other docs and this guide. A script or test that needs the name reads `PRODUCT_NAME` from `packages/gantt/src/product.ts`, or uses a neutral name. Do not add a file to `BRAND_FILES` to pass the check: the list only shrinks (decision 0013 in `docs/decisions/`).

A pull request with a user-visible change adds its line under `## Unreleased` in `CHANGELOG.md`, never under a published version: what a developer must change, or what is new. `packages/gantt/tests-node/changelog-sealed.test.ts` fails when a section whose version is in `packages/gantt/releases.json` differs from its `changelog` digest there; the release pull request renames `## Unreleased` to the new version and records the digest the test prints.

### Package size

The `package size` step of the `hosted` job (`scripts/package-size.ts`) packs `packages/gantt/` and compares the tarball with the one at the branch's merge base with `main`; no file holds a size limit, so a pull request never edits one. A push to `main` records its tarball size as the artifact `package-size-<bytes>` (kept 90 days) and never fails on size. A branch fails when its own growth is over 8,192 B. For a growth that is meant, add the line `package-size: A devteam-1-<n>` (the product owner decision that allows it) to the pull request body and re-run the job; the check reads the body when it runs. If the merge base has no record, the nearest recorded commit on `main`'s first-parent history stands in and the log names it; if none is recorded, the step exits 2 (unknown). The cap is per pull request: many small growths each pass, and the record on `main`, one artifact per commit, is where the trend reads.

The `npm-publish` workflow runs `scripts/package-size.ts --published`: it prints the tarball's growth since the version now on npm and stops before `npm publish` when the growth is over 65,536 B and the `## <version>` section of `CHANGELOG.md` has no line `package-size: grew <N> B` with N within 4,096 B of the growth.

## Checking the install prompt

The README's first-screen prompt is tested on a real assistant by hand, never in CI, because each run spends quota. After `bash scripts/build-web.sh --no-bundle` at the repo root, then `pnpm install --frozen-lockfile` and `pnpm build:lib` in `packages/gantt/` (the check reads `packages/gantt/dist-lib/` and names these steps when it is missing), run `node scripts/agent-install-check.ts claude` (capped at 1 USD) or `node scripts/agent-install-check.ts codex` (capped at 600 s), once per assistant. It copies the test app (`scripts/fixtures/agent-install/app/`) to a temp folder with a git history on `main` and a `pre-push` hook, serves the packed, unreleased package from a local registry (`scripts/lib/local-registry.ts`), runs the prompt, and prints one line for each check (`scripts/lib/agent-install-check.ts`). It writes the results to `results.json` in the kept folder, or to the file named by `--json`. `--record <dir>` also keeps, for the install video, the assistant's stream, the page screenshot, the timings and a story file for `scripts/narrate.ts` (`scripts/lib/agent-install-record.ts`); record both assistants into the same folder, and a story says the install works the same in others only when the other assistant's run passed. Exit 0: every check passed. Exit 1: a check failed. Exit 2: the run could not happen (no assistant binary, no browser, no network, no build). `--app <vite-react|vue|next|html>` starts from another fresh app, `--fit` adds the fit task in a second fresh session, `--version <v>` tests that published version (no build needed), and the last line printed is a row of `docs/agent-install-matrix.md`, which holds the command that re-runs every app and its table. Claude runs as `sonnet` with no user settings, MCP servers or skills. The unit tests, `packages/gantt/tests-node/agent-install-run.test.ts`, use stub assistants.

## Releasing

`main` is staging: every push to it deploys to the Pages preview at `main.gantt-book.pages.dev`. `release` is production (the hosted site): a push to it deploys with the Pages production branch. `release` moves only by a fast-forward to a green `main`, so what testers open is a `main` commit whose checks passed.

The product page (`apps/site`, without `launch.html`, with the built package copied beside it as `package/`: its hero chart mounts from it) is a separate Pages project, `gantt-product-page`. A push to `main` deploys it to the private preview at `main.gantt-product-page.pages.dev`; a PR does not. A push to `release` publishes it to the `public-site` project (the product's own domain) only while the repository variable `SITE_LIVE` is `true` and `scripts/site-claims.ts` finds no claim left in the pages. The features page (`apps/site/features.html`) links each feature to a heading of the package README or a chapter of the book; `node scripts/features-links.ts apps/site/features.html` fails when a link names a section that does not exist, and runs in the `select` job of `ci.yml` and the `docs` job of `pr.yml`.

To see the page with its live chart, build the package (`pnpm build:lib` in `packages/gantt/`), then `cp -r apps/site /tmp/site && cp -r packages/gantt/dist-lib /tmp/site/package` and serve `/tmp/site` with any static server that sends `.wasm` as `application/wasm` (`python3 -m http.server` does). Opened as a file, the page shows its screenshot instead.

## The book

The [book](../book/src/index.md) has two parts: "For developers", for a developer who puts the chart in a product and their coding assistant, and "Guide", for a person who uses a chart, with one chapter per user journey. No page about how this project is built, checked or structured is in the book: those pages are this file and [ARCHITECTURE.md](ARCHITECTURE.md). The book is hosted as the guide (the link in the README). CI deploys it from `main`. A preview link on each ready pull request waits on #455: pull-request CI does not post one yet.

To build the book yourself, install [mdBook](https://rust-lang.github.io/mdBook/) 0.5 and run `mdbook build book`; it writes `book/book/`, which git ignores. The `book` job in `.github/workflows/ci.yml` runs that build on every push and fails if a chapter file or an include is missing, or if the keys page lost the table it includes from `docs/KEYS.md`; the `hosted-browser` job's book spec and the `deploy` job use its build, not their own. A PR that adds a user-visible feature adds its chapter section in the same diff.

The look is the book's own mdBook theme in `book/theme/`: `index.hbs` and `css/variables.css` override the stock files (two themes, Light and Dark, in the chart's calm palette), `book.css` adds the components a chapter uses (the clip, the screenshots, the tip, the front page), and `favicon.svg` is the tab icon, a copy of `apps/site/favicon.svg`, the product's mark (`media/manifest.json` has its row).

A guide chapter is one task the reader wants to do. The shape, in order:

1. The H1 is the task in plain words, with the journey id in an HTML comment under it.
2. One or two sentences on what you can do.
3. The clip: a `<video class="clip">` with an MP4 and a poster, recorded by `demos/screencast/clip.mjs` from a step in `demos/screencast/steps-features.mjs`.
4. Numbered steps, each one action.
5. One or two screenshots, as `<figure class="shot">`, each with a caption that says what the numbers in the picture point at.
6. A tip, as `<div class="book-tip">`, and a "Good to know" list for limits.

The commands that record a chapter's clip and screenshots are in [DEMOS.md](DEMOS.md), "Recording the screencast" (the row for the book's clip). `clip.mjs` writes `<name>.mp4` (the clip), `<name>.webp` (the poster, the last frame) and the screenshots the step takes through `ctx.shot`, and refuses a clip or an image over the budget it holds as `CLIP_SECONDS`, `CLIP_BYTES` and `IMAGE_BYTES`.

For the open-a-schedule chapter, `apps/hosted/tests/j1-open.spec.ts` opens the office fit-out sample from the menu, opens your own file with the chooser and checks that no request is made for it, and opens a truncated file to see the reader's error. `packages/gantt/tests/open.spec.ts` opens files from the page (the Open button, Ctrl+O, a drop) and checks the rows, the unsupported panel and that no network request is made. The XER mapping is in [IMPORT.md](IMPORT.md). A chapter not yet rewritten in this form names its specs in its own "Where it is checked" section.

For a chapter that has a file under `packages/gantt/guide/` (the task dialog is the first), the feature's text, its goal and its steps, goes in that file, in words that fit any product, and the chapter includes it by anchor. The chapter keeps what is true only of the hosted page: the pictures, the sample links and the "Where it is checked" section. [The folder's README](../packages/gantt/guide/README.md) holds the format, and `node scripts/check-guide.ts` checks it.

## The screencast

The screencast at the top of the README is recorded by `demos/screencast/run.sh` ([DEMOS.md](DEMOS.md), "Recording the screencast"). A PR that adds a user-visible feature adds its step to `demos/screencast/steps-features.mjs` in the same diff, and the wrap of a sprint that merged one re-runs the script and commits the new media.

## License

The project is open source under the MIT license: `LICENSE` holds the text, Copyright (c) 2026 Inner Stack Labs LLC.

**Using it.** Use, copy, modify and embed it in any product, free of charge, under the MIT terms.

**Format readers.** The P6 XER reader (`crates/xer`) is its own crate, with no dependency on the engine, so other tools can use it alone ([decision 0004](decisions/0004-format-crates-licensed-separately.md)).

**Contributing.** Open a pull request. A contribution comes under the same MIT license as the rest of the project; there is no contributor agreement to sign. To report a bug or ask for a feature, open an issue in this repository; there is a template for each ([bug](../.github/ISSUE_TEMPLATE/bug.md), [feature request](../.github/ISSUE_TEMPLATE/feature.md)).
