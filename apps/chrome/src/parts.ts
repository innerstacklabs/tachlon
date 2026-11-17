// The chrome's own markup and how it goes into the chart (decision 0012). The chart leaves an empty part in its toolbar for each piece
// of chrome (`filters-menu`, `find`); the chrome builds its elements once, keeps them, and puts them into that part, inside the chart's
// root, never on the page. A switch to WebGL2 gives the chart a new toolbar, so the same elements go into the new part again and keep
// their state. Each element named with `data-part` takes its id the way the chart names its own: the part's id, whose prefix is the
// chart's (`chart-2-` for a page's second chart), before its name.
import type { Gantt, GanttEvents } from "gantt-core";

/** What a piece of chrome is given: the chart it drives, the chart's root, and `hear`, which adds a handler the chrome takes off when it goes. */
export interface ChromeContext {
  gantt: Gantt;
  root: HTMLElement;
  hear: <K extends keyof GanttEvents>(name: K, handler: (detail: GanttEvents[K]) => void) => void;
  /** Take a listener off with the chrome. */
  owns: (off: () => void) => void;
}

/** The attributes that name other elements by id; in the markup they name parts. */
const REFERENCES = ["aria-labelledby", "aria-describedby", "aria-controls", "aria-owns", "for"];

/** A piece of chrome: its elements, built once from `html`, and where they go. */
export interface Piece {
  /** The element called `name` in this piece; throws when the markup has none. */
  part<T extends HTMLElement>(name: string): T;
  /** Put the elements into the chart's part `slot` inside `root`, named with that chart's prefix. False when the chart has no such part. */
  place(root: HTMLElement): boolean;
}

export function piece(slot: string, html: string): Piece {
  const holder = document.createElement("div");
  holder.innerHTML = html;
  const nodes = [...holder.childNodes];
  const all = [...holder.querySelectorAll<HTMLElement>("*")];
  // Each reference as the markup wrote it, so naming the elements again for a new prefix starts from the names.
  const written = all.map((el) => REFERENCES.map((name) => [name, el.getAttribute(name)] as const).filter(([, v]) => v !== null));
  return {
    part<T extends HTMLElement>(name: string): T {
      const el = all.find((e) => e.dataset.part === name);
      if (el === undefined) throw new Error(`the chrome's markup has no ${name}`);
      return el as T;
    },
    place(root) {
      const target = root.querySelector<HTMLElement>(`[data-part=${slot}]`);
      if (target === null) return false;
      const prefix = target.id.endsWith(slot) ? target.id.slice(0, target.id.length - slot.length) : "";
      all.forEach((el, i) => {
        if (el.dataset.part !== undefined) el.id = prefix + el.dataset.part;
        for (const [name, value] of written[i]!) {
          el.setAttribute(name, value!.split(/\s+/).filter((ref) => ref !== "").map((ref) => prefix + ref).join(" "));
        }
      });
      target.replaceChildren(...nodes);
      return true;
    },
  };
}

/** Give focus back to the chart, so its keys act again. */
export const refocus = (root: HTMLElement): void => root.focus({ preventScroll: true });
