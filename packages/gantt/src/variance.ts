import { type Status } from "./api.js";
import { baselineText } from "./baseline-text.js";
import { nameOf } from "./field-names.js";
import { type Shell } from "./shell.js";
import { type ViewState } from "./view-state.js";

/**
 * What a baseline row count means to the page (gs#703): the status line reads `data-baseline` on the chart's root, and the table
 * shows its three variance columns (Start, Finish, Float against the baseline) while `data-variance` is on. They are on whenever a
 * baseline is set. The chart writes it itself, whatever sets the baseline (a toolbar's button, `commands.setBaseline`, an open):
 * `installBaselineMark` follows the view state, which reads the baseline only when the stamp moves, never per frame. The three
 * head cells are added and removed with the attribute, so with no baseline the head is the one `chart.html` has.
 */
const HEADS: [string, string, string][] = [
  ["var-start", "var-start", "var-start-title"],
  ["var-finish", "var-finish", "var-finish-title"],
  ["var-float", "var-float", "var-float-title"],
];

export function markBaseline(root: HTMLElement, rows: number, text: string): void {
  const table = root.querySelector<HTMLElement>("[data-part=table]");
  const float = table?.querySelector("[data-part=table-head] .cols > .float") ?? null;
  for (const [cls] of HEADS) table?.querySelector(`[data-part=table-head] .cols > .${cls}`)?.remove();
  if (rows > 0) {
    root.dataset.baseline = text;
    if (table !== null) table.dataset.variance = "true";
    float?.after(
      ...HEADS.map(([cls, label, title]) => {
        const head = document.createElement("span");
        head.className = cls;
        head.textContent = nameOf(root, label);
        head.title = nameOf(root, title);
        return head;
      }),
    );
  } else {
    delete root.dataset.baseline;
    if (table !== null) delete table.dataset.variance;
  }
}

/**
 * The chart's own follower of the baseline: it writes `data-baseline` and the variance head when the view state's baseline
 * moves in rows or day (a change of the slipped or added count writes nothing), whoever set it. The loop calls `sync` each
 * frame; the comparison is two reads of fields the view state keeps, and it reads the app for nothing.
 */
export function installBaselineMark({ root }: Pick<Shell, "root">, view: (s: Status) => ViewState): { sync: (s: Status) => void } {
  let rows = 0;
  let day = "";
  return {
    sync: (s) => {
      const b = view(s).baseline;
      const nextRows = b?.rows ?? 0;
      const nextDay = b?.dayText ?? "";
      if (nextRows === rows && nextDay === day) return;
      rows = nextRows;
      day = nextDay;
      markBaseline(root, rows, baselineText(root, rows, day));
    },
  };
}
