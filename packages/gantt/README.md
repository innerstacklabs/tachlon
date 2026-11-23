# tachlon

Blazingly fast, powerful Gantt and scheduling for the web. Built for developers who work with an AI assistant.

Using an AI assistant? Paste this:

```text
Add a Tachlon Gantt chart to this app. Install the npm package tachlon and follow the AGENTS.md inside it (more at tachlon.com/llms.txt). Put the chart on one existing page and change nothing else. Run the build and npx tachlon doctor until both pass. Commit on a new branch; do not push.
```

## Install

```sh
npm install tachlon
```

Tachlon is free and open source under the MIT license: use it on any page, in any product, with no key and no trial. The engine is a WebAssembly file and the chart draws on the GPU (WebGPU, or WebGL2 where WebGPU is missing). It needs no Rust toolchain.

The package holds the chart's script, its stylesheet, TypeScript declarations, the engine file `app_bg.wasm` beside the script, and the task data's schema. It runs in the browser: use it from a bundler such as Vite or Next.js, or serve the files as they are.

## Quick start

Give `mount` an element and the tasks to draw. The element is a `div` with the class `gantt` and a box with a height:

```html
<div id="chart" class="gantt" style="width: 800px; height: 300px"></div>
```

```js
import { mount } from "tachlon";
import "tachlon/gantt.css";

const tasks = [
  { id: 1, name: "Design", start: "2026-11-02", duration: 3 },
  { id: 2, name: "Build", start: "2026-11-05", duration: 5 },
  { id: 3, name: "Ship", start: "2026-11-12", duration: 1 },
];
const links = [{ from: 1, to: 2 }, { from: 2, to: 3 }];

const chart = await mount(document.getElementById("chart"), { tasks, links });
chart.on("select", ({ task }) => console.log("picked task", task));
```

In TypeScript, `mount` takes an `HTMLDivElement`: write `document.getElementById("chart") as HTMLDivElement`.

`mount` returns a promise of the chart. `start` is `YYYY-MM-DD`, and a task that other tasks name as their `parent` (a folder) needs none, since it takes its dates from them. `parent` is the `id` of the task above, and `null` is no parent, as leaving it out is, so rows from JSON or a database go in as they are. `duration` counts working days (or the `durationUnit` below). A link is finish-to-start unless it gives a `type` of `"SS"`, `"FF"` or `"SF"`. A task takes an optional `color`, `"#rrggbb"` with no alpha (`{ id: 1, name: "Design", start: "2026-11-02", duration: 3, color: "#2e7d32" }`), and its bar is drawn in that color, except that a critical bar keeps the critical color; a `color` in any other form is not read, and `fromTasks` names it in `leftOut`. The name on a colored bar is black or white and reads at 4.5:1 on the bar and on its progress for the tested palette only (gold, navy, teal, amber, violet and gray, in the light and the dark theme), not for every color. A task also takes an optional `label`, the text its bar shows in place of its name (`{ id: 1, name: "Design", start: "2026-11-02", label: "Design v2" }`); the task list keeps the name, and a `label` that is not text is not read and `fromTasks` names it in `leftOut`. `fromDhtmlx` reads a task's `color` the same way, and has no `label`. A task also takes an optional `key`, your own id as text (`{ id: 1, name: "Design", start: "2026-11-02", key: "6f1c2d5e-0b7a-4c1e-9a53-2d8f7e4b1a90" }`); it stays with the task through edits and saves, and a `key` that is not text, such as a number, is refused by `validate` (send a numeric id as a string). A link takes the same optional `key` (`{ from: 1, to: 2, key: "e2a9c7f1-5d38-4b06-9c4e-8f1a6d3b2e75" }`), kept the same way. A key is not empty and is unique among the tasks, and among the links; `validate` names a repeat. A link that repeats a pair of tasks is dropped with its key. `fromDhtmlx` has no `key`. A task also takes limits as plain data: `snet` (start no earlier than, read in place of `start`), `snlt` (start no later than), `fnet` and `fnlt` (finish no earlier and no later than), `mso` and `mfo` (a day it must start or finish on; give one of the two) and `alap: true` (as late as possible), each date written like `start` (`{ id: 1, name: "Pour", start: "2026-11-02", duration: 2, fnlt: "2026-11-06" }`). A date alone on a finish limit (`fnet`, `fnlt`, `mfo`) means the end of the last working minute of that day on the task's calendar, and on a start limit (`snet`, `snlt`, `mso`) the first working minute; it never means midnight. A task with no duration also takes `milestone: "start"` or `"finish"`: the chart puts it at the start of a working period or at the end of one (for example, after a task that ends at 17:00, `"start"` is 08:00 the next working day and `"finish"` is 17:00 that day), and `type: "milestone"` alone keeps its meaning; `validate` refuses any other value. The chart schedules by the limits and saves them; `validate` refuses a date that is not one, an `alap` that is not true or false, and a task that gives both `mso` and `mfo`; a limit on a task with children is not read. A task also takes what really happened: `actualStart` and `actualFinish` (dates; a date alone is the first working minute of that day, and the end of its last), `remaining` (working days of work left, as `duration` counts) and `resume` (the date paused work picks up again), and the data as a whole takes `dataDate`, the day the update is as of (`{ tasks: [{ id: 1, name: "Pour", start: "2026-11-02", duration: 4, actualStart: "2026-11-02", remaining: 2 }], dataDate: "2026-11-04" }`; `mount` takes `dataDate` beside `tasks`). A host that passes plain data (`mount`, `openHeadless`, `fromTasks`) can write `projectStart` beside `tasks` and `dataDate`: a date, read as 00:00 of that day; no task starts earlier, and a task's `start` is optional when it is given. `validate` refuses a `projectStart` that is not a date. Without it nothing changes. The chart plans the rest of the work from those facts, as it does for a P6 file. `validate` refuses `actualFinish`, `remaining` or `resume` on a task with no `actualStart`, an `actualFinish` before the `actualStart`, and a task that gives both `progress` and `remaining`; the `progress` of a task with `remaining` is left to the chart, which derives it. A progress key on a task with children is not read. The data also takes `options`, the schedule options, each left out for its default: `startedLateStart` (`"p6"` shows the late start of a started task's remaining work, as Primavera P6 does; `"project"` shows its actual start, as Microsoft Project does) and `unstartedRemaining` (`"on"` schedules a task that has not started with its `remaining` as its length; `"off"` ignores `remaining` until the task starts) (`{ tasks: [{ id: 1, name: "Pour", start: "2026-11-02", duration: 5, actualStart: "2026-11-02", remaining: 3 }], options: { startedLateStart: "project" } }`; `Headless`, `fromTasks` and `mount` (as `options`, with `tasks`) read it). `validate` refuses a member that is not one of the two and a value that is not one of its own, and a chart loaded from that document saves them. A program that hands the chart its own list of tasks can now also hand it its working calendars: which weekdays are worked, how many hours a day, and the holidays or extra working days. It can also say whether its durations are in days, hours or minutes, and the chart plans with exactly those. `calendars` is a list of `{ key, workDays, hoursPerDay, exceptions }` (`{ key: "site", workDays: [1, 2, 3, 4], hoursPerDay: 10, exceptions: [{ date: "2026-12-25", working: false }] }`), or of `{ key, week, exceptions }` where `week` is seven lists of `[from, to]` minutes of the day, Monday first; a task's `calendar` names one by `key`, and a task with none takes the first calendar (with no `calendars`, or an empty list, there is one Monday to Friday calendar, 08:00 to 12:00 and 13:00 to 17:00). `durationUnit` is `"days"` (the default), `"hours"` or `"minutes"` and says what every `duration`, `remaining` and `lag` counts: a day is the working day of the calendar that measures the value (`hoursPerDay`, or the average worked day of a `week`), an hour is 60 working minutes, and a part of a minute rounds to the nearest. A lag day is counted on the successor's calendar unless `lagCalendar` is `"predecessor"`, `"24h"` (a full 24 hours) or `{ calendar: key }`. An exception before the schedule's first Monday is left out and named in `leftOut`. `validate` refuses a bad unit, a calendar with no key or a repeated key, a calendar with both forms or no worked day, `workDays` outside 1 to 7, `hoursPerDay` outside 0 to 16, a day that two exceptions name, and a `calendar` or `lagCalendar` that names no calendar. `mount` and `openHeadless` take `calendars`, `durationUnit` and `lagCalendar` beside `tasks`, as do the React, Vue and Angular components.

If your server recalculates the schedule, hand the answer back with `chart.update(records)` and the chart changes in place, with no new `mount`:

```js
const result = chart.update([{ key: "6f1c2d5e", duration: 5 }, { key: "e2a9c7f1", start: "2026-11-09", progress: 40 }]);
if (!result.ok) console.warn(result.reason, result.record, result.key);
```

