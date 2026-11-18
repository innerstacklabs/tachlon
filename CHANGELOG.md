# Changelog

What changed in each npm release, newest first. Each entry says what you must change in your own code, then what is new. The decision records in `docs/decisions/` hold the reasons.

## Unreleased

### New

- `mount({ beforeEdit })` (a prop or input of the same name in React, Vue and Angular, which ask the latest function without mounting the chart again) gives the chart a function it asks before each change a person makes. The function sees the action and the changes it would make with their old and new values (the README's "Control editing" says which actions carry values), and answers a string to refuse the change with that reason, which the chart shows. A refusal changes nothing and carries the new `EditRefusal` reason `"refused"`; a throw or a promise refuses the change, fires `error` with the source `beforeEdit` and carries the new reason `"beforeEditFailed"`. A `switch` over the reasons with no `default` needs both new cases.
- The by-key update (`App.update_tasks` and `Session::update_tasks`, no JavaScript call yet) also takes `name` and `progress` in a record. A name is checked as a typed rename is, and a progress (a whole percent from 0 to 100) as a typed percent is, and is refused if it sets an actual after the data date. A progress is taken against the dates the record's `duration`, `start` and `snet` leave, so the chart works the plan out twice when a call mixes them, and once otherwise.
- The app can put the chart back to before a change the host failed to save (`App.rollback_to` and `Session::rollback_to`, with `App.acknowledge` for a saved change; no JavaScript call yet). It takes away the changes made after it too, as one commit with the source `rollback`, and refuses with `rollbackGone` when it can no longer go back that far, so the host loads its own copy.

## 0.3.0

Read "Breaking changes" first if you use 0.2.0. A page that mounts the chart with `schedule` and listens only to `select`, `edit` and `open` needs none of them.

### Breaking changes

**Events**

- **The `move` event's `start` is a date text, not minutes.** It was minutes from the origin Monday; it is now `YYYY-MM-DDTHH:mm`, and the event also carries `key`. This reaches `Gantt.on("move")`, React `onMove`, Vue `@move` and Angular `(chartMove)`. To update, delete the code that turned minutes into a date and read the text. Decision 0017.

**Plain data** (`fromTasks`, `mount({ tasks, links })`, `openHeadless`)

- **Names that 0.2.0 ignored are now read.** A task member `key`, `calendar`, `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `alap`, `actualStart`, `actualFinish`, `remaining` or `resume`, a link member `key`, and the top-level `dataDate`, `mustFinishBy`, `options`, `calendars`, `durationUnit` and `lagCalendar` now mean something and are checked. If your objects carry a field of your own under one of these names, rename it before you pass the data in. Decisions 0016 to 0018.
- **`key` must be text.** A number is refused: send `String(id)`. `snet` is read in place of `start` when you give it. `mso` and `mfo` do not go together. `actualFinish` and `resume` need an `actualStart`, as does `remaining` unless `options.unstartedRemaining` is `"on"`, and `remaining` is not given with `progress`. An unknown member or value inside `options` is refused. `validate` lists each one. In TypeScript, `PlainTask` and `PlainLink` now type these members, so a field of yours with another type fails to compile.

**Document format** (`toDocument()`, `.gantt` files)

- **A document from 0.2.0 opens unchanged. A document that 0.3.0 saves with a host key (task, link or calendar), with `options` or with a `must_finish_by` date is refused by 0.2.0.** The version stays 1, and a schedule with no keys, default options and no date saves the same text as before. To update, move every reader to 0.3.0 before you save keys. Decisions 0010, 0016 and 0018.
- **`ScheduleDocument` types are wider.** `data_date`, and a task's `actual_start`, `actual_finish` and `remaining`, are `number | null` where they were `null`. Add a check where your code assumed `null`. The document may also hold a task `key`, a limit `resume` and a top-level `options` and `must_finish_by`.
- **A value outside the engine's range no longer opens.** A duration or remaining time over about 100 years, a lag beyond the same, or a date more than 1,000 years from the origin is refused by the document reader, and by an edit. Nothing a person types in good faith reaches it. Decision 0020.

**Types you build or switch over**

- **`EditRefusal` has six more reasons:** `readOnly`, `emptyKey`, `hasKey`, `keyTaken`, `unknownTask` and `unknownLink`. A `switch` over the reasons with no `default` needs the new cases.
- **`ProposeResult` has four more reasons:** `finishBeforeStart`, `actualAfterDataDate`, `duration` and `badPercent`. `ProposedChange` and `ProposedKey` have a `progress` kind. A `switch` over the reasons or the kinds with no `default` needs the new cases.
- **Interfaces gained members.** `ViewState.groupBy`, `Commands.setGroupBy`, `Commands.assignKey`, `Gantt.snapshot`, `Gantt.setPending`, `Gantt.setReadOnly`, `Gantt.readOnly`, `HeadlessSchedule.leftOut` and `HeadlessSchedule.results` are required, and so are the event payload members: `key` on `select` and `move`, and `commits`, `seq` and `snapshot` on `edit`. Code that only calls or listens is unaffected; a test double or a copy of the type needs them.

**Behavior with no type change**

- **Negative float is no longer cut off at the project start.** A late start can now fall before the project start, so a task that cannot meet a late limit reports its true float: a two-day task that must finish by the first Monday at 17:00 reads -1 working day, where it read 0, in the chart's Float column, in `explain` and in `results`. The count of late tasks in the Checks grows with it. Change any threshold that read float.
- **A length beyond the range is refused where it was answered.** `whatIfLonger` for a task that would run longer than about 100 years refuses with reason `other`. An imported P6 file with a lag or duration over about 100 years used to fail to open, and an MS Project file with one was held to 100 years of 365 days. Both open with a counted note now: a lag past the range reads 0 and a duration reads the largest length.
- **A command-line flag that takes no value refuses one.** `--skill=false` and `--dry-run=false` used to read as on; the flags `--dry-run`, `--json`, `--no-install`, `--plain`, `--skill`, `--help` and `--version` now exit 2 with `<flag> takes no value`. To turn a flag off, leave it out.
- **`mount` with no `schedule`, `document` or `tasks` opens an empty chart.** It rejected before. Code that used the rejection to detect "no data" checks the task count instead.
- **A typed actual start or finish after the data date is refused** in the cell and the task dialog. A file whose actual dates are after the data date opens with one counted note.
- **Exported files change.** For a schedule that did not come from an XER file, the XER export writes the project start as the earlier of the earliest task start and the data date, so the file reads back with its dates. It also keeps both date rules of a task that has two when at most one of them is a pin, a start-no-later-than, a finish-no-later-than or as-late-as-possible, where it dropped one; `leftOut` still names a task that loses a rule. Both exports add a `leftOut` line when a task has a host key or the schedule has a must-finish-by date, which the formats cannot carry.

### New

**Mounting and options**

- `mount({ document })` starts the chart on a saved document in one call, instead of an empty start and `load`. React, Vue and Angular take a `document` prop or input. A `schedule` wins over a `document`, and a `document` over `tasks`.
- `mount({ dataDate })` and `mount({ options })` (with the same props and inputs) set the day the data is as of and the schedule options. `options.startedLateStart` is `"p6"` (default) or `"project"`: the late start a started task shows is the late start of its remaining work, or its actual start. `options.unstartedRemaining` is `"off"` (default) or `"on"`: a task that has not started and has a remaining duration is scheduled with that length. The defaults give the 0.2.0 answers.
- `mount({ mustFinishBy })` (a prop or input of the same name in React, Vue and Angular) gives plain `tasks` the date the project must finish by, as `YYYY-MM-DD[THH:mm]`; a date alone is the end of that day. It is read with `tasks`, as `dataDate` is.
- `Gantt.load({ document, unsaved: true })` shows the loaded schedule as unsaved, for a copy of work no save holds, so opening another file asks first. A chart that switches to WebGL2 and loses edits keeps its unsaved mark.
- `mount({ assistant: { send, into } })` writes a chat panel into your element. Each message calls your `send` with the conversation, so the page sends nothing anywhere else, and a change the model proposes waits for the person to accept or reject it.
- `mount({ readOnly })`, `Gantt.setReadOnly(on)` and `Gantt.readOnly()` switch the chart read-only for the person using it, at any time and without a restart. The cell editor and the task dialog do not open, every other edit is refused with `"readOnly"`, and a click still selects a row. The React and Vue `readOnly` prop and the Angular `readOnly` input do the same and never mount the chart again.
- `Gantt.setPending(keys)` dims the rows that wait for your own recalculation and tells a screen reader they are busy.
- `Commands.setGroupBy(codeType)` groups the rows under one header per activity code value; `null` ungroups. `ViewState.groupBy` reports it. A group opens and closes with a click or `Alt+Left` and `Alt+Right`.

**Editing and events**

- A task keeps the `key` your host gave it through edits, saves and loads. Links and calendars carry one too. `select` and `move` events carry `key` (`null` when there is none).
- `Commands.assignKey({ task } | { link }, key)` gives a key to a task or link made in the chart after your host saved it. It is not an edit: no undo step, no revision.
- `Commands.select(task)` and `Commands.scrollTo({ task })` take a task's key as text as well as the chart's own task number, so a page that lists tasks by its own id selects one and brings it into view without row numbers. A key no live task holds, a deleted task's included, answers `false` and changes nothing. `ScrollTarget` is `{ task: number | string } | { date: string }`.
- `Commands.propose({ kind: "progress", task, actualStart | actualFinish | remaining | progress })` holds a task's reported progress as a pending change, so a host field app can post progress for a scheduler to approve. The chart draws it as a dashed bar, changes nothing until a person accepts it, and Accept applies it as one undo step. `acceptProposed` and `rejectProposed` take `kind: "progress"`, and the `propose` tool of `mount({ assistant })` takes it too.
- The `edit` event carries `commits`, one per undo step, each with its `source` (`typed`, `drag`, `dialog`, `paste`, `proposal`, `undo`, `redo`) and its changes with old and new values; `seq` counts them, and `snapshot: true` says commits were lost, so save `snapshot()` instead. Moving a task in the outline (indent, outdent or a move) records `parent` and `after` for the task and for each sibling that changes place with it. Types `Change`, `Commit` and `CommitSource`. Decision 0019.
- The Rust host API's before-edit function (`Session::set_before_edit`, no JavaScript call yet) is shown the old and new values for a drag, the task dialog, a paste, a link drag, and an undo or redo of a task-field step, as it is for a plain edit and a group; it can refuse by value, each dialog field as planned against the chart. A paste carries the name of each task it creates and a link drag the finish-to-start link it makes with no lag, and no other field of either. A delete, restore, outline move, link edit or data-date edit carries the records its commit writes; an insert carries only the place and name the edit names, and a baseline edit the `baseline` field, although its commit writes no change. A plan of two or more of these (a group, a multi-step undo or redo) carries its task-field values and no structural ones. An undo or redo of a data-date edit and `discardWhatIf` carry no values, only their kind and every task they touch, both ends of a link included. An outline move names the moved task only, not the siblings it adopts or releases.
- A program that keeps its own copy of the schedule can send new durations and start-no-earlier-than dates for many tasks at once, each task named by its key (`App.update_tasks` and `Session::update_tasks`, no JavaScript call yet; a duration is a whole number of working minutes, and `start` is read as `snet`). A record takes `key`, `duration`, `start` and `snet` and no other member, a summary task is refused, and one refused record refuses the whole call and changes nothing. The chart works the plan out again once and keeps what the person had selected; their undo list is left as it was, and no `edit` event fires.
- A started task takes a `resume` date, the day its remaining work picks up again. The MS Project reader keeps it.
- A typed list of predecessors (`3, 5SS+1d`: Activity IDs with an optional link type and lag) makes, changes and removes the links into a task as one undo step, and its `edit` event commit lists each link change. The wasm `App` takes it (`set_predecessors_text`, read back by `predecessors_text`); the chart's cells and `Commands` do not call it yet.

**Plain data and the headless solve**

- `Gantt.snapshot()` and `toPlain` return the whole schedule as plain `tasks` and `links`, with `dataDate`, `options` and `leftOut` (one line for each thing plain data cannot carry). `fromTasks` reads the result back.
- A must-finish-by date goes in plain data as `mustFinishBy` (`fromTasks`, `openHeadless`, `validate`, `schema.json`; `snapshot()` and `toPlain` return it) and in a document as `must_finish_by`, in minutes from `origin`, written only when set; a value out of range, or given twice, is refused. Float is measured against the earlier of the date and the computed finish: a date the plan cannot meet gives negative float, in the chart and in `results`, and a date it beats changes nothing. Decisions 0010 and 0017.
- Plain data carries a task's limits (`snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `alap`), its actuals (`actualStart`, `actualFinish`, `remaining`, `resume`), `key`, and the schedule's `dataDate` and `options`. `schema.json` lists them.
- Plain data carries working calendars: `calendars` (each a `key` with `workDays` and `hoursPerDay`, or a `week`, and `exceptions` of holidays or extra working days), a task's `calendar` by key (the first calendar is every other task's), `durationUnit` (`"days"`, the default, `"hours"` or `"minutes"`, for every duration, remaining time and lag) and `lagCalendar` (`"successor"`, the default, `"predecessor"`, `"24h"` or `{ calendar }`). `mount`, `openHeadless` and the React, Vue and Angular components take them beside `tasks`; with none of them the schedule is planned as in 0.2.0, on one Monday to Friday calendar. An exception before the schedule's first Monday is left out and named in `leftOut`. `snapshot()` and `toPlain` do not return calendars: `leftOut` says so for a schedule that has one.
- `openHeadless` takes plain `{ tasks, links, dataDate, options }` or `{ document }` as well as file bytes, and throws a `PlainDataError` that lists every problem. The schedule it returns has `leftOut` and `results()`: each task's early and late start and finish, total and free float, `critical` and `finished`, with its `key`.

**Import and export**

- A P6 or MS Project file with a value out of range opens with one counted note per kind, naming the first five tasks.
- A P6 file saved from a schedule your host built keeps the project start, and a task with two date limits keeps both.

**The task list**

- A schedule whose tasks name resources shows a Resources column at the right end of the list: each task's names joined by commas, cut with an ellipsis when they do not fit, and the whole list in the tooltip; a summary row and every row mid-drag leave it empty. A schedule with no resource names looks as it did. The list only: print, copy and export do not carry it. `--gantt-res-col` sets its width.
