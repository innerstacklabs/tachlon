// The app's chrome (decision 0012): the Filters menu, the find box and the toolbar's baseline, row, level, zoom, data date and row count
// controls, which drive the chart only through its public API (`Gantt`: `commands`, `state()`, `filterInputs()`, `busy()` and `on()`).
// Private, shared by the developer page (`packages/gantt/`) and the hosted app (`apps/hosted/`). The chart leaves an empty part in its toolbar for each
// piece (`filters-menu`, `find`, `baseline-tools`, `row-tools`, `zoom-tools`, `readouts`, `path-line`, `blank-note`); `installChrome` fills them, and fills the new
// ones again after a switch to WebGL2, which replaces the chart's elements but keeps the `Gantt`. A chart mounted without it has none of them.
import type { Gantt, OpenOptions, ViewState } from "gantt-core";
import baselineHtml from "./baseline.html?raw";
import { installBlankNote } from "./blank-note.js";
import blankHtml from "./blank-note.html?raw";
import { installChecks } from "./checks.js";
import { installExport } from "./export.js";
import dialogsHtml from "./file-dialogs.html?raw";
import noticeHtml from "./file-notice.html?raw";
import toolsHtml from "./file.html?raw";
import { installFileOnly } from "./file-only.js";
import { installFiltersMenu } from "./filters-menu.js";
import filtersHtml from "./filters-menu.html?raw";
import { installFind } from "./find.js";
import findHtml from "./find.html?raw";
import { installKeysHelp } from "./keys-help.js";
import keysHtml from "./keys-tools.html?raw";
import { installOpen } from "./open.js";
import { installPathLine } from "./path-line.js";
import pathHtml from "./path-line.html?raw";
import { type ChromeContext, piece } from "./parts.js";
import readoutsHtml from "./readouts.html?raw";
import rowsHtml from "./rows.html?raw";
import { installSaveMark } from "./save-mark.js";
import { installTab } from "./tab.js";
import { installToolbar } from "./toolbar.js";
import { installUnsupported } from "./unsupported.js";
import zoomHtml from "./zoom.html?raw";

/** The chrome installed on one chart. */
export interface Chrome {
  /** Open `file` in the chart as the toolbar's Open button does: the file line says "Opening …", and a failure or a project list shows in the toolbar. */
  open(file: File, options?: OpenOptions): Promise<void>;
  /** Set `file`, such as last month's, as the baseline: the host's "Compare with a file..." calls it. A refusal shows in the chart's alert line and changes nothing; the tasks only the file has are listed in a closable panel. */
  compare(file: File): Promise<void>;
  /** Take the chrome's handlers off the chart and its root. The chart's `destroy` empties the root, elements and all. */
  destroy(): void;
}

/** Put the chrome into the chart `gantt` that `mount` started in `root`. */
export function installChrome(gantt: Gantt, root: HTMLElement): Chrome {
  const offs: (() => void)[] = [];
  const chrome: ChromeContext = {
    gantt,
    root,
    hear: (name, handler) => void offs.push(gantt.on(name, handler)),
    owns: (off) => void offs.push(off),
  };
  const menu = piece("filters-menu", filtersHtml);
  const bar = piece("find", findHtml);
  const pieces = {
    baseline: piece("baseline-tools", baselineHtml),
    rows: piece("row-tools", rowsHtml),
    zoom: piece("zoom-tools", zoomHtml),
    readouts: piece("readouts", readoutsHtml),
  };
  const files = piece("file-tools", toolsHtml);
  const notice = piece("file-notice", noticeHtml);
  const keys = piece("keys-tools", keysHtml);
  const path = piece("path-line", pathHtml);
  const pathLine = installPathLine(path);
  const blankSlot = piece("blank-note", blankHtml);
  const blank = installBlankNote(blankSlot);
  const dialogs = piece("file-dialogs", dialogsHtml);
  const filters = installFiltersMenu(chrome, menu);
  const find = installFind(chrome, bar);
  const toolbar = installToolbar(chrome, pieces);
  const open = installOpen(chrome, files, dialogs, notice);
  const notes = installUnsupported(chrome, files, dialogs);
  const fileOnly = installFileOnly(chrome, dialogs);
  const mark = installSaveMark(files);
  const tab = installTab(chrome);
  installExport(chrome, files, dialogs);
  installChecks(chrome, files, dialogs);
  installKeysHelp(chrome, keys, dialogs);
  // The file line, the unsaved mark, the tab and the import notes follow `state()`; the notes only when the state carries a new list.
  let listed: readonly string[] | null = null;
  const follow = (state: ViewState): void => {
    open.sync(state);
    mark(state);
    tab(state);
    pathLine(state);
    blank(state);
    if (state.importNotes !== listed) notes((listed = state.importNotes));
  };
  const place = (): void => {
    for (const p of [menu, bar, files, notice, keys, path, blankSlot, dialogs, ...Object.values(pieces)]) p.place(root);
    toolbar.placed();
  };
  place();
  chrome.hear("change", (state) => {
    filters.sync(state);
    find.follow(state);
    toolbar.sync(state);
    follow(state);
  });
  find.follow(gantt.state());
  follow(gantt.state());
  toolbar.reset();
  chrome.hear("edit", find.recount);
  chrome.hear("move", find.recount);
  chrome.hear("open", () => {
    filters.reset();
    fileOnly.hide();
    find.reset();
    toolbar.reset();
  });
  chrome.hear("find", find.focus);
  // The switch to WebGL2 gives the chart a new toolbar: the same elements go into its parts, with what they held.
  chrome.hear("error", ({ source }) => {
    if (source === "backend") place();
  });
  return {
    open: open.open,
    compare: async (file) => {
      const answer = await toolbar.compare(file);
      if (answer !== undefined) fileOnly.show(answer);
    },
    destroy: () => {
      for (const off of offs.splice(0)) off();
    },
  };
}