A record names its task by `key` and may carry `name`, `duration`, `start`, `snet`, `snlt` (start no later than), `fnet` and `fnlt` (finish no earlier and no later than) and `progress` (a whole percent); a member you leave out keeps its value. `start`, `snet`, `snlt`, `fnet` and `fnlt` each take a date, or `null` for none, and a date alone on `fnet` or `fnlt` is the end of the last working minute of that day on the task's calendar, and on `snet` or `snlt` its first working minute. `duration` counts the `durationUnit` you mounted with, and `start` is kept as the task's start-no-earlier-than unless the record has an `snet` or a `constraint`. `constraint` is `{ type, date }`, with a `type` of `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `so` or `fo`, or `null` for none: it sets the one date limit and clears the rest of `snet`, `snlt`, `fnet` and `fnlt` (`so`, start on, sets `snet` and `snlt` to its date, and `fo`, finish on, sets `fnet` and `fnlt`: neither moves the bar, and float can go negative; `mso` and `mfo` are `notUpdatable` unless they are the pin the task holds, as below). A record that gives `constraint` and one of those six members is refused as `badRecords`. A record may also carry `hostStart` and `hostFinish`, the early start and finish your own code computed (`YYYY-MM-DD`, or `YYYY-MM-DDTHH:mm`; a finish that is a date alone is the end of that day): they set nothing, and are compared with the dates the chart computes once every record is applied. The selection, the scroll position and the undo list stay as they were, and the call is not an edit: no `edit` event fires, no undo step is added and `beforeEdit` is not asked. It answers `{ ok: true, updated }`, with a `differs` list beside them when a host date is not the chart's (each entry names the `record`, `key` and `member`, `"start"` or `"finish"`, with the `host` date and the `chart` date written the same way; a date that matches is not named, and with none to name there is no `differs`), or a refusal that changed nothing, with the `reason`, a `message`, and the `record` (from 0), `member` and `key` when they are known: `badRecords` (not an array), `busy` (a cell is being edited, a bar or link is being dragged, or the chart is switching to WebGL2), `unknownKey`, `duplicateKey`, `notUpdatable` (a limit it does not set: as-late-as-possible, and a must-start-on or must-finish-on that is not the pin the task holds; a summary task), `badValue`, `badPercent`, `emptyName`, `invalidName` or `actualAfterDataDate`. Hold your own copy of a task's `key`: a task number (`task`) is valid only until the next load, open or remount.

`dateForm` governs the dates you send as well as the ones you receive. Mounted with `dateForm: "day"`, `update`, `hostEdit` and `setData` take every date as `YYYY-MM-DD`, the way the chart wrote it to you, so you can write back the dates you were given with no conversion: a date alone is read on the task's calendar, as above. A date with a time of day (`2026-11-09T08:00`) is refused with `reason: "badValue"`, the `record`, the `member` and a message that names `dateForm`.

After an `update`, each cell in view whose shown value changed is highlighted for 1.5 seconds: the values you sent and the ones the new plan moved, such as the dates of a successor. With `prefers-reduced-motion: reduce` the highlight does not fade, and goes when the 1.5 seconds end. A row you marked with `setPending` stops being dimmed when an `update` writes it; a refused `update` marks nothing and changes nothing.

If your page also edits tasks itself (a panel of its own, a server push) and you want one undo list, give those edits to `chart.hostEdit(records)`: the records and the answer are `update`'s, but the call is one undo step on the person's list, so one Undo takes back your values and the next Undo takes back the person's edit before them. An `edit` event fires with the commit source `"host"`, which your `save` function never gets, because you made the change. It is taken in a read-only chart, `beforeEdit` is not asked, and a record that sets `progress` or changes a task with actual dates is refused as `"hasActuals"`, and one that changes a group of fields your capability profile turned off, or whose old value it does not store, as `"notAllowed"` (`member` says which; the `message` names the task: call `update` for those, which makes no undo step).

For an action your page takes outside the chart, such as assigning a worker in its own panel, `chart.pushHostEntry("assign-7")` puts a marker for it on the same undo list. When a person's Undo reaches the marker, the schedule does not change and an `edit` event fires with a commit whose `source` is `"undo"` and whose `host` is `"assign-7"`: undo your own action then (a Redo gives `"redo"`). `chart.clearHistory()` empties the undo and redo lists, for example when your page releases its edit lock; a failed `save` then cannot be rolled back across it, and the chart asks for a `snapshot`. When your own edit made a step on the list impossible to replay, Undo and Redo drop that step, go on to the next one and say so once in the alert line.

If your rows use other names, mount with `fields`, `{ tasks: { name: "title", start: "plannedStart", duration: "days", parent: "parentId", key: "id" }, links: { from: "source", to: "target" } }`: each entry is a chart field and your name for it, and a field with no entry keeps its name. You pass your rows as they are, to `mount`, `update` and `setData`, with no cast: with a map their type is `readonly object[]` (`mount<F>` takes the type `F` of your `fields`, and `Gantt<F>`, `MountOptions<F>` and the three components follow it; a name that is not a chart field does not compile), and with none it is `PlainTask[]` and `TaskUpdate[]` as before. When both `key` and `id` have an entry, the member you name for `id` is the row's id and the one for `key` fills it only for a row that has none. The `set` and `was` of every change the chart tells you of (an `edit` event, a `save` change set, a before-edit call) use your names, so you write no rename function. `key: "id"` makes your `id` the row's key. The map renames members and converts no value (`durationUnit` and `dateForm` do that), is read once, when the chart starts, and does not apply to a change's own members (`record`, `op`, `key`, `cascade`), a `schedule` change or `snapshot()`. `mount` rejects an entry for a field the chart does not have, one name given to two fields, and a name that is another chart field's own name with no entry of its own (`key: "id"` is the exception); a `fields` name that no row carries logs one console warning.

A member of a row that the chart has no field for (`status`, `owner`; with a map, also your own `name` when `name` is mapped to `title`) is a custom field. The chart keeps it with the row, by the row's text `key`, and never reads, shows or edits it. It comes back unchanged under the same name in a create's `set` and a delete's `was`, so the create that undoes a delete gives your API its columns again. `update` and `setData` set one by value, a row the person makes has none, and `chart.state().customFields` (`{ tasks, links }`, each a list of names) lists those seen. The chart no longer counts these members in `leftOut`, so a misspelled member (`durration`) is kept, not reported. A row with no `key` has nowhere to keep them: its unknown members are left out and counted as before. Opening another file forgets them, and the saved document does not hold them.

If your page holds the rows, hand them back with `chart.setData({ tasks, links, dataDate })`, the same lists `mount` took, in place of calling `update` yourself. The chart compares each task with the rows it was last given, by `key`, and sets only the `name`, `start`, `duration`, `progress` and date limits (`constraint`, or `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`) that differ, as `update` does (it refuses an `mso` or `mfo` that is not the pin the chart holds, so the call is refused; a host that stores a date alone rereads a mid-day pin as the day's first (`mso`) or last (`mfo`) working minute, so it differs and is refused): the selection, the scroll position and the undo list stay, the call is not an edit, and the same arrays again do nothing. Every other member of a task (`color`, `label`, `end`, `calendar`, `milestone`, `alap`, the actuals, your own members) is ignored, not set and not compared, and one record `update` refuses (a summary's dates, for example) refuses the whole call. A member you leave out keeps its last value, and a value equal to the last one is no change, so rows that are a render old do not undo an edit a person saved. It answers as `update` answers, all or nothing, or `{ ok: false, reason: "rowsChanged", message }` with nothing changed when a task or a link was added, removed or changed, a task moved in the list or has another `id` or `parent`, a task has no text `key`, or the chart holds no rows of yours (it was mounted with a `document`, or has since switched to WebGL2, or opened, loaded or cleared a schedule): mount the chart again with the rows then. A `dataDate` (a day written `YYYY-MM-DD`, as `setDataDate` reads it) beside them is set in place when it differs from the one the chart was last given, as one undo step and with no start again (the scroll position and the lists stay): the chart calls `setDataDate`, and the same day does nothing. A day that is not a real `YYYY-MM-DD` refuses the whole call with `reason: "badDate"` and changes nothing, and so does a `setDataDate` refusal (`busy`, `readOnly`); the day is set before the rows, so rows that `update` then refuses leave the new day set. The three components set a new `dataDate` prop or input with this call and mount the chart again only when it is refused or taken away.

The task list shows a Predecessors column for a schedule whose tasks have `id`s. Click a cell, type a list such as `3, 5SS+1d` (an `id`, then an optional link type and lag) and press Enter: each item becomes a link into that task, as one undo step. A list with a mistake shows the reason under the cell and changes nothing; a summary has no editor.

`forceWebgl: true` draws with WebGL2 and never tries WebGPU. If WebGPU fails, the chart moves to WebGL2 by itself and fires an `error` event.

`locale` is the language and region the chart runs in, a BCP 47 tag such as `"de"` or `"pt-BR"`. The chart writes it as the root's `lang` and shows its numbers in that region's style (`1.234,5` in German); without it the numbers read as `en-US` and the root keeps the page's own `lang`. A tag that is not a language tag rejects `mount` with a `RangeError`.

`texts` is the chart's own words in your language: an object from a key to a text, for the buttons, labels and groups of the toolbar, the task list and the task dialog, and the names of its columns and fields (the head cells it adds, the hover card's rows) (`{ fit: "Anpassen", "fit-title": "Das ganze Projekt zeigen (f)" }`). `packages/gantt/src/texts-en.ts` lists every key with its English; a key you leave out stays English, and a key the chart does not know is ignored. The sentences the chart writes (a file that cannot be opened, the saved note, how many rows are selected, the baseline's confirmation, a render error) are in the same file as templates with `{name}` holes (`"rows-selected": "{count} Zeilen markiert"`); a sentence about a count has a key per plural form of your `locale` after a dot (`bytes.one`, `bytes.few`, `bytes.many`, `bytes.other`), and the chart picks the one `Intl.PluralRules` gives for the count. With `locale`, the keys of its refusal sentences (`App.default_texts` in `docs/WASM_API.md`, such as `alreadyLinked`, and `notAllowedBarMove` for a gesture your `capabilities` turned off) word what the chart refuses an edit with. `mount` and the React, Vue and Angular components take it.

`fonts` adds TrueType or OpenType font-file bytes to bundled Liberation Sans, uses `locale` for shaping, and reshapes labels so the chart can draw scripts the bundled face lacks.

Hear that fallback with `chart.on("error", ({ source, message, fatal }) => console.warn(source, message))`; `source` is `"backend"` for it, and `fatal` is `true` when the chart cannot go on.

`mount` rejects with a `MountError` if the first schedule cannot be read or the first frame cannot start. Its `message` says why, and its `fix` says what to do when the cause is known.

## Start errors

A `MountError`'s `code`, when it has one, is one of these, and its `fix` is the sentence in the second column. Its `docs` points at this table, which is inside the installed package.

| Code | Fix |
|---|---|
| `TACHLON_DATA_INVALID` | Correct the fields `message` lists and call `mount` again; `validate(data)` checks plain task data against the same rules without mounting. |
| `TACHLON_ENGINE_FETCH` | Serve the engine file (`app_bg.wasm`, the file the chart's script names with `new URL`) from the address the page asks for, with status 200; the `message` has the status. |
| `TACHLON_ENGINE_NOT_WASM` | The address answers with something other than the engine. Serve the file `app_bg.wasm` itself, as `application/wasm`, and make a single-page fallback route skip `.wasm` files. |
| `TACHLON_ENGINE_LOAD_FAILED` | The file is the engine, but it did not start. Rebuild so `app.js` and `app_bg.wasm` come from the same build, and read the browser console and the error in `cause` for the underlying error. |
| `TACHLON_ENGINE_CSP` | Add `'wasm-unsafe-eval'` to the page's `script-src` (or `default-src`) in its Content-Security-Policy, so the browser may compile the engine. |
| `TACHLON_GPU_NONE` | The browser has neither WebGPU nor WebGL2 here. Try a current Chrome, Edge, Firefox or Safari with hardware acceleration on; a headless or remote desktop session often has none. |
| `TACHLON_FILE_UNREADABLE` | The browser could not read the `File` given as `schedule`: it was moved, deleted or is locked. Pick the file again and call `mount` again. |
| `TACHLON_SCHEDULE_UNREADABLE` | The chart could not read the schedule as a Primavera XER file, a Microsoft Project XML file or a schedule document. Open it in the app it came from, or export it again as XER or XML, then call `mount` again. |

## No bundler

A plain page with no build step can load the package from a CDN. `import { mount } from "tachlon"` needs an import map to say where `tachlon` is, because a page has nothing else to resolve a bare name:

```html
<link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/tachlon@0.3.1/dist-lib/gantt.css">
<div id="chart" class="gantt" style="width: 800px; height: 300px"></div>
<script type="importmap">
  { "imports": { "tachlon": "https://cdn.jsdelivr.net/npm/tachlon@0.3.1/dist-lib/tachlon.js" } }
