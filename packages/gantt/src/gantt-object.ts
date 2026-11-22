// The object `mount` hands back (`Gantt`), built from the parts one start of the chart made. It is a facade: each method checks the
// chart is alive and calls a part; none of them holds state of its own beyond "destroyed".
import { download, type LoadResult, type LoadSource, toChecksReport, toExportResult } from "./document.js";
import { createCommands, guardCommands } from "./commands.js";
import { loadSource } from "./load-source.js";
import { readFilterInputs, type ViewState } from "./view-state.js";
import { takeDown, type Shell } from "./shell.js";
import type { OpenGuard } from "./open-guard.js";
import type { OpenedFile, OpenOptions } from "./open-schedule.js";
import type { PageSize, PrintPages, Status } from "./api.js";
import { pagePng } from "./image-export.js";
import { xlsxFile } from "./xlsx.js";
import { snapshot } from "./snapshot.js";
import { localDate, measureText, printColumns } from "./print-sheet.js";
import type { Gantt, MountOptions } from "./index.js";
import { pushHostEntry, update } from "./update.js";
import type { SetData } from "./host-data.js";
import type { UpdateResult } from "./gantt-types.js";
import type { FieldMap } from "./field-map.js";
import type { HasOn } from "./events.js";
import type { Speed } from "./speed.js";

/** What the object reads and calls on the start that built it. */
export interface GanttParts {
  shell: Shell;
  reason: (err: unknown) => string;
  /** The open file's document name, and its setter (`load` names the schedule). */
  name: () => string;
  setName: (name: string) => void;
  save: () => Promise<void>;
  /** Writes the note at the bottom right (`#save-note`, a live region) and shows it. */
  note: (text: string) => void;
  /** Prints once on that paper (`installPrintSheet`). */
  printOn: (size: PageSize) => void;
  openFile: (file: File, options?: OpenOptions) => Promise<void>;
  adopt: (file: OpenedFile, lines: string[], ms: number, expected: readonly string[], fileName: string | null, unsaved?: boolean) => void;
  busy: () => boolean;
  /** Show a refusal in the alert line (`RowEdit.refuse`). */
  refuse: (err: unknown) => void;
  /** Mark these task slots as pending (`installTable`). */
  setPending: (slots: readonly number[]) => void;
  wrote: (slots: readonly number[]) => void;
  /** What the host stores now; `load` fits a document or file to it. */
  capabilities: () => MountOptions["capabilities"];
  /** The token and the swap check `load` shares with `openFile`. */
  guard: OpenGuard;
  /** The host's own reader of the view state (not the loop's). */
  hostView: { read: (s: Status) => ViewState };
  events: HasOn;
  /** The open's timings and the frames' numbers (`speed.ts`). */
  speed: () => Speed;
  /** The unit of the host's durations (`MountOptions.durationUnit`). */
  unit: MountOptions["durationUnit"];
  /** The form of the host's dates (`MountOptions.dateForm`). */
  dateForm: MountOptions["dateForm"];
  /** The host's names for its rows (`field-map.ts`). */
  fields: FieldMap;
  /** `setData` against the rows this start was given (`host-data.ts`). */
  setData: SetData;
  /** The host's capability profile as the chart holds it: what a missing save handler turns off is merged in (`save-handlers.ts`). */
  profile: (host: MountOptions["capabilities"]) => MountOptions["capabilities"];
}

