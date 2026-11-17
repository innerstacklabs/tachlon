import { type ChromeContext, type Piece, refocus } from "./parts.js";

/** The most lines the panel lists; a longer list ends with one line saying how many were left out. */
const MAX_LINES = 500;

/** What the notice shows after Dismiss. */
const QUIET_MARK = "ⓘ";

/**
 * The importer's notes for a file (`state().importNotes`): a one-line notice in the toolbar ("5 import notes") that lists them when clicked and
 * can be dismissed. Dismissing makes the notice quiet (a small mark; a click still lists the notes). Opening a file
 * shows its notes in full again, or hides the notice when the new file has none.
 * Focus: folding the list returns it to the notice, and dismissing returns it to the chart, whose keys act while focus
 * is inside it.
 * Returns the function that shows the notes of the file just opened.
 */
export function installUnsupported(chrome: ChromeContext, tools: Piece, dialogs: Piece): (lines: readonly string[]) => void {
  const { root } = chrome;
  const panel = dialogs.part("unsupported");
  const notice = tools.part<HTMLButtonElement>("import-notes");
  const setOpen = (open: boolean): void => {
    panel.hidden = !open;
    notice.setAttribute("aria-expanded", String(open));
  };
  notice.addEventListener("click", () => setOpen(panel.hidden === true));
  const fold = (): void => {
    setOpen(false);
    (notice.hidden ? root : notice).focus({ preventScroll: true });
  };
  dialogs.part("unsupported-close").addEventListener("click", fold);
  panel.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !panel.hidden) {
      event.stopPropagation();
      fold();
    }
  });
  dialogs.part("import-notes-dismiss").addEventListener("click", () => {
    setOpen(false);
    // Quiet, not gone: the notice shrinks to a small mark, and a click on it still lists the notes.
    notice.textContent = QUIET_MARK;
    refocus(root);
  });
  return (lines) => {
    setOpen(false);
    notice.hidden = lines.length === 0;
    if (lines.length === 0) return;
    const left = lines.length - MAX_LINES;
    const total = `${lines.length} import ${lines.length === 1 ? "note" : "notes"}`;
    notice.textContent = total;
    notice.setAttribute("aria-label", total);
    notice.title = total;
    dialogs.part("unsupported-title").textContent = `${total}: what was not read from this file${left > 0 ? `, first ${MAX_LINES} shown` : ""}`;
    const list = dialogs.part("unsupported-list");
    list.textContent = (left > 0 ? [...lines.slice(0, MAX_LINES), `and ${left} more`] : lines).join("\n");
    list.scrollTop = 0;
  };
}
