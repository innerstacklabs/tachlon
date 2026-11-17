# Gantt

Read `docs/ARCHITECTURE.md` first. Review rules are in `REVIEW.md`. The checkout lives at `~/code/<repo>`, where
`<repo>` is this repository's name on GitHub; a task works in its own worktree, never in that checkout. This file
does not write the product's name: the `name` architecture check allows it only in `BRAND_FILES`.

## Gates

Run these from the repo root before a push. The `rust` job in `.github/workflows/ci.yml` runs the
four cargo gates, then `cargo deny check advisories`:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo doc --no-deps
cargo deny check advisories
```

The strict lint rules sit in the root `Cargo.toml` (`[workspace.lints.clippy]`), and every crate opts in with `[lints] workspace = true`;
`architecture::lints::every_crate_opts_in_to_the_strict_lints` (check 16) enforces it. To count what a new crate still has to fix before it opts in, run:

```sh
cargo clippy -p <crate> --all-targets -- -W clippy::pedantic -W clippy::unwrap_used -W clippy::panic_in_result_fn -W clippy::allow_attributes -W clippy::allow_attributes_without_reason -W clippy::dbg_macro -W clippy::todo -W clippy::print_stdout -W clippy::print_stderr
```

`crates/app/src/web/` compiles for wasm32 only, so the gates above skip it. For a change under
`crates/app/src/web/`, also run what the `wasm` job runs:

```sh
cargo clippy --target wasm32-unknown-unknown -p app -p timeline -p xer -- -D warnings
bash scripts/build-web.sh --no-bundle
```

For a change under `packages/gantt/`, or to `crates/app/src/web/`, also run `pnpm build && pnpm typecheck`
in `packages/gantt/` (the `web` job runs both, then the Playwright specs). Both need the generated
`packages/gantt/src/wasm/` that `build-web.sh` writes.

## Build output

Put the cargo target directory under `~/.cache/cargo-target/<repo>/<branch>`, with `<repo>` as above and the
branch name's `/` written as `-`, never under `/tmp`:

```sh
export CARGO_TARGET_DIR=~/.cache/cargo-target/<repo>/<branch>
```

## Where things are

- `docs/decisions/`: the accepted decision records.
- `docs/WASM_API.md`: the wasm exports and the JSON shapes.
- `README.md`: the front page for a first-time reader: the pitch, what works today, how to try it
  and the keys table (the book's keys page includes that table).
- `docs/features.md`: the feature matrix against other Gantt charts, with its evidence and sources. Edit the table
  there, then run `node scripts/features-matrix.ts --write` to copy it into the README's "How it compares" section.
- `docs/DEMOS.md`, `docs/IMPORT.md`, `docs/MEASURING.md`: the demo page and its URL options, the
  file readers, and how to test, bench and measure; they must match behaviour.
- `docs/CONTRIBUTING.md`: where the project came from, the gates, the book and screencast rules,
  the license.
- `book/`: the mdBook user guide, one chapter per journey; run `pnpm docs:book` in `packages/gantt/` (after `bash scripts/build-web.sh --no-bundle`), then `mdbook build book` (built in CI, deployed to Cloudflare Pages as the guide).
