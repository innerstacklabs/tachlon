# Engine input ranges

Status: accepted. Decided by karpathy, 2026-11-16 (gantt#2110, gantt#2106; devteam-1 answers 1497 and 1499). A line marked `Decided here:` is the drafter's choice where the review left one open; it stands until karpathy overrules it.

## Context

The engine's dates and durations are `i64` minutes (`Instant` and `Minutes` in `crates/engine/src/calendar.rs`), and its mutators accept almost any value. `Schedule::add_task` checks the calendar and a negative duration or remaining, and nothing else (`crates/engine/src/model.rs`). `add_dependency` checks the two ends and no lag (`model.rs`), and `add_link` calls it (`crates/engine/src/links.rs`). `set_dependency` and `set_dependency_replay` store any lag (`links.rs`). `set_snet` and `set_limit` store any instant (`model.rs`; `crates/engine/src/model/limit.rs`). The seat on gantt#2098 found that a link lag near `i64::MIN` overflows the calendar arithmetic: a panic in a debug build and a wrapped instant in a release build (gantt#2106). Hosts will now pass plain data to the headless solve (gantt#2081), so inputs stop coming only from our own editors and importers. Code is cited at `origin/main` `6b05b07f`, by file and symbol. The review and gantt#2106 say `update_link`; no function has that name at head, and the two functions are `set_dependency` and `set_dependency_replay`.

## Decision

1. **One engine module states every range as a named constant:** `crates/engine/src/model/range.rs`. No other file states a bound for these inputs. `Decided here:` the module name, and the values, because the review named none:

   | Constant | Value | Applies to |
   |---|---|---|
   | `MAX_DATE` | `1000 * 366 * DAY` minutes from the origin, either side (`MIN_DATE = -MAX_DATE`) | `snet`, the limit dates `snlt`, `fnet`, `fnlt` and `resume`, both pins, `actual_start`, `actual_finish`, the data date, the must-finish-by date |
   | `MAX_DURATION` | `100 * 366 * DAY` working minutes | `duration` and `remaining`, each from 0 |
   | `MAX_LAG` | `MAX_DURATION` | a lag, from `-MAX_LAG` |

   Reason for the sizes: `MAX_DATE` is under 2^29 and `MAX_DURATION` under 2^26, so the longest chain the engine can hold (2^32 tasks, each with a full duration and a full lag) sums to under 2^59, below the 2^61 at which the engine already floors a late date (`LATE_FLOOR` in `crates/engine/src/cpm/admissible.rs`). No project spans 1,000 years, and a P6 file that carries a sentinel date for "none" is far outside. `crates/engine/tests/ranges.rs` checks these sizes. That sum is in working minutes, and a calendar turns it into elapsed minutes: 2^32 such tasks span 1.9 x 10^18 minutes on a 40-hour week (2,400 working minutes), under 2^61, and 4.6 x 10^21 on a one-minute week, over `i64::MAX`. The sum stays under 2^61 for a calendar of at least 1,979 working minutes a week; a one-minute week passes it at about 2.2 million full-size tasks, a chain no editor or importer makes in good faith. The calendar's own bound, `MAX_EXCEPTION_DAY` (`calendar.rs`), stays where it is.
2. **The mutators enforce the ranges and return one typed error.** `Decided here:` the error is `ScheduleError::OutOfRange { input, task or dependency, value }`, where `input` names the field. On an error nothing has changed. Built so far (gantt#2106, lags and durations): `OutOfRange { input: RangeInput, item: RangeItem, value }`, where `input` is `Duration` or `Lag` and `item` a `Task` or a `Dependency`; a negative duration stays `NegativeDuration`. The checks go in:
   - `add_task`: every field of the `Task` it is given, the `Limit` and its pin included;
   - `add_dependency`, which also covers `add_link`;
   - `set_dependency` and `set_dependency_replay` (`links.rs`), the review's `update_link`;
   - the setters: `set_snet`, `set_limit`, `set_duration`, `set_actual_start`, `set_actual_finish`, `set_remaining`, `set_data_date` and `set_finish_by`. `Decided here:` `set_snet`, the actuals, the data date and the must-finish-by date are in the list although gantt#2106 names only lags, limits, pins, durations and remaining, because they are instants that reach the same calendar arithmetic. `set_data_date` returns nothing today and gains a `Result`.
3. **A check in the setters alone would miss most inputs,** which is why `add_task` and `add_dependency` check too. These callers build a `Task` or a `Dependency` literal from raw integers and never call a setter for those fields:
   - the document reader: `read_task` in `crates/app/src/document/read/items.rs` (the `Task`, with `read_limit`), added by `task` in `document/read/build.rs`; the link in `add_link` in `items.rs`; the data date in `read` in `document/read.rs`;
   - the MSPDI importer: `read_task` in `crates/import/src/tasks.rs` and `add_links` in `crates/import/src/tasks/links.rs`;
   - the XER importer: `build` in `crates/import/src/xer_activities.rs` (two `add_task` calls) and `build` in `crates/import/src/xer_links.rs`;
   - insert in the chart: `run_insert` in `crates/app/src/structure.rs`.
   Plain data reaches the engine only through the document reader, so it needs no check of its own.
4. **Doors only translate the typed error.** No door states a range.
   - A document or plain data is refused: the reader's error names the record, the field and the value, as it already does for the engine's other refusals (`task` in `build.rs`, `add_link` in `items.rs`).
   - An import does not fail. An importer pre-checks with the same constants, leaves the value out and writes one counted note per input kind, carrying the count and the first five task ids (the shape of A devteam-1-1494). `Decided here:` what "left out" means per input: a limit date, a pin, an actual date or a remaining value is not set; a lag is read as 0, as a percent lag already is (`add_links` in `tasks/links.rs`); a duration is read as `MAX_DURATION`.
   - An edit in the chart shows the refusal through the existing path.
5. **Tests.** One test per door feeds `i64::MIN`, `i64::MAX` and one past each bound, and each runs red with its check removed. The brute-force test gives the same answer in a debug build and a release build for every in-range input (gantt#2106).

## Consequences

- `set_snet` runs on every drag frame (`Session::run` in `crates/app/src/edit.rs`, the `Edit::Move` arm); its check is two integer compares. The PR states the drag bench before and after.
- Undo and redo go through `set_dependency_replay` and the setters, so a value that was in range when recorded replays; history never holds one that is not.
- A document saved by an older build with an out-of-range value no longer opens. No editor or importer produces one in good faith (A devteam-1-1497), so no migration is planned; the refusal names the value.
- `ScheduleError` gains a variant, so every `match` on it that lists variants changes (`ScheduleError` in `crates/engine/src/model/error.rs`).
- gantt#2106 may still split at refinement (lags first, then limits and pins, then the readers); each part cites this record.

## Rows this governs

gantt#2106 (every part), and through it gantt#2081 (plain data to the headless solve), gantt#2087 (`update(records)` applies inputs through the same mutators) and gantt#2074 (`mustFinishBy` is a date and takes `MAX_DATE`).