</script>
<script type="module">
  import { mount } from "tachlon";
  const tasks = [{ id: 1, name: "Design", start: "2026-11-02", duration: 3 }, { id: 2, name: "Build", start: "2026-11-05", duration: 5 }];
  await mount(document.getElementById("chart"), { tasks, links: [{ from: 1, to: 2 }] });
</script>
```

The script loads `app_bg.wasm` from the address beside itself, so the one script URL is enough. To serve the files from your own site instead, copy `node_modules/tachlon/dist-lib/` there and point the stylesheet link and the import map at your copy.

## Use it in React

`tachlon/react` is a React component, `Gantt`, from the same package. It takes the options of `mount` as props (`schedule`, `document`, `tasks`, `links`, `projectStart`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `constraintForm`, `lagCalendar`, `options`, `forceWebgl`, `readOnly`, `capabilities`, `beforeEdit`, `fields`, `save`, `keys`, `newKey`, `locale`, `texts`, `fonts` and `wasmUrl`, the address to load the engine from) and the chart's events as callbacks (`onSelect`, `onMove` and the rest). It mounts the chart when it mounts and calls `destroy()` when it unmounts. React 18 or later is a peer dependency that you install yourself; an app with no React needs none. The `assistant` option is not a prop yet: for the chat box, call `mount` (see "Tools for an AI assistant"). A new `capabilities` (see "What a host allows") is set on the chart that is mounted and does not mount it again; a chart that mounts again starts on the latest one.

```jsx
import { Gantt } from "tachlon/react";
import "tachlon/gantt.css";

