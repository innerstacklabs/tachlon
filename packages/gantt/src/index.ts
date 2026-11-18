// The chart's entry (gs#534): `mount` puts the chart in a root element and returns what
// a host page may call. Everything about the schedule and the drawing lives in the WASM module; this file
// starts it and wires the shell's parts together, each of which is a module beside it. The chart's markup
// (`chart.html`) and its stylesheets (`styles/`) belong to this entry; the page gives only the root. The page that calls
// `mount` today is `main.ts` (the developer page). Every export here is a row under "Page entry" in
// docs/WASM_API.md, and the `entry` architecture check keeps the two lists equal.
import { App } from "./wasm/app.js";
import { engine, load, MountError, renderError, type StartSource } from "./mount-load.js";
import { createGantt } from "./gantt-object.js";
import type { Gantt, MountOptions } from "./gantt-types.js";
import { Metrics } from "./measure.js";
import { speedOf } from "./speed.js";
import { type PageKnobs, type RunFacts, takePageKnobs } from "./page-knobs.js";
import { watchReducedMotion } from "./motion.js";
import { documentName, download, gzip } from "./document.js";
import { createOpenGuard } from "./open-guard.js";
import { applyTexts, numberFormat, setFonts, setLang } from "./locale.js";
import { keepNames } from "./field-names.js";
import { keepMessages, say, sayFor } from "./messages.js";
import { localeNames } from "./names.js";
import { installOpen, type OpenedFile } from "./open-schedule.js";
import { installTestHook } from "./test-hook.js";
import { armInitPanic } from "./test-panic.js";
import { type PublicApp, type Status } from "./api.js";
import { installTheme } from "./theme.js";
import { createApp, reason } from "./boot.js";
import { guardBeforeEdit } from "./before-edit.js";
import { refused } from "./edit-result.js";
import { mountAssistantPanel } from "./assistant/panel.js";
import { installCard } from "./card.js";
import { installCellEdit } from "./cell-edit.js";
import { installCopy } from "./copy.js";
import { installPaste } from "./paste.js";
import { mountSwitching, type Own, type Run, type Started, startTakingDown } from "./webgl-switch.js";
import { installCursor } from "./cursor.js";
import { claimIds, element, nameParts, optional } from "./dom.js";
import { installIdColumn } from "./id-column.js";
import { installFitButton } from "./fit-button.js";
import { installKeys } from "./keys.js";
import { installLegend } from "./legend.js";
import { installPrintSheet } from "./print-sheet.js";
import { installLinkEdit } from "./link-edit.js";
import { startLoop } from "./loop.js";
import { installMoved } from "./moved.js";
import { installSelectionCount } from "./selection-count.js";
import { installProgressToggle } from "./progress-toggle.js";
import { installBaselineMark } from "./variance.js";
import { installVarianceToggle } from "./variance-toggle.js";
import { installColumnDrag } from "./column-drag.js";
import { installColumnsMenu } from "./columns-menu.js";
import { installPointer } from "./pointer.js";
import { type Shell } from "./shell.js";
import { installTaskDialog } from "./task-dialog.js";
import { installUnsaved } from "./unsaved.js";
import { installSplit } from "./split.js";
import { installTable } from "./table.js";
import { installTableAria } from "./table-aria.js";
import { installRowEdit } from "./row-edit.js";
import { installFilterScroll } from "./filter-scroll.js";
import { createViewState, type OpenFacts } from "./view-state.js";
import chartHtml from "./chart.html?raw";
import { PRODUCT_NAME } from "./product.js";
import "./styles/fonts.css";
import "./styles/tokens.css";
import "./styles/chrome.css";
import "./styles/toolbar-fit.css";
import "./styles/table.css";
import "./styles/table-cells.css";
import "./styles/columns.css";
import "./styles/column-order.css";
import "./styles/panels.css";
import "./styles/moved.css";
import "./styles/picker.css";
import "./styles/export.css";
import "./styles/link-edit.css";
import "./styles/legend.css";
import "./styles/print.css";

