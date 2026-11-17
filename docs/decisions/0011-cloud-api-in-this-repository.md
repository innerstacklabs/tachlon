# The cloud API lives in this repository, in apps/cloud

Status: accepted. Decided by Dave, 2026-10-27 (gs#704, ruling 2).

## Context

The hosted app (decision 0007) is static files. The cloud work (gs#704) adds server code: an API under `/api/docs` that stores documents (gs#704, ruling 2, comment 6480674126). Decision 0007 named this as its first trigger for moving the app to its own repository: "The hosted app needs server-side code, storage, accounts or a secret of its own."

Dave, 2026-10-27, verbatim: *"to start though i prefer a mono repo"*. The same words settled decision 0007.

## Decision

1. **The server code stays in this repository, in `apps/cloud/`.** It is a project of its own, like `apps/hosted/`: its own manifest, build, tests and CI job, and its own checks. The gates of the Rust workspace do not cover it, and its checks do not gate the chart.
2. **Decision 0007's trigger 1 has fired, and the answer is the same as before: stay in one repository for now.** The other three triggers still stand as 0007 lists them, and a move to a repository of its own is still `git filter-repo --path`.
3. **The chart never names the cloud API.** No file of the component holds `/api/docs` or imports from `apps/cloud`.
4. **The hosted page reaches the cloud API through one file, `apps/hosted/src/store.ts`.** No other file in `apps/hosted/src` names `/api/docs` or imports from `apps/cloud`. The rest of the page calls `store.ts`, so a change of server, or of how the page stores a document, is one file.
5. **The `hosting` architecture check keeps 3 and 4 as far as it reads** (`crates/app/tests/architecture/hosting.rs` and `hosting/cloud.rs`; `docs/ARCHITECTURE.md`, "Checks"). The route rule and the import rule read the scripts under `apps/hosted/src` and the component's files. A dependency rule reads `dependencies`, `devDependencies` and `optionalDependencies` of `apps/hosted/package.json` and `web/package.json`, and fails on one that resolves into `apps/cloud` (a `workspace:`, `file:`, `link:` or `portal:` path, or the package name `@gantt/cloud`), since a declared package lets any file import it. Inline scripts in `apps/hosted/index.html`, non-script files in `apps/hosted/src` and `apps/hosted/public/` are not read; review holds them to 4. Its threat model is the one of decision 0007: a good-faith author who forgets. A route built at run time is not caught.

## Consequences

- `apps/cloud` holds the only code that runs on a server. `docs/ARCHITECTURE.md`, "Layers", shows it beside `apps/hosted`.
- `docs/PRODUCT.md`, "Later, speculative: a hosted service and MCP servers", says "Today: a hosted app, no server". That stays true until the first deploy of `apps/cloud` and is then replaced by the amendment recorded there.
- `store.ts` is the one place the page may import from `apps/cloud` (for example its request and response types). The check does not stop `store.ts` importing more than it should; review does.
- This record comes before the code it governs (decision 0003). No code under `apps/cloud` lands with it.
