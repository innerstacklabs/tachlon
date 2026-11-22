import { type Status } from "./api.js";
import { reason } from "./boot.js";
import { optional } from "./dom.js";
import { type Shell, watching } from "./shell.js";

export interface RowEdit {
  /** Insert a row below the selection and put the cursor on its name, so typing names it. A refusal shows in the alert line and changes nothing. */
  insert: () => void;
  /** Delete the selected link when one is selected, else the selected row, with the same refusal. */
  remove: () => void;
  /** Indent the selected row under the row above it. A refusal shows in the alert line and changes nothing. */
  indent: () => void;
  /** Outdent the selected row to just after its parent; it adopts the siblings below. Same refusal line. */
  outdent: () => void;
  /** Move the selected row above or below its sibling, subtree with it. Same refusal line. */
  moveUp: () => void;
  moveDown: () => void;
  /** Show a refusal in the alert line (the text of a thrown error); the next successful edit or open clears it. */
  refuse: (err: unknown) => void;
  /** Show a hint that is not a failure in the alert line, in its info style; cleared the same way as a refusal. */
  hint: (message: string) => void;
  /** Forget the link refusal last seen, before an Undo or Redo press: its report shows again when it is worded as the one before. Two presses inside one frame show only the second. */
  rearm: () => void;
  /** Clear a refusal once the schedule changed or another file opened. */
  sync: (s: Status) => void;
}

/** The Insert, Delete, indent and outdent row commands, shared by the keys and the toolbar buttons (gs#50), and the alert line a refused link drag shows (gs#347). */
export function installRowEdit(shell: Shell, inserted: () => void): RowEdit { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { app, root, status } = shell;
  const line = optional<HTMLElement>(root, "edit-error");
  const text = optional<HTMLElement>(root, "edit-error-text");
  const close = optional<HTMLButtonElement>(root, "edit-error-close");
  // The lines are cut with an ellipsis at 420px: whichever code writes a message (this file, the file opener, the data-date box), the full
  // text is the span's title while it is cut, and no title while it is whole.
  for (const id of ["edit-error-text", "open-error-text"]) {
    const span = optional<HTMLElement>(root, id);
    if (span === null) continue;
    const sync = (): void => {
      span.title = span.scrollWidth > span.clientWidth ? (span.textContent ?? "") : "";
    };
    watching(shell, new MutationObserver(sync)).observe(span, { childList: true, characterData: true, subtree: true });
    watching(shell, new ResizeObserver(sync)).observe(span);
  }
  // The revision and epoch the refusal was shown at: the next successful edit or open clears it.
  let shownAt: { revision: number; epoch: number } | null = null;
  // The link refusal the last status carried, and whether the line shows one: a link refusal ends with the next press.
  let linkSeen: string | null | undefined = null;
  let lineIsLink = false;
  const clear = (): void => {
    shownAt = null;
    lineIsLink = false;
    if (line === null || text === null) return;
    text.textContent = "";
    line.hidden = true;
    delete line.dataset.kind;
  };
  const show = (message: string, fromLink: boolean, kind: "error" | "info" = "error"): void => {
    if (line === null || text === null) {
      console.warn(`gantt: ${message}`);
      return;
    }
    const s = status();
    shownAt = { revision: s.revision, epoch: s.epoch };
    lineIsLink = fromLink;
    line.dataset.kind = kind;
    text.textContent = message;
    line.hidden = false;
  };
  const refuse = (err: unknown): void => show(reason(err), false);
  const hint = (message: string): void => show(message, false, "info");
  close?.addEventListener("click", () => {
    clear();
    root.focus({ preventScroll: true });
  });
  const run = (command: () => unknown): void => {
    if (status().gesture !== "idle") return;
    try {
      command();
      // The edit moved `Status.stamp`, so a search showing recounts its matches on the next frame and keeps the selection the
      // edit chose (the new task, or the next row) instead of jumping to the first match (`Find.sync`).
      clear();
    } catch (err) {
      refuse(err);
    }
  };
  return {
    insert: () =>
      run(() => {
        app.insert_task();
        inserted();
      }),
    // A selected link goes first: Delete removes what the person last pointed at, not the row behind it.
    remove: () => run(() => (app.selected_link() === undefined ? app.delete_selected() : app.remove_selected_link())),
    indent: () => run(() => app.indent_selected()),
    outdent: () => run(() => app.outdent_selected()),
    moveUp: () => run(() => app.move_selected_up()),
    moveDown: () => run(() => app.move_selected_down()),
    refuse,
    hint,
    rearm: () => {
      linkSeen = undefined;
    },
    sync: (s) => {
      // A link drag the engine refused shows its message in the same line; the next press clears it.
      const link = s.link.refusal;
      if (link !== linkSeen) {
        linkSeen = link;
        if (link !== null) show(link, true);
        else if (lineIsLink) clear();
      }
      if (shownAt !== null && (s.revision !== shownAt.revision || s.epoch !== shownAt.epoch)) clear();
    },
  };
}
