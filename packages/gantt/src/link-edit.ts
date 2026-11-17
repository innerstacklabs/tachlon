import { optional } from "./dom.js";
import { type RowEdit } from "./row-edit.js";
import { type Shell } from "./shell.js";

export interface LinkEdit {
  /** The box is open. */
  active: () => boolean;
  /** Close the box, if open, and change nothing. */
  end: () => void;
  /**
   * Open the box on the selected link, holding its type and lag (`FS`, `SS+2d`), selected. False, doing nothing, when no link
   * is selected, a drag is in progress, or the box is already open.
   */
  begin: () => boolean;
}

/**
 * The link editor (gs#703): a small box beside the alert line. `Enter` applies the text through `App.set_link` as one undo step
 * and closes the box; a refusal shows the engine's reason in the alert line and keeps the box open, changing nothing; `Escape`
 * closes it. It reads and writes only when a key is pressed, never per frame.
 */
export function installLinkEdit({ app, root, status }: Shell, rows: RowEdit): LinkEdit {
  const box = optional<HTMLElement>(root, "link-edit");
  const input = optional<HTMLInputElement>(root, "link-edit-input");
  let dep: number | null = null;
  // The epoch the box opened at: another file opened since then has other links under the same ids.
  let epoch = 0;
  // `restore` gives focus back to the chart (Enter, Escape). A blur closes the box without it: focus is already going to the
  // control the person chose, such as the find box, and must stay there.
  const close = (restore: boolean): void => {
    if (dep === null || box === null) return;
    dep = null;
    box.hidden = true;
    if (restore) root.focus({ preventScroll: true });
  };
  const end = (): void => close(true);
  const commit = (link: number, field: HTMLInputElement): void => {
    // The link is gone, or the file was replaced while the box was open: close it and apply nothing.
    if (status().epoch !== epoch || app.link_text(link) === undefined) {
      end();
      return;
    }
    if (field.value === app.link_text(link)) {
      end();
      return;
    }
    try {
      app.set_link(link, field.value);
    } catch (err) {
      rows.refuse(err);
      return;
    }
    end();
  };
  input?.addEventListener("keydown", (e) => {
    // The chart's keys stay out of the box; the browser's undo would move focus to the find box, and Ctrl+Z is the chart's.
    e.stopPropagation();
    if ((e.ctrlKey || e.metaKey) && !e.altKey && (e.key.toLowerCase() === "z" || e.key.toLowerCase() === "y")) e.preventDefault();
    if (e.key === "Escape") {
      e.preventDefault();
      end();
    } else if (e.key === "Enter" && dep !== null) {
      e.preventDefault();
      commit(dep, e.currentTarget as HTMLInputElement);
    }
  });
  input?.addEventListener("blur", () => close(false));
  return {
    active: () => dep !== null,
    end,
    begin: () => {
      if (box === null || input === null || dep !== null || status().gesture !== "idle") return false;
      const link = app.selected_link();
      const text = link === undefined ? undefined : app.link_text(link);
      if (link === undefined || text === undefined) return false;
      dep = link;
      epoch = status().epoch;
      input.value = text;
      box.hidden = false;
      input.focus();
      input.select();
      return true;
    },
  };
}
