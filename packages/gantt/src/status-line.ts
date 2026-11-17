// The developer page's status line (the `s` key): the live numbers of the run, one line under the chart. It reads facts the
// public API does not give (the hover, the drag's recompute, the open's timings, the bars drawn), so it is this page's part and
// not the app's chrome: `main.ts` hands it to the chart as a page knob (`page-knobs.ts`) and shows or hides it on `toggle-status`.
import { type BaselineSummary, type Status } from "./api.js";
import { createBaselineClauses } from "./baseline-text.js";
import { type OpenTimings } from "./open-schedule.js";
import type { RunFacts, StatusLine } from "./page-knobs.js";

/** A status-line segment without the label's column padding. */
function compact(line: string): string {
  return line.replace(/^(\w+) {2,}/, "$1 ");
}

const ms = (value: number, fmt: Intl.NumberFormat): string => (value < 10 ? value.toFixed(1) : fmt.format(Math.round(value)));

export function openText(name: string, t: OpenTimings, fmt: Intl.NumberFormat): string {
  const m = (value: number): string => ms(value, fmt);
  return `open ${name} ${m(t.total)} ms = read ${m(t.read)} + paint ${m(t.paint)} + copy ${m(t.copy)} + parse ${m(t.parse)} + build ${m(t.build)} + compute ${m(t.compute)} + swap ${m(t.swap)} + first frame ${m(t.firstFrame)}`;
}

/** A chart's own developer-line form of the clauses: `vs baseline: 14 slipped, finish +3 days`, joined again only when its clauses are a new array. */
export function createBaselineSummaryText(): (b: BaselineSummary | null, fmt: Intl.NumberFormat) => string | null {
  const baselineClauses = createBaselineClauses();
  let textParts: readonly string[] | null = null;
  let text: string | null = null;
  return (b, fmt) => {
    const parts = baselineClauses(b, fmt);
    if (parts !== textParts) {
      textParts = parts;
      text = parts === null ? null : `vs baseline: ${parts.join(", ")}`;
    }
    return text;
  };
}

/** The live numbers, one line (the `s` key shows and hides it). The segments the old overlay carried, shortened. */
export function statusText(
  s: Status,
  lines: string[],
  info: RunFacts,
  fmt: Intl.NumberFormat,
  baselineSummaryText: ReturnType<typeof createBaselineSummaryText>,
): string {
  const { fallback, tableRows } = info;
  const hover = s.hover
    ? s.hover.summary
      ? `task ${s.hover.task} (summary)`
      : `task ${s.hover.task}${s.hover.critical ? " (critical)" : ""}, float ${s.hover.float} min`
    : null;
  const drag = s.drag
    ? `task ${s.drag.task}${s.drag.held ? `, HELD by predecessors (${fmt.format(Math.round((s.drag.es - s.drag.wanted) / 60))} h later)` : ""}, recompute ${s.drag.recomputeMs.toFixed(1)} ms`
    : null;
  const selected = s.selected
    ? `task ${fmt.format(s.selected.task)}: ${fmt.format(s.selected.backCount)} driving back${s.selected.backCapped ? " (capped at 2,000)" : ""}, ${fmt.format(s.selected.forwardCount)} driven ahead${s.selected.forwardCapped ? " (capped at 2,000)" : ""}`
    : null;
  return [
    `${s.backend}${fallback ? ` (WebGPU failed: ${fallback})` : ""}`,
    `${fmt.format(s.n)} tasks (${info.unsupported === undefined ? "generated" : "imported"} + solved in ${info.loadMs.toFixed(0)} ms)`,
    ...lines.map(compact),
    info.open === null ? null : openText(info.open.name, info.open.timings, fmt),
    `rows ${fmt.format(s.rows[0])}-${fmt.format(s.rows[1])} of ${fmt.format(s.rowCount)}`,
    tableRows.closest<HTMLElement>(".gantt")?.dataset.baseline ?? null,
    baselineSummaryText(s.baseline, fmt),
    hover === null ? null : `hover ${hover}`,
    drag === null ? null : `drag ${drag}`,
    selected === null ? null : `selected ${selected}`,
    `ticks ${s.level} · ${s.bars} bars (${s.criticalBars} critical), ${s.arrows} arrows`,
    `table ${tableRows.childElementCount} rows in the DOM`,
  ]
    .filter((part): part is string => part !== null)
    .join("  |  ");
}

/** The page's status line: `place` makes its element in each start of the chart, `toggle` shows or hides the current one. */
export function createStatusLine(): StatusLine & { toggle: () => void } {
  const fmt = new Intl.NumberFormat("en-US");
  const baseline = createBaselineSummaryText();
  let shown: HTMLDivElement | null = null;
  return {
    place: (canvas) => {
      const line = document.createElement("div");
      line.dataset.part = "status-line";
      line.hidden = true;
      canvas.after(line);
      shown = line;
      return line;
    },
    text: (s, lines, facts) => statusText(s, lines, facts, fmt, baseline),
    toggle: () => {
      if (shown !== null) shown.hidden = !shown.hidden;
    },
  };
}
