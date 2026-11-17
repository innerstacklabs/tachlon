# The hosted app is a separate project

Status: accepted. Decided by Dave, 2026-10-27 05:32 and 05:52 (gs#511, the issue and the ruling that followed it).

## Context

Dave, 2026-10-27 05:32 (Telegram), with the old name replaced: *"i think we need a hosted version of [Tachlon] and the user guide to refer to that. the user shouldn't have to clone the repo and run demos/50k/run.sh for example."*

Dave, 2026-10-27 05:52, verbatim: *"i think our priority pretty soon should probably be getting the hosting up and scenarios that start from the hosted user experience. of course we need to keep architecture discipline the hosting ultimately should probably be another project independent of the component itself. to start though i prefer a mono repo."*

The page readers open is therefore a product of its own: a host page, a sample menu and a deploy. Decision 0005 already makes the component a public API behind one entry, `web/src/index.ts`, which the hosted app imports as `gantt-core`. The open question was how to stop the two from growing into each other while they share a repository.

## Decision

1. **The hosted app is a separate project, `apps/hosted/`.** It has its own `package.json`, build, CI job (`hosted`), publishing through the `deploy` job into the Cloudflare Pages project `gantt-book` (the site root; the book is at `/book/`), behind the same Access policy as the book.
2. **It depends on the component only through `gantt-core`** (`web/src/index.ts`) and, through that entry, the Public WASM surface (decision 0005, item 3).
3. **The component holds no hosting URL, flag or sample path.**
4. **Samples live only in `apps/hosted/`.**
5. **The `hosting` architecture check keeps 2 and 3** (`crates/app/tests/architecture/hosting.rs`, check 12; `docs/ARCHITECTURE.md`, "Checks"). It fails when a file of the component names the hosting, when a file of the hosted app imports anything but its own files, `gantt-core`, a `node:` module or a package its `package.json` declares, and when the Vite alias or the `tsconfig.json` path is not exactly `web/src/index.ts`. Its threat model is a good-faith author who forgets: a specifier built at run time is not caught.

## Move it to its own repository when any one of these happens

1. The hosted app needs server-side code, storage, accounts or a secret of its own (anything beyond static files; for example the SaaS parts of PRODUCT.md section 8). *This trigger has fired (gs#704): decision 0011 keeps the server code in this repository, in `apps/cloud`, and adds a rule to the `hosting` check.*
2. The component is published as a package (decision 0005, item 2), so the app can pin a released version instead of a path alias.
3. The two need different visibility or licences (for example a public component repo and a private app).
4. The app has to ship a component version other than `main`'s (an independent rollback or release cadence).

## How it moves

`git filter-repo --path apps/hosted` keeps the app and its history. Then the alias in `apps/hosted/vite.config.ts` and the `paths` entry in `apps/hosted/tsconfig.json` become a package dependency on `gantt-core`, `crates/app/tests/architecture/hosting.rs` is deleted (the boundary is then the repository itself), and the app's part of the `deploy` job (the `hosted-dist` download and the site root) moves with the app.

## Consequences

- One pull request can still change the entry and the app together. That is the cost of a shared repository; the entry stays the only door.
- The check turns the app's reach into the component into a failing test instead of a review comment.
- A change to the component that the hosted app needs goes through the entry first: add the export to `web/src/index.ts` and its row under "Page entry" in `docs/WASM_API.md`, then use it from `apps/hosted/`.
- The size budget covers the hosted app (`size.rs`): 400 lines for a source file, 600 for a test file, with `apps/hosted/dist` and `apps/hosted/public/samples` skipped as generated output.
- Number 0006 is held by PR #452. If that PR has not merged when this lands, `docs/ARCHITECTURE.md` names 0007 beside 0001 to 0005 and the gap stays.

## Stopgap (2026-10-29)

Until S7 (gs#540) lands the app's own Pages project and Access application, the hosted app is also served inside the book's site, at `https://gantt-book.pages.dev/app/`, so invited testers have one link and one sign-in. Dave's meeting with the first tester is on 2026-10-30, and the separate project and its Access application need hand setup that is not done yet. The `deploy` job downloads the `hosted-dist` artifact into the book's output before it publishes, and `apps/hosted` builds with relative asset URLs (`base: "./"`), so one build works at `/` and at `/app/`.

This changes where the app is served, not the decision: the app stays its own project, behind the same `gantt-core` door, and the `deploy-app` job and the `gantt-app` project in item 1 remain the plan. When S7 lands, the `/app/` copy and the `hosted-dist` download in `deploy` are removed.

## Correction (2026-10-29): the layout that replaced the stopgap

The stopgap above no longer describes the site. #833 moved the hosted app to the site root and the book to `/book/`, both in the `gantt-book` Pages project, now reachable at the custom domain `https://app.tachlon.com` (the project's pages.dev address has the same layout) behind the same Cloudflare Access sign-in. The `deploy` job builds the book into `site/book`, downloads the `hosted-dist` artifact into `site/`, and publishes `site`. Old `/app/` and `/app/<path>` links get a 301 to the same path at the root through a `_redirects` file.

What the paragraphs above got wrong, from this date: the app is not served at `https://gantt-book.pages.dev/app/`, there is no "`/app/` copy" for S7 to remove, and the `hosted-dist` download in `deploy` stays as the way the app reaches the one site. What stands: the app is its own project behind the `gantt-core` door, and `base: "./"` still lets one build work at any path. Whether gs#540 (the app's own Pages project and Access application, S7) is still needed is for that issue to say; this correction only records the layout.

## Correction (2026-10-31): gs#540 is superseded, and the job and project item 1 names

Ruling on gs#540 (the app's own Pages project and Access application, S7): its separate-project layout is superseded by the one-project layout above. One Pages project, `gantt-book`, already serves the app and its sign-in; the hosted app ships from the existing `deploy` job, and no separate `gantt-app` project or `deploy-app` job is made. This answers the question the earlier correction left to that issue. The issue itself is closed by its owner, not by this record.

Item 1 and "How it moves" named a `deploy-app` job and a `gantt-app` Pages project. `.github/workflows/ci.yml` has neither and never did: the app publishes through the `deploy` job into the `gantt-book` project, as the correction above describes. Item 1 and "How it moves" now say that, and the stopgap's last paragraph (which still repeats the old names) is superseded by this entry. `scripts/test-deploy-conditions.mjs` now fails when the Decision or How it moves section backticks a job id or a `gantt-*` project that `ci.yml` does not contain.
