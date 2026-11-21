import type { Gantt } from "gantt-core";
import type { ChromeContext, Piece } from "./parts.js";

/** The most lines the dialog lists; a longer list ends with one line saying how many were left out. */
const MAX_LINES = 200;

/** The P6 version `Gantt.exportXer` writes in the ERMHDR line (`crates/import/src/export/xer/rows.rs`); the dialog names it, because a contract asks for a stated version. */
const P6_VERSION = "19.12";

/** One format the dialog offers: its button part, the file extension, the MIME type, what the result line adds after the file name, and the call that writes it. */
interface Format {
  part: string;
  extension: string;
  mime: string;
  as: string;
  write: (gantt: Gantt) => ReturnType<Gantt["exportXer"]>;
}

const FORMATS: Format[] = [
  { part: "export-mspdi", extension: "xml", mime: "application/xml", as: "", write: (gantt) => gantt.exportMspdi() },
  { part: "export-xer", extension: "xer", mime: "application/octet-stream", as: ` as P6 ${P6_VERSION}`, write: (gantt) => gantt.exportXer() },
];

/** Hand `bytes` to the browser as a download called `fileName`. */
function download(fileName: string, mime: string, bytes: Uint8Array<ArrayBuffer>): void {
  const url = URL.createObjectURL(new Blob([bytes], { type: mime }));
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  // The browser has the blob by the time a task has run; releasing it earlier can cancel the download.
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}

/**
 * The Export… button (gs#703): it opens a small dialog with two formats, MS Project XML and P6 XER. Choosing one downloads `<name>.xml` or `<name>.xer`,
 * the name Save uses (`untitled` for a sample), and lists in the dialog what the file cannot carry; the XER line names the P6 version it is written as.
 * Opening it lists what a P6 save leaves out of this schedule (`Gantt.exportXerLeftOut`). It reads `Gantt.exportMspdi` and `Gantt.exportXer`.
 */
export function installExport(chrome: ChromeContext, tools: Piece, dialogs: Piece): void {
  const { gantt } = chrome;
  const dialog = dialogs.part<HTMLDialogElement>("export-dialog");
  const result = dialogs.part("export-result");
  const list = dialogs.part("export-left-out");
  const show = (text: string, lines: string[]): void => {
    result.textContent = text;
    result.hidden = false;
    const left = lines.length - MAX_LINES;
    list.textContent = (left > 0 ? [...lines.slice(0, MAX_LINES), `and ${left} more`] : lines).join("\n");
    list.hidden = lines.length === 0;
  };
  tools.part("export-button").addEventListener("click", () => {
    // What a P6 save leaves out of this schedule, listed before anything is written.
    const lost = gantt.exportXerLeftOut();
    if (lost.length > 0) show(`Saving as P6 XER ${P6_VERSION} will leave out:`, lost);
    else result.hidden = list.hidden = true;
    dialog.showModal();
  });
  dialogs.part("export-close").addEventListener("click", () => dialog.close());
  for (const format of FORMATS) {
    dialogs.part(format.part).addEventListener("click", () => {
      try {
        // The name Save gives the document, so Export and Save name one schedule alike (`load({ document })` and a renamed .gantt included).
        const own = (gantt.toDocument() as { name?: unknown } | null)?.name;
        const name = typeof own === "string" && own !== "" ? own : "untitled";
        const exported = format.write(gantt);
        const file = `${name}.${format.extension}`;
        download(file, format.mime, exported.bytes);
        const n = exported.leftOut.length;
        show(
          n === 0 ? `Downloaded ${file}${format.as}. Nothing was left out.` : `Downloaded ${file}${format.as}. ${n} ${n === 1 ? "item was" : "items were"} left out:`,
          exported.leftOut,
        );
      } catch (err) {
        show(`Could not export: ${err instanceof Error ? err.message : String(err)}`, []);
      }
    });
  }
}
