import { type Shell, listening } from "./shell.js";

/** The most names one paste adds; a longer list is refused with a message and changes nothing. */
const MOST = 500;

/** Ctrl+V and Cmd+V (gs#703): with focus in the chart, a `paste` of plain text, one task name per line, inserts a row per name below the selected row as one undo step. It selects the first new row and scrolls to it. */
export function installPaste(shell: Shell, refuse: (err: unknown) => void): void {
  const { root, app, status } = shell;
  listening(shell, root, "paste", (e: ClipboardEvent) => {
    const from = e.target instanceof Element ? e.target : document.activeElement;
    if (!from || !root.contains(from)) return;
    // Typing in a cell or the find box pastes as the browser does.
    if (from instanceof HTMLInputElement || from instanceof HTMLTextAreaElement || (from instanceof HTMLElement && from.isContentEditable)) return;
    const text = e.clipboardData?.getData("text/plain") ?? "";
    // A tab means rows copied from a table (Ctrl+C in the chart), not a list of names: v1 does not take it.
    if (text.includes("\t") || status().gesture !== "idle") return;
    const names = text.split(/\r?\n/).map((line) => line.trim()).filter((line) => line !== "");
    if (names.length === 0) return;
    e.preventDefault();
    try {
      if (names.length > MOST) throw new Error(`Paste at most ${MOST} rows at a time.`);
      const [first] = app.insert_many(names);
      app.select_task(first);
    } catch (err) {
      refuse(err);
    }
  });
}