// New `tasks` or `links` go to `chart.setData` in place: the scroll position, the selection and the undo list stay.
<Gantt tasks={tasks} links={links} style={{ width: 800, height: 300 }} onSelect={({ task }) => console.log("picked task", task)} />;
```

To call `chart.load` or `chart.open` (to open a file the user picks), give `onReady`: `<Gantt onReady={(chart) => (chartRef.current = chart)} />`. It is called once each time the chart mounts, with the chart `mount` resolves to, and never for a chart removed before it started. The component `npx tachlon init` writes in a React project takes the same `onReady` prop. A start that fails is thrown while `Gantt` renders, so wrap it in an error boundary to show your own message; `onError` hears the chart's `error` events once it has started.

A new `tasks`, `links` or `dataDate` goes to `chart.setData({ tasks, links, dataDate })` on the chart that is mounted, in one call, so the chart does not start again. It starts again when `setData` refuses the rows (a task or link added, removed or changed, or a task with no `key`), when a prop is taken away, when a task differs in a member `setData` ignores (`color`, `label`, `end`, `calendar`, `milestone`, `alap`, the actuals) or leaves out a `start`, `duration`, `progress`, `constraint` or limit it had (`null` clears a date limit), so no change is dropped, and when the start is still pending; a new `schedule`, `document`, `projectStart` or other mount option starts it again too.

To hand the chart the values your server recalculated, without starting it again, give `records`, a new array of `chart.update` records, and `onUpdate`: `<Gantt tasks={tasks} records={records} onUpdate={(result) => console.log(result)} />`. A new array calls `update` on the chart that is mounted, so the scroll position, the selection and the undo list stay, and `onUpdate` gets the answer, a refusal too. A new array that comes while the chart is starting is applied after it, the latest one only. The first array is not applied, and neither is one that comes with a new `tasks` that starts the chart again (it starts from `tasks`): put those values in `tasks`.

## Use it in Vue

`tachlon/vue` is a Vue 3 component, `Gantt`, from the same package: `import { Gantt } from "tachlon/vue"` and `import "tachlon/gantt.css"`. It takes the options of `mount` as props (`schedule`, `document`, `tasks`, `links`, `projectStart`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `constraintForm`, `lagCalendar`, `options`, `forceWebgl`, `readOnly`, `capabilities`, `beforeEdit`, `fields`, `save`, `keys`, `newKey`, `locale`, `texts`, `fonts` and `wasmUrl`) and emits the chart's events (`select`, `move`, `toggle-status` and the rest). It mounts the chart when it mounts and calls `destroy()` when it unmounts. Vue 3 or later is a peer dependency that you install yourself; an app with no Vue needs none. The `assistant` option is not a prop yet: for the chat box, call `mount` (see "Tools for an AI assistant"). A new `capabilities` (see "What a host allows") is set on the chart that is mounted and does not mount it again; a chart that mounts again starts on the latest one. A new `tasks`, `links` or `dataDate` goes to `chart.setData({ tasks, links, dataDate })` on the chart that is mounted, in one call, so the scroll position, the selection and the undo list stay; it mounts again when `setData` refuses the rows, a prop is taken away, a task differs in a member `setData` ignores (`color`, `end` and the rest), or the start is still pending, as the React component does; give every task its own text `key`, since a task with none mounts the chart again.

```vue
<Gantt :tasks="tasks" :links="links" style="width: 800px; height: 300px" @select="({ task }) => console.log('picked task', task)" @ready="(chart) => (chartRef = chart)" />
```

`@ready` gets the chart `mount` resolves to, once each time it mounts, to call `chart.load` or `chart.open`. A start that fails is thrown while `Gantt` renders: catch it with `onErrorCaptured` or the app's `errorHandler`.

`:records` and `@update` do the same in Vue (`<Gantt :tasks="tasks" :records="records" @update="(result) => console.log(result)" />`): a new array of `chart.update` records is applied to the chart that is mounted, in place, `@update` gets the answer, and the first array, or one that comes with a `tasks` that mounts the chart again, is not applied.

## Use it in Angular

`tachlon/angular` is an Angular standalone component, `Gantt` (selector `gantt-chart`), from the same package: `import { Gantt } from "tachlon/angular"`, add it to your component's `imports` and add `tachlon/gantt.css` to your styles. It takes the options of `mount` as inputs (`schedule`, `document`, `tasks`, `links`, `projectStart`, `dataDate`, `mustFinishBy`, `calendars`, `durationUnit`, `dateForm`, `constraintForm`, `lagCalendar`, `options`, `forceWebgl`, `readOnly`, `capabilities`, `beforeEdit`, `fields`, `save`, `keys`, `newKey`, `locale`, `texts`, `fonts` and `wasmUrl`) and has an output for each of the chart's events, named `chart` and the event: `chartSelect`, `chartMove`, `chartToggleStatus` and the rest (the prefix keeps `(chartChange)` and `(chartSelect)` apart from the browser's own `change` and `select` events, which an input inside the chart fires and which Angular would also deliver). It mounts the chart once its view exists and calls `destroy()` when the component is destroyed. Angular 17 or later is a peer dependency that you install yourself; an app with no Angular needs none. The `assistant` option is not an input yet: for the chat box, call `mount` (see "Tools for an AI assistant"). A new `capabilities` (see "What a host allows") is set on the chart that is mounted and does not mount it again; a chart that mounts again starts on the latest one.

```html
<gantt-chart [tasks]="tasks" [links]="links" style="width: 800px; height: 300px" (chartSelect)="picked($event.task)" (ready)="chart = $event" />
```

Keep `tasks` and `links` the same objects between changes: the chart starts again when they change. `(ready)` gets the chart `mount` resolves to, once each time it mounts, to call `chart.load` or `chart.open`. Outputs that have a subscriber when the chart mounts are the ones it listens for, so give `(chartFind)` or `(chartChooseFile)` only when your app answers the key. A start that fails goes to your app's `ErrorHandler`. The chart runs outside Angular's zone, and each output emits inside it.

`[records]` and `(update)` do the same in Angular (`<gantt-chart [tasks]="tasks" [records]="records" (update)="answered($event)" />`): a new array of `chart.update` records is applied to the chart that is mounted, in place, `(update)` gets the answer inside the zone, and the first array, or one that comes with a new `tasks`, is not applied.

## Load a P6 or MS Project file

The chart opens Primavera P6 XER files, MS Project XML files, and its own `.gantt` documents. Pass `chart.load` an object with the `File` as its `file` property:

```js
const result = await chart.load({ file });
if (!result.ok) console.error(result.error);
```

`load` returns a result and never throws; a bad file leaves the schedule as it was. `load({ document, unsaved: true })` shows the loaded schedule as unsaved, for a copy of work no save holds, so opening another file asks first. To start a chart from a file, pass it to `mount` as `schedule` instead of `tasks`: `mount(element, { schedule: file })`. To start on a saved document, pass what `chart.toDocument()` returned as `document`: `mount(element, { document })`, with no `load` after it (a `schedule` wins over a `document`, and a `document` over `tasks`). A `mount` with no `schedule`, `document` or `tasks` starts with no tasks and waits for `chart.load({ document })`. `chart.open(file)` replaces the schedule the same way and also takes an `OpenOptions` for a file with several projects.

`chart.toDocument()` returns the open schedule as a document that `chart.load({ document })` reads back. `chart.snapshot()` returns the open schedule as plain `tasks` and `links`, and `dataDate`, `mustFinishBy` and `options` when it has them (`fromTasks` reads them back) with each task's current start, and `leftOut` names what plain data cannot carry; `toDocument()` stays the lossless save. `chart.exportXer()` and `chart.exportMspdi()` return the open schedule as a P6 XER or MS Project XML file (`bytes`, and `leftOut`, one line for each thing the format cannot carry). `chart.exportXerLeftOut()` lists the activity codes, baselines and deadlines a P6 save would leave out of the open schedule, without writing it.

Already have DHTMLX Gantt or Bryntum Gantt data? `fromDhtmlx(data)` and `fromBryntum(data)` read it into a result whose `document` is what `chart.load({ document })` takes (`fromBryntum` returns `{ ok: false, error }` when it cannot read the data). `fromTasks({ tasks, links })` does the same for plain tasks and links, and `validate({ tasks, links })` names each problem in them by path with a fix. `fromTasks` throws a `PlainDataError` (its `issues` list each problem) when `validate` would refuse the data, and `fromDhtmlx` throws an `Error` that names the task or link it could not read.

## Add it to a Vite app

Inside a Vite project (with or without React), `npx tachlon init` installs the package with the project's own manager (found from its lockfile, then its `packageManager` field, then its workspace root, else npm) and writes a chart file beside your code: `src/tachlon-chart.js`, `.ts`, `.jsx` or `.tsx`, or the same name at the project folder when there is no `src/`. The file mounts the chart with three sample tasks and imports `tachlon/gantt.css`; the React component removes the chart when it unmounts, and the plain file returns a function that removes it. The command prints the one line to add to a page and the command to start the app. It writes new files only: never an existing page, Vite config, lockfile or tsconfig.

- `--dry-run` prints the plan and writes nothing.
- `--dir <folder>` names the project folder; without it the nearest `package.json` at or above the working folder is the project.
- `--spec <spec>` installs that spec (a tarball path, a version range) instead of this package's own version; `--no-install` installs nothing.
- `--plain` writes the module that needs no framework (a function that takes the chart's `div`), for a project whose bundler `init` does not know. The module imports the package by name and imports its stylesheet, which a browser resolves only through a bundler, or through an import map and a stylesheet link on the page (see "No bundler" above); `doctor` fails a project with no bundler whose page loads a module script and has no import map.
- `--skill` also writes `.claude/skills/tachlon/SKILL.md`, a short skill for an AI assistant: when to use the package, where its `AGENTS.md` is, and what never to do. It is a new file only, listed by `--dry-run` and `--json`; a file already there is left as it is.

A second run changes nothing and says so. `init` exits 2 and writes nothing when there is no `package.json`, when the folder is a workspace root and `--dir` is not given, when the project is neither Vite nor Next.js (it names the bundler it found, or says there is none, and offers `--plain`), when a file it would write already exists with other contents (it names the path), or when the install fails. In a Next.js project (an `app/` or `pages/` folder, in the project folder or in `src/`), `init` instead writes the chart as a client component, `components/tachlon-chart.jsx` or `.tsx` beside that folder, and copies the installed engine to `public/app_bg.wasm`, which the component loads with `wasmUrl: "/app_bg.wasm"`; it exits 2 when there is neither folder, or when the package is not in `node_modules` to copy the engine from.

## Check an install with `tachlon doctor`

When the chart does not draw, run `npx tachlon doctor` in the project. It reads the project and writes nothing in it. It prints one line for each check (`ok`, `FAIL` or `skip`) and a one-sentence `fix:` under each failure and under each `skip` line. `--dir <folder>` names the project folder; a workspace root needs it. The checks, with the code each failure carries:

- `package.json` lists the package and `node_modules` holds one copy of it: `NOT_LISTED`, `NOT_INSTALLED`, `MANY_COPIES`.
- The installed engine file `dist-lib/app_bg.wasm` is there, starts with `\0asm`, and has the size the package's build recorded: `ENGINE_MISSING`, `ENGINE_NOT_WASM`, `ENGINE_UNRECORDED`, `ENGINE_SIZE`.
- A source file imports `tachlon/gantt.css`: `STYLESHEET_MISSING`.
- In a Next app, the engine copy `public/app_bg.wasm` that `init` writes is the installed engine, not one left from an older version: `ENGINE_COPY_MISSING`, `ENGINE_COPY_STALE`.
- A Content-Security-Policy in the project's config (`next.config.*`, `vercel.json`, `netlify.toml`, `public/_headers`, `index.html`, `public/index.html`) lets the browser compile the engine: `CSP_WASM_EVAL` when its `script-src` (or `default-src`) lacks `'wasm-unsafe-eval'` (or `'unsafe-eval'`). A report-only policy blocks nothing and is skipped; with no policy found there is no finding.

`--url <page>` also fetches the page that mounts the chart (two requests, plus up to five of the page's module scripts when its HTML names no `.wasm` address; a page it cannot fetch, or that does not answer 200, is exit 2) and checks what the site serves. The policy in its `Content-Security-Policy` header and `<meta>` tags gets the same `CSP_WASM_EVAL` check, and `CSP_CONNECT_SRC` when `connect-src` does not allow the engine's origin, the page's own origin included. The engine is the `.wasm` address the page's HTML names (a preload link first), or else the first one named in the page's own `<script type="module" src>` files on the same origin (five at most, so a Vite build's hashed engine file is found; a chunk loaded later is not read), or `/app_bg.wasm` in a Next app. It must answer 200 (`ENGINE_FETCH`), start with `\0asm` (`ENGINE_NOT_WASM`) and have the type `application/wasm` (`ENGINE_CONTENT_TYPE`), and in a Next app it must equal the installed engine (`ENGINE_URL_STALE`). Outside Next, a page that names no `.wasm` address in either place gets `ENGINE_URL_SKIPPED`: open the page in a browser and request the address from its network panel.

`--data <file>` also runs `validate()` on a task data file, with one `DATA_INVALID` finding for each problem, or `DATA_NOT_JSON` when the file is not JSON.

`doctor` also asks a real browser which graphics path the chart will use, because a Node process has no WebGPU or WebGL2 of its own. It opens a page of its own, headless, in the first Chrome, Edge or Chromium it finds (`TACHLON_BROWSER` names one; otherwise it looks for `google-chrome`, `google-chrome-stable`, `chromium`, `chromium-browser` and `microsoft-edge` on the path, then for the Chrome and Edge apps on a Mac, then for a Chromium that Playwright installed in its cache folder, `PLAYWRIGHT_BROWSERS_PATH` or its usual place) and reports one finding. The chart tries WebGPU first and falls back to WebGL2, and so does the page:

- `GPU_WEBGPU` (`ok`) when the browser has a WebGPU adapter, with the adapter's name.
- `GPU_WEBGL2` (`ok`) when it has no WebGPU adapter but has WebGL2, with the renderer's name.
- `GPU_NONE` (`FAIL`) when it has neither, the case where `mount` rejects with `TACHLON_GPU_NONE`; the `fix:` line says what to try.

That is the browser on the machine that runs `doctor`, not the one a visitor uses. A headless browser on a machine with no graphics card may name a software renderer such as SwiftShader, which is the answer, not a fault. When no browser is found, or the one found fails to answer, the line reads `skip GPU_SKIPPED` with the reason, which carries the browser's first error line, and it does not fail the run: open the page in a browser and read the error `mount` rejects with. When that error line names the sandbox (on recent Ubuntu it is blocked for an unprivileged browser), `doctor` runs the browser once more without it; a finding from that run says `doctor retried the browser without the sandbox`, and if the retry fails too, the skip gives both reasons.

Exit 0 means no check failed, 1 means at least one did, and 2 means `doctor` could not run: no `package.json`, a workspace root without `--dir`, a file it could not read, or a flag it does not know. This run is from a fixture whose engine file was deleted from `node_modules`:

```text
doctor in .
  ok   INSTALLED: tachlon 0.1.0 at node_modules/tachlon
  ok   ONE_COPY: one copy is installed
  FAIL ENGINE_MISSING: node_modules/tachlon/dist-lib/app_bg.wasm is missing
       fix: Delete the package from node_modules and install it again with `npm install`, and stop any cleanup step that deletes .wasm files.
  ok   STYLESHEET_OK: a source file imports tachlon/gantt.css
  ok   GPU_WEBGL2: the browser has no WebGPU adapter but has WebGL2 (ANGLE (Google, Vulkan 1.3.0 (SwiftShader Device (Subzero) (0x0000C0DE)), SwiftShader driver)), so the chart draws with WebGL2
