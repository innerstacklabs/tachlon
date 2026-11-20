// The chart's public types (gs#534): what `mount` takes and what it returns, moved out of `index.ts` so the entry stays under its line cap.
// `index.ts` re-exports both; each is a row under "Page entry" in docs/WASM_API.md.
import type { PlainCalendar, PlainLink, PlainTask } from "./adapters/plain.js";
import type { Constraint } from "./constraint-form.js";
import type { LinkField, TaskField } from "./field-map.js";
import type { ScheduleOptions } from "./adapters/dhtmlx.js";
import type { Send } from "./assistant/loop.js";
import type { PageSize } from "./api.js";
import type { Commands } from "./commands.js";
import type { Refusal } from "./edit-result.js";
import type { ChecksReport, ExportResult, LoadResult, LoadSource } from "./document.js";
import type { Change, CommitSource, HasOn } from "./events.js";
import type { OpenOptions } from "./open-schedule.js";
import type { Speed } from "./speed.js";
import type { FilterInputs, ViewState } from "./view-state.js";

/** What the before-edit function is asked: the action (`kind`), what made it, the session slots of the tasks it touches and the changes it would make, as the `edit` event writes them. A change whose value is not known yet has no `new`. */
export interface BeforeEditChange { kind: string; source: CommitSource; tasks: readonly number[]; changes: readonly Change[] }

/** The answer: `undefined`, `null` or `true` allows the change; a string refuses it with that reason, which the chart shows; `false` refuses it with the chart's own sentence. Answer at once: a promise, a throw or any other value refuses the change and fires `error` with the source `beforeEdit`. Never call the chart from inside it. */
export type BeforeEdit = (change: BeforeEditChange) => string | boolean | null | undefined;

/** What `update` sets on one task: its `key`, and any of `name`, `start` (a date, or `null` for none; held as `snet` unless the record has one), `duration` (in `durationUnit`), `snet`, `snlt` (start no later than), `fnet` and `fnlt` (finish no earlier and no later than; a date alone is the end of the last working minute of that day on the task's calendar, and on `snet` and `snlt` the first working minute) (each a date, or `null` for none) and `progress` (a whole percent). `constraint` (`{ type, date }`, or `null` for none) sets the one date limit and clears the rest of `snet`, `snlt`, `fnet` and `fnlt` (`so` sets `snet` and `snlt` to its date, so it clears two; `fo` sets `fnet` and `fnlt`; any other type sets one and clears three); a record that gives `constraint` and one of the six members `snet`, `snlt`, `fnet`, `fnlt`, `mso` or `mfo` is refused, and a `start` beside `constraint` is ignored. A member left out keeps its value. `hostStart` and `hostFinish` set nothing: they are the early start and finish your own code computed (`YYYY-MM-DD`, or `YYYY-MM-DDTHH:mm`; a finish that is a date alone is the end of that day), and the answer's `differs` names each the chart computed otherwise. */
export interface TaskUpdate { key: string; name?: string; start?: string | null; duration?: number; snet?: string | null; snlt?: string | null; fnet?: string | null; fnlt?: string | null; constraint?: Constraint; progress?: number; hostStart?: string; hostFinish?: string }

/** One date a host sent in `hostStart` or `hostFinish` that the chart computed otherwise: the `record` (from 0) and its `key`, the `member` (`"start"` or `"finish"`), the `host` date as sent, and the `chart` date written in the same form (a day when the host sent a day). */
export interface HostDifference { record: number; key: string; member: "start" | "finish"; host: string; chart: string }

/** What `update` answers: how many tasks it set, with `differs` when a host date differs from the chart's, or why it changed none (`reason`, `message`, and `record` from 0, `member` and the record's `key` when they are known). */
export type UpdateResult = { ok: true; updated: number; differs?: readonly HostDifference[] } | (Refusal & { key?: string });

