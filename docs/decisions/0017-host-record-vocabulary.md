# The host record vocabulary

Status: accepted. Decided by karpathy, 2026-11-16 (gantt#2110; devteam-1 answer 1499 accepts the architecture review of the host API, gantt#2076 comment 6938711824). A line marked `Decided here:` is the drafter's choice where the review left one open; it stands until karpathy overrules it. Amends [decision 0010](0010-document-format.md) and [decision 0012](0012-embeddable-component.md).

## Context

A host page talks to the component through plain records: tasks, links, calendars and a few schedule values. Four surfaces written in one week spell the same record four ways. Plain data in uses `start`, working days of 480 minutes and `YYYY-MM-DD[THH:mm]` (`fromTasks` in `packages/gantt/src/adapters/plain.ts`, with `WORKING_DAY` from `adapters/dhtmlx.ts`). The document uses `actual_start`, minutes and minutes from an origin (`TASK` and `read_task` in `crates/app/src/document/read/items.rs`; `read_limit` in `read/limit.rs`). The edit log uses `start` for the start-no-earlier-than limit, `actual_start`, minutes as text and `YYYY-MM-DD HH:MM` (`Change` and `log_typed` in `crates/app/src/edit_log.rs`). The planned `results()` uses `key`, `id` and working minutes (answer A devteam-1-1496 item 3). At 0.3.0 these are public API, and a host would write one converter per surface. Code is cited at `origin/main` `6b05b07f`, by file and symbol.

## Decision

1. **One vocabulary for every host-facing record.** Plain data in (`fromTasks`), the snapshot out (`snapshot()`, `toPlain`), `results()`, the changes inside a commit (decision 0019), `update(records)` and the before-edit payload use the names, units and date forms of the tables below, and no others. A surface may carry fewer fields; it never renames or re-units one.
2. **The document keeps its own form.** The Tachlon document (decisions 0010 and 0012) stays the storage format: snake_case keys, integer minutes, instants as minutes from `origin`, tasks and links by position. The "Document" column below is the mapping. The Rust reader stays the one typed check for every door that writes a document.
3. **Names are camelCase.** The short limit names `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `alap` are accepted as they are (A devteam-1-1490 item 3).
4. **A value has a JSON type, not a text form.** A date is text, a duration is a number, a flag is a boolean, and "none" is `null`. No surface writes a number as text or an absent value as an empty string.
5. **Durations and lags carry one explicit unit per payload.** The top-level member `durationUnit` is `"days"` (default), `"hours"` or `"minutes"`, and it applies to every field marked *duration* below, in and out. A host that stores whole days sends them with the default; a host that stores integer hours sets `"hours"` and sends them unchanged. An hour is 60 working minutes. A day is the working day of the calendar that measures the value (the task's calendar for `duration`, `remaining`, `float` and `freeFloat`; the link's lag calendar for `lag`): `hoursPerDay` for a simple calendar, else the average working day the app already uses (`day_minutes` in `crates/app/src/edit/duration.rs`). After conversion a value that is not a whole minute is rounded to the nearest minute, as plain data does today (`fromTasks` in `plain.ts`). Output is the exact quotient, so it can be a fraction. `Decided here:` one unit member per payload and not unit-suffixed field names, because a suffix would give each duration three names. `Decided here:` computed floats follow `durationUnit` too; this replaces "floats in working minutes" in A devteam-1-1496 item 3, because one field has one unit.
6. **Dates are wall-clock text with no zone: `YYYY-MM-DD` or `YYYY-MM-DDTHH:mm`.** Input takes either form on every date field. A date-only value is read by the kind of its field:
   - *start kind* (`projectStart`, `dataDate`, `start`, `resume`): 00:00 of that day.
   - *finish kind* (`mustFinishBy`): the end of that day, 24:00, which is 00:00 of the next day.
   - *a date limit* (`snet`, `snlt`, `mso`, `fnet`, `fnlt`, `mfo`, and the two halves of the typed kinds `so` and `fo`) is a bound on a working minute, so a date alone is read on the task's calendar and never as midnight. `Decided here:` a start limit (`snet`, `snlt`, `mso`, `so`) is the first working minute of that day, and a finish limit (`fnet`, `fnlt`, `mfo`, `fo`) is the end of that day's last working minute: 16:00 on an 08:00 to 16:00 calendar, where the end of the day, 00:00 of the next, falls a working day late. A day with no working time reads as an actual does: 00:00 for a start limit and 24:00 for a finish limit. This is the same in the separate members and in the typed `constraint`, in `mount`, `update` and `setData`, and in the limit editor (`day_edge` in `crates/app/src/host/read.rs`, `limitAt` in `packages/gantt/src/adapters/plain-limits.ts`). It replaces what this rule held before: a finish limit read as the end of the day, and `snet`, `snlt` and `mso` read as 00:00, which moves them to the first working minute (the schedule of `snet` and `mso` does not change, since the engine starts at the next working minute; the late start of `snlt` does). `start` stays 00:00 and is not a limit, so `start` beside `snlt` on one date alone are two instants and not `so`: send `so`, or give a time.
   - `Decided here:` a date-only `actualStart` is the first working minute of that day on the task's calendar, and a date-only `actualFinish` is the end of its last working minute (00:00 and 24:00 when the day has no working time), because an actual is a fact drawn on the bar, not a bound. How the cell editor reads a typed actual today was not checked.
7. **Output dates.** Output writes `YYYY-MM-DDTHH:mm`. `Decided here:` the top-level member `dateForm`, `"minute"` (default) or `"day"`, makes every output date `YYYY-MM-DD`: a start-kind field is the day of its instant, and a finish-kind field (`finish`, `lateFinish`, `actualFinish` and the finish limits) is the day of its instant less one minute, which is what the limit cells already show (`edit/limit.rs`). A host with date-only columns then stores what it receives. Reason: a host can store dates only. `Decided here:` `dateForm: "day"` also governs the dates a host hands in through `update`, `hostEdit` and `setData`: each is `YYYY-MM-DD`, read by rule 6 on the task's calendar, so a host writes back what it was given, and a date with a time of day is refused as `badValue` (product owner, 2026-11-18). `start` stays 00:00 (rule 6), and the plain `tasks` read at `mount` take either form as before.
8. **`durationUnit` and `dateForm` describe the conversation, not the schedule.** They are top-level members of plain data in and are repeated in every plain payload out. `mount` and the headless open take the same two members for a session opened from a document. They are never stored in the document.
9. **Unknown members.** A top-level member or an `options` member that is not in this record is refused. An unknown member of a task, link or calendar is left out and counted in `leftOut`, as today (`KNOWN_TASK`, `KNOWN_LINK` and `leftOut` in `plain.ts`).
10. **Names and shapes chosen in the tables.** `Decided here:` each of these is new in this record, picked so that a host with its own database sends its columns unchanged, and each is karpathy's to overrule:
    - `projectStart` as its own field, and `start` optional when it is given, because a host's activities can carry no start;
    - `snet: null` for "no start floor", so a snapshot can be read back;
    - `code` for the Activity ID, apart from `id` (decision 0016);
    - `finish` for the computed early finish, with `end` kept as an input-only member with its merged meaning. `end` is the one date that does not follow rule 6;
    - `milestone` with `"start"` and `"finish"`, in place of a second meaning for `type`, which a link already uses;
    - `float` kept from row tl2081rs for total float, beside `freeFloat`;
    - `wbsCode`, `physicalPercent`, `mustFinishBy`, `lagCalendar`, `buffer` and `bufferCalendarDays` as the camelCase of the names the code or the issues already use;
    - a calendar named by `key` from a task, the simple calendar form `workDays` with `hoursPerDay`, and an exception as `{ date, working }`;
    - `after`, in changes only, for a task's place under its parent.

### The schedule (top level)

| Field | Dir | Value | Document |
|---|---|---|---|
| `name` | in, out | text | `name` |
| `projectStart` | in, out | date, start kind. No unstarted task starts earlier. With it, a task's `start` is optional | No field at head. Maps to the `floor` of every task and to the `snet` of each task that has no later one (`read_task` in `items.rs`; `TaskMeta::floor` in `crates/app/src/meta.rs`). `origin` stays the Monday on or before the earliest date (`fromTasks` in `plain.ts`) |
| `dataDate` | in, out | date, start kind | `data_date` (`read` in `document/read.rs`) |
| `mustFinishBy` | in, out | date, finish kind | `must_finish_by`, minutes from `origin`, written only when a date is set (decision 0010, amendment; gantt#2074) |
| `lagCalendar` | in, out | `"successor"` (default), `"predecessor"`, `"24h"` or `{ "calendar": key }` | `lag_calendar` (`lag_calendar` in `items.rs`) |
| `durationUnit` | in, out | `"days"`, `"hours"`, `"minutes"` | Not stored (rule 8) |
| `dateForm` | in, out | `"minute"`, `"day"` | Not stored (rule 8) |
| `options` | in, out | object, decision 0018 | `options` |
| `calendars`, `tasks`, `links` | in, out | arrays of the records below | `calendars`, `tasks`, `links` |
| `projectFinish` | out | date, finish kind | Not stored (solved) |
| `buffer`, `bufferCalendarDays` | out | duration; whole calendar days (A devteam-1-1490 item 1) | Not stored: `Schedule::buffer` derives them from the solution, so no surface takes them in (gantt#2074) |

### A task

| Field | Dir | Value | Document |
|---|---|---|---|
| `id` | in, out | text or number: the task's handle inside one payload, which `parent`, `from` and `to` name. Not an identity (decision 0016) | Position in `tasks` |
| `key` | in, out | text: the host's own identity for the task; `null` out when it has none (decision 0016) | `key` (`read_task` in `items.rs`) |
| `task` | out | number: the session slot. Session-only (decision 0016) | None |
| `code` | in, out | text or `null`: the Activity ID column. Absent on input, it is `String(id)` as today (`fromTasks` in `plain.ts`) | `code` |
| `name` | in, out | text | `name` |
| `parent` | in, out | the `id` of the summary task above (the WBS parent), or absent; out in a change: its key, else `{ "task": n }` (the session slot), or `null` for a root | `parent`, `outline_level` |
| `after` | out, in a change only | the key of the sibling above, else `{ "task": n }` (the session slot), or `null` for the first (decision 0019) | Position in `tasks` |
| `wbsCode` | in, out | text or `null` | `wbs_code` |
| `calendar` | in, out | the `key` of a calendar; absent is the first; out in a change, a calendar with no key is `{ "calendar": n }` (the session slot) | `calendar` (position) |
| `start` | in, out | date, start kind. Out: the early start. In: the wanted start, held as `snet` unless the task has an `snet` member (A devteam-1-1495 item 3) | In: `snet`. Out: not stored (solved) |
| `finish` | out | date, finish kind: the early finish | Not stored |
| `end` | in | date. With `start` it gives the duration when `duration` is absent. A date-only `end` is 00:00 of that day, so the day itself is not worked (`fromTasks` in `plain.ts`). Never written | `duration` |
| `duration` | in, out | duration, 0 or more | `duration` (minutes) |
| `milestone` | in, out | `"start"`, `"finish"` or absent (gantt#2088) | `limit.milestone`; read only on a task with no duration (`Limit::milestone` in `crates/engine/src/model/limit.rs`) |
| `snet` | in, out | date, start limit (rule 6), or `null` for no start floor | `snet` |
| `snlt` | in, out | date, start limit (rule 6) | `limit.snlt` |
| `fnet`, `fnlt` | in, out | date, finish limit (rule 6) | `limit.fnet`, `limit.fnlt` |
| `mso` | in, out | date, start limit (rule 6). One of `mso` and `mfo`, not both (`read_limit`) | `limit.pin_start` |
| `mfo` | in, out | date, finish limit (rule 6) | `limit.pin_finish` |
| `alap` | in, out | boolean | `limit.alap` |
| `resume` | in, out | date, start kind | `limit.resume` |
| `actualStart` | in, out | date (rule 6) | `actual_start` |
| `actualFinish` | in, out | date (rule 6) | `actual_finish` |
| `remaining` | in, out | duration, 0 or more | `remaining` (minutes) |
| `progress` | in, out | number 0 to 100: percent complete, derived from `remaining` once that is set (decision 0008). A task that gives both is refused (A devteam-1-1490 item 3) | `progress` (`read_task` in `items.rs`) |
| `physicalPercent` | in, out | number 0 to 100 with up to two decimals, stored as given and never derived (gantt#2080) | None at head |
| `color`, `label` | in, out | `#rrggbb`; text | `color`, `label` |
| `resources` | in, out | list of a name, or `{ name, units }`: units a whole percent from 1 to 1000 (100 when a name stands alone), at most 64 entries, a name at most 100 characters (gantt#1875) | None: kept for display; an MS Project file gives units 100 |
| `fields` | out, in a create change only | the task as the change writes it: `parent`, `after`, `code`, `wbsCode`, `calendar`, `color`, `label`, `name`, `snet`, `duration`, `actualStart`, `actualFinish`, `remaining` and the eight limits (decision 0019) | None |
| `lateStart`, `lateFinish` | out | dates, start kind and finish kind. `lateStart` is read through the accessor of decision 0018 | Not stored |
| `float` | out | duration: total float, may be negative | Not stored |
| `freeFloat` | out | duration (gantt#2084) | Not stored |
| `critical`, `finished` | out | booleans | Not stored |

### A link

| Field | Dir | Value | Document |
|---|---|---|---|
| `key` | in, out | text or `null` out (decision 0016) | `key` (row tl2082lc; no field at head, `add_link` in `items.rs`) |
| `link` | out | number: the session slot. Session-only | None |
| `from`, `to` | in, out | the `id` of the predecessor and of the successor; out in a change: the task's `key`, else `{ "task": n }` (the session slot) | `pred`, `succ` (positions) |
| `type` | in, out | `"FS"` (default), `"SS"`, `"FF"`, `"SF"` | `kind` |
| `lag` | in, out | duration, may be negative | `lag` (minutes) |
| `lagCalendar` | in, out | as the schedule's, or absent; out in a change, a calendar with no key is `{ "calendar": n }` (the session slot) | `lag_calendar` |
| `fields` | out, in a create change only | the link as the change writes it: `from`, `to`, `type`, `lag`, and `lagCalendar` when the link has its own (decision 0019) | None |

A link's `id` is still tolerated on input and dropped (`KNOWN_LINK` in `plain.ts`).

A task `delete` in a commit comes with a link `delete` for every link in force to or from that task, listed before it, so a host that saves from the records drops the links too. A link between two tasks deleted in one commit is listed once. The undo of that delete lists the matching link `create`s, each once, after the task `create`s, and its redo the deletes again.

### A calendar

| Field | Dir | Value | Document |
|---|---|---|---|
| `key` | in, out | text (decision 0016) | `key` (row tl2082lc) |
| `workDays` | in, out | ISO weekday numbers, 1 for Monday to 7 | `week` |
| `hoursPerDay` | in, out | number | `week`: `Decided here:` one period per work day from 08:00, `[480, 480 + 60 × hoursPerDay]` |
| `week` | in, out | seven lists of `[from, to]` minutes of the day, in place of the two members above | `week` (`build_calendar` in `items.rs`) |
| `exceptions` | in, out | list of `{ "date": "YYYY-MM-DD", "working": boolean }` | `[day, periods]`: no periods, or the default day's (`build_calendar` in `items.rs`) |

With no `calendars`, the one calendar of today stays: Monday to Friday, 08:00 to 12:00 and 13:00 to 17:00 (`WORKING_WEEK` in `packages/gantt/src/adapters/dhtmlx.ts`).

## Consequences

Renames and changes this table forces on merged code and written rows:

- **Edit log, PR #2104 (merged, `crates/app/src/edit_log.rs`).** Field `start` becomes `snet`, `actual_start` becomes `actualStart` and `actual_finish` becomes `actualFinish`. A date changes from `YYYY-MM-DD HH:MM` to `YYYY-MM-DDTHH:mm`, a duration from minutes as text to a number in `durationUnit`, and an empty string to `null`. The row that moves the log to the commit point (tl2077lb and tl2077lc as one row) makes these changes.
- **`move` event (merged, `GanttEvents` in `packages/gantt/src/events.ts`).** `start` is minutes from the origin Monday today; it becomes a date, and the event gains `key` (decision 0016). `select` gains `key`.
- **tl2081rs (`results()`, written).** `id`, which the row defines as the task's code, becomes `code`, and the session slot is `task`, the name the app's own JSON already uses for it (`health_task` in `crates/app/src/health.rs`). Floats are in `durationUnit`, not working minutes.
- **tl2103so and tl2103pk (written).** The top-level `started_late_start` and `unstarted_remaining` become members of `options` (decision 0018).
- **tl2077ev (written).** The flat `changes` list becomes `commits` (decision 0019), each change in this vocabulary.
- **tl2081lm, PR #2109 (open).** No rename. It gains `snet: null` for "no start floor", so that a snapshot read back does not turn a solved start into a limit.
- **tl2081ac (written).** No rename: `actualStart`, `actualFinish`, `remaining`, `resume` and `dataDate` stand. `remaining` is in `durationUnit`, not always working days.
- **tl2077sn (running).** The snapshot writes `id`, `key`, `code` and `task` as decision 0016 says, `finish` beside `start`, and `snet` (a date or `null`) on every task.
- **Plain data (merged).** `end` and the default unit of days stand. `start` stops being required when `projectStart` is given (the `TASK` table in `packages/gantt/src/adapters/validate.ts`). `type: "milestone"` (`fromTasks` in `plain.ts`) is still read and keeps its meaning; `milestone` says whether that task sits at a period's start or its end (gantt#2088). The fixed `WORKING_DAY = 480` (`adapters/dhtmlx.ts`) goes with the calendar-and-unit row (A devteam-1-1496 item 5).

Not renamed: the page's own reads (`TaskDetails` in `packages/gantt/src/api.ts`) are chrome API, not host records.

A host that models a mandatory date as a floor plus a ceiling (Primavera P6's "Start On" and "Finish On") sends the typed `constraint` with the type `so` or `fo` (decision 0022, rule 9), or the separate members `snet` with `snlt`, or `fnet` with `fnlt`, on one date. It does not send `mso` or `mfo`, which are pins (P6's "Mandatory Start" and "Mandatory Finish").

## Rows this governs

gantt#2081 (tl2081lm, tl2081ac, tl2081hd, tl2081rs and the calendar-and-unit row), gantt#2077 (tl2077sn, tl2077ev, and tl2077lb with tl2077lc), gantt#2082 (tl2082lc), gantt#2087 (`update(records)`), gantt#2085 (the before-edit payload), gantt#2074, gantt#2080, gantt#2084, gantt#2086, gantt#2088, gantt#2103 and gantt#2105.
