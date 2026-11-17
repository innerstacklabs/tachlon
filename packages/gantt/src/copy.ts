import { type Shell, listening } from "./shell.js";

/** Ctrl+C and Cmd+C (gs#703): with focus in the chart, a `copy` puts the range (or the selected row) on the clipboard as tab-separated text. */
export function installCopy(shell: Shell): void {
  const { root, app } = shell;
  listening(shell, root, "copy", (e: ClipboardEvent) => {
    const from = e.target instanceof Element ? e.target : document.activeElement;
    if (!from || !root.contains(from)) return;
    // Typing in a cell or the find box, or text the reader selected, copies as the browser does.
    if (from instanceof HTMLInputElement || from instanceof HTMLTextAreaElement || (from instanceof HTMLElement && from.isContentEditable)) return;
    if ((window.getSelection()?.toString() ?? "") !== "") return;
    const text = app.range_text();
    if (text === undefined || !e.clipboardData) return;
    e.clipboardData.setData("text/plain", text);
    e.preventDefault();
  });
}