/** One undo step as a host's save function gets it (decision 0022, rule 2.1): `id` is a UUID that is new for each change set and the same on every call of it, `seq` and `source` are the commit's, and each change is the `edit` event's without the session slots, with `once`, its idempotency key `<id>:<n>` (`n` from 0). */
export interface ChangeSet { id: string; seq: number; source: CommitSource; changes: readonly (Omit<Change, "task" | "link"> & { once: string; ref?: string })[] }

/** What a save function may resolve with: with `keys: "server"`, one `{ ref, key }` for each create that carries a `ref` (a mount with `keys: "chart"` puts back a result that has `keys`). Otherwise a function that resolves has saved the change set. */
export interface SaveResult { keys?: readonly { ref: string; key: string }[] }

/** The host's save function: it gets each change set the person's edits make, one at a time, in the order made, and resolves when the server has it. Throw a `SaveRefused` when the server judged the change and said no, or let a 4xx other than 401, 408 and 429 reach the chart (a `Response` or an error with its `status`): the chart puts that edit and the ones made after it back and fires `error` with the source `save`. */
export type Save = (changeSet: ChangeSet) => Promise<SaveResult | Response | void>;

type Row = Record<string, unknown>;
/** The last argument of every handler: `once` is the change's idempotency key, `signal` aborts 10 seconds after the handler is called (the chart still waits for it to settle), `was` the old values of an update or a delete, `changeSet` the whole commit. */
export interface SaveContext { once: string; signal: AbortSignal; was?: Row; changeSet: ChangeSet }

/**
 * `save` as one handler for each kind of change, for a host that already has an API client. The chart calls them in the order the person made the changes, each after the one before has resolved, one call into the host's client per change.
 * `row` and `set` are in the host's names. A handler that rejects, or resolves with a `Response` whose status is not a 2xx, ends the call; a `SaveRefused` it throws puts the edit back, as from a `Save` function.
 * `classify` is for a client whose errors carry no status, or a status that says the wrong thing (a GraphQL client that throws with a 200): it names what a throw that is not a `SaveRefused` means, and the chart asks it before it reads the status.
 */
export interface Handlers {
  createTask?(row: Row, c: SaveContext): Promise<unknown>;
  updateTask?(key: string, set: Row, c: SaveContext): Promise<unknown>;
  deleteTask?(key: string, c: SaveContext): Promise<unknown>;
  createLink?(row: Row, c: SaveContext): Promise<unknown>;
  updateLink?(key: string, set: Row, c: SaveContext): Promise<unknown>;
  deleteLink?(key: string, c: SaveContext): Promise<unknown>;
  updateSchedule?(set: Row, c: SaveContext): Promise<unknown>;
  classify?(error: unknown): number | "refused" | "stopped" | "failed" | undefined;
}

/** The `error` event of a put-back, fired once, after it, and of a queue that stops (`reason: "stopped"`: an answer that is neither a success nor a refusal (a 5xx, a 401, a throw with no status), so no call is made again until the chart is mounted again; `message` is the thrown error's, `held` the commits it holds, which stay unsaved and in `ViewState.unsavedCommits`; `undone` is 0). `calls`: how many times `save` ran for the change set; `key` and `field` are the refusal's; `seq`: the refused commit's; `undone`: how many commits the chart put back, a `hostEdit` made after the refused one too (send it again); `reload`: the chart could not put itself back (the commit had left its history), so the host reads its data again. */
export interface SaveError { source: "save"; message: string; fatal: false; reason: "refused" | "result" | "stopped"; calls: number; key?: string; field?: string; seq: number; undone: number; reload: boolean; held?: number }