// The data adapters (gs#1097, tl#1079, gs#1098, tl#1249): `fromDhtmlx`, `fromTasks`, `validate`, `fromBryntum`.
export { fromBryntum, fromDhtmlx, fromTasks, PlainDataError, toPlain, validate } from "./adapters/index.js";
export type { BryntumResult, DhtmlxOptions, DhtmlxResult, PlainCalendar, PlainIssue, PlainLink, PlainOptions, PlainResult, PlainTask, PlainValidation, ScheduleDocument } from "./adapters/index.js";
// The chart's own types (`gantt-types.ts`).
export type { BeforeEdit, BeforeEditChange, Gantt, MountOptions, TaskUpdate, UpdateResult } from "./gantt-types.js";
export type { Commands, ScrollTarget } from "./commands.js";
export type { Change, Commit, CommitSource, GanttEvents } from "./events.js";
export type { OpenOptions, PickerProject } from "./open-schedule.js";
export type { PageSize } from "./api.js";
export type { Speed } from "./speed.js";
export type { AcceptResult, CompareResult, EditRefusal, EditResult, LinkResult, Refusal, RowResult } from "./edit-result.js";
export type { Filter, PlainFilter } from "./set-filter.js";
export type { WhatIfMoved, WhatIfResult } from "./what-if.js";
export type { ExplainLink, ExplainResult } from "./explain.js";
export type { HealthCount, HealthCounts, HealthResult, HealthTask } from "./health.js";
export type { CriticalList, CriticalPathResult } from "./critical-path.js";
export type { ProposedChange, ProposedKey, ProposeResult } from "./propose.js";
// The tool definitions of an AI assistant (gantt#1250): `toolDefinitions`, `callTool`.
export { callTool, toolDefinitions } from "./ai-tools.js";
export type { ToolDefinition, ToolResult } from "./ai-tools.js";
export type { CheckKind, CodeValue, FilterInputs, RowFilterName, StatusKind, ViewState, ZoomLevel } from "./view-state.js";
export { MountError, preloadEngine } from "./mount-load.js";
// The product's name (packages/gantt/src/product.ts), for a host page that shows it.
export { PRODUCT_NAME };

/**
 * Start the chart in `root`: the page gives a `div` with the class `gantt` and a box with a height, and the chart
 * puts its own elements inside it, with ids of their own, so a page may hold several charts; its stylesheets come
 * with this module. Rejects with a `MountError` (a `code`, a `fix` and `docs` when the cause is known) when the engine cannot load, the schedule cannot be read or the first frame cannot be set up; any
 * other failure is a bug and is thrown as it is. A WebGPU failure, at the start or in a later frame, starts the
 * chart again on WebGL2 in the same root with the same schedule (`webgl-switch.ts`); the page is never reloaded.
 */
export async function mount(root: HTMLDivElement, options: MountOptions): Promise<Gantt> {
  numberFormat(options.locale); // a tag that is not a language tag throws here, before the chart touches the root
  // The developer page's knobs (`page-knobs.ts`, not an export of this entry); a host page sets none.
  const page = takePageKnobs();
  const restart = page === undefined ? undefined : (why: string) => page.restart(why);
  // A measured run starts its script again on the WebGL2 start, as the reload it replaces did, so it starts on the page's own schedule:
  // the document of the one its script had edited has no stress head task, which the drag phase needs.
  const measured = page?.measure != null;
  // The page's own `document`, or its `tasks` and `links`, are held only until the chart has started: a restart on WebGL2 reads the running chart's schedule
  // (`webgl-switch.ts`), so a large list is not kept alive for it. No closure below holds `options`, only the fields it needs.
  let plain: StartSource = options.document === undefined ? { tasks: options.tasks, links: options.links, dataDate: options.dataDate, mustFinishBy: options.mustFinishBy, options: options.options, calendars: options.calendars, durationUnit: options.durationUnit, lagCalendar: options.lagCalendar } : { document: options.document };
  const { schedule, wasmUrl, forceWebgl, locale, texts, fonts, assistant, durationUnit } = options;
  const start = (source: StartSource, run: Run): Promise<Started> => {
    const again = measured && schedule !== undefined && source !== undefined && "document" in source;
    const from = again ? schedule : (source ?? plain);
    if (from === undefined) return Promise.reject(new MountError("schedule", "The chart could not read its schedule to start again: the page's tasks and links were let go once it had started."));
    return startTakingDown((own) => startIn(root, page, from, again ? { ...run, unsaved: false } : run, own, wasmUrl, locale, texts, fonts, assistant, durationUnit));
  };
  const gantt = await mountSwitching(root, { schedule, forceWebgl, readOnly: options.readOnly, beforeEdit: options.beforeEdit }, start, page?.fallback ?? null, restart);
  plain = undefined;
  return gantt;
}

