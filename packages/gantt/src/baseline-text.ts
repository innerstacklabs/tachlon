// The baseline's words (gs#703): the one-line confirmation the Baseline button's title and the status line show, and the
// clauses of the summary chip. They are the chart's, so the baseline buttons, the chip and the status line all read them here.
import { type BaselineSummary } from "./api.js";
import { say } from "./messages.js";

/** The baseline's one-line confirmation, in the chart's language: `Baseline set at data date 14 Oct: 59 rows`, or `Baseline set: 59 rows` when it was set with no data date. */
export function baselineText(root: object, rows: number, setAt: string): string {
  return say(root, setAt === "" ? "baseline-set" : "baseline-set-at", { count: rows, day: setAt });
}

const plural = (n: number, one: string, many: string): string => (Math.abs(n) === 1 ? one : many);

/**
 * A chart's own clause builder: how the schedule differs from the baseline, in the words of the "what moved" note, one clause
 * each: `14 slipped`, `finish +3 days`, `critical 2 joined, 1 left`. A clause with nothing to say is left out (the finish, the
 * critical path, the added and removed rows); the slipped count always shows. The clauses are built again only when a count
 * changes, so a frame that finds the same counts reads the array it made before. Each chart part makes its own, so two charts
 * on a page never rebuild each other's.
 */
export function createBaselineClauses(): (b: BaselineSummary | null, fmt: Intl.NumberFormat) => readonly string[] | null {
  let lastSummary: BaselineSummary | null = null;
  let lastSummaryParts: string[] | null = null;
  return (b, fmt) => {
    if (b === null) {
      lastSummary = null;
      lastSummaryParts = null;
      return null;
    }
    if (lastSummary !== null && lastSummaryParts !== null && sameSummary(lastSummary, b)) return lastSummaryParts;
    const parts = [`${fmt.format(b.slipped)} slipped`];
    if (b.finish_days !== 0) {
      parts.push(`finish ${b.finish_days > 0 ? "+" : "-"}${fmt.format(Math.abs(b.finish_days))} ${plural(b.finish_days, "day", "days")}`);
    }
    if (b.joined !== 0 || b.left !== 0) parts.push(`critical ${fmt.format(b.joined)} joined, ${fmt.format(b.left)} left`);
    if (b.added !== 0) parts.push(`${fmt.format(b.added)} added`);
    if (b.removed !== 0) parts.push(`${fmt.format(b.removed)} removed`);
    lastSummary = { ...b };
    lastSummaryParts = parts;
    return parts;
  };
}

function sameSummary(a: BaselineSummary, b: BaselineSummary): boolean {
  return (
    a.slipped === b.slipped &&
    a.finish_days === b.finish_days &&
    a.joined === b.joined &&
    a.left === b.left &&
    a.added === b.added &&
    a.removed === b.removed
  );
}
