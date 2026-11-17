// The chart's public types (gs#534): what `mount` takes and what it returns, moved out of `index.ts` so the entry stays under its line cap.
// `index.ts` re-exports both; each is a row under "Page entry" in docs/WASM_API.md.
import type { PlainCalendar, PlainLink, PlainTask } from "./adapters/plain.js";
import type { ScheduleOptions } from "./adapters/dhtmlx.js";
import type { Send } from "./assistant/loop.js";
import type { PageSize } from "./api.js";
import type { Commands } from "./commands.js";
import type { ChecksReport, ExportResult, LoadResult, LoadSource } from "./document.js";
import type { HasOn } from "./events.js";
import type { OpenOptions } from "./open-schedule.js";
import type { Speed } from "./speed.js";
import type { FilterInputs, ViewState } from "./view-state.js";

/** What `mount` takes: a `schedule`, a saved `document`, or `tasks` and `links` as `fromTasks` reads them; with none of the three the chart starts with no tasks and waits for `load`. */
export interface MountOptions {
  /** A schedule file to read (XER or MS Project XML, every project of it, or a `.gantt` document), or the demo or the stress fixture at `n` tasks. */
  schedule?: File | { demo: number } | { stress: number };
  /** A saved schedule document, the JSON another chart's `toDocument()` returned, to start on in one call instead of `load({ document })` after an empty start. Read when there is no `schedule`, and it wins over `tasks`. A document that cannot be read rejects `mount` with a `MountError`. The chart keeps it no longer than it keeps `tasks`. */
  document?: unknown;
  /** Plain tasks to draw when there is no `schedule` and no `document`, and the links between them (`fromTasks`). The chart keeps neither list once it has started; it holds the schedule they made. With no `schedule`, `document` or `tasks` the chart opens with no tasks and waits for `load` or `open`. */
  tasks?: PlainTask[];
  links?: PlainLink[];
  /** The day the plain `tasks` are as of (`YYYY-MM-DD[THH:mm]`, 00:00 of the day when it is a date alone): work that has not started cannot start before it. Read with `tasks`, and kept no longer than they are. */
  dataDate?: string;
  /** The date the project must finish by, for the plain `tasks` (`YYYY-MM-DD[THH:mm]`, the end of that day when it is a date alone), as `fromTasks` reads it: float is measured against the earlier of it and the computed finish, as in the headless results. Read with `tasks`, and kept no longer than they are. */
  mustFinishBy?: string;
  /** The schedule options of decision 0018 (`startedLateStart`, `unstartedRemaining`) for the plain `tasks`, as `fromTasks` reads them. Read with `tasks`, and ignored when there is a `schedule` or a `document`: each carries its own. */
  options?: ScheduleOptions;
  /** The working calendars of the plain `tasks` (`PlainCalendar`): a task's `calendar` names one by `key`, and the first is every other task's. Without them the one Monday to Friday calendar, 08:00 to 17:00 with an hour at noon, is used. Read with `tasks`, and ignored when there is a `schedule` or a `document`. */
  calendars?: PlainCalendar[];
  /** What the plain `tasks` count `duration`, `remaining` and `lag` in: `"days"` (default; a day is the working day of the calendar that measures the value), `"hours"` or `"minutes"`. Read with `tasks`, and kept no longer than they are. */
  durationUnit?: "days" | "hours" | "minutes";
  /** The calendar a lag day is counted on for the plain `tasks`: `"successor"` (default), `"predecessor"`, `"24h"` or `{ calendar: key }`. Read with `tasks`, and kept no longer than they are. */
  lagCalendar?: "successor" | "predecessor" | "24h" | { calendar: string };
  /** Draw with WebGL2 and never try WebGPU. Without it a WebGPU failure switches the chart to WebGL2 in its root, and `on("error")` says so. */
  forceWebgl?: boolean;
  /** Start the chart read-only (`Gantt.setReadOnly`). Without it the chart can be edited. */
  readOnly?: boolean;
  /** Where to fetch the engine file (`app_bg.wasm`) from, a URL or an address read against the page's own. Without it the chart loads the file beside its script (`dist-lib/app_bg.wasm`). When `preloadEngine` already started the load, its `wasmUrl` is the one used. */
  wasmUrl?: string | URL;
  /** The language and region the chart runs in, a BCP 47 tag such as `"de"` or `"pt-BR"`: it is written as the root's `lang`, and the chart's numbers show in that region's style (`1.234,5` for `"de"`). Without it the numbers read as `"en-US"` and the root's `lang` stays the page's own. A tag that is not a language tag rejects `mount` with a `RangeError`, before it touches the root. */
  locale?: string;
  /** `fonts` is a mount option; the text engine adds those faces, reshapes its labels, and takes its locale tag from `locale`, so a chart can draw scripts the bundled faces lack. */
  fonts?: ReadonlyArray<ArrayBuffer | Uint8Array>;
  /** The chart's own words in the host's language: the text, `title`, `aria-label` and `placeholder` of its buttons, labels and groups, by key (`packages/gantt/src/texts-en.ts` lists the keys and the English). The sentences the chart writes while it runs are in it too, as templates with `{name}` holes and, for a count, one key per plural form of `locale` (`bytes.one`, `bytes.few`). A key the object leaves out stays English; a key the chart has no use for is ignored. With `locale`, the keys of `App.default_texts` (`alreadyLinked`) also word its refusals. */
  texts?: Readonly<Record<string, string>>;
  /** Write the assistant's chat panel into `into`, an element of the host's own (the panel is styled as the chart is, from a `.gantt` element at or above `into`, or its own where there is none): each message sent runs a turn that calls `send`, the host's call to its own model (the whole conversation in, the model's next reply out), so the page sends the conversation nowhere else. Without it the page has no panel and sends nothing. A switch to WebGL2 starts the panel again with an empty history. */
  assistant?: { send: Send; into: HTMLElement };
}

/** What `mount` returns: the chart, as a host page may drive it. */
export interface Gantt extends HasOn {
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
  save(): Promise<void>;
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
  /** The open schedule as a document, the JSON that `save` writes (gzipped) and `load({ document })` reads. Reads the session and changes nothing. */
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
  /** Mark the rows with these keys as waiting for the host's own recalculation, in place of the ones marked before: the chart dims them and tells a screen reader they are busy until the host calls `setPending` again without them (`[]` clears every mark). A hint, not a lock: the rows stay selectable and editable. Answers the keys that name no live task (a deleted task's counts), which it leaves out. The marks go when `open`, `load` or `newSchedule` replaces the schedule, and when a switch to WebGL2 restarts the chart (its `error` event, source `backend`, says so: mark the rows again). */
  setPending(keys: readonly string[]): string[];
  /** The last open's phases and the last 120 frames' numbers. It calls no app method, so polling it never wakes a sleeping loop. */
  speed(): Speed;
  /**
   * Take the chart down: stop the frame loop, remove what it put on `window`, `document` and the media queries, disconnect its observers,
   * free the WASM app (which lets the GPU device go) and empty the root. Safe to call twice. Every other method throws once it has run
   * (`open` and `save` reject), and a call already waiting on the app, such as an `open` that has not resolved, is not waited for.
   */
  destroy(): void;
  commands: Commands; // The page's own buttons; also the `detail` of a bubbling `gantt:ready` event on the root.
}
