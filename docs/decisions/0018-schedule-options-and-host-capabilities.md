# Schedule options and host capabilities

Status: accepted. Decided by karpathy, 2026-11-16 (gantt#2110; devteam-1 answer 1499 accepts the architecture review of the host API, gantt#2076 comment 6938711824, section 3; option names and values by Dave, 2026-11-16 21:17, devteam-1 answer 1496 item 7). A line marked `Decided here:` is the drafter's choice where the review left one open; it stands until karpathy overrules it. This is the record gantt#2103 and gantt#2105 ask for.

## Context

Two hosts can want different answers from the same network. The first two cases come from fitting a host's data to the engine: what the late start of a started task reads (gantt#2105), and whether an unstarted task's remaining duration sets its length (gantt#2103). Dave made each a Tachlon option so that no host writes adapter code for it. `Schedule` has no settings value today, only single fields with single setters (the `Schedule` struct in `crates/engine/src/model.rs`), each copied into the pass by hand (the `Pass` value in `Schedule::compute`, `crates/engine/src/cpm.rs`; `Pass` in `cpm/pass.rs`). The rows as first written carried each option as its own top-level document key, but the document's top level is a fixed list of 13 keys and refuses any other (`TOP` in `crates/app/src/document/read.rs`; `fields_allowing` in `document/fields.rs`), so every later option would change the reader, the writer, the types of three adapters and the schema. A host's capability profile (gantt#2090) is a different kind of thing: it limits what a person may create and never changes a computed number. Code is cited at `origin/main` `6b05b07f`, by file and symbol.

## Decision

### Schedule options

1. **One `options` object travels with the schedule.** The engine holds one settings value on `Schedule` (`crates/engine/src/model/options.rs`); the document has one optional top-level key, `options`; plain data has one optional top-level member, `options`. Adding an option adds a member, never a top-level key or a `Schedule` field.
2. **The member names are the same in all three.** `Decided here:` the document and plain data both spell them in camelCase (`startedLateStart`, `unstartedRemaining`), and the adapters pass `options` through as one object without reading it, so a new option changes no adapter. The Rust field is the snake_case spelling of the same name, mapped in one table in `options.rs`. This replaces the document spellings `started_late_start` and `unstarted_remaining` of A devteam-1-1496 item 7; the values there stand.
3. **An unknown member is refused,** in the document reader and in the plain validator, with an error that names it. So is an unknown value.
4. **Each option is an enum with named values, never a boolean.** The first two:

   | Member | Values | Default | Meaning |
   |---|---|---|---|
   | `startedLateStart` | `"p6"`, `"project"` | `"p6"` | The late start reported for a started, unfinished task. `p6`: the late start of the remaining work, which is what Primavera P6 shows. `project`: the actual start, which is what Microsoft Project shows (as replicated by MPXJ). Float and the critical path are the same under both |
   | `unstartedRemaining` | `"off"`, `"on"` | `"off"` | `on`: an unstarted task that has a remaining duration is scheduled with that length. `off`: remaining is ignored until the task starts, as today (`Task::work` in `crates/engine/src/model/progress.rs`) |

   `Decided here:` `"off"` and `"on"` as text, not `false` and `true`, so that a third value later is not a type change.
5. **Each default is one named constant** in `options.rs` (`DEFAULT_STARTED_LATE_START`, `DEFAULT_UNSTARTED_REMAINING`), and nothing else states a default: not the reader, the adapters, the schema or the docs' examples.
6. **The document writes `options` only when a member differs from its default, and then only those members,** in a fixed order. A schedule on defaults keeps its saved text (decision 0010 item 6). An older build refuses a document that holds `options`, as it refuses one that holds a task `key`.
7. **`startedLateStart` changes what is reported, not the solution** (A devteam-1-1496 item 8). The solution's late start stays the remaining work's, because the backward pass reads it. One accessor, `Schedule::late_start_shown(task, dates)`, applies the option. **Only the accessor reads a late start outside the engine:** an architecture test refuses any read of `TaskDates::ls` under `src` in `crates/app`, `crates/timeline` and `crates/import` other than through it. `Decided here:` the test covers `src` only; tests that compare solved dates are outside the rule. At head there is one such read in `src` (`Session::task_details` in `crates/app/src/select.rs`, found by `git grep` for `.ls`); it moves to the accessor, and `results()`, the snapshot, columns, exports and print use the accessor from their first line.
8. **Options are tested as named sets, not as every combination.** The sets are `default` (`p6`, `off`) and `project-conventions` (`project`, `on`). The conformance suite (gantt#2083) runs each of its networks under the set it names. One test asserts that the `default` set gives the same answers as today's brute-force oracle, and it runs red when a default constant changes. A new option adds a member to each set and, at most, one new named set.

### Host capabilities

9. **Capabilities are a separate value passed at mount** (and to the headless open): the capability profile of gantt#2090. Its members are that issue's to define.
10. **They are enforced at the edit gate** of decision 0019, as its second check, after read-only and before the host's before-edit function. Nothing in the engine reads them, and they never change a computed number.
11. **They are never stored.** Not in the document, plain data, a snapshot or the undo history. An architecture test refuses a reference to the capability type from `crates/app/src/document`. A session opened with no profile allows everything.
12. **The line between the two:** a setting that changes a computed or reported number is a schedule option and travels with the schedule; a setting that limits what a person may do is a capability and belongs to the host. `durationUnit` and `dateForm` (decision 0017) are neither: they describe how values are written.

## Consequences

- Row tl2103so: its choice 5 becomes the one `options` object, and it adds the architecture test of rule 7. `unstarted_remaining: true` becomes `unstartedRemaining: "on"`, and the value `"actual"` in the row text is `"project"`.
- Row tl2103pk becomes "plain data passes `options` through" and adds the validator's refusal of an unknown member.
- gantt#2090's rows are written against the gate, not the engine, and wait for decision 0019.
- The document gains a 14th top-level key. [Decision 0010](0010-document-format.md) is amended to say so.
- The headless `results()` (tl2081rs) and tl2103so meet at the accessor: whichever merges second uses it.

## Rows this governs

gantt#2103 and gantt#2105 (tl2103so, tl2103pk), gantt#2090 (every row), gantt#2083 (the named sets), gantt#2081 (tl2081rs reads the late start through the accessor), gantt#2085 (the gate order).
