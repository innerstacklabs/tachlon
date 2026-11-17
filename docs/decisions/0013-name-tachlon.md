# The product is named tachlon, and the old names leave the repository

Status: accepted. Decided by Dave, 2026-10-31 20:59 (terminal; gs#1120). Supersedes the rename in gs#524.

## Context

The project had two names before this one. It was called `gantt-spike` until gs#524, then Mantis Gantt (decision 0006, which called it a working code name). Both are vestiges: every doc, script and agent has to translate them.

## Decision

1. **The product is named `tachlon`.** It is the only product name this record uses. The retired names are written here once, above, as history.
2. **The rename is done in parts (gs#1120).** Each part is its own pull request and cleans the files in its area: the code names, the visible product name and the packages, the docs and the book, the repository and the runner label, then the lifeos side.
3. **A guard comes first, and it only shrinks.** Check 13 (`name.rs`, `the_old_name_stays_out_of_the_repository`) fails on a tracked file or a tracked path that names a retired name, in any case. Its exemption is the allowlist, `crates/app/tests/architecture/name/allowlist.txt`: one exact path per line, sorted bytewise. A listed file that no longer names a retired name fails the check, and so does a listed path that is not tracked, so a pull request that cleans or moves a file deletes or corrects its line. A pull request that adds a line to the list is blocked in review (`REVIEW.md`).
4. **History records keep their own words.** A decision record, a design note or a research note written under an old name stays as written and stays on the allowlist. A line of a `docs/**/*.md` file that ends with `<!-- verbatim -->` is a verbatim quote and is not searched.
5. **The select job in `.github/workflows/ci.yml` matches the same names.** A change that names one runs the code jobs, so a docs-only pull request cannot add a line that the architecture tests would reject.

## Consequences

- The finish line of gs#1120 is an allowlist that holds only the history records and this record.
- A pull request that merges after the guard and writes a retired name is red until it uses the new name.
- Decision 0006 stands as written.