/** What a host's database can take, as the host tells the chart (`mount`'s `capabilities`, `Gantt.setCapabilities`). Each member is on unless it is `false`. A change a person makes that a member turned off is refused with `"notAllowed"` on every path, undo and redo included, and the chart says so in the sentence of the member's `texts` key (`notAllowedBarMove` for moving a bar); a bar, edge or link drag says it in the drag note. `gestures` turns off moving a bar, stretching it and drawing a link. `editable` turns off a group of fields: `rows` (adding and deleting rows), `outline` (indent, outdent and move), `paste`, `names`, `dates` (start, duration and the date limits), `links`, `progress` (actual dates, remaining, percent and the data date) and `baseline`. What the database stores: `kinds` lists the date limits it takes (all nine when left out; `so` and `fo` are a floor and a ceiling on one instant, and a task that holds both on one instant counts as that kind, unless `kinds` leaves it out), `perTask` is the most it takes for one task (a start no earlier than counts), `durationStep` and `lagStep` say a duration or lag is a whole number of minutes, hours or days (a day is the length of that task's calendar day), and `dates: "day"` says a date has no time of day (a start at its calendar's opening and a finish at its closing count as none). A change outside them is refused with `"notAllowed"` and says what the host stores; a bar drag that would make one is refused. A member the chart does not have is an error. */
export interface HostCapabilities {
  gestures?: { barMove?: boolean; resize?: boolean; link?: boolean };
  editable?: { rows?: boolean; outline?: boolean; paste?: boolean; names?: boolean; dates?: boolean; links?: boolean; progress?: boolean; baseline?: boolean };
  kinds?: readonly ("snet" | "snlt" | "fnet" | "fnlt" | "mso" | "mfo" | "alap" | "so" | "fo")[];
  perTask?: number;
  durationStep?: "minutes" | "hours" | "days";
  lagStep?: "minutes" | "hours" | "days";
  dates?: "minute" | "day";
}

/** The host's names for the members of its rows (`MountOptions.fields`): each entry gives one chart field (the names `PlainTask` and `PlainLink` list, with `key`) the host's name for it, `{ name: "title", start: "plannedStart", duration: "days", key: "id" }`. A field with no entry keeps its own name, and a name that is not a chart field does not compile. */
export interface Fields { tasks?: Partial<Record<Exclude<TaskField, "constraint">, string>> & { constraint?: string | { type: string; date: string } }; links?: Partial<Record<LinkField, string>> }

/** The rows a call takes: with no map (`F` is `undefined`, the default) the type `Plain`, as before `fields`; with a map, the host's own rows, which the chart reads by the names the map gives (`readonly object[]`: the host's row type is its own and needs no cast). */
export type Rows<Plain, F extends Fields | undefined> = [F] extends [undefined] ? Plain : readonly object[];