1 problem found.
```

Do the `fix:`, then run `doctor` again; a clean run ends with `No problems found.` and exit 0. A run whose `GPU_SKIPPED` check did not happen ends with `No problems found, GPU not checked.` instead, which is not the same as a checked GPU: exit 0 there means no check failed, not that the graphics path works. `--json` prints `{ "dir": ..., "findings": [...] }` instead, with the same exit code. Each finding has `code`, `status` (`"pass"`, `"fail"` or `"skipped"`), `what`, and `fix` (empty on a pass), so a script or an assistant can read the failures without parsing text.

## The stylesheet

`import "tachlon/gantt.css"` once in your app. The chart's colors, bar shape, row height and type are CSS tokens, in light and dark, that your own CSS can set.

Set a token on the chart's root element, for example `.my-host .gantt { --gantt-bar: #0a7; }`; your rule wins over the stylesheet whatever its order. `npx tachlon describe tokens` prints every token with what it styles and its default.

## Ask the engine

`chart.commands` holds typed calls that ask the engine about the schedule. A question changes nothing: no edit, no undo step, no `change` event. Each call answers `{ ok: true, ... }` or `{ ok: false, reason, message }` and never throws; it answers `"busy"` while a bar is pressed or dragged, and returns `undefined` once `chart.destroy()` has run. A task is its task number, the `task` a `select` event gives.

`whatIf` and `whatIfLonger` answer the tasks that move, and `explain` answers the chain of driving predecessors, nearest first:

```js
const slip = chart.commands.whatIf([2]);
// { ok: true, finishWas, finishNow, moved: [{ task, id, name, startWas, startNow, finishWas, finishNow }] }
const why = chart.commands.explain(3);
// { ok: true, start, chain: [{ task, id, name, float }], capped }
```

`health` answers `counts`, each `{ count, tasks }` holding the true count and the first 50 tasks: `openEnds`, `hardConstraints`, `negativeFloat`, `missingLogic`, `openStarts`, `highFloat`, `highDuration`, `invalidDates`, `missingResources` and `missed`.

`propose` holds a change apart from the schedule while the chart draws a dashed ghost where each task it would move goes, and the call answers as `whatIf` does. Up to 10 changes are held; a second change of the same kind to a task replaces the first. `acceptProposed` leaves the other held changes held, and `rejectProposed` returns `true` when that change was held.

`npx tachlon describe commands` prints every call with its arguments and what it does. `npx tachlon describe methods` prints the chart's own methods, each with its whole doc comment, and `npx tachlon describe refusals` prints the `reason` of each refusal, with the fix where there is one.

## Tools for an AI assistant

`toolDefinitions(form)` gives an assistant six tools, `whatIf`, `whatIfLonger`, `explain`, `health`, `criticalPath` and `propose` (it holds an edit as a ghost for a person to accept; no tool accepts), as tool definitions in the `"mcp"`, `"anthropic"` or `"openai"` form, and `callTool(chart.commands, name, args)` runs the one the assistant picks, answering as the command does. Pass the definitions to your assistant's API, then pass each tool call it makes to `callTool` (an OpenAI call sends its `arguments` as JSON text: `JSON.parse` it first). A call it cannot run is a result with `ok: false`, never a throw.

To put the chat box beside the chart, give `mount` an `assistant`: `{ send, into }`, where `into` is an element of your page (the panel takes the chart's styling and your theme tokens from a `.gantt` element at or above `into`, and brings its own where there is none) and `send(messages)` is your own call to your model (the conversation in, `{ content, toolCalls? }` out). With `assistant` set, the page sends the whole conversation, your messages and the tool results, to your `send` and nowhere else; what `send` does with it is yours to say. Without `assistant` the page has no chat box and sends nothing. A switch to WebGL2 starts the panel again with an empty history. This `send` calls an OpenAI-shaped endpoint of your own:

```js
const tools = toolDefinitions("openai");
const wire = (m) => m.role === "tool" ? { role: "tool", tool_call_id: m.callId, content: JSON.stringify(m.result) }
  : m.toolCalls?.length ? { role: "assistant", content: m.content, tool_calls: m.toolCalls.map((c) => ({ id: c.id, type: "function", function: { name: c.name, arguments: JSON.stringify(c.arguments) } })) }
  : { role: m.role, content: m.content };
const send = async (messages) => {
  const res = await fetch("/my-model", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ messages: messages.map(wire), tools }) });
  const reply = (await res.json()).choices[0].message;
  return { content: reply.content ?? "", toolCalls: reply.tool_calls?.map((c) => ({ id: c.id, name: c.function.name, arguments: JSON.parse(c.function.arguments) })) };
};
await mount(root, { tasks, assistant: { send, into: document.getElementById("assistant") } });
```

In Node, with no page, `import { openHeadless } from "tachlon/headless"` reads an XER or MS Project file (`openHeadless(bytes)`), the plain `{ tasks, links }` the chart takes (`openHeadless({ tasks, links })`) or a document (`openHeadless({ document })`), and answers `whatIf`, `whatIfLonger`, `explain`, `health` and `criticalPath` as the chart's commands do, with the same results and no canvas, and `results()` lists every task's early and late dates, total and free float (working days) and critical flag, each with its `key` (the name your program gave the task, `null` for a task with none) and `code`; `task` in an entry is the session slot, valid only until the next open, so match by `key`. With `dateForm: "day"` beside `tasks` (`openHeadless({ tasks, links, dateForm: "day" })`), `start`, `finish`, `lateStart` and `lateFinish` answer as `YYYY-MM-DD`, as `mount`'s `dateForm: "day"` writes a change: a start is its day, a finish at `T00:00` is the day before, and any other finish is its own day, so a host that stores days compares each answer to its own with no conversion. Plain data that `validate` refuses throws a `PlainDataError`, as `fromTasks` does (catch it by `err.name`; `err.issues` lists the problems), and the returned schedule's `leftOut` lists what was not read. `callTool` and `toolDefinitions` come from the same import and run the assistant's tools over that schedule (`propose` is answered with `"unknownTool"`, as it holds a ghost the chart draws), and `free()` releases it when you are done (a `callTool` after that answers `"destroyed"`).

An assistant that edits your project also has a skill: `npx tachlon init --skill` writes `.claude/skills/tachlon/SKILL.md`. It tells the assistant to read `node_modules/tachlon/AGENTS.md` first, to run `npx tachlon describe` before it writes code against an option or an event, and to run `npx tachlon doctor` when the chart does not draw or does not load.

## Your product's user guide

The package ships a user guide for the chart, written for any product: one file for each feature, in `node_modules/tachlon/dist-lib/guide/`, under the package's MIT license. `npx tachlon describe guide` prints the prompt that turns those files into your own product's documentation. It tells your assistant to read how your product uses the chart, learn its name and voice, keep only the features your options turn on, write the pages, and list the screenshots to take in your own themed app. It never invents a feature and never copies a picture. Paste this one line into your assistant:

```text
Follow `npx tachlon describe guide` to write this product's user guide from the chart's own.
```

`--json` prints the same prompt as data.

## The schema

`tachlon/schema.json` is the JSON Schema of the task data `mount` reads (`tasks` and `links`). Read it from `node_modules/tachlon/dist-lib/schema.json`, or give it to a tool or an assistant that writes task data.

## Save, export and read the state

```js
await chart.download(); // downloads `<name>.gantt`
const { rowCount } = chart.state(); // `chart.on("change", ...)` fires when it moves
```

- `chart.download()` downloads the schedule as a `.gantt` document and clears the unsaved mark; it never writes over the opened file.
- `chart.exportCsv()` downloads the visible list as `<name>.csv`.
- `chart.exportXlsx()` downloads the same list as an Excel file, `<name>.xlsx`, with the dates as Excel dates.
- `chart.copyList()` puts the same list on the clipboard as tab-separated text, and rejects when the browser refuses.
- `chart.exportImage({ page: n })` downloads a 2112 by 1632 pixel picture of printed page n (the first by default) as `<name>.png`, or `<name>-page-<n>.png` past the first; a page that does not exist, or a number that is not whole, rejects. `chart.printedPages()` returns how many pages print, the last page this takes.
- `chart.printOn("a4")` opens the print dialog for that paper (`"letter"` or `"a4"`), for this one print.
- `chart.newSchedule()` replaces the schedule with an empty one, asking first when changes are unsaved.
- `chart.checks()` counts the schedule's quality checks (`missingLogic`, `openEnds`), with no pass or fail.
- `chart.backend()` says whether `"WebGPU"` or `"WebGL2"` is drawing.
- `chart.state()` reads what a toolbar shows: the filters, row and task counts, zoom and selection.
- `chart.filterInputs()` lists what the filter controls offer; read it again after an `open` or an edit.
- `chart.commands.assignKey({ task }, key)` hands back the key the host gave a row the chart created, after the host saved it (`{ link }` for a drawn link); it is no edit, so there is no undo step and the schedule is not marked unsaved. It answers `{ ok: true }` or a refusal (`"keyTaken"`, `"hasKey"`, `"emptyKey"`, `"unknownTask"`, `"unknownLink"`). The slot is valid only until the next load, open or remount.
- `chart.busy()` is true while a cell is being edited or a bar is dragged, when edit commands answer `"busy"`.
- `chart.setPending(keys)` marks the rows with these keys as waiting for your own recalculation, in place of the marks before: the chart dims them and tells a screen reader they are busy until you call it again without them (`[]` clears all). It is a hint, not a lock: the rows stay selectable and editable, and it is no edit, so it makes no undo step. It answers the keys that name no live task. The marks go when `open`, `load` or `newSchedule` replaces the schedule, and at a switch to WebGL2 (the `error` event with source `backend`), so mark the rows again then; a call right after `load` or `open` resolves marks the new schedule, and a call during the switch throws.
- `chart.speed()` returns the last open's phases and the last 120 frames' timings; polling it never wakes a sleeping loop.

## Events

`chart.on(name, handler)` calls `handler` with the event's payload and returns the function that removes it. An event fires once per change, never once per frame. A handler added to `load` after the first frame is still called, once. With a handler on `find`, the browser's find does not open. The task number in a `select` payload is valid only until the next load or open: store your own `key`, never the number. The `source` of an `error` payload is `"open"`, `"frame"`, `"backend"` or `"beforeEdit"`, and its `projects` lists the projects of a multi-project XER file.

`npx tachlon describe events` prints every event with its payload and when it fires.

An `edit` event carries the undo steps made since the last one, oldest first. Each commit is `{ seq, source, changes }`: `seq` counts from 1 in a session and is never reused, `source` says where the step came from (`typed`, `drag`, `dialog`, `paste`, `proposal`, `undo` or `redo`), and each change is one task, link or the data date, `{ record, op, key, set, was }`: an update's `set` holds the members that changed with their new values and `was` the same members as they were, a create's `set` is the whole record, a delete's `was` is the whole record it removed, and `cascade: true` marks a link delete that follows from a task delete in the same step. A `duration`, `remaining` or `lag` there is in the `durationUnit` the chart mounted with (a day is the working day of the task's calendar, or of the link's lag calendar), and a date is `YYYY-MM-DDTHH:mm`, or `YYYY-MM-DD` when the chart mounted with `dateForm: "day"` (a finish is then the last day worked). A `save` change set and the before-edit function get the same forms. A task's date limit is one member there, `constraint: { type, date }` or `null`, with a `type` of `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `so` or `fo` (`so` is `snet` and `snlt` on one date, `fo` is `fnet` and `fnlt` on one date; the first that is set, in that order of `mso`, `mfo`, `so`, `fo`, `snlt`, `fnlt`, `snet`, `fnet`, when the engine holds several); mount with `constraintForm: "separate"` to get the six members `snet`, `snlt`, `fnet`, `fnlt`, `mso` and `mfo` instead. In `fields`, `constraint` takes one name of yours, or `{ type: "limitType", date: "limitDate" }` for two members that are both `null` for none; `mount` rejects it under `"separate"`. A bar drag is one commit, which comes with an `edit` at its release (fired even when the revision did not move), so a held bar carries none. `seq` is the latest commit's. When `snapshot` is true, commits were lost (more than 1,000 unread, or the chart moved to WebGL2): save `chart.snapshot()` and read on from `seq`.

## Read-only

Start the chart read-only with the `readOnly` option of `mount`, or switch it for the person using it at any time: `chart.setReadOnly(true)` and `chart.setReadOnly(false)`; `chart.readOnly()` says which. The chart does not start again, and the schedule is not an edit (no undo step, no `edit` event). In the React, Vue and Angular components `readOnly` is a prop or input, and changing it calls `setReadOnly` on the chart that is mounted. `npx tachlon describe options` prints `readOnly` and every other option of `mount`.

While it is on, nothing changes the schedule: the cell editor and the task dialog do not open, and the chart says it is read-only; a bar, finish-edge or link drag, the keys (insert, delete, indent, outdent, undo, redo), paste and the edit commands are refused (the commands answer `{ ok: false, reason: "readOnly" }`). A click on a row still selects it. A bar or finish-edge drag already past the drag threshold when you switch finishes; a link drag in flight, or a bar press still under the threshold, is refused at release. A switch to WebGL2 keeps the setting.

## What a host allows

A host database often holds less than the chart can express. Tell the chart which kinds of change the database can take with `mount(root, { tasks, capabilities: { gestures: { barMove: false }, editable: { outline: false } } })`, or at any time with `chart.setCapabilities(profile)` (`undefined` allows everything again). Every member is on unless it is `false`. `gestures` has `barMove`, `resize` and `link`. `editable` has the groups `rows` (insert and delete), `outline` (indent, outdent and move), `paste`, `names`, `dates` (start, duration and the date limits), `links`, `progress` (actual dates, remaining, percent and the data date) and `baseline`, so the outline can stay locked while rows and progress stay editable. Five more members say what the database stores: `kinds` (a list of `"snet"`, `"snlt"`, `"fnet"`, `"fnlt"`, `"mso"`, `"mfo"`, `"alap"`, `"so"` and `"fo"`, all when left out; a task whose start floor and ceiling sit on one instant holds `"so"`, and one whose finish floor and ceiling do holds `"fo"`, unless `kinds` leaves it out), `perTask` (the most date limits for one task, counting a start no earlier than), `durationStep` and `lagStep` (`"minutes"`, `"hours"` or `"days"`: a typed value is a whole number of them, and a day is that task's calendar day) and `dates` (`"minute"`, or `"day"` for dates with no time of day; a start at its calendar's opening and a finish at its closing count as none). A member the chart does not have throws from `setCapabilities` and rejects `mount`.

A change the host turned off is refused on every path: the cell editor, the task dialog, a drag, the keys, paste, undo and redo (for what the step replays), an accepted proposal and the edit commands, which answer `{ ok: false, reason: "notAllowed", message }`. The chart says it in the alert line, or in the drag note for a drag, with one `texts` key and sentence for each gesture or group a profile turns off (`notAllowedBarMove` "Moving a bar is not allowed.", `notAllowedResize` "Resizing a bar is not allowed.", `notAllowedLink` "Drawing a link is not allowed.", `notAllowedRows` "Adding and deleting rows is not allowed.", `notAllowedOutline` "Changing the outline is not allowed.", `notAllowedPaste` "Pasting rows is not allowed.", `notAllowedNames` "Changing names is not allowed.", `notAllowedDates` "Changing dates and durations is not allowed.", `notAllowedLinks` "Changing links is not allowed.", `notAllowedProgress` "Changing progress and the data date is not allowed.", `notAllowedBaseline` "Changing the baseline is not allowed."), and `notStored` "Saving allows {what}." for a value outside what it stores. A bar drag that would make a value the host does not store is refused until drags snap to its step, and a dialog that sets a start and a finish together checks the duration against the old start. The check comes after read-only and before `beforeEdit`, which is not asked about a change the profile refused. A switch to WebGL2 keeps the profile; the profile is not saved in the document.

## Control editing

Give `mount` a `beforeEdit` function, as in `mount(root, { tasks, links, beforeEdit: ({ tasks }) => (tasks.some(isLocked) ? "A planner has locked this task." : undefined) })`, and the chart asks it before each change a person makes (a typed edit, a bar or link drag, the task dialog, paste, undo, redo), once for each action and after read-only. A refusal changes nothing: no schedule change, no undo step, no `edit` event. `npx tachlon describe options` prints `beforeEdit` with the other options of `mount`; what the function receives and how it answers are below.

Answer at once. `undefined`, `null` or `true` lets the change through. A string refuses it and the chart shows the string as the reason. `false` refuses it with the chart's own sentence, "This change is not allowed.". A throw, a promise or any other value refuses the change, changes nothing and fires one `error` event with the source `"beforeEdit"`. A command refused by a string or `false` answers `{ ok: false, reason: "refused", message }`; one refused by a throw or a promise answers `reason: "beforeEditFailed"` with the sentence "This change was not made: the page's before-edit function failed.".

The function gets `{ kind, source, tasks, changes }`: the action, what made it (a `source` of an `edit` event's commits), the task numbers it touches, and the change records it would make, one change per record as an `edit` event carries them, with `set` and `was`. These carry `changes`: a typed edit of a start, duration, name, progress or limit, a `group` of them (an accepted proposal), a drag (the start or duration its first frame asks for; the value at release is not known yet), the task dialog (each field, planned against the chart before any applies), paste (a task create for each name, with no other field), undo and redo (the step they replay), a link drag released on a bar (the finish-to-start link it makes, with no lag), and a typed delete, restore, outline move, link edit or data-date edit (the records its commit writes), insert (the place and name) or baseline edit (the `baseline` field). A plan of two or more of these (a group, or an undo or redo of several) carries its task-field values and no structural ones. These carry `kind`, `source` and `tasks` only, with `changes` empty: an undo or redo of a data-date edit, the press that starts a link drag, and a discarded what-if. One example for each source:

```js
{ kind: "setDuration", source: "typed", tasks: [27], changes: [{ record: "task", op: "update", key: null, task: 27, set: { duration: 17 }, was: { duration: 0 } }] }
{ kind: "group", source: "proposal", tasks: [2, 3], changes: [{ record: "task", op: "update", key: null, task: 3, set: { duration: 9 }, was: { duration: 6.4 } }, { record: "task", op: "update", key: null, task: 2, set: { snet: "2026-11-09T00:00" }, was: { snet: null } }] }
{ kind: "drag", source: "drag", tasks: [6], changes: [{ record: "task", op: "update", key: null, task: 6, set: { snet: "2026-01-12T07:00" }, was: { snet: null } }] }
{ kind: "setTaskFields", source: "dialog", tasks: [2], changes: [{ record: "task", op: "update", key: null, task: 2, set: { name: "Pour deck" }, was: { name: "Pour slab" } }] }
{ kind: "insertMany", source: "paste", tasks: [], changes: [{ record: "task", op: "create", key: null, task: 30, set: { name: "Footing" } }] }
{ kind: "undo", source: "undo", tasks: [27], changes: [{ record: "task", op: "update", key: null, task: 27, set: { duration: 0 }, was: { duration: 17 } }] }
{ kind: "redo", source: "redo", tasks: [27], changes: [{ record: "task", op: "update", key: null, task: 27, set: { duration: 17 }, was: { duration: 0 } }] }
```

When one task dialog sends dependent fields (a start with a finish, a duration with a finish), each is planned against the chart before any applies, so the new value the function sees for the dependent field can differ from the one the commit writes. Never call the chart from inside the function: a call that reaches the chart (`toDocument`, `state`, a command such as `commands.insertRow`) throws and the change is refused with an `error` event, except that, while a cell is being edited, an edit command returns `busy` and does nothing (`undo`, `redo` and `layoutJson` catch the throw and return `false` with no refusal and no error event, and `select`, `zoomIn`, `setFilter`, `whatIf` and `explain` throw even then). The function cannot wait for an approver: refuse the change with a reason such as "Sent to the planner for approval", send it on, and when it is approved hold it with `chart.commands.propose` and accept it with `acceptProposal`. The chart asks your function again for that accept, with the source `proposal`, so let a `proposal` through for a change you approved. The React, Vue and Angular components take `beforeEdit` as a prop or input and ask the latest function they were given, without mounting the chart again.

## Connect to your backend

A host that already has an API client gives `save` one handler for each kind of change. The chart calls the right one for each edit, in the order the person made them, each after the one before has resolved, with the row, its key and what changed. You write no save queue, no repeat and no put-back. A change made by `hostEdit` is not sent, because your page made it; a person's Undo or Redo of it is sent like any other change, so your backend learns it.

```tsx
import { Gantt, type Handlers } from "tachlon/react";
import "tachlon/gantt.css";
import { api, type Task, type Link } from "./api"; // your client: it sends c.once and c.signal

const fields = { // the chart's names on the left, your API's on the right; any other member of a row comes back untouched
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const save: Handlers = { // one call into your API per change, in the order the person made them
  createTask: (row, c) => api.createTask(row, c),
  updateTask: (key, set, c) => api.updateTask(key, set, c),
  deleteTask: (key, c) => api.deleteTask(key, c),
  createLink: (row, c) => api.createLink(row, c),
  updateLink: (key, set, c) => api.updateLink(key, set, c),
  deleteLink: (key, c) => api.deleteLink(key, c),
  updateSchedule: (set, c) => api.updateSchedule(set, c),
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && (e.reason === "stopped" ? alert("Saving stopped. Reload the page.") : reload())} style={{ height: 480 }} />;
}
```

- **The last argument,** `c`, carries `once`, the request's idempotency key (`<id>:<n>`); `signal`, aborted 10 seconds after the handler is called (the chart still waits for the handler to settle); `was`, the old values; and `changeSet`, the whole commit.
- **Throw a `SaveRefused` when the server says no.** A handler that rejects, or resolves with a `Response` whose status is not a 2xx, ends the call. A `SaveRefused` puts that edit and the ones after it back and fires `error` with the source `"save"`; read your data again there, as the example does. Any other throw (a 5xx, a lost connection) stops the queue: see `unsavedCommits` below.
- **A handler that resolves with an object that has an `error`, `errors` or `userErrors` member is read as saved,** and the console warns once for that handler. The check reads the top level only: a client that nests `userErrors` deeper is not caught, so its handler throws when it finds one.
- **Read `chart.state().unsavedCommits` to know how many edits the chart has not saved yet,** and `chart.on("change", ...)` tells you when that number moves. The queue counts it: it is 0 again after each commit is saved, put back (with the commits behind it), or dropped, and 0 with no `save`. Never count `edit` events yourself: a commit made before a put-back and delivered after it (below its floor) is not sent, and a switch to WebGL2 or `destroy()` drops the commits still waiting without sending them. A call that never settles keeps the count above 0. **A `save` whose answer is neither a success nor a refusal stops the queue (a 5xx, a 401, a lost connection; the table under "Types and API reference" has the rest):** no later call is made, that commit and every later one stay unsaved and counted until `destroy()`, and `error` fires once with `reason: "stopped"`, the thrown `message` and `held`, the commits it holds. Today a host tells the person, and mounts the chart again, which drops the held edits (read your data again as you mount); retry and discard are not in the chart yet. `chart.state().unsaved` is the other thing: the file mark, for a host that saves files.
- **The chart reads the object it holds at each call,** so a new `save` object on every render is safe.
- **A handler you leave out turns its edits off.** At start the console warns once, naming each missing handler and the edits it turns off (`createLink (new links)`). The chart refuses those edits before they show (`This page cannot save new links: it gave no createLink.`, the `cannot-save` key of `texts`) and does not ask `beforeEdit` about them; this conservative check also refuses undoing the delete of a task that has a link without `createLink`, even when the other end stays deleted, and deleting a linked task without `deleteLink`; with no `createLink` the person cannot draw a link either (`capabilities.gestures.link` is off, and your other members stay). The gate reads the current handlers at each action; link drawing is set at start and at each `setCapabilities`.

A host that saves a whole commit at once, through a batch route, a transaction or a command stack of its own, gives `save` a function in place of the object. The chart calls it with the change set of each commit, `{ id, seq, source, changes }`, in the order the person made them and one at a time: the next call waits until the one before has resolved. Each change is `{ record, op, key, set, was, once }`, in your names, and `once` is its idempotency key. This function sends each change to its own route:

```tsx
import { Gantt, type Save } from "tachlon/react";
import { SaveRefused } from "tachlon";
import "tachlon/gantt.css";
import type { Task, Link } from "./api-types"; // your own rows

const fields = {
  tasks: { key: "id", name: "title", start: "plannedStart", duration: "days", parent: "parentId" },
  links: { key: "id", from: "source", to: "target" },
};

const routes = { task: "/api/tasks", link: "/api/links", schedule: "/api/schedule" };
const methods = { create: "POST", update: "PATCH", delete: "DELETE" };

const save: Save = async ({ changes }) => {
  for (const c of changes) {
    const url = c.op === "create" || c.record === "schedule" ? routes[c.record] : `${routes[c.record]}/${encodeURIComponent(c.key!)}`;
    const headers = { ...(c.set ? { "content-type": "application/json" } : {}), "idempotency-key": c.once };
    const res = await fetch(url, { method: methods[c.op], headers, body: JSON.stringify(c.set), signal: AbortSignal.timeout(10_000) });
    if (res.status === 409 || res.status === 422) throw new SaveRefused({ message: (await res.json()).message }); // a sentence the person can act on: the edit is put back
    if (!res.ok) throw new Error(`save failed: ${res.status}`); // anything else stops the queue, and `error` says so (`reason: "stopped"`)
  }
};

export function Schedule({ tasks, links, reload }: { tasks: Task[]; links: Link[]; reload: () => void }) {
  return <Gantt tasks={tasks} links={links} fields={fields} dateForm="day"
    save={save} onError={(e) => e.source === "save" && (e.reason === "stopped" ? alert("Saving stopped. Reload the page.") : reload())} style={{ height: 480 }} />;
}
```

- **A `SaveRefused` puts that commit and every commit made after it back,** and the chart shows its `message` with the number of edits put back and fires `error` with the source `"save"`; read your data again there, as the example does. A `hostEdit` made after the refused commit goes back with it and `undone` counts it: your page sends it again.
- **After the chart removes a row your data dropped (`App.remove_task`), a save that is later refused answers reload:** the person's steps that named the row are forgotten, nothing is put back, and `error` has `reload: true`, so read your data again.
- **Host link changes use `App.add_link_by_key`, `App.set_link_by_key` and `App.remove_link_by_key`:** the chart updates without mounting again; these are not edits and keep the person's history (see [the WASM API](../../docs/WASM_API.md)).
- **A batch endpoint is one request in place of the loop.** Send `changes` whole and make the one server transaction all or nothing, so a refusal puts back the whole commit.
- **The chart reads the function it holds when it sends a commit,** so passing a new `save` between two edits is safe: the second edit goes to the new function. The React, Vue and Angular components do not mount the chart again for it.

### Keys for the rows a person makes

With `save`, every task and link you pass needs a `key`, and no two tasks and no two links share one: `mount` rejects a row that breaks this (`noKey`, `duplicateKey`). The check reads the `save` the chart holds at mount, so a React, Vue or Angular `Gantt` with no `save` takes keyless rows and gives a new row no key (you assign it, as before); a `save` that arrives later finds those rows still keyless, so give every row a key first. A row the person makes gets its key before its change set is written, and every change that names the row uses it.

- **`keys: "chart"` (default):** the chart makes the key (`crypto.randomUUID()`, or your `newKey: () => string`) and the create carries it as `key` and, under your name for `key`, in `set`. Store it as the row's id, or in a unique column.
- **`keys: "server"`:** the create carries `key: null` and a `ref`, and every row in the change set that names it holds `{ ref }`. Answer with the key the server made: a handler resolves with the key, an object that has it under your name for `key`, or a 2xx `Response` whose JSON body is the row; a `save` function resolves with `{ keys: [{ ref, key }] }`, one for each create that has a `ref`. A handler gets a row's key in place of its `{ ref }` once its create has answered. The undo of a delete and the redo of a create carry the key the row had and a `ref` too, and a member that names that row holds `{ ref }`. The server may answer such a row with another key, and the chart takes it. A handler that answers none, and a `save` function that leaves it out of `keys`, leave the row under the key it had, so a `save` function puts the `key` of each create that has one in its own ref map before it reads the creates that name it.
- **A result the chart cannot use** (a missing, extra, repeated or taken key, a `keys` that is not a list of `{ ref, key }`, or `keys` under `keys: "chart"`) puts the edit back, and `error` fires with `reason: "result"` and a message that names the mistake.
- **Current limit of server keys (innerstacklabs/tachlon#139):** when the server answers a restored row with another key, the chart holds it under the new key, and your next rows list must name it by that key. `setData` answers `rowsChanged` for a row whose key the chart was not given. A queued commit that names the row in `parent`, `from` or `to` follows it to the new key under your own field names (`fields`).

## Stop the chart

`chart.destroy()` stops the frame loop, frees the engine, and empties the element. Call it when the page removes the chart. It is safe to call twice; every other method throws once it has run, except `chart.commands`, whose calls return `undefined`.

## The engine file

The script loads `app_bg.wasm` from the address beside itself. Serve it with status 200 as `application/wasm`. To serve it from elsewhere, pass `wasmUrl` to `mount`. Under a Content-Security-Policy, add `'wasm-unsafe-eval'` to `script-src`.

To start loading it before the chart mounts, call `preloadEngine()` (or `preloadEngine({ wasmUrl })`) first thing, so the file downloads while your page reads its data; `mount` then joins that load instead of fetching the file again.

## License

Copyright (c) 2026 Inner Stack Labs LLC. Tachlon is open source under the MIT license (the package's `license` field is `MIT`): use, copy, modify and embed it in any product, free of charge. It is provided as is, with no warranty. The full text is the repository's `LICENSE` file.

## Types and API reference

The TypeScript declarations (`dist-lib/types`, the package's `types` entry) are the reference: they cover the whole API, every exported member carries its documentation comment, and your editor shows it on hover.

To see what the installed version offers, run `npx tachlon describe`: it answers from the package in `node_modules`, so it always matches the version you installed.

### What an answer from `save` means

The chart reads each throw of `save` or of a handler, and each `Response` it resolves with, by its status: the first number from 100 to 599 in a `Response`'s `status`, else in a thrown error's `status`, `response.status` or `statusCode`. Any other number, 0 included, is no status.

| Status | Resolved | Thrown |
|---|---|---|
| 2xx | saved | a failure: an error that says 200 is not a save |
| 401 | the queue stops | the same |
| 408, 429, 5xx | a failure: the queue stops | the same |
| any other, a 403 included | a refusal | the same |

A thrown `SaveRefused` is a refusal whatever else it carries. For any other throw, a handlers object's `classify(error)` is asked next: a number from 100 to 599 is judged by the table, `"refused"`, `"stopped"` and `"failed"` are those outcomes, and `undefined` leaves the error to the table. After `classify` returns a number outside that range, the chart goes to the last step: no status is a failure. Any other answer, a word outside those three or a non-number, is a failure too, and a `classify` that throws stops the queue with its own error. A GraphQL client that throws with a 200 needs `classify`, because a thrown 2xx is a failure: return `"refused"` for the errors the server judged.

A refusal puts the edit and every edit behind it back. Its message is the `SaveRefused`'s, a thrown error's `message` or the `message` of a response's JSON body, else "The server refused the change" and the status; `key` and `field` come from the `SaveRefused` or the body, and in the handlers form a refusal with no `key` names the row of the change whose handler was refused, when that change is a task's. The chart selects the task `key` names (in a `save` function the chart cannot tell which change was refused, so `key` names a task) and shows the message at the cell the `field` names, in your name for it, until the person's next edit or until you start the chart again with new data (a changed `tasks` prop in React does that today), which wipes it. A refused link selects no task, whatever key its refusal names. Every row whose edit is not saved yet is dimmed until its call settles, apart from the rows you marked with `setPending`. A failure or a stop leaves the edits unsaved and stops the queue; repeating and asking the person to sign in again are not in the chart yet.
