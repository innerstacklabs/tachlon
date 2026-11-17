import { element } from "./dom.js";
import { type Shell, watching } from "./shell.js";

/** The list's narrowest width (the least the columns need) and the widest the chart leaves it: the canvas keeps 320 px. */
const LIST_MIN = 420;
const CANVAS_MIN = 320;
/** One arrow key moves the divider this far. */
const KEY_STEP = 16;
const TOKEN = "--gantt-table-w";
/** The ID track before the page has set it from the file (tokens.css: --gantt-id-col). */
const ID_TRACK_FALLBACK = 92;

export interface Split {
  /** The divider, the one element a key aimed at it belongs to. */
  handle: HTMLElement;
  /** An arrow key on the focused divider moves it; true when the key was the divider's. */
  key(e: KeyboardEvent): boolean;
}

/**
 * The list divider (gs#412): drag the strip on the table's right edge to set `--gantt-table-w` in px on the root;
 * a double-click puts the value back to what the host's page has set now (the default 50% unless the host set its own).
 * The canvas follows through its resize observer. Nothing is saved, so a reload returns to the default.
 */
export function installSplit(shell: Shell): Split { // oxlint-disable-line max-lines-per-function -- existing, over cap
  const { root } = shell;
  const handle = element<HTMLDivElement>(root, "split");
  const table = element<HTMLDivElement>(root, "table");
  // What the page set inline, and what this divider last wrote: an inline value that is not the last one written is the page's.
  let host = root.style.getPropertyValue(TOKEN);
  let written = host;
  const hostValue = (): string => {
    const inline = root.style.getPropertyValue(TOKEN);
    if (inline !== written) host = inline;
    return host;
  };

  /** What `--gantt-table-w` means on screen: the table's width less the ID column's and the variance columns' tracks and gaps, which sit outside it. */
  const idExtra = (): number => {
    const px = (token: string, fallback: number): number => parseFloat(getComputedStyle(root).getPropertyValue(token)) || fallback;
    const ids = table.dataset["ids"] === "true" ? px("--gantt-id-col", ID_TRACK_FALLBACK) + 6 : 0;
    return ids + (table.dataset["variance"] === "true" ? 3 * (px("--gantt-var-col", 52) + 6) : 0);
  };
  const current = (): number => table.getBoundingClientRect().width - idExtra();
  const bounds = (): [number, number] => {
    const max = root.clientWidth - CANVAS_MIN - idExtra();
    return [LIST_MIN, Math.max(LIST_MIN, max)];
  };
  /** Write the token (empty removes it); the page's own value is read first, so it is not mistaken for ours later. */
  const write = (value: string): void => {
    hostValue();
    if (value === "") root.style.removeProperty(TOKEN);
    else root.style.setProperty(TOKEN, value);
    written = root.style.getPropertyValue(TOKEN);
    describe();
  };
  const set = (width: number): void => {
    const [lo, hi] = bounds();
    write(`${Math.min(hi, Math.max(lo, Math.round(width)))}px`);
  };
  const describe = (): void => {
    const [lo, hi] = bounds();
    handle.setAttribute("aria-valuemin", String(lo));
    handle.setAttribute("aria-valuemax", String(hi));
    handle.setAttribute("aria-valuenow", String(Math.round(current())));
  };

  /** A move to the right widens the list, and in a right-to-left chart (the list on the right) it narrows it. */
  const sign = (): number => (getComputedStyle(root).direction === "rtl" ? -1 : 1);
  let drag: { x: number; width: number } | null = null;
  handle.addEventListener("pointerdown", (e) => {
    if (e.button !== 0) return;
    // No text selection starts from the drag; the strip still takes focus for the arrow keys.
    e.preventDefault();
    handle.focus({ preventScroll: true });
    handle.setPointerCapture(e.pointerId);
    drag = { x: e.clientX, width: current() };
    handle.dataset["dragging"] = "true";
  });
  handle.addEventListener("pointermove", (e) => {
    if (drag !== null) set(drag.width + sign() * (e.clientX - drag.x));
  });
  const end = (): void => {
    drag = null;
    delete handle.dataset["dragging"];
  };
  handle.addEventListener("pointerup", end);
  handle.addEventListener("pointercancel", end);
  handle.addEventListener("dblclick", () => write(hostValue()));
  handle.addEventListener("focus", describe);
  // The limits and the value depend on the chart's width, the ID column's track (the root's style, set from the file) and the columns the
  // table shows (its data attributes): say them again when one changes, never per frame.
  watching(shell, new ResizeObserver(describe)).observe(root);
  watching(shell, new MutationObserver(describe)).observe(root, { attributes: true, attributeFilter: ["style"] });
  watching(shell, new MutationObserver(describe)).observe(table, { attributes: true, attributeFilter: ["data-ids", "data-variance"] });

  return {
    handle,
    key(e) {
      if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return false;
      if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return false;
      e.preventDefault();
      set(current() + sign() * (e.key === "ArrowRight" ? KEY_STEP : -KEY_STEP));
      return true;
    },
  };
}
