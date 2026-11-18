# Review rules

Every review seat reads this file. The diff and the issue text come with the prompt. For what the
code must keep, read `docs/ARCHITECTURE.md` ("What each part keeps", "Checks").

## Threat model

A good-faith agent or person making mistakes. Nothing here assumes hostile input.

## Block on

- A schedule or summary that differs from a full `compute()` after any edit, undo or redo.
- A new test that still passes with the change removed.
- A per-frame change with no frame-time read from a GPU machine. CI has no GPU: its software GL
  run is a smoke test, not a result (docs/MEASURING.md, "Measuring frame times").
- A file over its size budget (`docs/ARCHITECTURE.md`, "Checks", the `size` check).
- Behaviour, a key or a flag changed without the page that describes it in the same diff: the
  keys table in `docs/KEYS.md`, `docs/DEMOS.md`, `docs/IMPORT.md` or `docs/MEASURING.md`.
- A `#[allow]` without a reason, or an `#[expect]` whose reason is not an argument.
- A citation by line number instead of by heading, in code, docs, a commit or a PR.
- A line added to the old-name allowlist (`crates/app/tests/architecture/name/allowlist.txt`), or a file added to `BRAND_FILES` in `crates/app/tests/architecture/name.rs`. The lists only shrink (decision 0013), and the product name is allowed under `examples/` only.

## Note, do not block

- A finding that needs deliberately hostile input.
- A limit that `main` already has.

## Evidence to ask for

- For each new test, the break that turns it red: the line removed or changed, and the failing
  run.
- For a performance claim, the bench or frame number, the machine and the command that made it.