/** One start of the chart in `root`, on `source`, as `run` says; `own` holds what `destroy` takes down, and its app once made. */
async function startIn(root: HTMLDivElement, page: PageKnobs | undefined, source: NonNullable<StartSource>, run: Run, own: Own, wasmUrl: string | URL | undefined, locale: string | undefined, texts: MountOptions["texts"], fonts: MountOptions["fonts"], assistant: MountOptions["assistant"], unit: MountOptions["durationUnit"]): Promise<Started> { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { disposers } = own;
  root.innerHTML = chartHtml;
  applyTexts(root, texts);
  keepNames(root, texts);
  keepMessages(root, texts, locale);
  // The chart's number, given back by `destroy`, or by `start` when this start throws (a retry in a fresh root is then the page's first
  // chart again; the markup left in this root keeps its ids until the host empties or removes the root).
  disposers.push(claimIds(root));
  nameParts(root, root);
  const brand = root.querySelector<HTMLElement>(".tb-brand");
  if (brand !== null) brand.textContent = PRODUCT_NAME;
  root.setAttribute("role", "group");
  root.setAttribute("aria-label", "Gantt chart");
  setLang(root, locale);
  const canvas = element<HTMLCanvasElement>(root, "timeline");
  const overlay = element<HTMLDivElement>(root, "overlay");
  const tableRows = element<HTMLDivElement>(root, "table-rows");
  // The status line is the developer page's (`page-knobs.ts`); a host page has none.
  const line = page?.statusLine;
  const statusLine = line === undefined ? null : line.place(canvas);
  if (statusLine !== null) nameParts(root, statusLine);
  const { forceWebgl, fallback } = run;

  const wasm = await engine(wasmUrl);
  // The benchmark driver splits load here: the engine's code is loaded and can run (demos/bench/side.ts).
  performance.mark("library-ready");
  const started = performance.now();
  const { schedule, unsupported } = await load(source, page?.groups);
  const box = canvas.getBoundingClientRect();
  let app: PublicApp;
  try {
    if (page !== undefined && run.first) armInitPanic(page.params);
    app = await createApp(canvas, box, forceWebgl, schedule);
  } catch (err) {
    throw await renderError(reason(err), err);
  }
  own.app = app;
  app.set_read_only(run.readOnly());
  if (run.beforeEdit !== undefined) app.set_before_edit(guardBeforeEdit(run.beforeEdit, (message) => run.events.fail("beforeEdit", message)));
  const themeTokens = JSON.parse(App.theme_tokens()) as string[];
  const { sync: syncTheme, stop: stopTheme } = installTheme(root, app, themeTokens);
  disposers.push(stopTheme);
  disposers.push(watchReducedMotion((reduced) => app.set_reduced_motion(reduced)));
  // The host's words the app has a sentence for go with the locale names; the page's own keys stay with the page (`App.set_locale` refuses a key it does not know).
  const sentences = JSON.parse(App.default_texts()) as Record<string, string>;
  if (locale !== undefined) app.set_locale(localeNames(locale, Object.fromEntries(Object.entries(texts ?? {}).filter(([key]) => key in sentences))));
  setFonts(app, locale, fonts);
  const info: RunFacts = { loadMs: performance.now() - started, unsupported, open: null, fallback, tableRows };
  const status = (): Status => JSON.parse(app.status()) as Status;
  const fmt = numberFormat(locale);
  console.info(`gantt: backend ${app.backend()}, ${app.task_count()} tasks, ready in ${info.loadMs.toFixed(0)} ms`);
  if (fallback) console.warn(`gantt: running on WebGL2 because WebGPU failed: ${fallback}`);

  // What the schedule is called: the opened file's name without its extension, which Save writes into the document and names the download by.
  const docName = "document" in source ? (source.document as { name?: unknown } | null)?.name : undefined;
  let name = typeof docName === "string" && docName !== "" ? docName : documentName(source instanceof File ? source.name : undefined);
  // The note a save leaves, bottom right: hidden by the next edit (the save mark's callback) or the next open.
  const saveNote = optional<HTMLDivElement>(root, "save-note");
  const note = (text: string): void => {
    if (saveNote === null) return;
    saveNote.textContent = text;
    saveNote.hidden = false;
  };
  const save = async (): Promise<void> => {
    const at = status();
    const bytes = await gzip(app.save(name));
    download(`${name}.gantt`, bytes);
    saveState.saved(at);
    note(say(root, "saved", { file: `${name}.gantt` }));
  };

  const shell: Shell = { root, canvas, app, status, fmt, disposers };
  installCopy(shell);
  const printOn = installPrintSheet(shell);
  installLegend(shell);
  // The cell editor and the task dialog ask this before they open: read-only shows its sentence in the alert line and they stay shut.
  const refuseReadOnly = (): boolean => app.read_only() && (rows.refuse(refused("readOnly").message), true);
  const taskDialog = installTaskDialog(shell, refuseReadOnly);
  const edit = installCellEdit(shell, () => table.invalidate(), (task, field, move) => cursor.moved(task, field, move), taskDialog.open, refuseReadOnly);
  const rows = installRowEdit(shell, () => cursor.toName());
  installPaste(shell, rows.refuse);
  const idColumn = installIdColumn(shell);
  const links = installLinkEdit(shell, rows);
  const cursor = installCursor(shell, edit, idColumn.on, links.active);
  const split = installSplit(shell);
  // Two readers: the loop's, whose last state is what the last `change` carried, and the host's `state()`. A host's read
  // between frames must not move the state the event compares against, or an event could carry a state equal to the last sent.
  // What the open file shows a host: `adopt` sets the name and notes, the save mark's callback the unsaved flag.
  // A mount with a File starts on it; the notes are what the panel lists from the first frame.
  const open: OpenFacts = { fileName: source instanceof File ? source.name : null, unsaved: false, importNotes: Object.freeze([...(unsupported ?? [])]) };
  const openFacts = (s: Status): OpenFacts => (saveState.sync(s), open);
  const view = createViewState(app, openFacts);
  const hostView = createViewState(app, openFacts);
  // The events outlive this start (the switch to WebGL2 starts another): they read this start's view state, commits and task keys from now on.
  run.reads({ view: (s) => view.read(s), commits: () => app.take_commits(), ref: (task) => app.task_ref(task) });
  const events = run.events;
  installKeys(shell, events.request, rows, cursor, () => card, split, links, save, taskDialog);
  const pointer = installPointer(shell, edit.active, (at) => metrics.moved(at));
  // Turning a filter on scrolls the chart to the rows it shows; the chart reads its own drawn bars for that. The rest of the strip is the app's chrome (`apps/chrome`).
  const filterScroll = installFilterScroll(app);
  installFitButton(shell);
  installVarianceToggle(shell);
  installColumnsMenu(shell);
  installColumnDrag(shell);
  const baselineMark = installBaselineMark(shell, (s) => view.read(s));
  const s0 = status();
  const saveState = installUnsaved(s0, (unsaved) => {
    open.unsaved = unsaved;
    if (unsaved && saveNote !== null) saveNote.hidden = true;
  }, run.unsaved);
  const progress = installProgressToggle(shell, (on) => table.showProgress(on));
  const table = installTable(shell, edit, idColumn.on, s0, cursor, progress.on);
  const tableAria = installTableAria(shell, s0);
  const card = installCard(shell, edit, pointer.position);
  const moved = installMoved(shell);
  const selectionCount = installSelectionCount(root);

  // What a replaced schedule resets on the page; the file opener and `load` both end here. `adopted` is kept for a switch to WebGL2.
  let adopted: Run["file"] = null;
  const adopt = (file: OpenedFile, lines: string[], ms: number, expected: readonly string[], fileName: string | null = file.name, carried = false): void => {
    if (carried) saveState.carry(status());
    adopted = { file, lines, expected, fileName };
    name = documentName(file.name);
    if (saveNote !== null) saveNote.hidden = true;
    info.unsupported = lines;
    info.loadMs = ms;
    info.open = null;
    progress.opened();
    open.fileName = fileName;
    open.importNotes = Object.freeze(lines.filter((line) => !expected.some((begin) => line.startsWith(begin))));
    hook.opened(file, lines);
  };

  // `unsaved` reads now, not at the last frame that drew: the frame that failed may have been the first.
  const unsaved = (): boolean => (saveState.sync(status()), open.unsaved);
  const busy = (): boolean => edit.active() || app.busy();
  const guard = createOpenGuard({ busy, unsaved, say: sayFor(root) });
  const opener = installOpen({
    root,
    app,
    reason,
    fail: (message, projects) => events.fail("open", message, false, projects),
    frames: () => status().frames,
    unsaved,
    guard,
    timed: (file) => {
      if (file.timings !== undefined) info.open = { name: file.name, timings: file.timings };
    },
    opened: adopt,
  });

  const hook = installTestHook({ app, status, syncTheme, loadMs: info.loadMs, fallback, unsupported: info.unsupported });
  if (run.file !== null) adopt(run.file.file, run.file.lines, 0, run.file.expected, run.file.fileName);
  // Keys act while focus is inside the chart, so the app starts with focus on it; a host page would leave this to its own flow.
  root.focus({ preventScroll: true });

  const metrics = new Metrics(
    { app, canvas, status, input: pointer.input, params: page?.params ?? new URLSearchParams(), wasm, fallback },
    page?.measure ?? null,
  );
  metrics.onDone = (outcome) => {
    hook.measured(outcome);
    page?.measured(outcome);
  };

  // The developer page's `?groups=`: the stress fixture has a `Phase` code, and the rows are grouped before the first frame.
  if (page?.groups && "stress" in source && !app.set_group_by("Phase")) throw new MountError("schedule", "The stress fixture could not be grouped by Phase.");
  startLoop(shell, {
    metrics,
    events,
    // Only an automatic WebGPU choice switches; a forced WebGL2 run reports.
    lost: !forceWebgl && app.backend() === "WebGPU" ? run.lost : null,
    reload: page === undefined ? undefined : () => page.restart(null),
    epoch: s0.epoch,
    overlay,
    statusLine,
    statusText: (s, lines) => (line === undefined ? "" : line.text(s, lines, info)),
    fileOpener: opener,
    beside: [rows, filterScroll, baselineMark],
    idColumn,
    table,
    tableAria,
    edit,
    card,
    moved,
    selectionCount,
    pointer,
    saveState,
  });

  const gantt = createGantt({
    shell, reason, name: () => name, setName: (n) => { name = n; }, save, note, printOn, openFile: (f, options) => opener.openFile(f, options),
    adopt, busy, guard, hostView, events, setPending: table.setPending, speed: () => speedOf(info.open, metrics.view()), unit,
  });
  // The panel is written beside the chart, not in its root; the switch to WebGL2 takes it down with this start and writes a new one with no history.
  if (assistant !== undefined) disposers.push(mountAssistantPanel(assistant.into, assistant.send, gantt.commands, texts).destroy);
  return { gantt, hook, changed: () => status().epoch !== s0.epoch || status().revision !== s0.revision, unsaved, file: () => adopted };
}
