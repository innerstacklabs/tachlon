// The object `mount` hands back (`Gantt`), built from the parts one start of the chart made. It is a facade: each method checks the
// chart is alive and calls a part; none of them holds state of its own beyond "destroyed".
import { download, type LoadResult, type LoadSource, toChecksReport, toExportResult } from "./document.js";
import { createCommands, guardCommands } from "./commands.js";
import { loadSource } from "./load-source.js";
import { readFilterInputs, type ViewState } from "./view-state.js";
import { takeDown, type Shell } from "./shell.js";
import type { OpenGuard } from "./open-guard.js";
import type { RowEdit } from "./row-edit.js";
import type { OpenedFile, OpenOptions } from "./open-schedule.js";
import type { PageSize, PrintPages, PublicApp, Status } from "./api.js";
import { pagePng } from "./image-export.js";
import { xlsxFile } from "./xlsx.js";
import { snapshot } from "./snapshot.js";
import { localDate, measureText, printColumns } from "./print-sheet.js";
import type { Gantt, MountOptions } from "./index.js";
import { lagMinutes, pushHostEntry, send, toApp, update } from "./update.js";
import { readAnswer, refused, type Refusal } from "./edit-result.js";
import { rowsChanged, type LinkChange, type LinkChanges, type SetData } from "./host-data.js";
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
  /** The alert line's part: a refused Undo or Redo shows in it. */
  rows: Pick<RowEdit, "refuse" | "rearm">;
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

/** `setData`'s link drops, then sets, then adds, by `App.remove_link_by_key`, `App.set_link_by_key` and `App.add_link_by_key`: how many of each the chart took, how many changed it (a drop of a link it holds no live one of, that a person removed or it never held, is taken as already gone), and the refusal that stopped them. */
function setLinks(app: PublicApp, unit: MountOptions["durationUnit"], { adds, sets, drops }: LinkChanges): { unlinked: number; relinked: number; linked: number; changed: number; refusal?: { link: LinkChange; done: Refusal; what: string } } {
  let unlinked = 0;
  let changed = 0;
  for (const link of drops) {
    const done = readAnswer(app.remove_link_by_key(JSON.stringify(link.key === undefined ? { pred: link.pred, succ: link.succ } : { key: link.key })));
    if (!done.ok && done.reason !== "unknownKey" && done.reason !== "unknownLink") return { unlinked, relinked: 0, linked: 0, changed, refusal: { link, done, what: "was not dropped" } };
    unlinked += 1;
    changed += done.ok ? 1 : 0;
  }
  const minutes = [...sets, ...adds].some(({ lag }) => lag !== undefined) ? lagMinutes(app, unit) : undefined;
  const body = ({ key, pred, succ, type, lag }: LinkChange): string => JSON.stringify({ ...(key === undefined ? {} : { key }), pred, succ, ...(type === undefined ? {} : { kind: type }), ...(lag === undefined ? {} : { lag: minutes!(lag, pred, succ) }) });
  let relinked = 0;
  for (const link of sets) {
    const done = readAnswer(app.set_link_by_key(body(link)));
    if (!done.ok) return { unlinked, relinked, linked: 0, changed: changed + relinked, refusal: { link, done, what: "was not set" } };
    relinked += 1;
  }
  let linked = 0;
  for (const link of adds) {
    const done = readAnswer(app.add_link_by_key(body(link)));
    if (!done.ok) return { unlinked, relinked, linked, changed: changed + relinked + linked, refusal: { link, done, what: "was refused" } };
    linked += 1;
  }
  return { unlinked, relinked, linked, changed: changed + relinked + linked };
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
  const commands = guardCommands(createCommands(shell, p.reason, p.busy, p.rows), () => destroyed);
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
      const answer = p.setData(rows, shell.status().epoch, (records, day, adds, drops, links) => {
        if ((JSON.parse(app.task_slots(adds.map(({ record }) => record.key))) as (number | null)[]).some((slot) => slot !== null)) return rowsChanged("a task of the rows that the chart lacks has a key it already holds.");
        const moved = day === undefined ? undefined : commands.setDataDate(day);
        if (moved !== undefined && !moved.ok) return moved;
        if (records.length + adds.length + drops.length + links.adds.length + links.sets.length + links.drops.length > 0 && p.busy()) return refused("busy");
        const read = toApp(app, p.unit, p.dateForm, [...records, ...adds.map(({ record }) => record)]);
        if (!Array.isArray(read)) return read;
        const set = records.length === 0 ? Object.freeze({ ok: true as const, updated: 0 }) : send(app, read.slice(0, records.length), p.wrote);
        if (!set.ok) return set;
        let added = 0;
        for (const { record, parent, after } of adds) {
          const done = readAnswer(app.add_task(JSON.stringify(read[records.length + added]), parent, after, after === undefined));
          if (!done.ok) {
            const why = `task ${record.key} was refused (${done.message})`;
            return { ...done, key: record.key, message: records.length + added > 0 || moved !== undefined ? `Partly applied: ${why}. What was applied stays; give the same rows again to add the rest.` : `Nothing changed: ${why}.`, added };
          }
          added += 1;
        }
        const { unlinked, relinked, linked, changed, refusal } = setLinks(app, p.unit, links);
        if (refusal !== undefined) {
          const { link, done, what } = refusal;
          const why = `link ${link.key ?? `${link.pred} to ${link.succ}`} ${what} (${done.message})`;
          return { ...done, ...(link.key === undefined ? {} : { key: link.key }), message: records.length + added + changed > 0 || moved !== undefined ? `Partly applied: ${why}. What was applied stays; give the same rows again to apply the rest.` : `Nothing changed: ${why}.`, added, unlinked, relinked, linked };
        }
        const slots = JSON.parse(app.task_slots(drops)) as (number | null)[]; // a key no live task holds (a row a person deleted) stays as it is: their Undo and its columns too
        const removed: string[] = [];
        const skipped: string[] = []; // `setData` holds these rows until the chart holds them again
        for (const [i, key] of drops.entries()) {
          const done = slots[i] === null ? undefined : readAnswer(app.remove_task(key));
          if (done !== undefined && !done.ok) {
            p.fields.custom.drop("tasks", removed);
            const why = `task ${key} was not dropped (${done.message})`;
            return { ...done, key, message: records.length + added + changed + removed.length > 0 || moved !== undefined ? `Partly applied: ${why}. What was applied stays; give the same rows again to drop the rest.` : `Nothing changed: ${why}.`, added, unlinked, relinked, linked, dropped: i, skipped };
          }
          if (done === undefined) skipped.push(key);
          else removed.push(key);
        }
        p.fields.custom.drop("tasks", removed);
        return { ...set, updated: set.updated + added + changed + removed.length, skipped };
      }, (keys) => (JSON.parse(app.task_slots(keys)) as (number | null)[]).map((slot) => slot !== null));
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