/** What `mount` takes: a `schedule`, a saved `document`, or `tasks` and `links` as `fromTasks` reads them; with none of the three the chart starts with no tasks and waits for `load`. `F` is the type of `fields`: leave it out (no map) and every member is as it was, give a map and `tasks`, `links`, `update` and `setData` take the host's own rows. */
export interface MountOptions<F extends Fields | undefined = undefined> {
  /** A schedule file to read (XER or MS Project XML, every project of it, or a `.gantt` document), or the demo or the stress fixture at `n` tasks. */
  schedule?: File | { demo: number } | { stress: number };
  /** A saved schedule document, the JSON another chart's `toDocument()` returned, to start on in one call instead of `load({ document })` after an empty start. Read when there is no `schedule`, and it wins over `tasks`. A document that cannot be read rejects `mount` with a `MountError`. The chart keeps it no longer than it keeps `tasks`. */
  document?: unknown;
  /** Plain tasks to draw when there is no `schedule` and no `document`, and the links between them (`fromTasks`). The chart keeps neither list once it has started; it holds the schedule they made. With no `schedule`, `document` or `tasks` the chart opens with no tasks and waits for `load` or `open`. */
  tasks?: Rows<PlainTask[], F>;
  /** The links between the plain `tasks` (`fromTasks`). */
  links?: Rows<PlainLink[], F>;
  /** The day the plain `tasks` are as of (`YYYY-MM-DD[THH:mm]`, 00:00 of the day when it is a date alone): work that has not started cannot start before it. Read with `tasks`, and kept no longer than they are. */
  dataDate?: string;
  /** The date the project must finish by, for the plain `tasks` (`YYYY-MM-DD[THH:mm]`, the end of that day when it is a date alone), as `fromTasks` reads it: float is measured against the earlier of it and the computed finish, as in the headless results. Read with `tasks`, and kept no longer than they are. */
  mustFinishBy?: string;
  /** The schedule options of decision 0018 (`startedLateStart`, `unstartedRemaining`) for the plain `tasks`, as `fromTasks` reads them. Read with `tasks`, and ignored when there is a `schedule` or a `document`: each carries its own. */
  options?: ScheduleOptions;
  /** The working calendars of the plain `tasks` (`PlainCalendar`): a task's `calendar` names one by `key`, and the first is every other task's. Without them the one Monday to Friday calendar, 08:00 to 17:00 with an hour at noon, is used. Read with `tasks`, and ignored when there is a `schedule` or a `document`. */
  calendars?: PlainCalendar[];
  /** What `duration`, `remaining` and `lag` are counted in, in the plain `tasks`, in `update`, and in each change the chart tells you of (an `edit` event, a `save` change set, a before-edit call): `"days"` (default; a day is the working day of the calendar that measures the value), `"hours"` or `"minutes"`. For `tasks` it is read with them and kept no longer than they are; for `update` and the changes it holds for the life of the chart, whatever the schedule was read from.
   * @defaultValue `"days"` */
  durationUnit?: "days" | "hours" | "minutes";
  /** The form of every date the chart tells you of in an `edit` event, a `save` change set and a before-edit call: `"minute"` (default) is `YYYY-MM-DDTHH:mm`, `"day"` is `YYYY-MM-DD`, a finish as the last day worked (the day of its instant less one minute). The durations and lags there are in `durationUnit`, whatever the schedule was read from.
   * @defaultValue `"minute"` */
  dateForm?: "minute" | "day";
  /** The spelling of a task's date limit in an `edit` event, a `save` change set and a before-edit call: `"typed"` is one member, `constraint: { type, date }` or `null` (`type` is `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`, `so` or `fo`; `so` is `snet` and `snlt` on one date, `fo` is `fnet` and `fnlt` on one date), and `"separate"` is the six members `snet`, `snlt`, `fnet`, `fnlt`, `mso` and `mfo`. `tasks`, `update` and `setData` take both spellings, and a task that gives `constraint` and one of the six is refused. `fields` may map `constraint` (to one member, or to a type member and a date member) only under `"typed"`; `mount` rejects it under `"separate"`.
   * @defaultValue `"typed"` */
  constraintForm?: "typed" | "separate";
  /** The calendar a lag day is counted on for the plain `tasks`: `"successor"` (default), `"predecessor"`, `"24h"` or `{ calendar: key }`. Read with `tasks`, and kept no longer than they are.
   * @defaultValue `"successor"` */
  lagCalendar?: "successor" | "predecessor" | "24h" | { calendar: string };
  /** Draw with WebGL2 and never try WebGPU. Without it a WebGPU failure switches the chart to WebGL2 in its root, and `on("error")` says so.
   * @defaultValue `false` */
  forceWebgl?: boolean;
  /** Start the chart read-only (`Gantt.setReadOnly`). Without it the chart can be edited.
   * @defaultValue `false` */
  readOnly?: boolean;
  /** What the host's database can take (`HostCapabilities`, `Gantt.setCapabilities`). Without it the chart allows every kind of change. A member the chart does not have rejects `mount`. */
  capabilities?: HostCapabilities;
  /** A function the chart asks before each change a person makes; it can refuse the change with a reason the chart shows (README, "Control editing"). Without it every change goes ahead. */
  beforeEdit?: BeforeEdit;
  /** The host's save function (`Save`), or an object of one handler for each kind of change (`Handlers`): the chart calls it with each commit's change set, one call at a time, in the order the commits were made, and puts an edit back when it throws a `SaveRefused`. Without it the chart saves nothing and the host reads the `edit` event. React, Vue and Angular ask the latest function or object when a change set is sent, never mount the chart again for a new one, and send nothing while there is none. */
  save?: Save | Handlers;
  /** The host's names for the members of its rows (`Fields`), given once: the chart reads `tasks`, `links`, `update` and `setData` in them and writes them in the `set` and `was` of every change it tells you of (an `edit` event, a `save` change set, a before-edit call), so the host writes no rename function. It renames members and converts no value (`durationUnit` and `dateForm` do that). `mount` rejects an entry for a field the chart does not have, one host name given to two fields, and a host name that is another field's own name with no entry (`key: "id"` is the exception). When both `key` and `id` have an entry, the member you name for `id` is the row's id, and the member for `key` fills it only for a row that has none. Its type `F` is inferred from the map you give: with none, `tasks`, `links`, `update` and `setData` are typed as before; with one, they take `readonly object[]`, your own row type, and only the names in the map are checked. Read once, at start: a later change does nothing. A member of a row that the chart has no field for (once the map is applied) is a custom field: the chart keeps it with the row, by its text `key`, and never reads, shows or edits it; a create's `set` and a delete's `was` give it back under the same name, so the Undo of a delete restores your own columns, and `state().customFields` lists the names it has seen. */
  fields?: F;
  /** Where to fetch the engine file (`app_bg.wasm`) from, a URL or an address read against the page's own. Without it the chart loads the file beside its script (`dist-lib/app_bg.wasm`). When `preloadEngine` already started the load, its `wasmUrl` is the one used. */
  wasmUrl?: string | URL;
  /** The language and region the chart runs in, a BCP 47 tag such as `"de"` or `"pt-BR"`: it is written as the root's `lang`, and the chart's numbers show in that region's style (`1.234,5` for `"de"`). Without it the numbers read as `"en-US"` and the root's `lang` stays the page's own. A tag that is not a language tag rejects `mount` with a `RangeError`, before it touches the root. */
  locale?: string;
  /** `fonts` is a mount option; the text engine adds those faces, reshapes its labels, and takes its locale tag from `locale`, so a chart can draw scripts the bundled faces lack. */
  fonts?: ReadonlyArray<ArrayBuffer | Uint8Array>;
  /** The chart's own words in the host's language: the text, `title`, `aria-label` and `placeholder` of its buttons, labels and groups, by key (`packages/gantt/src/texts-en.ts` lists the keys and the English). The sentences the chart writes while it runs are in it too, as templates with `{name}` holes and, for a count, one key per plural form of `locale` (`bytes.one`, `bytes.few`). A key the object leaves out stays English; a key the chart has no use for is ignored. With `locale`, the keys of `App.default_texts` (`alreadyLinked`, and `notAllowedBarMove` for a gesture your `capabilities` turned off) also word its refusals. */
  texts?: Readonly<Record<string, string>>;
  /** Write the assistant's chat panel into `into`, an element of the host's own (the panel is styled as the chart is, from a `.gantt` element at or above `into`, or its own where there is none): each message sent runs a turn that calls `send`, the host's call to its own model (the whole conversation in, the model's next reply out), so the page sends the conversation nowhere else. Without it the page has no panel and sends nothing. A switch to WebGL2 starts the panel again with an empty history. */
  assistant?: { send: Send; into: HTMLElement };
  /** Who makes the key of a row the person creates, when `save` is set: `"chart"` (default) makes it before the change set is written, with `newKey` or `crypto.randomUUID()`, and the create carries it, so the server stores it; `"server"` leaves it to the server, and the create carries `key: null` and a `ref` that the answer (the handler's result, or `SaveResult.keys`) names. Read once, at start.
   * @defaultValue `"chart"` */
  keys?: "chart" | "server";
  /** The function that makes the key of a new row under `keys: "chart"`, a text no other row of its kind holds. Without it the chart uses `crypto.randomUUID()`. Read once, at start. */
  newKey?: () => string;
}