/** The `Gantt` of one start; `destroy` takes down what `shell.disposers` holds, then frees the app and empties the root. */
export function createGantt(p: GanttParts): Gantt { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { shell } = p;
  const { app, root, disposers } = shell;
  let destroyed = false;
  const alive = (): void => {
    if (destroyed) throw new Error("this chart was destroyed");
  };
  const printPages = (): PrintPages => app.print_pages(document.title, localDate(), undefined, measureText, printColumns(shell.root)) as PrintPages;
  /** `update` or `hostEdit` of the host's records: the members of a record that the chart has no field for are kept once it took the rest (decision 0022, rule 8.3). */
  const hostUpdate = (records: unknown, call?: "host_edit"): UpdateResult => {
    const taken = Array.isArray(records) ? p.fields.custom.take("tasks", records) : undefined;
    const answer = update(app, p.busy, p.unit, p.dateForm, taken?.rows?.map(p.fields.tasks.toChart) ?? records, p.wrote, call);
    if (answer.ok) taken?.commit();
    return answer;
  };
  const commands = guardCommands(createCommands(shell, p.reason, p.busy, p.refuse), () => destroyed);
  return {
    commands,
    open: async (file, options) => {
      alive();
      await p.openFile(file, options);
    },
    download: async () => {
      alive();
      await p.save();
    },
    exportCsv: () => {
      alive();
      const file = `${p.name()}.csv`;
      // The byte order mark (EF BB BF) makes Excel read a non-ASCII task name as UTF-8.
      download(file, new TextEncoder().encode(`\uFEFF${app.list_csv()}`), "text/csv");
      const { rowCount, n } = shell.status();
      p.note(`Exported ${rowCount === n ? rowCount : `${rowCount} of ${n}`} rows to ${file}`);
    },
    exportXlsx: async () => {
      alive();
      const file = `${p.name()}.xlsx`;
      // The sheet and the row count are read together, before the file is packed, so the note counts the rows the file holds.
      const { rowCount, n } = shell.status();
      const bytes = await xlsxFile({ name: p.name(), sheetXml: app.list_xlsx_sheet() });
      if (destroyed) return;
      download(file, bytes, "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet");
      p.note(`Exported ${rowCount === n ? rowCount : `${rowCount} of ${n}`} rows to ${file}`);
    },
    copyList: async () => {
      alive();
      const { rowCount, n } = shell.status();
      await navigator.clipboard.writeText(app.list_text());
      if (!destroyed) p.note(`Copied ${rowCount === n ? rowCount : `${rowCount} of ${n}`} rows`);
    },
    printOn: (size) => {
      alive();
      p.printOn(size);
    },
    printedPages: () => {
      alive();
      return printPages().length;
    },
    exportImage: async (options) => {
      alive();
      const pages = printPages();
      const page = options?.page ?? 1;
      if (!Number.isInteger(page) || page < 1 || page > pages.length) {
        const shown = Number.isInteger(page) ? `${page}` : `"${String(page)}"`;
        throw new Error(`exportImage: page ${shown} is not one of the ${pages.length} printed page${pages.length === 1 ? "" : "s"}`);
      }
      const png = await pagePng(pages[page - 1]!);
      if (!destroyed) download(page === 1 ? `${p.name()}.png` : `${p.name()}-page-${page}.png`, png, "image/png");
    },
    load: async (source: LoadSource): Promise<LoadResult> => {
      alive();
      return loadSource(app, source, p.adopt, p.setName, p.guard, p.capabilities);
    },
    newSchedule: async (): Promise<LoadResult> => {
      alive();
      const busy = p.guard.busy();
      if (busy !== null) return { ok: false, error: `Not started. ${busy}` };
      if (p.hostView.read(shell.status()).unsaved && !window.confirm("Start a new schedule? The changes to the current schedule are not saved and will be lost.")) {
        return { ok: false, error: "Not started. The current schedule was kept." };
      }
      return loadSource(app, { blank: true }, p.adopt, p.setName, p.guard, p.capabilities);
    },
    toDocument: () => {
      alive();
      return JSON.parse(app.save(p.name())) as unknown;
    },
    snapshot: () => {
      alive();
      return snapshot(app, p.name());
    },
    exportXer: () => {
      alive();
      return toExportResult(app.export_xer());
    },
    exportXerLeftOut: () => {
      alive();
      return app.export_xer_left_out() as string[];
    },
    exportMspdi: () => {
      alive();
      return toExportResult(app.export_mspdi());
    },
    checks: () => {
      alive();
      return toChecksReport(app.checks());
    },
    backend: () => {
      alive();
      return app.backend() === "WebGPU" ? "WebGPU" : "WebGL2";
    },
    state: () => {
      alive();
      return p.hostView.read(shell.status());
    },
    filterInputs: () => {
      alive();
      return readFilterInputs(app);
    },
    busy: () => {
      alive();
      return p.busy();
    },
    setReadOnly: (on) => {
      alive();
      app.set_read_only(on);
    },
    readOnly: () => {
      alive();
      return app.read_only();
    },
    setCapabilities: (capabilities) => {
      alive();
      app.set_capabilities(JSON.stringify(p.profile(capabilities) ?? {}));
    },
    setPending: (keys) => {
      alive();
      const slots = JSON.parse(app.task_slots([...keys])) as (number | null)[];
      p.setPending(slots.filter((slot): slot is number => slot !== null));
      return keys.filter((_, i) => slots[i] === null);
    },
    update: (records) => {
      alive();
      return hostUpdate(records);
    },
    hostEdit: (records) => {
      alive();
      return hostUpdate(records, "host_edit");
    },
    pushHostEntry: (token) => { alive(); return pushHostEntry(app, token); },
    clearHistory: () => { alive(); app.clear_history(); },
    setData: (data) => {
      alive();
      const custom = { tasks: p.fields.custom.take("tasks", data.tasks), links: p.fields.custom.take("links", data.links) };
      const rows = { ...p.fields.data({ tasks: custom.tasks.rows, links: custom.links.rows }), dataDate: data.dataDate };
      const answer = p.setData(rows, shell.status().epoch, (records, day) => {
        const moved = day === undefined ? undefined : commands.setDataDate(day);
        if (moved !== undefined && !moved.ok) return moved;
        return records.length === 0 ? Object.freeze({ ok: true, updated: 0 }) : update(app, p.busy, p.unit, p.dateForm, records, p.wrote);
      });
      if (answer.ok) [custom.tasks, custom.links].forEach((taken) => taken.commit());
      return answer;
    },
    speed: () => {
      alive();
      return p.speed();
    },
    on: (name, handler) => { alive(); return p.events.on(name, handler); },
    destroy: () => {
      if (destroyed) return;
      destroyed = true;
      // The loop is the last part installed, so it stops first and nothing draws on an app that is about to be freed.
      takeDown(disposers);
      app.free();
      root.replaceChildren();
    },
  };
}
