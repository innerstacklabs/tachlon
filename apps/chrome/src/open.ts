import { type OpenOptions, PRODUCT_NAME, type PickerProject, type ViewState } from "gantt-core";
import { type ChromeContext, type Piece, refocus } from "./parts.js";

const count = new Intl.NumberFormat("en-US");
/** What the tooltip calls a file, by its extension. */
const FORMAT_NAMES: Record<string, string> = { xer: "P6 XER", xml: "MS Project XML", gantt: `${PRODUCT_NAME} document` };

export interface Open {
  /** The file line beside the Open button, from the view state (the `change` event). */
  sync: (state: ViewState) => void;
  /** `Gantt.open` as the Open button calls it: the line says "Opening …", and an error or a project list shows in the toolbar. */
  open: (file: File, options?: OpenOptions) => Promise<void>;
}

/**
 * Open a schedule from the toolbar: the Open button, Ctrl+O (the chart's `choose-file` event) and a file dropped on the chart. The file goes to
 * `Gantt.open`; the file line shows `state().fileName`, a failure arrives as the `error` event, and a file with several projects as the same
 * event with their list, which the project dialog shows. Nothing here makes a network request.
 */
export function installOpen(chrome: ChromeContext, tools: Piece, dialogs: Piece, notice: Piece): Open {
  const { gantt, root } = chrome;
  const button = tools.part<HTMLButtonElement>("open-button");
  const input = tools.part<HTMLInputElement>("open-input");
  const info = tools.part("open-info");
  const fileText = tools.part("open-file");
  const errorLine = notice.part("open-error");
  const errorText = notice.part("open-error-text");
  const picker = dialogs.part<HTMLDialogElement>("picker");
  const pickerList = dialogs.part("picker-list");
  const listen = (target: EventTarget, type: string, run: (event: never) => void): void => {
    target.addEventListener(type, run as EventListener);
    chrome.owns(() => target.removeEventListener(type, run as EventListener));
  };
  // The file this chrome is opening (the project list the chart reports belongs to it), and the note a drop of several files adds to the line.
  let opening: { file: File; options: OpenOptions; ignored: number } | null = null;
  let others: { name: string; text: string } | null = null;
  // Whether a file has been named since the chrome went on.
  let named = false;

  const showError = (text: string): void => {
    errorText.textContent = text;
    errorLine.hidden = false;
  };
  const clearError = (): void => {
    errorText.textContent = "";
    errorLine.hidden = true;
  };
  const show = (state: ViewState): void => {
    const name = state.fileName;
    const extra = others !== null && others.name === name ? others.text : "";
    const format = FORMAT_NAMES[name?.split(".").pop()?.toLowerCase() ?? ""];
    const title = name === null ? "" : `${name}${format === undefined ? "" : ` · ${format}`} · ${count.format(state.taskCount)} tasks${extra}`;
    // As the tab does (`tab.ts`): a schedule with no file name after a file was named (New schedule) reads `untitled`; a sample with no file keeps an empty line.
    if (name !== null) named = true;
    const text = name === null ? (named ? "untitled" : "") : name + extra;
    // Written only when it differs: this runs on every `change`.
    if (fileText.textContent !== text) fileText.textContent = text;
    if (info.title !== title) info.title = title;
  };

  const begin = async (file: File, ignored = 0, options: OpenOptions = {}): Promise<void> => {
    if (picker.open) picker.close();
    clearError();
    opening = { file, options, ignored };
    fileText.textContent = `Opening ${file.name}…`;
    try {
      await gantt.open(file, options);
    } finally {
      opening = null;
    }
    // During a switch to WebGL2 the chart refuses `state()`: the open still took the file, and the line follows the chart's next `change`.
    let state: ViewState;
    try {
      state = gantt.state();
    } catch {
      return;
    }
    // An open that took the file replaces the note: this file's own, or none.
    if (state.fileName === file.name) others = ignored > 0 ? { name: file.name, text: ` · ${count.format(ignored)} other ${ignored === 1 ? "file" : "files"} ignored` } : null;
    show(state);
  };

  /** Fill and show the project dialog. Choosing a row reads the file again with that selection; Esc and Cancel change nothing. */
  const showPicker = ({ file, options, ignored }: NonNullable<typeof opening>, projects: readonly PickerProject[]): void => {
    dialogs.part("picker-title").textContent = `${file.name} holds ${count.format(projects.length)} projects`;
    const choice = (label: string, id: string): HTMLButtonElement => {
      const choose = document.createElement("button");
      choose.type = "button";
      choose.dataset.project = id;
      choose.textContent = label;
      choose.addEventListener("click", () => {
        picker.close();
        void begin(file, ignored, { ...options, project: id });
      });
      return choose;
    };
    const total = projects.reduce((sum, project) => sum + project.activities, 0);
    const buttons = [
      ...projects.map((p) => choice(`${p.id} · ${p.name === "" ? "(no name)" : p.name} · ${count.format(p.activities)} activities`, p.id)),
      choice(`Open all ${count.format(projects.length)} projects (${count.format(total)} activities)`, "all"),
    ];
    pickerList.replaceChildren(...buttons);
    picker.showModal();
    buttons[0]?.focus();
  };
  // Esc, Cancel or a choice closed it: the chart gets focus back.
  listen(picker, "close", () => refocus(root));
  listen(dialogs.part("picker-cancel"), "click", () => picker.close());
  listen(notice.part("open-error-close"), "click", () => {
    clearError();
    refocus(root);
  });

  chrome.hear("error", ({ source, message, projects }) => {
    if (source !== "open") return;
    if (projects !== undefined && opening !== null) showPicker(opening, projects);
    else showError(message);
  });
  chrome.hear("open", clearError);
  // With the chart's toolbar gone the input is not in the page, and a chooser for it would still open.
  chrome.hear("choose-file", () => input.isConnected && input.click());
  button.title = `Open a schedule: a P6 XER, an MS Project XML or a ${PRODUCT_NAME} file (Ctrl+O), or drop one on the chart.`;
  listen(button, "click", () => input.click());
  listen(input, "change", () => {
    const file = input.files?.[0];
    // The same file chosen twice must fire again.
    input.value = "";
    if (file !== undefined) void begin(file);
  });

  // A drop opens the first file and says how many others were ignored. Only a drag that carries files is taken.
  const carriesFiles = (e: DragEvent): boolean => e.dataTransfer?.types.includes("Files") ?? false;
  const over = (e: DragEvent): void => {
    if (!carriesFiles(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    root.dataset.dropping = "true";
  };
  listen(root, "dragenter", over);
  listen(root, "dragover", over);
  listen(root, "dragleave", (e: DragEvent) => {
    if (e.relatedTarget instanceof Node && root.contains(e.relatedTarget)) return;
    delete root.dataset.dropping;
  });
  listen(root, "drop", (e: DragEvent) => {
    if (!carriesFiles(e)) return;
    e.preventDefault();
    delete root.dataset.dropping;
    const files = e.dataTransfer?.files;
    const file = files?.[0];
    if (file !== undefined) void begin(file, (files?.length ?? 1) - 1);
  });
  // A file dropped beside the chart would make the browser leave the page for it: refuse it on the whole page, with the no-drop cursor.
  // A drag a chart or the host page's own drop zone took is already prevented: the guards of the page's other charts leave it be.
  const guard = (e: DragEvent): void => {
    if (e.defaultPrevented || !carriesFiles(e)) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "none";
  };
  listen(document, "dragover", guard);
  listen(document, "drop", guard);
  return { sync: show, open: (file, options) => begin(file, 0, options) };
}