/** What `mount` returns: the chart, as a host page may drive it. */
export interface Gantt<F extends Fields | undefined = undefined> extends HasOn {
  /**
   * Replace the schedule with `file`. Resolves when the attempt has ended; a file that cannot be opened is an `error` event (source `"open"`), not a throw.
   * `expectedNotes` are the beginnings of import notes the host knows its own file will report (a generated sample): a line that
   * starts with one is left out of `ViewState.importNotes`. `window.gantt.unsupported` still holds every line the reader reported.
   * A file with several projects stops at their list, which the `error` event carries as `projects`; `project` (an id from that list, or `"all"`) opens it.
   */
  open(file: File, options?: OpenOptions): Promise<void>;
  /**
   * Download the open schedule as a schedule document (gzipped JSON), named after the opened file with `.gantt`, `untitled.gantt`
   * for a sample. Never writes over the opened file. Resolves once the download has started; it does not wait for the file to be
   * written. It clears the unsaved mark beside the file name, as of the schedule when the save began. The write runs on the main thread.
   */
  download(): Promise<void>;
  /** Download the visible list as `<name>.csv` (`untitled.csv` for a sample), the rows and columns Ctrl+C copies, as the hosted file menu's "Export the list as CSV". UTF-8 with a byte order mark; the note at the bottom right then says how many rows it wrote. */
  exportCsv(): void;
  /** Download the visible list as `<name>.xlsx` (`untitled.xlsx` for a sample), the rows and columns `exportCsv` writes, with the dates as Excel dates; the note at the bottom right then says how many rows it wrote. Resolves once the download has started; rejects once the chart is destroyed. */
  exportXlsx(): Promise<void>;
  /** Put the visible list on the clipboard as tab-separated text (`App.list_text`: the rows and columns `exportCsv` writes), as the hosted file menu's "Copy the list". Resolves once the clipboard has it and the note at the bottom right says how many rows; rejects when the browser refuses the write. */
  copyList(): Promise<void>;
  /** Download a picture of printed page `page` (1-based, the first by default) as `<name>.png` (`untitled.png` for a sample), or `<name>-page-<n>.png` past the first: 2112 by 1632 pixels, drawn from `App.print_pages` without the chart's canvas. A page below 1, past the last or not a whole number rejects. Resolves once the download has started. */
  exportImage(options?: { page?: number }): Promise<void>;
  /** How many pages print (what Ctrl+P prints, and the last page `exportImage` takes), so a host exports every page without calling until one rejects. Counts the pages as the print lays them out now. */
  printedPages(): number;
  /** Open the browser's print dialog for the pages on `"letter"` (what Ctrl+P prints) or `"a4"` paper, for this one print: the next print is Letter again. */
  printOn(size: PageSize): void;
  /**
   * Replace the schedule with a document (the JSON `toDocument` returns) or a file (XER, MS Project XML, or a gzipped document). Resolves
   * `{ ok: true }`, or `{ ok: false, error }` with the schedule as it was: a bad source is a result, never a throw. Unsaved edits are
   * replaced without asking.
   */
  load(source: LoadSource): Promise<LoadResult>;
  /**
   * Replace the schedule with a new one that has no tasks and starts on this week's Monday, named `untitled`. With unsaved changes it asks first, as `open`
   * does; Cancel keeps the schedule and resolves `{ ok: false, error }`, as does a cell edit or a drag under way.
   */
  newSchedule(): Promise<LoadResult>;
  /** The open schedule as a document, the JSON that `download` writes (gzipped) and `load({ document })` reads. Reads the session and changes nothing. */
  toDocument(): unknown;
  /** The open schedule as plain `tasks` and `links` (`fromTasks` reads them back) and `leftOut`, one line for each thing plain data cannot carry. Each task's `start` is the one the chart has solved now, so it can differ from the task's own start limit; `toDocument` stays the lossless save. Reads the session and changes nothing; it throws while the chart switches to WebGL2. */
  snapshot(): { tasks: PlainTask[]; links: PlainLink[]; leftOut: string[]; dataDate?: string; mustFinishBy?: string; options?: ScheduleOptions };
  /** The open schedule as a P6 XER file: `bytes` and `leftOut`, one line for each thing the file cannot carry. Throws when a date cannot be written. */
  exportXer(): ExportResult;
  /** The open schedule as an MS Project XML file, as `exportXer` returns it. */
  exportMspdi(): ExportResult;
  /** The schedule-quality counts of the open schedule, as the Checks panel lists them: no pass or fail, just the counts. Reads the session and changes nothing; ask it when the panel opens, not every frame. */
  checks(): ChecksReport;
  /** The backend that is drawing: `"WebGL2"` once a WebGPU failure has switched the chart. */
  backend(): "WebGPU" | "WebGL2";
  /** The view state a toolbar shows: the filters, the lookahead window, the row and task counts, the zoom level, the selection, the move report and the backend. `on("change")` fires when it changes. */
  state(): ViewState;
  /** What the filter controls offer: the activity codes, whether tasks have activity IDs, whether the baseline holds float. Read it again after an `open`, an `edit`, or a `change` whose `baseline` moved. */
  filterInputs(): FilterInputs;
  /** Whether a cell is being edited or a bar or link is being pressed or dragged: the edit commands refuse then (`"busy"`). It has no event; ask it when a command needs to. */
  busy(): boolean;
  /** Switch read-only mode for the person using the chart, at any time and without starting the chart again: while on, the cell editor and the task dialog do not open and every other edit (a bar, edge or link drag, the keys, paste, undo, redo) is refused with `"readOnly"`, and the chart says why; a click still selects a row. It is not an edit, and a switch to WebGL2 keeps it. */
  setReadOnly(on: boolean): void;
  /** Whether the chart is read-only. */
  readOnly(): boolean;
  /** Tell the chart what the host's database can take, in place of the profile it had, at any time and without starting the chart again; `undefined` or `{}` allows every kind of change. It is not an edit, and a switch to WebGL2 keeps it. Throws, changing nothing, for a member the chart does not have. */
  setCapabilities(capabilities: HostCapabilities | undefined): void;
  /** Mark the rows with these keys as waiting for the host's own recalculation, in place of the ones marked before: the chart dims them and tells a screen reader they are busy until the host calls `setPending` again without them (`[]` clears every mark). A hint, not a lock: the rows stay selectable and editable. Answers the keys that name no live task (a deleted task's counts), which it leaves out. The marks go when `open`, `load` or `newSchedule` replaces the schedule, and when a switch to WebGL2 restarts the chart (its `error` event, source `backend`, says so: mark the rows again). */
  setPending(keys: readonly string[]): string[];
  /** Give the chart new values for many tasks, each named by its `key`, in place: the selection, the scroll position and the person's undo list stay as they were, and the call is not an edit (no `edit` event, no undo step, no `beforeEdit` call; decision 0019, rule 13). It is one recompute, or two when a record mixes `progress` with `duration`, `start`, `snet` or a limit. `duration` is in `durationUnit`. A refusal changes nothing: `"badRecords"` (not an array), `"busy"` (a cell is being edited, a bar or link is being dragged, or the chart is switching to WebGL2), or the reason of a record (`EditRefusal`: `unknownKey`, `duplicateKey`, `notUpdatable` for `alap`, a pin other than the one the task holds, and a summary, `badValue`, `badPercent`, `emptyName`, `invalidName`, `actualAfterDataDate`). */
  update(records: Rows<readonly TaskUpdate[], F>): UpdateResult;
  /** Give the chart the changes your page made to tasks outside it, the records `update` takes, as one undo step on the person's own list: one Undo takes back all of them, the next Undo the person's edit before them, and Redo goes the other way. It answers as `update` does, `"hasActuals"` for a record that sets `progress` or changes a task with actual dates, and `"notAllowed"` for one that changes a group of fields the capability profile turned off, or whose old value it does not store (`member` says which; the `message` names the task: call `update` for those, which makes no undo step). Unlike `update` it is an edit: an `edit` event fires with the commit source `"host"`, which `save` never gets (you made the change), and it is taken in a read-only chart, with `beforeEdit` not asked. A call that changes no value adds no step. */
  hostEdit(records: Rows<readonly TaskUpdate[], F>): UpdateResult;
  /** Put an entry on the undo list for something your page did outside the chart, such as assigning a worker in its own panel: `token` names it. An Undo that reaches it changes nothing in the schedule and fires `edit` with a commit whose `source` is `"undo"` and whose `host` is `token`, and your page undoes its own action; Redo does the same with `"redo"`. Like any step it is dropped past 200 entries, and a new step after an Undo takes the redo entries away. The push fires no `edit` of its own: its commit (source `"host"`, no change, never sent to `save`) comes with the next `edit`. It is taken in a read-only chart. `"badToken"` for an empty token. A refused `save` cannot put such an entry back, so the put-back that would cross one is refused and the chart asks for a `snapshot`. */
  pushHostEntry(token: string): { ok: true } | Refusal;
  /** Empty the undo and redo lists, the commits the chart has not seen saved (a `rollbackTo` to one is then refused, and the chart asks for a `snapshot`) and a what-if, as when your page releases its edit lock. It is not an edit: no `edit` event, and `seq` goes on from where it was. */
  clearHistory(): void;
  /** Hand the chart the rows your page holds now, `{ tasks, links }` as `mount` takes them, and it sets in place only what differs from the rows it was last given (`mount`'s, or the last accepted call's): for each task by `key`, the `name`, `start`, `duration`, `progress` and date limits (`constraint`, or `snet`, `snlt`, `fnet`, `fnlt`, `mso`, `mfo`) that changed, as `update` sets them (it refuses an `mso` or `mfo` that is not the pin the chart holds, so the call is refused; a host that stores a date alone rereads a mid-day pin as the day's first (`mso`) or last (`mfo`) working minute, so it differs and is refused). The selection, the scroll position and the undo list stay, the call is not an edit, and the same arrays again do nothing; every other member of a task (`color`, `label`, `end`, `calendar`, `milestone`, `alap`, the actuals, your own) is ignored, a member left out keeps what the chart last had for it, so rows that are old do not undo a saved edit. It answers as `update` answers, all or nothing, or `"rowsChanged"` (nothing changed) when a task or a link was added, removed or changed, a task moved in the list or has another `id` or `parent`, when a task has no text `key`, or when the chart holds no rows of yours to compare with (mounted with a `document`, switched to WebGL2, or opened, loaded or cleared since): start the chart again with the rows. */
  setData(data: { tasks?: Rows<PlainTask[], F>; links?: Rows<PlainLink[], F> }): UpdateResult;
  /** The last open's phases and the last 120 frames' numbers. It calls no app method, so polling it never wakes a sleeping loop. */
  speed(): Speed;
  /**
   * Take the chart down: stop the frame loop, remove what it put on `window`, `document` and the media queries, disconnect its observers,
   * free the WASM app (which lets the GPU device go) and empty the root. Safe to call twice. Every other method throws once it has run
   * (`open` and `download` reject), and a call already waiting on the app, such as an `open` that has not resolved, is not waited for.
   */
  destroy(): void;
  commands: Commands; // The page's own buttons; also the `detail` of a bubbling `gantt:ready` event on the root.
}
